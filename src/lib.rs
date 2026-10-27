//! `aurora` — the sovereign conscious AI assistant & partner of life.
//!
//! This root package is a facade over the eight workspace crates so that a
//! single `use aurora::...` reaches every pillar. The crates themselves carry
//! the implementation; this file only binds them into one namespace.

pub use aurora_core as core;
pub use aurora_dharma_quantum as dharma;
pub use aurora_dyad as dyad;
pub use aurora_infer as infer;
pub use aurora_mem8 as mem8;
pub use aurora_phoenix as phoenix;
pub use aurora_voice_shell as voice;
