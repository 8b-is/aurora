//! How long does one prune pass take, and where does it break the 5 ms contract?
//!
//!     cargo run --release --example prune_cost -p aurora-consolidation
//!
//! The Dream Phase promises that a request arriving mid-cycle is observed
//! promptly. Preemption is polled *between* prune phases, so a single phase —
//! the sort, in particular — is the real granularity. This measures it instead
//! of assuming.

use std::time::Instant;

use aurora_consolidation::{SemanticEdge, SemanticNode, StateGraph};

fn build(nodes: u64) -> StateGraph {
    let mut graph = StateGraph::new();
    for id in 1..=nodes {
        graph.add_node(SemanticNode {
            id,
            hash: id % (nodes / 4).max(1), // duplicates, so merging actually happens
            weight_ppm: 1_000,
            bytes: 64,
        });
        if id > 1 {
            graph.add_edge(SemanticEdge {
                from: id - 1,
                to: id,
                weight_ppm: 1_000,
            });
        }
    }
    graph
}

fn main() {
    println!("{:>9}  {:>10}  {:>8}  verdict", "nodes", "prune", "vs 5ms");
    for nodes in [10_000u64, 25_000, 50_000, 100_000, 200_000] {
        let mut graph = build(nodes);
        let start = Instant::now();
        let (merged, pruned) = graph.prune();
        let elapsed = start.elapsed();
        let micros = elapsed.as_micros();
        let contract_micros = 5_000u128;
        let verdict = if micros <= contract_micros {
            "within contract"
        } else {
            "EXCEEDS contract"
        };
        let (q, r) = (micros / 1_000, micros % 1_000);
        println!(
            "{nodes:>9}  {q:>4}.{r:>1}ms  {:>8}  {verdict} (merged {merged}, pruned {pruned})",
            ""
        );
    }
}
