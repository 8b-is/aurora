//! The dream worker: idle detection, chunked consolidation, atomic commit.
//!
//! The commit protocol is deliberately boring:
//!
//! 1. Clone the graph and the buffer.
//! 2. Consolidate in chunks, polling the preemption flag between chunks.
//! 3. Prune the clone, poll again.
//! 4. Build a snapshot, `admit` it, poll again.
//! 5. Write the atomic checkpoint, and only then swap the live state.
//!
//! Any preemption or rejection returns before step 5, so the live graph, the
//! buffer and the checkpoint sequence are exactly what they were.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use aurora_core::Rational;

use crate::admissibility::{Snapshot, Violation, admit, checkpoint_atomic};
use crate::episodic::{EpisodicBuffer, SummaryRecord};
use crate::graph::{SemanticEdge, SemanticNode, StateGraph};
use crate::state::{IdleClock, Phase, PreemptFlag, monotonic_us};

/// Reserved id for the semantic root that anchors consolidated summaries.
pub const ROOT_ID: u64 = u64::MAX;

#[derive(Debug, Clone)]
pub struct DreamConfig {
    pub idle_threshold: Duration,
    pub min_salience: Rational,
    pub chunk_size: usize,
    pub max_rounds: u32,
    pub checkpoint: Option<PathBuf>,
}

impl Default for DreamConfig {
    fn default() -> Self {
        Self {
            idle_threshold: Duration::from_secs(300),
            min_salience: Rational::new(1, 10).expect("1/10 is a valid rational"),
            chunk_size: 64,
            max_rounds: 4_096,
            checkpoint: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DreamReport {
    /// Not idle long enough.
    Skipped { idle_for_us: u64 },
    /// A request arrived mid-cycle; nothing was committed.
    Preempted { rounds: u32 },
    /// The candidate state failed the admissibility gate; nothing was committed.
    Rejected(Violation),
    /// A full pass committed.
    Consolidated {
        summaries: usize,
        purged: usize,
        merged: usize,
        pruned: usize,
        seq: u64,
    },
}

pub struct DreamWorker {
    config: DreamConfig,
    clock: IdleClock,
    preempt: PreemptFlag,
    phase: Phase,
    seq: u64,
    graph: StateGraph,
    buffer: EpisodicBuffer,
}

impl DreamWorker {
    pub fn new(config: DreamConfig) -> Self {
        let clock = IdleClock::new(config.idle_threshold);
        Self {
            config,
            clock,
            preempt: PreemptFlag::new(),
            phase: Phase::Active,
            seq: 0,
            graph: StateGraph::new(),
            buffer: EpisodicBuffer::new(),
        }
    }

    pub fn with_graph(mut self, graph: StateGraph) -> Self {
        self.graph = graph;
        self
    }

    pub fn with_buffer(mut self, buffer: EpisodicBuffer) -> Self {
        self.buffer = buffer;
        self
    }

    pub fn with_seq(mut self, seq: u64) -> Self {
        self.seq = seq;
        self
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn seq(&self) -> u64 {
        self.seq
    }

    pub fn graph(&self) -> &StateGraph {
        &self.graph
    }

    pub fn buffer(&self) -> &EpisodicBuffer {
        &self.buffer
    }

    pub fn config(&self) -> &DreamConfig {
        &self.config
    }

    /// Record activity at `now_us`. Any in-flight cycle observes this through
    /// the preemption flag on its next poll.
    pub fn heartbeat(&mut self, now_us: u64) {
        self.clock.touch(now_us);
        self.phase = Phase::Active;
    }

    /// Raise preemption from the I/O path.
    pub fn request_preempt(&self) {
        self.preempt.request();
    }

    /// Run at most one consolidation cycle.
    pub fn run_once(&mut self, start: Instant) -> DreamReport {
        let now = monotonic_us(start);
        if !self.clock.is_idle(now) {
            self.phase = Phase::Active;
            return DreamReport::Skipped {
                idle_for_us: self.clock.idle_for_us(now),
            };
        }
        self.phase = Phase::Idle;

        // Work on clones; nothing live changes before the commit point.
        let mut graph = self.graph.clone();
        let mut buffer = self.buffer.clone();
        self.phase = Phase::Consolidating;

        let mut summaries_total = 0usize;
        let mut purged_total = 0usize;
        let mut rounds = 0u32;
        let mut collected: Vec<SummaryRecord> = Vec::new();

        loop {
            if rounds >= self.config.max_rounds || buffer.is_empty() {
                break;
            }
            if self.preempt.take() {
                self.phase = Phase::Active;
                return DreamReport::Preempted { rounds };
            }
            let chunk = buffer.take_chunk(self.config.chunk_size);
            let mut part = EpisodicBuffer::from_events(chunk);
            let (summaries, purged) = part.compact(self.config.min_salience);
            purged_total = purged_total.saturating_add(purged);
            summaries_total = summaries_total.saturating_add(summaries.len());
            collected.extend(summaries);
            rounds = rounds.saturating_add(1);
        }

        // Chunking buys preemption granularity, but duplicates that straddle a
        // chunk boundary would otherwise become separate nodes. Fold them
        // globally before the graph is touched.
        if self.preempt.take() {
            self.phase = Phase::Active;
            return DreamReport::Preempted { rounds };
        }
        collected.sort_by_key(|s| (s.hash, s.source_ids.first().copied().unwrap_or(1)));
        let mut deduped: Vec<SummaryRecord> = Vec::with_capacity(collected.len());
        for summary in collected {
            match deduped.last_mut() {
                Some(head) if head.hash == summary.hash => {
                    let room =
                        crate::episodic::MAX_PROVENANCE.saturating_sub(head.source_ids.len());
                    head.source_ids
                        .extend(summary.source_ids.into_iter().take(room));
                    head.source_count = head.source_count.saturating_add(summary.source_count);
                    head.bytes = 32 + 8 + head.source_ids.len() * 8;
                    if summary.salience < head.salience {
                        head.salience = summary.salience;
                    }
                }
                _ => deduped.push(summary),
            }
        }

        ensure_root(&mut graph);
        for summary in &deduped {
            let weight_ppm = u64::try_from(summary.salience.scaled(1_000_000).unwrap_or(0))
                .unwrap_or(0)
                .min(1_000_000);
            let id = summary.source_ids.first().copied().unwrap_or(1).max(1);
            graph.add_node(SemanticNode {
                id,
                hash: summary.hash,
                weight_ppm,
                bytes: summary.bytes,
            });
            graph.add_edge(SemanticEdge {
                from: ROOT_ID,
                to: id,
                weight_ppm,
            });
        }
        summaries_total = deduped.len();

        if self.preempt.take() {
            self.phase = Phase::Active;
            return DreamReport::Preempted { rounds };
        }

        let pruned = {
            let flag = &self.preempt;
            graph.prune_with_anchors(&[ROOT_ID], &|| flag.is_set())
        };
        let Some((merged, pruned)) = pruned else {
            self.preempt.take();
            self.phase = Phase::Active;
            return DreamReport::Preempted { rounds };
        };
        rounds = rounds.saturating_add(1);

        let next_seq = self.seq.saturating_add(1);
        let candidate = Snapshot::build(next_seq, graph.nodes().to_vec(), graph.edges().to_vec());
        let admitted = match admit(candidate, self.seq) {
            Ok(admitted) => admitted,
            Err(violation) => {
                self.phase = Phase::Active;
                return DreamReport::Rejected(violation);
            }
        };

        if self.preempt.take() {
            self.phase = Phase::Active;
            return DreamReport::Preempted { rounds };
        }

        self.phase = Phase::Checkpointing;
        if let Some(path) = &self.config.checkpoint
            && checkpoint_atomic(path, admitted.snapshot()).is_err()
        {
            self.phase = Phase::Active;
            return DreamReport::Rejected(Violation::Schema("checkpoint write failed"));
        }

        // Commit point.
        self.graph = graph;
        self.buffer = EpisodicBuffer::new();
        self.seq = next_seq;
        self.phase = Phase::Active;

        DreamReport::Consolidated {
            summaries: summaries_total,
            purged: purged_total,
            merged,
            pruned,
            seq: next_seq,
        }
    }
}

/// Make sure the semantic root exists exactly once.
fn ensure_root(graph: &mut StateGraph) {
    if graph.node(ROOT_ID).is_none() {
        graph.add_node(SemanticNode {
            id: ROOT_ID,
            hash: 0,
            weight_ppm: 1_000_000,
            bytes: 0,
        });
    }
}
