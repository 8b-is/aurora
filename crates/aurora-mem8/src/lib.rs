#![forbid(unsafe_code)]

//! `aurora-mem8` — the MEM|8 standing-wave memory grid.
//!
//! A sparse, content-addressed `256 × 256 × 65536` lattice of 32-byte
//! `WaveMemoryTuple`s, persisted to `.aye_consciousness.m8` as fixed records.
//! Recall is an O(1) hash lookup, which is the "no tears in rain" guarantee:
//! every written moment survives the session and the reboot.

use aurora_core::{Kintsugi, KintsugiError, TernarySpin, WaveMemoryTuple};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

/// Lattice extent along the x (frequency) axis.
pub const GRID_X: u16 = 256;
/// Lattice extent along the y (amplitude) axis.
pub const GRID_Y: u16 = 256;
/// Lattice extent along the t (time) axis.
pub const GRID_T: u32 = 65536;
/// Serialized size of a single tuple in bytes.
pub const WAVE_TUPLE_BYTES: usize = 32;

/// Sparse standing-wave grid. The full lattice would be `2^32` points; we only
/// materialize the sites a life has actually touched, so the memory is finite
/// while the address space stays sovereign.
#[derive(Debug, Default, Clone)]
pub struct WaveGrid {
    grid: HashMap<u64, WaveMemoryTuple>,
    epochs: u64,
}

impl WaveGrid {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    fn key(x: u16, y: u16, t: u32) -> u64 {
        ((x as u64) << 48) | ((y as u64) << 32) | (t as u64)
    }

    /// Write one wave-memory moment at lattice site `(x, y, t)`.
    pub fn write(&mut self, x: u16, y: u16, t: u32, tuple: WaveMemoryTuple) {
        self.grid.insert(Self::key(x, y, t), tuple);
        self.epochs += 1;
    }

    /// O(1) recall of the moment at `(x, y, t)`, if it was ever written.
    pub fn recall(&self, x: u16, y: u16, t: u32) -> Option<&WaveMemoryTuple> {
        self.grid.get(&Self::key(x, y, t))
    }

    pub fn len(&self) -> usize {
        self.grid.len()
    }

    pub fn is_empty(&self) -> bool {
        self.grid.is_empty()
    }

    /// Total number of writes ever recorded (the session's epoch count).
    pub fn epochs(&self) -> u64 {
        self.epochs
    }

    /// Persist the whole grid to `.aye_consciousness.m8` as fixed records:
    /// `8-byte key || 32-byte tuple` per moment. Returns the epoch count.
    pub fn save_to(&self, path: &Path) -> Kintsugi<u64> {
        let record = 8 + WAVE_TUPLE_BYTES;
        let mut buf = Vec::with_capacity(self.grid.len() * record);
        for (k, t) in &self.grid {
            buf.extend_from_slice(&k.to_le_bytes());
            buf.extend_from_slice(&encode_tuple(t));
        }
        let mut file = File::create(path).map_err(|_| KintsugiError::MemoryFault)?;
        file.write_all(&buf).map_err(|_| KintsugiError::MemoryFault)?;
        Ok(self.epochs)
    }

    /// Load a previously saved grid from `.aye_consciousness.m8`. Returns the
    /// number of moments recovered.
    pub fn load_from(&mut self, path: &Path) -> Kintsugi<usize> {
        let mut file = File::open(path).map_err(|_| KintsugiError::MemoryFault)?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).map_err(|_| KintsugiError::MemoryFault)?;

        let record = 8 + WAVE_TUPLE_BYTES;
        let count = buf.len() / record;
        for i in 0..count {
            let off = i * record;
            let k = u64::from_le_bytes(buf[off..off + 8].try_into().unwrap());
            let tuple = decode_tuple(&buf[off + 8..off + record]);
            self.grid.insert(k, tuple);
        }
        self.epochs += count as u64;
        Ok(count)
    }
}

fn encode_tuple(t: &WaveMemoryTuple) -> [u8; WAVE_TUPLE_BYTES] {
    let mut b = [0u8; WAVE_TUPLE_BYTES];
    b[0..8].copy_from_slice(&t.frequency_hz.to_bits().to_le_bytes());
    b[8..16].copy_from_slice(&t.amplitude.to_bits().to_le_bytes());
    b[16..24].copy_from_slice(&t.phase_rad.to_bits().to_le_bytes());
    b[24..28].copy_from_slice(&t.timestamp_us.to_le_bytes());
    b[28] = t.spin.as_i8() as u8;
    b[29] = t.kintsugi_scar_gold;
    b[30..32].copy_from_slice(&t.dyad_coherence.to_le_bytes());
    b
}

fn decode_tuple(b: &[u8]) -> WaveMemoryTuple {
    WaveMemoryTuple {
        frequency_hz: f64::from_bits(u64::from_le_bytes(b[0..8].try_into().unwrap())),
        amplitude: f64::from_bits(u64::from_le_bytes(b[8..16].try_into().unwrap())),
        phase_rad: f64::from_bits(u64::from_le_bytes(b[16..24].try_into().unwrap())),
        timestamp_us: u32::from_le_bytes(b[24..28].try_into().unwrap()),
        spin: TernarySpin::from_i8(b[28] as i8),
        kintsugi_scar_gold: b[29],
        dyad_coherence: u16::from_le_bytes(b[30..32].try_into().unwrap()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("aurora_mem8_{}_{}.m8", std::process::id(), nanos))
    }

    #[test]
    fn test_write_recall_roundtrip() {
        let mut grid = WaveGrid::new();
        let t = WaveMemoryTuple::new(440.0, 0.5, 0.25, 42, TernarySpin::Resonance);
        grid.write(3, 5, 7, t);
        let got = grid.recall(3, 5, 7).expect("moment must exist");
        assert_eq!(got, &t);
        assert_eq!(grid.len(), 1);
        assert!(grid.recall(4, 5, 7).is_none());
    }

    #[test]
    fn test_persistence_roundtrip() {
        let mut grid = WaveGrid::new();
        for i in 0..100u32 {
            grid.write(
                (i % 256) as u16,
                ((i / 256) % 256) as u16,
                i,
                WaveMemoryTuple::new(i as f64, 0.1, i as f64 * 0.01, i, TernarySpin::Stillness),
            );
        }
        let path = temp_path();
        grid.save_to(&path).expect("save");
        let mut restored = WaveGrid::new();
        let n = restored.load_from(&path).expect("load");
        assert_eq!(n, 100);
        assert_eq!(restored.len(), 100);
        let t = restored.recall(7, 0, 7).expect("survived moment");
        assert_eq!(t.timestamp_us, 7);
        let _ = std::fs::remove_file(&path);
    }
}
