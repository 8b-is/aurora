//! Episodic buffer compaction: raw interaction logs → deterministic summaries.
//!
//! The buffer is append-only during active sessions. A consolidation pass
//! purges entries below the salience floor and distils the survivors into one
//! summary per stable payload hash, in a deterministic order (hash, then id), so
//! two runs over the same input produce byte-identical summaries.

use aurora_core::Rational;
use aurora_mem8::MemoryEvent;

/// Provenance kept per summary. Beyond this many source ids only the count is
/// retained, so a summary's footprint is bounded no matter how many rows it
/// distils.
pub const MAX_PROVENANCE: usize = 64;

/// A distilled semantic record. `source_ids` preserves provenance so the
/// original episodic rows can be found in the archive, capped at
/// [`MAX_PROVENANCE`]; `source_count` keeps the true total.
///
/// `bytes` is the footprint of the record itself (hash, provenance, count), not
/// the size of the source text it stands in for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryRecord {
    pub hash: u64,
    pub salience: Rational,
    pub source_ids: Vec<u64>,
    pub source_count: u64,
    pub bytes: usize,
}

impl SummaryRecord {
    /// Footprint of the record: 32-byte hash, 8-byte count, 8 bytes per retained id.
    fn footprint(&self) -> usize {
        32 + 8 + self.source_ids.len().saturating_mul(8)
    }

    /// Returns the total size of the source payloads represented by this record.
    /// This is used to track the actual data volume being distilled.
    pub fn payload_bytes(&self, events: &[MemoryEvent]) -> usize {
        events
            .iter()
            .filter(|e| self.source_ids.contains(&e.id) || self.source_count > 0)
            .map(|e| e.payload.len())
            .sum()
    }
}

/// Append-only episodic buffer.
#[derive(Debug, Default, Clone)]
pub struct EpisodicBuffer {
    events: Vec<MemoryEvent>,
}

impl EpisodicBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_events(events: Vec<MemoryEvent>) -> Self {
        Self { events }
    }

    pub fn push(&mut self, event: MemoryEvent) {
        self.events.push(event);
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Remove and return up to `limit` rows from the front of the buffer.
    /// Gives the worker a bounded unit of work between preemption checks.
    pub fn take_chunk(&mut self, limit: usize) -> Vec<MemoryEvent> {
        let take = limit.min(self.events.len());
        self.events.drain(..take).collect()
    }

    /// Approximate retained footprint in bytes, counting a 32-byte header per row.
    pub fn bytes(&self) -> usize {
        self.events
            .iter()
            .map(|e| e.payload.len().saturating_add(32))
            .sum()
    }

    /// Distil the buffer: drop rows below `min_salience`, then collapse rows
    /// that share a payload hash into one summary. The buffer is emptied on
    /// success — the summaries are the durable artefact.
    ///
    /// Returns the summaries and the number of purged rows.
    pub fn compact(&mut self, min_salience: Rational) -> (Vec<SummaryRecord>, usize) {
        let mut kept: Vec<MemoryEvent> = Vec::with_capacity(self.events.len());
        let mut purged = 0usize;
        for event in self.events.drain(..) {
            if event.salience < min_salience {
                purged += 1;
            } else {
                kept.push(event);
            }
        }

        // Deterministic grouping: sort by (payload hash, id) then fold runs.
        kept.sort_by_key(|e| (fnv1a(e.payload.as_bytes()), e.id));

        let mut summaries: Vec<SummaryRecord> = Vec::new();
        for event in kept {
            let hash = fnv1a(event.payload.as_bytes());
            match summaries.last_mut() {
                Some(summary) if summary.hash == hash => {
                    if summary.source_ids.len() < MAX_PROVENANCE {
                        summary.source_ids.push(event.id);
                    }
                    summary.source_count = summary.source_count.saturating_add(1);
                    summary.bytes = summary.footprint();
                    // Conservative salience: a merged record is only as salient
                    // as its weakest contributor.
                    if event.salience < summary.salience {
                        summary.salience = event.salience;
                    }
                }
                _ => {
                    let record = SummaryRecord {
                        hash,
                        salience: event.salience,
                        source_ids: vec![event.id],
                        source_count: 1,
                        bytes: 0,
                    };
                    summaries.push(SummaryRecord {
                        bytes: record.footprint(),
                        ..record
                    });
                }
            }
        }

        (summaries, purged)
    }
}

/// FNV-1a 64-bit. Chosen because it is a few lines, allocation-free and stable
/// across platforms — the summaries must be reproducible.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}
