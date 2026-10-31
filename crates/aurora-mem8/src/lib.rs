#![forbid(unsafe_code)]
//! AURORA MEM|8 v2: integer wave history and recallable task events.
//! Versioned, bounded snapshots with CRC32, exclusive writer locks, file and
//! directory synchronization, and atomic replacement. Legacy files are rejected.
use aurora_core::{Kintsugi, KintsugiError, Rational, SCALE, TernarySpin, WaveMemoryTuple};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const GRID_X: u16 = 256;
pub const GRID_Y: u16 = 256;
pub const GRID_T: u32 = 65536;
pub const WAVE_TUPLE_BYTES: usize = 32;
const MAX_BYTES: u64 = 64 * 1024 * 1024;
const WAVE_MAGIC: &[u8; 8] = b"MEM8AU02";
const EVENT_MAGIC: &[u8; 8] = b"MEM8EV02";

fn io<T>(r: std::io::Result<T>) -> Kintsugi<T> {
    r.map_err(|_| KintsugiError::MemoryFault)
}
fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}
fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}
fn lock(path: &Path) -> Kintsugi<File> {
    io(fs::create_dir_all(parent(path)))?;
    let f = io(OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(sidecar(path, ".lock")))?;
    f.try_lock().map_err(|_| KintsugiError::Busy)?;
    Ok(f)
}
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for b in bytes {
        crc ^= u32::from(*b);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & (0u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}
fn read_snapshot(path: &Path, magic: &[u8; 8]) -> Kintsugi<Vec<u8>> {
    let f = io(File::open(path))?;
    if io(f.metadata())?.len() > MAX_BYTES {
        return Err(KintsugiError::InvalidData);
    }
    let mut b = Vec::new();
    io(f.take(MAX_BYTES + 1).read_to_end(&mut b))?;
    if b.len() > MAX_BYTES as usize {
        return Err(KintsugiError::InvalidData);
    }
    if b.get(..8) != Some(magic.as_slice()) {
        return Err(KintsugiError::LegacyFormat);
    }
    if b.len() < 20 {
        return Err(KintsugiError::InvalidData);
    }
    let end = b.len() - 4;
    if u32::from_le_bytes(b[end..].try_into().unwrap()) != crc32(&b[..end]) {
        return Err(KintsugiError::InvalidData);
    }
    Ok(b)
}
fn atomic_write(path: &Path, mut bytes: Vec<u8>) -> Kintsugi<u32> {
    if bytes.len().saturating_add(4) > MAX_BYTES as usize {
        return Err(KintsugiError::InvalidData);
    }
    let crc = crc32(&bytes);
    bytes.extend_from_slice(&crc.to_le_bytes());
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| KintsugiError::MemoryFault)?
        .as_nanos();
    let tmp = sidecar(path, &format!(".{}.{nonce}.tmp", std::process::id()));
    let result = (|| {
        let mut f = io(OpenOptions::new().write(true).create_new(true).open(&tmp))?;
        io(f.write_all(&bytes))?;
        io(f.sync_all())?;
        io(fs::rename(&tmp, path))?;
        io(io(File::open(parent(path)))?.sync_all())?;
        Ok(crc)
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}

#[derive(Debug, Default, Clone)]
pub struct WaveGrid {
    grid: HashMap<u64, WaveMemoryTuple>,
    history: Vec<(u64, WaveMemoryTuple)>,
    generation: Option<u32>,
}
impl WaveGrid {
    pub fn new() -> Self {
        Self::default()
    }
    fn key(x: u16, y: u16, t: u32) -> Kintsugi<u64> {
        if x >= GRID_X || y >= GRID_Y || t >= GRID_T {
            return Err(KintsugiError::InvalidData);
        }
        Ok((u64::from(x) << 48) | (u64::from(y) << 32) | u64::from(t))
    }
    pub fn write(&mut self, x: u16, y: u16, t: u32, tuple: WaveMemoryTuple) -> Kintsugi<()> {
        let key = Self::key(x, y, t)?;
        if tuple.amplitude_ppm > SCALE || tuple.phase_turns_ppm >= SCALE {
            return Err(KintsugiError::InvalidData);
        }
        self.history.push((key, tuple));
        self.grid.insert(key, tuple);
        Ok(())
    }
    pub fn recall(&self, x: u16, y: u16, t: u32) -> Option<&WaveMemoryTuple> {
        self.grid.get(&Self::key(x, y, t).ok()?)
    }
    pub fn history(&self) -> &[(u64, WaveMemoryTuple)] {
        &self.history
    }
    pub fn len(&self) -> usize {
        self.grid.len()
    }
    pub fn is_empty(&self) -> bool {
        self.grid.is_empty()
    }
    pub fn epochs(&self) -> u64 {
        self.history.len() as u64
    }
    pub fn save_to(&mut self, path: &Path) -> Kintsugi<u64> {
        let _lock = lock(path)?;
        if path.exists() {
            let old = read_snapshot(path, WAVE_MAGIC)?;
            // The generation check must compare against the CRC of the data portion
            // excluding the 4-byte CRC suffix itself.
            if self.generation != Some(crc32(&old[..old.len() - 4])) {
                return Err(KintsugiError::Busy);
            }
        } else if self.generation.is_some() {
            return Err(KintsugiError::MemoryFault);
        }
        let mut b = WAVE_MAGIC.to_vec();
        b.extend_from_slice(&self.epochs().to_le_bytes());
        for (key, tuple) in &self.history {
            b.extend_from_slice(&key.to_le_bytes());
            b.extend_from_slice(&encode_tuple(tuple));
        }
        let crc = atomic_write(path, b)?;
        self.generation = Some(crc);
        Ok(self.epochs())
    }
    pub fn load_from(&mut self, path: &Path) -> Kintsugi<usize> {
        let b = read_snapshot(path, WAVE_MAGIC)?;
        let count = u64::from_le_bytes(b[8..16].try_into().unwrap());
        if count != ((b.len() - 20) / 40) as u64 || (b.len() - 20) % 40 != 0 {
            return Err(KintsugiError::InvalidData);
        }
        let mut loaded = Self::new();
        for record in b[16..b.len() - 4].as_chunks::<40>().0 {
            let key = u64::from_le_bytes(record[..8].try_into().unwrap());
            let (x, y, t) = ((key >> 48) as u16, (key >> 32) as u16, key as u32);
            loaded.write(x, y, t, decode_tuple(&record[8..])?)?;
        }
        loaded.generation = Some(crc32(&b[..b.len() - 4]));
        *self = loaded;
        Ok(count as usize)
    }
}
fn encode_tuple(t: &WaveMemoryTuple) -> [u8; 32] {
    let mut b = [0; 32];
    b[..8].copy_from_slice(&t.frequency_hz.to_le_bytes());
    b[8..16].copy_from_slice(&t.timestamp_us.to_le_bytes());
    b[16..20].copy_from_slice(&t.amplitude_ppm.to_le_bytes());
    b[20..24].copy_from_slice(&t.phase_turns_ppm.to_le_bytes());
    b[24] = (t.spin.as_i8() + 1) as u8;
    b[25] = t.kintsugi_scar_gold;
    b[26..28].copy_from_slice(&t.dyad_coherence.to_le_bytes());
    b
}
fn decode_tuple(b: &[u8]) -> Kintsugi<WaveMemoryTuple> {
    if b[24] > 2 || b[28..].iter().any(|v| *v != 0) {
        return Err(KintsugiError::InvalidData);
    }
    let mut t = WaveMemoryTuple::new(
        u64::from_le_bytes(b[..8].try_into().unwrap()),
        u32::from_le_bytes(b[16..20].try_into().unwrap()),
        u32::from_le_bytes(b[20..24].try_into().unwrap()),
        u64::from_le_bytes(b[8..16].try_into().unwrap()),
        TernarySpin::from_i8(b[24] as i8 - 1),
    )?;
    t.kintsugi_scar_gold = b[25];
    t.dyad_coherence = u16::from_le_bytes(b[26..28].try_into().unwrap());
    Ok(t)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryEvent {
    pub id: u64,
    pub timestamp_us: u64,
    pub salience: Rational,
    pub payload: String,
}
/// Recallable semantic facts and task receipts, not just a telemetry directory.
pub struct MemoryLog;
impl MemoryLog {
    pub fn load(path: &Path) -> Kintsugi<Vec<MemoryEvent>> {
        if !path.try_exists().map_err(|_| KintsugiError::MemoryFault)? {
            return Ok(Vec::new());
        }
        let b = read_snapshot(path, EVENT_MAGIC)?;
        let count = u64::from_le_bytes(b[8..16].try_into().unwrap());
        if count > MAX_BYTES / 36 {
            return Err(KintsugiError::InvalidData);
        }
        let mut cursor = 16;
        let mut events = Vec::new();
        for index in 0..count {
            if cursor + 36 > b.len() - 4 {
                return Err(KintsugiError::InvalidData);
            }
            let h = &b[cursor..cursor + 36];
            let id = u64::from_le_bytes(h[..8].try_into().unwrap());
            let timestamp_us = u64::from_le_bytes(h[8..16].try_into().unwrap());
            let salience = Rational::new(
                i64::from_le_bytes(h[16..24].try_into().unwrap()),
                u64::from_le_bytes(h[24..32].try_into().unwrap()),
            )
            .map_err(|_| KintsugiError::InvalidData)?;
            let size = u32::from_le_bytes(h[32..36].try_into().unwrap()) as usize;
            cursor += 36;
            if id != index + 1
                || size > 262_144
                || cursor + size > b.len() - 4
                || salience < Rational::ZERO
                || salience > Rational::ONE
            {
                return Err(KintsugiError::InvalidData);
            }
            let payload = std::str::from_utf8(&b[cursor..cursor + size])
                .map_err(|_| KintsugiError::InvalidData)?
                .to_owned();
            cursor += size;
            events.push(MemoryEvent {
                id,
                timestamp_us,
                salience,
                payload,
            });
        }
        if cursor != b.len() - 4 {
            return Err(KintsugiError::InvalidData);
        }
        Ok(events)
    }
    pub fn append(path: &Path, payload: String, salience: Rational) -> Kintsugi<u64> {
        if payload.len() > 262_144 || salience < Rational::ZERO || salience > Rational::ONE {
            return Err(KintsugiError::InvalidData);
        }
        let _lock = lock(path)?;
        let mut events = Self::load(path)?;
        let id = events.len() as u64 + 1;
        let timestamp_us = u64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| KintsugiError::MemoryFault)?
                .as_micros(),
        )
        .map_err(|_| KintsugiError::MemoryFault)?;
        events.push(MemoryEvent {
            id,
            timestamp_us,
            salience,
            payload,
        });
        let mut b = EVENT_MAGIC.to_vec();
        b.extend_from_slice(&id.to_le_bytes());
        for e in events {
            b.extend_from_slice(&e.id.to_le_bytes());
            b.extend_from_slice(&e.timestamp_us.to_le_bytes());
            b.extend_from_slice(&e.salience.numerator().to_le_bytes());
            b.extend_from_slice(&e.salience.denominator().to_le_bytes());
            b.extend_from_slice(&(e.payload.len() as u32).to_le_bytes());
            b.extend_from_slice(e.payload.as_bytes());
        }
        atomic_write(path, b)?;
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dir() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "aurora-memory-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&p).unwrap();
        p
    }
    #[test]
    fn wave_history_restart_and_stale_writer() {
        let dir = dir();
        let path = dir.join("wave.m8");
        let mut grid = WaveGrid::new();
        let t = WaveMemoryTuple::new(
            432,
            500_000,
            250_000,
            u64::from(u32::MAX) + 1,
            TernarySpin::Resonance,
        )
        .unwrap();
        grid.write(1, 2, 3, t).unwrap();
        grid.write(1, 2, 3, t).unwrap();
        grid.save_to(&path).unwrap();
        let mut restored = WaveGrid::new();
        assert_eq!(restored.load_from(&path).unwrap(), 2);
        assert_eq!(restored.len(), 1);
        assert_eq!(restored.recall(1, 2, 3), Some(&t));
        grid.write(1, 2, 4, t).unwrap();
        grid.save_to(&path).unwrap();
        assert_eq!(restored.save_to(&path), Err(KintsugiError::Busy));
        assert!(grid.write(256, 0, 0, t).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn corrupted_or_truncated_load_is_transactional() {
        let dir = dir();
        let path = dir.join("wave.m8");
        let mut grid = WaveGrid::new();
        let t = WaveMemoryTuple::new(432, 0, 0, 1, TernarySpin::Stillness).unwrap();
        grid.write(0, 0, 0, t).unwrap();
        grid.save_to(&path).unwrap();
        let good = fs::read(&path).unwrap();
        for n in 0..good.len() {
            fs::write(&path, &good[..n]).unwrap();
            assert!(grid.load_from(&path).is_err());
            assert_eq!(grid.epochs(), 1);
        }
        let mut bad = good;
        bad[25] ^= 1;
        fs::write(&path, bad).unwrap();
        assert!(grid.load_from(&path).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn events_recall_facts_and_preserve_corrupt_files() {
        let dir = dir();
        let path = dir.join("events.m8");
        MemoryLog::append(
            &path,
            "repo=aurora; result=test_failed".into(),
            Rational::new(1, 2).unwrap(),
        )
        .unwrap();
        let events = MemoryLog::load(&path).unwrap();
        assert_eq!(events[0].salience.to_string(), "1/2");
        assert!(events[0].payload.contains("test_failed"));
        let held = lock(&path).unwrap();
        assert_eq!(
            MemoryLog::append(&path, "other".into(), Rational::ONE),
            Err(KintsugiError::Busy)
        );
        drop(held);
        fs::write(&path, b"legacy memory").unwrap();
        assert_eq!(
            MemoryLog::append(&path, "overwrite".into(), Rational::ONE),
            Err(KintsugiError::LegacyFormat)
        );
        assert_eq!(fs::read(&path).unwrap(), b"legacy memory");
        fs::remove_dir_all(dir).unwrap();
    }
}
