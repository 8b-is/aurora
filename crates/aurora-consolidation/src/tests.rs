//! Verification suite for the Dream Phase.
//!
//! The three contract tests required by the spec:
//!
//! * `preemption_preserves_state` — a request mid-cycle commits nothing.
//! * `memory_decreases_after_consolidation` — the footprint is bounded.
//! * `invalid_state_is_rejected` — the admissibility gate is deterministic.

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use aurora_core::Rational;
use aurora_mem8::MemoryEvent;

use crate::admissibility::{
    MAX_NODES, Snapshot, Violation, admit, checkpoint_atomic, load_snapshot, restore_previous,
};
use crate::episodic::{EpisodicBuffer, fnv1a};
use crate::graph::{SemanticEdge, SemanticNode, StateGraph};
use crate::state::{IdleClock, Phase};
use crate::worker::{DreamConfig, DreamReport, DreamWorker, ROOT_ID};

fn event(id: u64, payload: &str, salience: Rational) -> MemoryEvent {
    MemoryEvent {
        id,
        timestamp_us: id,
        salience,
        payload: payload.to_owned(),
    }
}

fn rational(p: i64, q: u64) -> Rational {
    Rational::new(p, q).expect("valid rational")
}

fn temp_path(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("aurora-dream-{tag}-{nanos}.m8"))
}

/// A worker that is idle immediately, so tests exercise the pass itself.
fn configured_worker() -> DreamWorker {
    let config = DreamConfig {
        idle_threshold: Duration::from_micros(0),
        ..DreamConfig::default()
    };
    DreamWorker::new(config)
}

#[test]
fn idle_threshold_is_respected() {
    let mut worker = DreamWorker::new(DreamConfig::default());
    let start = Instant::now();
    worker.heartbeat(0);
    let report = worker.run_once(start);
    assert!(
        matches!(report, DreamReport::Skipped { .. }),
        "fresh activity must skip: {report:?}"
    );
    assert_eq!(worker.phase(), Phase::Active);
}

#[test]
fn preemption_preserves_state() {
    let mut graph = StateGraph::new();
    graph.add_node(SemanticNode {
        id: 1,
        hash: 11,
        weight_ppm: 500_000,
        bytes: 10,
    });
    graph.add_node(SemanticNode {
        id: 2,
        hash: 22,
        weight_ppm: 500_000,
        bytes: 10,
    });
    graph.add_edge(SemanticEdge {
        from: 1,
        to: 2,
        weight_ppm: 1,
    });

    let mut buffer = EpisodicBuffer::new();
    for id in 1..=256u64 {
        buffer.push(event(id, &format!("episode-{id}"), rational(9, 10)));
    }

    let before_bytes = graph.bytes();
    let before_seq = 7;
    let mut worker = configured_worker()
        .with_graph(graph)
        .with_buffer(buffer)
        .with_seq(before_seq);
    let start = Instant::now();
    worker.heartbeat(0);
    worker.request_preempt();

    let report = worker.run_once(start);
    assert!(
        matches!(report, DreamReport::Preempted { .. }),
        "expected preemption: {report:?}"
    );
    assert_eq!(worker.seq(), before_seq, "sequence must not advance");
    assert_eq!(worker.graph().nodes().len(), 2, "graph must be untouched");
    assert_eq!(
        worker.graph().bytes(),
        before_bytes,
        "footprint must be untouched"
    );
    assert_eq!(worker.phase(), Phase::Active);
}

#[test]
fn memory_decreases_after_consolidation() {
    let mut graph = StateGraph::new();
    // Two duplicate nodes and one orphan, plus a real edge pair.
    graph.add_node(SemanticNode {
        id: 1,
        hash: 7,
        weight_ppm: 400_000,
        bytes: 100,
    });
    graph.add_node(SemanticNode {
        id: 2,
        hash: 7,
        weight_ppm: 400_000,
        bytes: 100,
    });
    graph.add_node(SemanticNode {
        id: 3,
        hash: 9,
        weight_ppm: 200_000,
        bytes: 40,
    });
    graph.add_node(SemanticNode {
        id: 4,
        hash: 13,
        weight_ppm: 100_000,
        bytes: 20,
    }); // orphan
    graph.add_edge(SemanticEdge {
        from: 1,
        to: 3,
        weight_ppm: 700,
    });
    graph.add_edge(SemanticEdge {
        from: 2,
        to: 3,
        weight_ppm: 300,
    });

    let mut buffer = EpisodicBuffer::new();
    for id in 1..=64u64 {
        // Only four distinct payloads, so distillation must actually shrink.
        buffer.push(event(id, &format!("episode-{}", id % 4), rational(3, 4)));
    }
    // Low-salience rows must be purged.
    for id in 65..=72u64 {
        buffer.push(event(id, "noise", rational(1, 100)));
    }

    let before_graph = graph.bytes();
    let before_buffer = buffer.bytes();
    let mut worker = configured_worker().with_graph(graph).with_buffer(buffer);
    let start = Instant::now();
    worker.heartbeat(0);

    let report = worker.run_once(start);
    let DreamReport::Consolidated {
        merged,
        pruned,
        purged,
        summaries,
        seq,
    } = report
    else {
        panic!("expected consolidation, got {report:?}");
    };
    assert!(merged >= 1, "duplicate hash must merge");
    assert!(pruned >= 1, "orphan must be pruned");
    assert_eq!(purged, 8, "all sub-threshold rows purged");
    assert_eq!(summaries, 4, "one summary per unique payload");
    assert_eq!(seq, 1);
    assert_eq!(
        worker.buffer().bytes(),
        0,
        "episodic buffer is drained on commit"
    );
    assert_eq!(worker.buffer().len(), 0);
    assert!(
        worker.graph().bytes() < before_graph + before_buffer,
        "footprint must not grow"
    );
    assert_eq!(worker.phase(), Phase::Active);
}

#[test]
fn invalid_state_is_rejected() {
    let nodes = vec![
        SemanticNode {
            id: 1,
            hash: 10,
            weight_ppm: 500_000,
            bytes: 8,
        },
        SemanticNode {
            id: ROOT_ID,
            hash: 0,
            weight_ppm: 1_000_000,
            bytes: 0,
        },
    ];
    let edges = vec![SemanticEdge {
        from: ROOT_ID,
        to: 1,
        weight_ppm: 1_000_000,
    }];
    let good = Snapshot::build(1, nodes.clone(), edges.clone());
    assert!(
        admit(good.clone(), 0).is_ok(),
        "baseline must be admissible"
    );

    // 1. Digest tampering.
    let mut tampered = good.clone();
    tampered.digest ^= 0xdead_beef;
    assert_eq!(admit(tampered, 0), Err(Violation::DigestMismatch));

    // 2. Weight above the capability ceiling (rebuild digest so only the bound trips).
    let mut heavy = nodes.clone();
    heavy[0].weight_ppm = 1_000_001;
    let heavy = Snapshot::build(2, heavy, edges.clone());
    assert_eq!(
        admit(heavy, 0),
        Err(Violation::Capability("node weight above 1_000_000 ppm"))
    );

    // 3. Dangling edge.
    let dangling = Snapshot::build(
        3,
        nodes.clone(),
        vec![SemanticEdge {
            from: ROOT_ID,
            to: 999,
            weight_ppm: 1,
        }],
    );
    assert_eq!(
        admit(dangling, 0),
        Err(Violation::Schema("edge references a missing node"))
    );

    // 4. Sequence regression.
    assert_eq!(
        admit(good.clone(), 1),
        Err(Violation::SequenceRegression {
            previous: 1,
            proposed: 1
        })
    );

    // 5. Node count ceiling is enforced without allocating.
    let mut oversized = good.clone();
    oversized.nodes = vec![
        SemanticNode {
            id: 1,
            hash: 1,
            weight_ppm: 1,
            bytes: 0
        };
        MAX_NODES + 1
    ];
    oversized.digest = oversized.compute_digest();
    assert_eq!(
        admit(oversized, 0),
        Err(Violation::Capability("node count exceeds ceiling"))
    );
}

#[test]
fn checkpoint_is_atomic_and_rolls_back() {
    let path = temp_path("checkpoint");
    let first = Snapshot::build(
        1,
        vec![SemanticNode {
            id: 1,
            hash: 5,
            weight_ppm: 9,
            bytes: 1,
        }],
        vec![],
    );
    let second = Snapshot::build(
        2,
        vec![SemanticNode {
            id: 1,
            hash: 5,
            weight_ppm: 10,
            bytes: 1,
        }],
        vec![],
    );

    checkpoint_atomic(&path, &first).expect("first checkpoint");
    checkpoint_atomic(&path, &second).expect("second checkpoint");

    let loaded = load_snapshot(&path).expect("load current");
    assert_eq!(loaded.checkpoint_seq, 2);
    assert_eq!(loaded, second);

    restore_previous(&path).expect("rollback");
    let restored = load_snapshot(&path).expect("load restored");
    assert_eq!(restored, first, "rollback returns the previous checkpoint");

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_extension("prev"));
    let _ = std::fs::remove_file(path.with_extension("tmp"));
}

#[test]
fn graph_prune_is_deterministic_and_renormalises() {
    let build = || {
        let mut graph = StateGraph::new();
        graph.add_node(SemanticNode {
            id: 4,
            hash: 2,
            weight_ppm: 100,
            bytes: 1,
        });
        graph.add_node(SemanticNode {
            id: 2,
            hash: 2,
            weight_ppm: 200,
            bytes: 1,
        });
        graph.add_node(SemanticNode {
            id: 3,
            hash: 3,
            weight_ppm: 300,
            bytes: 1,
        });
        graph.add_edge(SemanticEdge {
            from: 2,
            to: 3,
            weight_ppm: 1,
        });
        graph.add_edge(SemanticEdge {
            from: 4,
            to: 3,
            weight_ppm: 3,
        });
        graph.add_edge(SemanticEdge {
            from: 3,
            to: 3,
            weight_ppm: 9,
        }); // self-loop
        graph
    };

    let mut a = build();
    let mut b = build();
    let ra = a.prune();
    let rb = b.prune();
    assert_eq!(ra, rb, "prune must be deterministic");
    assert_eq!(ra, (1, 0), "one duplicate merged, no orphans");
    assert_eq!(a.nodes().len(), 2);
    assert_eq!(
        a.edges().len(),
        1,
        "parallel edges merged, self-loop dropped"
    );
    let sum: u64 = a.edges().iter().map(|e| e.weight_ppm).sum();
    assert_eq!(sum, 1_000_000, "renormalised weights sum to 1 ppm-whole");
}

#[test]
fn prune_aborts_without_mutating() {
    let mut graph = StateGraph::new();
    graph.add_node(SemanticNode {
        id: 1,
        hash: 1,
        weight_ppm: 1,
        bytes: 1,
    });
    let before = graph.nodes().len();
    let result = graph.prune_with(&|| true);
    assert!(result.is_none(), "abort predicate must be honoured");
    assert_eq!(
        graph.nodes().len(),
        before,
        "aborted prune leaves the graph intact"
    );
}

#[test]
fn fnv1a_is_stable() {
    assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
    assert_ne!(fnv1a(b"ab"), fnv1a(b"ba"));
}

#[test]
fn idle_clock_accounts_microseconds() {
    let mut clock = IdleClock::new(Duration::from_secs(300));
    clock.touch(1_000);
    assert!(!clock.is_idle(1_000 + 299_999_999));
    assert!(clock.is_idle(1_000 + 300_000_000));
    assert_eq!(clock.idle_for_us(1_000 + 300_000_000), 300_000_000);
}

#[test]
fn fnv_vector_matches_zig_cross_verifier() {
    // Pinned identically in zig/verify_consolidation.zig. If either side drifts,
    // summaries are no longer portable between the Rust and Zig implementations.
    assert_eq!(fnv1a(b"episode-0"), 0xd9f3_618d_c2d7_5aa7);
}
