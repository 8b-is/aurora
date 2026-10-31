//! Read-only index over an `AURSNAP1` snapshot.
//!
//! Layout (little-endian unless noted), 64-byte header:
//!
//! ```text
//!   0..8   magic "AURSNAP1"
//!   8..12  version (u32, = 1)
//!  12..16  flags (u32)
//!  16..24  epoch (u64)
//!  24..32  node_count (u64)
//!  32..40  expected_size (u64)
//!  40..48  arena_offset (u64)
//!  48..64  reserved
//! ```
//!
//! Then `node_count` entries of 32 bytes:
//!
//! ```text
//!   0..8    salience_micros (i64, millionths)
//!   8..12   payload_off (u32)
//!  12..16   payload_len (u32)
//!  16..20   id_off (u32)
//!  20..22   id_len (u16)
//!  22       scope (u8)
//!  23       pinned (u8)
//!  24..32   key_prefix (u64, BIG-endian)
//! ```
//!
//! Then the packed UTF-8 arena, then a 32-byte footer digest.
//!
//! `key_prefix` is the first eight bytes of the id, zero-padded and stored
//! big-endian, so comparing it as an integer is the same as comparing those
//! bytes lexicographically. Entries are sorted by id, so a binary search can
//! usually decide on the prefix alone and only touch the arena when two ids
//! share their first eight bytes.

use std::io::{Error, ErrorKind, Result};

pub const MAGIC: &[u8; 8] = b"AURSNAP1";
pub const VERSION: u32 = 1;
pub const HEADER_SIZE: usize = 64;
pub const NODE_STRIDE: usize = 32;
pub const FOOTER_SIZE: usize = 32;
pub const MINIMUM_ENVELOPE: usize = HEADER_SIZE + FOOTER_SIZE;

const OFF_VERSION: usize = 8;
const OFF_FLAGS: usize = 12;
const OFF_EPOCH: usize = 16;
const OFF_NODE_COUNT: usize = 24;
const OFF_EXPECTED_SIZE: usize = 32;
const OFF_ARENA_OFFSET: usize = 40;

const NODE_SALIENCE: usize = 0;
const NODE_PAYLOAD_OFF: usize = 8;
const NODE_PAYLOAD_LEN: usize = 12;
const NODE_ID_OFF: usize = 16;
const NODE_ID_LEN: usize = 20;
const NODE_SCOPE: usize = 22;
const NODE_PINNED: usize = 23;
const NODE_PREFIX: usize = 24;

fn invalid<T>(msg: &str) -> Result<T> {
    Err(Error::new(ErrorKind::InvalidData, msg))
}

fn read_u32(buf: &[u8], off: usize) -> Result<u32> {
    let bytes = buf
        .get(off..off + 4)
        .ok_or_else(|| Error::new(ErrorKind::UnexpectedEof, "u32 past buffer end"))?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("4 bytes")))
}

fn read_u64(buf: &[u8], off: usize) -> Result<u64> {
    let bytes = buf
        .get(off..off + 8)
        .ok_or_else(|| Error::new(ErrorKind::UnexpectedEof, "u64 past buffer end"))?;
    Ok(u64::from_le_bytes(bytes.try_into().expect("8 bytes")))
}

fn read_u16(buf: &[u8], off: usize) -> Result<u16> {
    let bytes = buf
        .get(off..off + 2)
        .ok_or_else(|| Error::new(ErrorKind::UnexpectedEof, "u16 past buffer end"))?;
    Ok(u16::from_le_bytes(bytes.try_into().expect("2 bytes")))
}

fn read_i64(buf: &[u8], off: usize) -> Result<i64> {
    let bytes = buf
        .get(off..off + 8)
        .ok_or_else(|| Error::new(ErrorKind::UnexpectedEof, "i64 past buffer end"))?;
    Ok(i64::from_le_bytes(bytes.try_into().expect("8 bytes")))
}

/// Capability scope codes; must match `SCOPE_TO_ID` in the Python writer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityScope {
    CoreRead = 1,
    CoreWrite = 2,
    MemoryEpisodic = 3,
    MemorySemantic = 4,
}

impl CapabilityScope {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::CoreRead),
            2 => Some(Self::CoreWrite),
            3 => Some(Self::MemoryEpisodic),
            4 => Some(Self::MemorySemantic),
            _ => None,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// First eight id bytes, zero-padded, big-endian.
///
/// Big-endian keeps integer order equal to byte order: `0x61_62..` sorts
/// before `0x62_61..`, exactly as `b"ab"` sorts before `b"ba"`.
#[inline]
pub fn pack_key_prefix(key: &[u8]) -> u64 {
    let mut buf = [0u8; 8];
    let n = key.len().min(8);
    buf[..n].copy_from_slice(&key[..n]);
    u64::from_be_bytes(buf)
}

/// One node decoded out of the mapping. Both slices borrow the index buffer.
#[derive(Clone, Copy, Debug)]
pub struct ZeroCopyNode<'a> {
    pub id: &'a [u8],
    pub payload: &'a [u8],
    pub salience_micros: i64,
    pub scope: CapabilityScope,
    pub pinned: bool,
}

/// Read-only view over a validated `AURSNAP1` buffer.
#[derive(Clone, Copy, Debug)]
pub struct AurSnapIndex<'a> {
    buf: &'a [u8],
    node_count: usize,
    arena_offset: usize,
    epoch: u64,
}

impl<'a> AurSnapIndex<'a> {
    /// Validate the structural invariants and build the index.
    ///
    /// Does not verify the footer digest; see the crate note.
    pub fn open(buf: &'a [u8]) -> Result<Self> {
        if buf.len() < MINIMUM_ENVELOPE {
            return invalid("buffer is smaller than the minimum envelope");
        }
        if &buf[..MAGIC.len()] != MAGIC {
            return invalid("bad magic");
        }
        let version = read_u32(buf, OFF_VERSION)?;
        if version != VERSION {
            return invalid("unsupported version");
        }
        if read_u32(buf, OFF_FLAGS)? != 0 || buf[48..64].iter().any(|byte| *byte != 0) {
            return invalid("unsupported flags or reserved bytes");
        }

        let epoch = read_u64(buf, OFF_EPOCH)?;
        let declared_count = read_u64(buf, OFF_NODE_COUNT)?;
        let expected_size = read_u64(buf, OFF_EXPECTED_SIZE)?;
        let declared_arena = read_u64(buf, OFF_ARENA_OFFSET)?;

        if expected_size != buf.len() as u64 {
            return invalid("declared size does not match the buffer length");
        }

        let node_count = usize::try_from(declared_count)
            .map_err(|_| Error::new(ErrorKind::InvalidData, "node count overflows usize"))?;
        let table_end = node_count
            .checked_mul(NODE_STRIDE)
            .and_then(|t| t.checked_add(HEADER_SIZE))
            .ok_or_else(|| Error::new(ErrorKind::InvalidData, "index table overflows"))?;

        if table_end
            != usize::try_from(declared_arena)
                .map_err(|_| Error::new(ErrorKind::InvalidData, "arena offset overflows usize"))?
        {
            return invalid("arena offset does not follow the index table");
        }
        if table_end > buf.len() - FOOTER_SIZE {
            return invalid("arena offset runs past the footer");
        }

        Ok(Self {
            buf,
            node_count,
            arena_offset: table_end,
            epoch,
        })
    }

    pub fn node_count(&self) -> usize {
        self.node_count
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn arena_offset(&self) -> usize {
        self.arena_offset
    }

    pub fn arena_limit(&self) -> usize {
        self.buf.len() - FOOTER_SIZE
    }

    #[inline]
    fn entry_offset(&self, index: usize) -> usize {
        HEADER_SIZE + index * NODE_STRIDE
    }

    #[inline]
    fn prefix_at(&self, index: usize) -> u64 {
        let e = self.entry_offset(index) + NODE_PREFIX;
        u64::from_be_bytes(
            self.buf[e..e + 8]
                .try_into()
                .expect("entry bounds are validated at open"),
        )
    }

    fn arena_slice(&self, offset: usize, len: usize) -> Result<&'a [u8]> {
        if offset < self.arena_offset {
            return invalid("slice starts before the arena");
        }
        let end = offset
            .checked_add(len)
            .ok_or_else(|| Error::new(ErrorKind::InvalidData, "slice offset overflow"))?;
        if end > self.arena_limit() {
            return invalid("slice ends past the arena limit");
        }
        Ok(&self.buf[offset..end])
    }

    pub fn id_at(&self, index: usize) -> Result<&'a [u8]> {
        if index >= self.node_count {
            return invalid("node index out of range");
        }
        let e = self.entry_offset(index);
        let off = read_u32(self.buf, e + NODE_ID_OFF)? as usize;
        let len = read_u16(self.buf, e + NODE_ID_LEN)? as usize;
        self.arena_slice(off, len)
    }

    fn payload_at(&self, index: usize) -> Result<&'a [u8]> {
        if index >= self.node_count {
            return invalid("node index out of range");
        }
        let e = self.entry_offset(index);
        let off = read_u32(self.buf, e + NODE_PAYLOAD_OFF)? as usize;
        let len = read_u32(self.buf, e + NODE_PAYLOAD_LEN)? as usize;
        self.arena_slice(off, len)
    }

    pub fn decode_node_at(&self, index: usize) -> Result<ZeroCopyNode<'a>> {
        if index >= self.node_count {
            return invalid("node index out of range");
        }
        let e = self.entry_offset(index);
        let scope_byte = self.buf[e + NODE_SCOPE];
        let scope = CapabilityScope::from_u8(scope_byte)
            .ok_or_else(|| Error::new(ErrorKind::InvalidData, "unknown capability scope"))?;
        if self.buf[e + NODE_PINNED] > 1 {
            return invalid("invalid pinned flag");
        }
        Ok(ZeroCopyNode {
            id: self.id_at(index)?,
            payload: self.payload_at(index)?,
            salience_micros: read_i64(self.buf, e + NODE_SALIENCE)?,
            scope,
            pinned: self.buf[e + NODE_PINNED] != 0,
        })
    }

    /// Binary search by full id, comparing each probe against the arena.
    ///
    /// This is the default: it makes no assumption about the ids beyond their
    /// sorted order, so a shared namespace prefix costs nothing extra.
    pub fn get_by_id(&self, target: &[u8]) -> Result<Option<ZeroCopyNode<'a>>> {
        let mut low = 0usize;
        let mut high = self.node_count;

        while low < high {
            let mid = low + ((high - low) >> 1);
            match self.id_at(mid)?.cmp(target) {
                std::cmp::Ordering::Equal => return self.decode_node_at(mid).map(Some),
                std::cmp::Ordering::Less => low = mid + 1,
                std::cmp::Ordering::Greater => high = mid,
            }
        }

        Ok(None)
    }

    /// Binary search that decides on the stored 8-byte prefix first.
    ///
    /// Faster when the first eight id bytes are mostly distinct. When ids
    /// share a prefix (a `"episodic:"` namespace, say), every probe falls back
    /// to the same arena comparison the plain search makes, so the extra prefix
    /// load costs nothing measurable: roughly neutral, not slower. Put a
    /// namespace in the scope byte anyway, so ids stay sortable across scopes.
    pub fn get_by_id_prefixed(&self, target: &[u8]) -> Result<Option<ZeroCopyNode<'a>>> {
        let target_prefix = pack_key_prefix(target);
        let mut low = 0usize;
        let mut high = self.node_count;

        while low < high {
            let mid = low + ((high - low) >> 1);
            match self.prefix_at(mid).cmp(&target_prefix) {
                std::cmp::Ordering::Less => low = mid + 1,
                std::cmp::Ordering::Greater => high = mid,
                std::cmp::Ordering::Equal => match self.id_at(mid)?.cmp(target) {
                    std::cmp::Ordering::Equal => return self.decode_node_at(mid).map(Some),
                    std::cmp::Ordering::Less => low = mid + 1,
                    std::cmp::Ordering::Greater => high = mid,
                },
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_python_writer_golden_snapshot() {
        let hex = include_str!("../../../tests/fixtures/aursnap1.hex").trim();
        let buf: Vec<u8> = hex
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        let index = AurSnapIndex::open(&buf).unwrap();
        assert_eq!(index.epoch(), 7);
        assert_eq!(index.node_count(), 2);
        let a = index.get_by_id(b"node/a").unwrap().unwrap();
        assert_eq!(a.payload, b"alpha\x00payload");
        assert_eq!(a.salience_micros, -500_000);
        assert_eq!(a.scope, CapabilityScope::CoreRead);
        assert!(!a.pinned);
        let b = index
            .get_by_id_prefixed("node/β".as_bytes())
            .unwrap()
            .unwrap();
        assert_eq!(b.payload, b"\xffbeta");
        assert_eq!(b.salience_micros, 5_000_000_000);
        assert_eq!(b.scope, CapabilityScope::MemorySemantic);
        assert!(b.pinned);
    }

    #[test]
    fn rejects_unknown_layout_flags_and_non_boolean_pinned() {
        let mut f = build(&[b"a"], &[b"payload"]);
        f.buf[OFF_FLAGS] = 0x62;
        assert!(AurSnapIndex::open(&f.buf).is_err());
        f.buf[OFF_FLAGS] = 0;
        f.buf[HEADER_SIZE + NODE_PINNED] = 2;
        assert!(
            AurSnapIndex::open(&f.buf)
                .unwrap()
                .decode_node_at(0)
                .is_err()
        );
    }

    struct Fixture {
        buf: Vec<u8>,
    }

    fn build(ids: &[&[u8]], payloads: &[&[u8]]) -> Fixture {
        assert_eq!(ids.len(), payloads.len());
        let n = ids.len();
        let arena_offset = HEADER_SIZE + n * NODE_STRIDE;
        let mut arena = Vec::new();
        let mut table = Vec::new();

        for (id, payload) in ids.iter().zip(payloads) {
            let id_off = arena_offset + arena.len();
            arena.extend_from_slice(id);
            let payload_off = arena_offset + arena.len();
            arena.extend_from_slice(payload);

            let mut entry = [0u8; NODE_STRIDE];
            entry[NODE_SALIENCE..NODE_SALIENCE + 8].copy_from_slice(&500_000i64.to_le_bytes());
            entry[NODE_PAYLOAD_OFF..NODE_PAYLOAD_OFF + 4]
                .copy_from_slice(&(payload_off as u32).to_le_bytes());
            entry[NODE_PAYLOAD_LEN..NODE_PAYLOAD_LEN + 4]
                .copy_from_slice(&(payload.len() as u32).to_le_bytes());
            entry[NODE_ID_OFF..NODE_ID_OFF + 4].copy_from_slice(&(id_off as u32).to_le_bytes());
            entry[NODE_ID_LEN..NODE_ID_LEN + 2].copy_from_slice(&(id.len() as u16).to_le_bytes());
            entry[NODE_SCOPE] = CapabilityScope::MemorySemantic.as_u8();
            entry[NODE_PREFIX..NODE_PREFIX + 8].copy_from_slice(&pack_key_prefix(id).to_be_bytes());
            table.extend_from_slice(&entry);
        }

        let total = arena_offset + arena.len() + FOOTER_SIZE;
        let mut buf = vec![0u8; total];
        buf[..8].copy_from_slice(MAGIC);
        buf[OFF_VERSION..OFF_VERSION + 4].copy_from_slice(&VERSION.to_le_bytes());
        buf[OFF_EPOCH..OFF_EPOCH + 8].copy_from_slice(&9u64.to_le_bytes());
        buf[OFF_NODE_COUNT..OFF_NODE_COUNT + 8].copy_from_slice(&(n as u64).to_le_bytes());
        buf[OFF_EXPECTED_SIZE..OFF_EXPECTED_SIZE + 8]
            .copy_from_slice(&(total as u64).to_le_bytes());
        buf[OFF_ARENA_OFFSET..OFF_ARENA_OFFSET + 8]
            .copy_from_slice(&(arena_offset as u64).to_le_bytes());
        buf[HEADER_SIZE..HEADER_SIZE + table.len()].copy_from_slice(&table);
        buf[arena_offset..arena_offset + arena.len()].copy_from_slice(&arena);
        Fixture { buf }
    }

    #[test]
    fn finds_present_and_missing_ids() {
        let ids: [&[u8]; 5] = [
            b"core:alpha",
            b"core:beta",
            b"memory:episodic:a",
            b"memory:episodic:b",
            b"zzz",
        ];
        let payloads: [&[u8]; 5] = [b"pa", b"pb", b"pc", b"pd", b"pe"];
        let f = build(&ids, &payloads);
        let index = AurSnapIndex::open(&f.buf).unwrap();

        assert_eq!(index.node_count(), 5);
        assert_eq!(index.epoch(), 9);

        for (id, payload) in ids.iter().zip(payloads) {
            let node = index.get_by_id(id).unwrap().expect("present");
            assert_eq!(node.id, *id);
            assert_eq!(node.payload, payload);
            assert_eq!(node.scope, CapabilityScope::MemorySemantic);
            assert!(!node.pinned);
        }

        assert!(index.get_by_id(b"core:gamma").unwrap().is_none());
        assert!(index.get_by_id(b"").unwrap().is_none());
        assert!(index.get_by_id(b"zzzz").unwrap().is_none());
    }

    #[test]
    fn shared_eight_byte_prefix_falls_back_to_full_id() {
        let ids: [&[u8]; 3] = [b"episodic:0001", b"episodic:0002", b"episodic:0003"];
        let payloads: [&[u8]; 3] = [b"1", b"2", b"3"];
        let f = build(&ids, &payloads);
        let index = AurSnapIndex::open(&f.buf).unwrap();

        for (id, payload) in ids.iter().zip(payloads) {
            let node = index.get_by_id(id).unwrap().expect("present");
            assert_eq!(node.payload, payload);
        }
        assert!(index.get_by_id(b"episodic:0004").unwrap().is_none());
        assert!(index.get_by_id(b"episodic:000").unwrap().is_none());
    }

    #[test]
    fn plain_and_prefixed_search_agree() {
        let ids: [&[u8]; 4] = [b"episodic:0001", b"episodic:0002", b"other", b"zzz"];
        let payloads: [&[u8]; 4] = [b"1", b"2", b"3", b"4"];
        let f = build(&ids, &payloads);
        let index = AurSnapIndex::open(&f.buf).unwrap();

        for (id, payload) in ids.iter().zip(payloads) {
            let plain = index.get_by_id(id).unwrap().expect("plain finds it");
            let prefixed = index
                .get_by_id_prefixed(id)
                .unwrap()
                .expect("prefixed finds it");
            assert_eq!(plain.id, prefixed.id);
            assert_eq!(plain.payload, payload);
            assert_eq!(prefixed.payload, payload);
        }
        let misses: [&[u8]; 3] = [b"episodic:0003", b"other2", b"aaaa"];
        for miss in misses {
            assert!(index.get_by_id(miss).unwrap().is_none());
            assert!(index.get_by_id_prefixed(miss).unwrap().is_none());
        }
    }

    #[test]
    fn rejects_structurally_invalid_buffers() {
        let ids: [&[u8]; 2] = [b"a", b"b"];
        let payloads: [&[u8]; 2] = [b"x", b"y"];

        let mut short = build(&ids, &payloads);
        short.buf.truncate(MINIMUM_ENVELOPE - 1);
        assert!(AurSnapIndex::open(&short.buf).is_err());

        let mut bad_magic = build(&ids, &payloads);
        bad_magic.buf[0] = b'X';
        assert!(AurSnapIndex::open(&bad_magic.buf).is_err());

        let mut bad_size = build(&ids, &payloads);
        bad_size.buf[OFF_EXPECTED_SIZE] ^= 0xFF;
        assert!(AurSnapIndex::open(&bad_size.buf).is_err());

        let mut bad_arena = build(&ids, &payloads);
        bad_arena.buf[OFF_ARENA_OFFSET] ^= 0x01;
        assert!(AurSnapIndex::open(&bad_arena.buf).is_err());
    }

    #[test]
    fn rejects_out_of_bounds_slices() {
        let ids: [&[u8]; 1] = [b"only"];
        let payloads: [&[u8]; 1] = [b"data"];
        let mut f = build(&ids, &payloads);
        let index = AurSnapIndex::open(&f.buf).unwrap();
        assert!(index.decode_node_at(0).is_ok());

        // Push id_off past the arena limit.
        let e = HEADER_SIZE + NODE_ID_OFF;
        let past_end = f.buf.len() as u32;
        f.buf[e..e + 4].copy_from_slice(&past_end.to_le_bytes());
        let index = AurSnapIndex::open(&f.buf).unwrap();
        assert!(index.decode_node_at(0).is_err());
    }

    #[test]
    fn prefix_packing_preserves_byte_order() {
        assert!(pack_key_prefix(b"ab") < pack_key_prefix(b"ba"));
        assert!(pack_key_prefix(b"a") < pack_key_prefix(b"ab"));
        assert_eq!(
            pack_key_prefix(b"episodic:1"),
            pack_key_prefix(b"episodic:2")
        );
        assert_eq!(
            pack_key_prefix(b"12345678"),
            u64::from_be_bytes(*b"12345678")
        );
        assert_eq!(pack_key_prefix(b""), 0);
    }
}
