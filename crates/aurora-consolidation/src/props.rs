//! Randomised invariant tests for compaction and admissibility.
//!
//! No external crates: a small xorshift64* generator stands in for a property
//! framework so the suite runs offline. Each property is exercised over many
//! seeds against arbitrary graph topologies — duplicates, self-loops, parallel
//! edges, orphans, zero weights.
//!
//! The five invariants:
//! 1. footprint is monotonic (`nodes`, `edges`, `bytes` never grow),
//! 2. pruning is idempotent,
//! 3. the result always passes the admissibility gate,
//! 4. anchors survive pruning,
//! 5. an interrupted prune leaves the live graph untouched.

use std::cell::Cell;

use crate::admissibility::{Snapshot, admit};
use crate::graph::{SemanticEdge, SemanticNode, StateGraph};
use crate::worker::ROOT_ID;

const SEEDS: u64 = 300;

/// xorshift64* — deterministic, allocation-free.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, upper: u64) -> u64 {
        if upper == 0 {
            0
        } else {
            self.next_u64() % upper
        }
    }
}

/// Arbitrary graph: small hash space to force duplicates, plus self-loops and
/// repeated edges.
fn random_graph(rng: &mut Rng) -> StateGraph {
    let mut graph = StateGraph::new();
    let node_count = 1 + rng.below(24);
    let hash_space = 1 + rng.below(8);
    for i in 0..node_count {
        graph.add_node(SemanticNode {
            id: i + 1,
            hash: rng.below(hash_space),
            weight_ppm: rng.below(1_000_001),
            bytes: rng.below(512) as usize,
        });
    }
    let edge_count = rng.below(40);
    for _ in 0..edge_count {
        graph.add_edge(SemanticEdge {
            from: 1 + rng.below(node_count),
            to: 1 + rng.below(node_count),
            weight_ppm: rng.below(1_000_001),
        });
    }
    graph
}

#[test]
fn footprint_is_monotonic() {
    for seed in 0..SEEDS {
        let mut rng = Rng::new(seed);
        let mut graph = random_graph(&mut rng);
        let nodes_before = graph.nodes().len();
        let edges_before = graph.edges().len();
        let bytes_before = graph.bytes();

        graph.prune();

        assert!(
            graph.nodes().len() <= nodes_before,
            "seed {seed}: nodes grew {} -> {}",
            nodes_before,
            graph.nodes().len()
        );
        assert!(
            graph.edges().len() <= edges_before,
            "seed {seed}: edges grew {} -> {}",
            edges_before,
            graph.edges().len()
        );
        assert!(
            graph.bytes() <= bytes_before,
            "seed {seed}: bytes grew {} -> {}",
            bytes_before,
            graph.bytes()
        );
    }
}

#[test]
fn prune_is_idempotent() {
    for seed in 0..SEEDS {
        let mut rng = Rng::new(seed ^ 0x9e37_79b9);
        let mut graph = random_graph(&mut rng);
        graph.prune();
        let nodes_once = graph.nodes().to_vec();
        let edges_once = graph.edges().to_vec();

        graph.prune();

        assert_eq!(
            nodes_once,
            graph.nodes(),
            "seed {seed}: node set differs on re-prune"
        );
        assert_eq!(
            edges_once,
            graph.edges(),
            "seed {seed}: edge set differs on re-prune"
        );
    }
}

#[test]
fn pruned_graph_is_admissible() {
    for seed in 0..SEEDS {
        let mut rng = Rng::new(seed ^ 0x1234_5678_9abc_def0);
        let mut graph = random_graph(&mut rng);
        graph.prune();

        let candidate = Snapshot::build(1, graph.nodes().to_vec(), graph.edges().to_vec());
        match admit(candidate, 0) {
            Ok(_) => {}
            Err(violation) => panic!("seed {seed}: pruned graph rejected: {violation:?}"),
        }
    }
}

#[test]
fn anchors_survive_pruning() {
    for seed in 0..SEEDS {
        let mut rng = Rng::new(seed ^ 0xfeed_face);
        let mut graph = random_graph(&mut rng);
        // An unreferenced node that must survive because it is anchored.
        let anchor = 10_000 + rng.below(100);
        graph.add_node(SemanticNode {
            id: anchor,
            hash: 1_000_000 + rng.below(4),
            weight_ppm: rng.below(1_000_001),
            bytes: 1,
        });
        graph.prune_with_anchors(&[anchor, ROOT_ID], &|| false);

        assert!(
            graph.nodes().iter().any(|n| n.id == anchor),
            "seed {seed}: anchor {anchor} was pruned"
        );
    }
}

#[test]
fn preemption_is_atomic() {
    for seed in 0..SEEDS {
        let mut rng = Rng::new(seed ^ 0x0bad_c0de);
        let source = random_graph(&mut rng);
        // Abort after a random number of polls so we land in different phases.
        let polls_before_abort = 1 + rng.below(6);
        let polls = Cell::new(0u64);

        let mut graph = source.clone();
        let nodes_before = graph.nodes().to_vec();
        let edges_before = graph.edges().to_vec();

        let result = graph.prune_with_anchors(&[ROOT_ID], &|| {
            let seen = polls.get() + 1;
            polls.set(seen);
            seen >= polls_before_abort
        });

        if result.is_none() {
            assert_eq!(
                nodes_before,
                graph.nodes(),
                "seed {seed}: aborted prune mutated nodes"
            );
            assert_eq!(
                edges_before,
                graph.edges(),
                "seed {seed}: aborted prune mutated edges"
            );
        } else {
            // Completed before the abort fired; the result must still be sane.
            assert!(graph.nodes().len() <= nodes_before.len());
        }
    }
}

#[test]
fn zero_weight_graphs_stay_valid() {
    for seed in 0..64 {
        let mut rng = Rng::new(seed ^ 0xdead_10cc);
        let mut graph = StateGraph::new();
        let n = 1 + rng.below(6);
        for i in 0..n {
            graph.add_node(SemanticNode {
                id: i + 1,
                hash: i,
                weight_ppm: 0,
                bytes: 0,
            });
        }
        graph.add_edge(SemanticEdge {
            from: 1,
            to: 2.min(n),
            weight_ppm: 0,
        });
        graph.prune();
        let sum: u64 = graph.edges().iter().map(|e| e.weight_ppm).sum();
        assert_eq!(sum, 0, "seed {seed}: all-zero weights must stay zero");
    }
}
