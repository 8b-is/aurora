//! Lifecycle primitives: the phase machine, the idle clock and the preemption flag.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Lifecycle phases of the dream worker.
///
/// `Active → Idle → Consolidating → Checkpointing → Active`.
/// A preemption returns to `Active` without passing through `Checkpointing`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Active,
    Idle,
    Consolidating,
    Checkpointing,
}

/// Monotonic idle tracker. Durations are integer microseconds.
#[derive(Debug, Clone)]
pub struct IdleClock {
    idle_threshold_us: u64,
    last_activity_us: u64,
}

impl IdleClock {
    pub fn new(idle_threshold: Duration) -> Self {
        Self {
            idle_threshold_us: idle_threshold.as_micros() as u64,
            last_activity_us: 0,
        }
    }

    /// The production default: five minutes of silence.
    pub fn default_threshold() -> Self {
        Self::new(Duration::from_secs(300))
    }

    /// Record activity at `now_us`, resetting the idle countdown.
    pub fn touch(&mut self, now_us: u64) {
        self.last_activity_us = now_us;
    }

    pub fn idle_for_us(&self, now_us: u64) -> u64 {
        now_us.saturating_sub(self.last_activity_us)
    }

    pub fn is_idle(&self, now_us: u64) -> bool {
        self.idle_for_us(now_us) >= self.idle_threshold_us
    }

    pub fn threshold_us(&self) -> u64 {
        self.idle_threshold_us
    }
}

/// Cooperative preemption flag. The I/O path raises it; the worker observes it
/// at every chunk boundary. `take` clears the flag so a later cycle is not
/// poisoned by a stale signal.
#[derive(Debug, Default)]
pub struct PreemptFlag {
    flag: AtomicBool,
}

impl PreemptFlag {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    pub fn take(&self) -> bool {
        self.flag.swap(false, Ordering::SeqCst)
    }

    pub fn is_set(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
}

/// Microseconds elapsed since `start`, saturating rather than wrapping.
pub fn monotonic_us(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX)
}
