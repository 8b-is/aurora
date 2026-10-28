#![forbid(unsafe_code)]

//! `aurora-consolidation` — the Dream Phase.
//!
//! A deterministic, integer-only background worker that runs when the AURORA
//! runtime has been idle long enough. It distils episodic logs into semantic
//! summaries, prunes and deduplicates the state graph, refuses any snapshot
//! that breaks the admissibility invariants, and commits through an atomic
//! checkpoint with a rollback path.
//!
//! Three properties are load-bearing:
//!
//! 1. **Bounded observation.** Preemption is checked at every chunk boundary,
//!    and `chunk_size` bounds the work between checks, so a request arriving
//!    mid-cycle is observed promptly instead of after the whole pass.
//! 2. **No partial writes.** Consolidation mutates clones; the live graph is
//!    replaced only after `admit` and the atomic checkpoint both succeed.
//! 3. **Zero float.** Every weight, salience and hash is integer or `Rational`.

pub mod admissibility;
pub mod episodic;
pub mod graph;
pub mod state;
pub mod worker;

pub use admissibility::{
    Admitted, MAX_EDGES, MAX_NODES, SNAPSHOT_VERSION, Snapshot, Violation, admit,
    checkpoint_atomic, restore_previous,
};
pub use episodic::{EpisodicBuffer, SummaryRecord};
pub use graph::{SemanticEdge, SemanticNode, StateGraph};
pub use state::{IdleClock, Phase, PreemptFlag, monotonic_us};
pub use worker::{DreamConfig, DreamReport, DreamWorker};

#[cfg(test)]
mod props;
#[cfg(test)]
mod tests;
