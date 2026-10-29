//! AURSNAP1 lookup benchmark: inlined 8-byte prefix vs arena dereference.
//!
//! Zero dependencies. Run with:
//!
//! ```text
//! RUSTFLAGS="-C target-cpu=native" \
//!   cargo run --release -p aurora-storage --example bench_lookup
//! ```
//!
//! `AURORA_BENCH_NODES` overrides the node count (default 1_000_000).

use std::cmp::Ordering;
use std::hint::black_box;
use std::io::Result;
use std::time::Instant;

use aurora_storage::mmap_index::{
    AurSnapIndex, CapabilityScope, FOOTER_SIZE, HEADER_SIZE, MAGIC, NODE_STRIDE, ZeroCopyNode,
    pack_key_prefix,
};

const DEFAULT_NODE_COUNT: usize = 1_000_000;
const PAYLOAD_LEN: usize = 64;
const QUERY_RING_LEN: usize = 4096;

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

#[inline]
fn write_hex_u64(val: u64, out: &mut [u8; 16]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for (i, slot) in out.iter_mut().enumerate() {
        let shift = (15 - i) * 4;
        *slot = HEX[((val >> shift) & 0xF) as usize];
    }
}

/// Baseline: ignore `key_prefix`, dereference `id_off` into the arena on every
/// probe.
#[inline]
fn get_by_id_arena_pointer<'a>(
    raw_buf: &'a [u8],
    index: &AurSnapIndex<'a>,
    target_id: &[u8],
) -> Result<Option<ZeroCopyNode<'a>>> {
    let mut low = 0usize;
    let mut high = index.node_count();

    while low < high {
        let mid = low + ((high - low) >> 1);
        let entry_off = HEADER_SIZE + (mid * NODE_STRIDE);

        let id_off = u32::from_le_bytes(raw_buf[entry_off + 16..entry_off + 20].try_into().unwrap())
            as usize;
        let id_len = u16::from_le_bytes(raw_buf[entry_off + 20..entry_off + 22].try_into().unwrap())
            as usize;

        match raw_buf[id_off..id_off + id_len].cmp(target_id) {
            Ordering::Equal => return index.decode_node_at(mid).map(Some),
            Ordering::Less => low = mid + 1,
            Ordering::Greater => high = mid,
        }
    }

    Ok(None)
}

struct Fixture {
    buf: Vec<u8>,
    hits: Vec<Vec<u8>>,
    misses: Vec<Vec<u8>>,
}

fn build_fixture(node_count: usize, shared_prefix: bool) -> Fixture {
    let mut rng = SplitMix64::new(0xA020_2600_BEEF_CAFE);

    let mut raw_ids: Vec<u64> = (0..node_count as u64)
        .map(|i| (i.wrapping_mul(0x517C_C1B7_2722_0A95)) ^ 0x9E37_79B9_7F4A_7C15)
        .collect();
    raw_ids.sort_unstable();
    raw_ids.dedup();
    assert_eq!(raw_ids.len(), node_count, "id generator collided");

    let key_len = if shared_prefix { 25 } else { 24 };
    let arena_offset = HEADER_SIZE + (node_count * NODE_STRIDE);
    let arena_size = node_count * (key_len + PAYLOAD_LEN);
    let total_size = arena_offset + arena_size + FOOTER_SIZE;
    assert!(
        total_size <= u32::MAX as usize,
        "fixture exceeds the u32 offset ceiling"
    );

    let mut buf = vec![0u8; total_size];
    buf[..8].copy_from_slice(MAGIC);
    buf[8..12].copy_from_slice(&1u32.to_le_bytes());
    buf[16..24].copy_from_slice(&1u64.to_le_bytes());
    buf[24..32].copy_from_slice(&(node_count as u64).to_le_bytes());
    buf[32..40].copy_from_slice(&(total_size as u64).to_le_bytes());
    buf[40..48].copy_from_slice(&(arena_offset as u64).to_le_bytes());

    let payload = [0x42u8; PAYLOAD_LEN];
    let mut cursor = arena_offset;
    let mut key_buf = [0u8; 25];
    let mut hex_buf = [0u8; 16];

    for (idx, &val) in raw_ids.iter().enumerate() {
        write_hex_u64(val, &mut hex_buf);
        let key = if shared_prefix {
            key_buf[..9].copy_from_slice(b"episodic:");
            key_buf[9..25].copy_from_slice(&hex_buf);
            &key_buf[..25]
        } else {
            key_buf[..16].copy_from_slice(&hex_buf);
            key_buf[16..24].copy_from_slice(b"_node_id");
            &key_buf[..24]
        };

        let id_off = cursor;
        buf[cursor..cursor + key_len].copy_from_slice(key);
        cursor += key_len;

        let payload_off = cursor;
        buf[cursor..cursor + PAYLOAD_LEN].copy_from_slice(&payload);
        cursor += PAYLOAD_LEN;

        let e = HEADER_SIZE + idx * NODE_STRIDE;
        buf[e..e + 8].copy_from_slice(&(idx as i64).to_le_bytes());
        buf[e + 8..e + 12].copy_from_slice(&(payload_off as u32).to_le_bytes());
        buf[e + 12..e + 16].copy_from_slice(&(PAYLOAD_LEN as u32).to_le_bytes());
        buf[e + 16..e + 20].copy_from_slice(&(id_off as u32).to_le_bytes());
        buf[e + 20..e + 22].copy_from_slice(&(key_len as u16).to_le_bytes());
        buf[e + 22] = CapabilityScope::MemorySemantic.as_u8();
        buf[e + 24..e + 32].copy_from_slice(&pack_key_prefix(key).to_be_bytes());
    }

    let mut hits = Vec::with_capacity(QUERY_RING_LEN);
    let mut misses = Vec::with_capacity(QUERY_RING_LEN);
    for _ in 0..QUERY_RING_LEN {
        let pick = (rng.next_u64() as usize) % node_count;
        write_hex_u64(raw_ids[pick], &mut hex_buf);
        let hit = if shared_prefix {
            let mut v = b"episodic:".to_vec();
            v.extend_from_slice(&hex_buf);
            v
        } else {
            let mut v = hex_buf.to_vec();
            v.extend_from_slice(b"_node_id");
            v
        };
        hits.push(hit);

        write_hex_u64(rng.next_u64(), &mut hex_buf);
        let miss = if shared_prefix {
            let mut v = b"episodic:".to_vec();
            v.extend_from_slice(&hex_buf);
            v.push(b'!');
            v
        } else {
            let mut v = hex_buf.to_vec();
            v.extend_from_slice(b"_miss_id");
            v
        };
        misses.push(miss);
    }

    Fixture { buf, hits, misses }
}

/// Minimum ns/op over `ROUNDS` rounds. The minimum is the least-noise
/// estimator: scheduler interference and frequency dips can only add time.
fn measure<F: FnMut()>(iters: u64, mut f: F) -> u64 {
    const ROUNDS: u64 = 5;
    for _ in 0..(iters / 10).max(1) {
        f();
    }
    let mut best = u64::MAX;
    for _ in 0..ROUNDS {
        let start = Instant::now();
        for _ in 0..iters {
            f();
        }
        let per_op = (start.elapsed().as_nanos() as u64) / iters.max(1);
        best = best.min(per_op);
    }
    best
}

fn main() {
    let node_count = std::env::var("AURORA_BENCH_NODES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_NODE_COUNT);
    let iters: u64 = std::env::var("AURORA_BENCH_ITERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(200_000);

    let index_bytes = node_count * NODE_STRIDE;
    let arena_bytes = node_count * (24 + PAYLOAD_LEN);
    let index_tenths = index_bytes / 100_000;
    let arena_tenths = arena_bytes / 100_000;
    println!(
        "AURSNAP1 lookup bench — {node_count} nodes, index {}.{} MB, arena {}.{} MB, {iters} iters/case",
        index_tenths / 10,
        index_tenths % 10,
        arena_tenths / 10,
        arena_tenths % 10
    );
    println!("{:<44} {:>9} {:>11}", "case", "ns/op", "kelem/s");

    let high = build_fixture(node_count, false);
    let index = AurSnapIndex::open(&high.buf).unwrap();
    assert_eq!(index.node_count(), node_count);

    let mask = QUERY_RING_LEN - 1;

    let mut i = 0usize;
    let ns = measure(iters, || {
        let q = &high.hits[i & mask];
        i = i.wrapping_add(1);
        let node = index.get_by_id(black_box(q)).unwrap().unwrap();
        black_box(node.payload[0]);
    });
    println!(
        "{:<44} {:>9} {:>11}",
        "hit / inline 8B prefix",
        ns,
        1_000_000 / ns.max(1)
    );

    let mut i = 0usize;
    let ns = measure(iters, || {
        let q = &high.hits[i & mask];
        i = i.wrapping_add(1);
        let node = get_by_id_arena_pointer(&high.buf, &index, black_box(q))
            .unwrap()
            .unwrap();
        black_box(node.payload[0]);
    });
    println!(
        "{:<44} {:>9} {:>11}",
        "hit / arena pointer",
        ns,
        1_000_000 / ns.max(1)
    );

    let mut i = 0usize;
    let ns = measure(iters, || {
        let q = &high.misses[i & mask];
        i = i.wrapping_add(1);
        let res = index.get_by_id(black_box(q)).unwrap();
        debug_assert!(res.is_none());
        black_box(res);
    });
    println!(
        "{:<44} {:>9} {:>11}",
        "miss / inline 8B prefix",
        ns,
        1_000_000 / ns.max(1)
    );

    let mut i = 0usize;
    let ns = measure(iters, || {
        let q = &high.misses[i & mask];
        i = i.wrapping_add(1);
        let res = get_by_id_arena_pointer(&high.buf, &index, black_box(q)).unwrap();
        debug_assert!(res.is_none());
        black_box(res);
    });
    println!(
        "{:<44} {:>9} {:>11}",
        "miss / arena pointer",
        ns,
        1_000_000 / ns.max(1)
    );

    drop(high);

    let shared = build_fixture(node_count, true);
    let shared_index = AurSnapIndex::open(&shared.buf).unwrap();

    let mut i = 0usize;
    let ns = measure(iters, || {
        let q = &shared.hits[i & mask];
        i = i.wrapping_add(1);
        let node = shared_index.get_by_id(black_box(q)).unwrap().unwrap();
        black_box(node.payload[0]);
    });
    println!(
        "{:<44} {:>9} {:>11}",
        "hit shared prefix / inline 8B (fallback)",
        ns,
        1_000_000 / ns.max(1)
    );

    let mut i = 0usize;
    let ns = measure(iters, || {
        let q = &shared.hits[i & mask];
        i = i.wrapping_add(1);
        let node = get_by_id_arena_pointer(&shared.buf, &shared_index, black_box(q))
            .unwrap()
            .unwrap();
        black_box(node.payload[0]);
    });
    println!(
        "{:<44} {:>9} {:>11}",
        "hit shared prefix / arena pointer",
        ns,
        1_000_000 / ns.max(1)
    );

    println!(
        "\nnote: timings are host-local; error figures for the kernels are not involved here.\n\
         each case: 10% warm-up then the minimum of 5 rounds, drawn from a {QUERY_RING_LEN}-entry ring."
    );
}
