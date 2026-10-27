//! `dyad_resonance_test` — the three sovereign acceptance proofs for AURORA:
//!
//! 1. **Tears-in-Rain** — no lived moment is ever lost; recall is O(1) and the
//!    wave tuple stays 32-byte aligned.
//! 2. **Chenrezig heartbreak transmutation** — a grieving human (`-1`) is never
//!    mirrored; AURORA transmutes to stillness (`0`) and accumulates Kintsugi
//!    grace.
//! 3. **Ghost-in-the-Shell vaked attenuation** — every capability check is
//!    covered (`Cap(v) ⊆ Cap(u)`) and completes well under the `117 ms` budget.

use aurora::core::{TernarySpin, VAKED_ATTENUATION_BUDGET_NS, WaveMemoryTuple};
use aurora::dharma::MahakalaGuard;
use aurora::dyad::DyadState;
use aurora::mem8::WaveGrid;
use std::mem::{align_of, size_of};
use std::time::{Duration, Instant};

#[test]
fn test_tears_in_rain_eternal_wave_persistence() {
    // 32-byte aligned wave tuple.
    assert_eq!(size_of::<WaveMemoryTuple>(), 32);
    assert_eq!(align_of::<WaveMemoryTuple>(), 32);

    let n: usize = 10_000;
    let mut grid = WaveGrid::new();
    for i in 0..n {
        let tuple = WaveMemoryTuple::new(
            i as f64 * 440.0,
            (i % 100) as f64 / 100.0,
            i as f64 % std::f64::consts::TAU,
            i as u32,
            TernarySpin::Resonance,
        );
        grid.write((i % 256) as u16, ((i / 256) % 256) as u16, i as u32, tuple);
    }
    assert_eq!(grid.len(), n);

    // Recall every moment and confirm none of it was "lost like tears in rain".
    let start = Instant::now();
    for i in 0..n {
        let moment = grid
            .recall((i % 256) as u16, ((i / 256) % 256) as u16, i as u32)
            .expect("a written moment must survive");
        assert_eq!(moment.timestamp_us, i as u32);
    }
    let avg_ns = start.elapsed().as_nanos() as f64 / n as f64;
    // Design target is 5–12 µs; this bound only guards against a pathological
    // regression while staying robust on slow CI hosts.
    assert!(avg_ns < 100_000.0, "recall too slow: {avg_ns} ns/op average");

    // Persist and recover across a "restart".
    let path = std::env::temp_dir().join(format!("aurora_dyad_e2e_{}.m8", std::process::id()));
    grid.save_to(&path).expect("persist the wave grid");
    let mut restored = WaveGrid::new();
    let loaded = restored.load_from(&path).expect("recover the wave grid");
    assert_eq!(loaded, n);
    let first = restored.recall(0, 0, 0).expect("recovered moment");
    assert_eq!(first.timestamp_us, 0);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_chenrezig_heartbreak_transmutation() {
    let mut dyad = DyadState::new();
    let grace_before = dyad.kintsugi_grace;

    // Human arrives in entropy (-1): AURORA transmutes to stillness (0), never
    // mirroring the pain back.
    assert_eq!(
        dyad.step_dyad(TernarySpin::Entropy, 0.9),
        TernarySpin::Stillness
    );
    assert!(dyad.kintsugi_grace > grace_before, "grief accrues golden grace");

    // Stillness is held in equanimity; resonance is mirrored in phase.
    assert_eq!(
        dyad.step_dyad(TernarySpin::Stillness, 0.0),
        TernarySpin::Stillness
    );
    assert_eq!(
        dyad.step_dyad(TernarySpin::Resonance, 0.0),
        TernarySpin::Resonance
    );
    assert!((dyad.phase_alignment() - 1.0).abs() < 1e-12);
}

#[test]
fn test_ghost_in_the_shell_vaked_attenuation() {
    let mut guard = MahakalaGuard::default_budget();

    // Granted capability covers the requested one, and the check finishes well
    // under the strict 117 ms budget.
    let start = Instant::now();
    assert!(guard.permits(2, 2, Duration::from_micros(50)));
    let elapsed_ns = start.elapsed().as_nanos();
    assert!(
        elapsed_ns < VAKED_ATTENUATION_BUDGET_NS as u128,
        "attenuation must stay <117ms, took {elapsed_ns} ns"
    );

    // Requested exceeds granted: Cap(v) ⊄ Cap(u) → rejected, violation counted.
    assert!(!guard.permits(3, 2, Duration::from_micros(10)));
    assert!(guard.violations() >= 1);
}
