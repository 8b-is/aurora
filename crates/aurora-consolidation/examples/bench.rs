//! Throughput bench for the Dream Phase.
//!
//!     cargo run --release --example bench -p aurora-consolidation
//!
//! Reports wall time for a synthetic episodic buffer and graph, plus the
//! resulting footprint. Deliberately dependency-free: the numbers are for
//! spotting regressions, not for publication.

use std::time::{Duration, Instant};

use aurora_consolidation::{
    DreamConfig, DreamWorker, EpisodicBuffer, SemanticEdge, SemanticNode, StateGraph,
};
use aurora_core::Rational;
use aurora_mem8::MemoryEvent;

fn main() {
    let env_u64 = |key: &str, default: u64| -> u64 {
        std::env::var(key)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    };
    let events: u64 = env_u64("AURORA_BENCH_EVENTS", 200_000);
    let distinct: u64 = env_u64("AURORA_BENCH_DISTINCT", 512);
    let chunk_size: usize = env_u64("AURORA_BENCH_CHUNK", 64) as usize;

    let mut buffer = EpisodicBuffer::new();
    for id in 1..=events {
        buffer.push(MemoryEvent {
            id,
            timestamp_us: id,
            salience: Rational::new(9, 10).expect("9/10"),
            payload: format!("episode-{}", id % distinct),
        });
    }
    let buffer_bytes = buffer.bytes();

    let mut graph = StateGraph::new();
    for id in 1..=2_000u64 {
        graph.add_node(SemanticNode {
            id,
            hash: id % 400, // 400 distinct hashes -> duplicates to merge
            weight_ppm: 100_000,
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
    let graph_bytes = graph.bytes();

    let config = DreamConfig {
        idle_threshold: Duration::from_micros(0),
        chunk_size,
        ..DreamConfig::default()
    };
    let mut worker = DreamWorker::new(config)
        .with_graph(graph)
        .with_buffer(buffer);
    worker.heartbeat(0);

    let start = Instant::now();
    let report = worker.run_once(start);
    let elapsed = start.elapsed();

    let micros = elapsed.as_secs_f64() * 1e6;
    println!("events            {events}");
    println!("elapsed           {:.1} ms", elapsed.as_secs_f64() * 1e3);
    println!(
        "throughput        {:.0} events/s",
        events as f64 / elapsed.as_secs_f64()
    );
    let before_total = buffer_bytes + graph_bytes;
    let after_total = worker.buffer().bytes() + worker.graph().bytes();
    let saved = 100.0 * (1.0 - after_total as f64 / before_total as f64);
    println!("footprint before  {before_total} B");
    println!("footprint after   {after_total} B  (-{saved:.1}%)");
    println!("per-event         {:.2} us", micros / events as f64);
    println!("report            {report:?}");
    println!(
        "json {{\"chunk\":{chunk_size},\"events\":{events},\"ms\":{:.1},\"ev_per_s\":{:.0},\"before\":{before_total},\"after\":{after_total},\"saved_pct\":{saved:.2},\"summaries\":{}}}",
        elapsed.as_secs_f64() * 1e3,
        events as f64 / elapsed.as_secs_f64(),
        match &report {
            aurora_consolidation::DreamReport::Consolidated { summaries, .. } => *summaries,
            _ => 0usize,
        }
    );
}
