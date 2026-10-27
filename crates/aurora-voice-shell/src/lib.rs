#![forbid(unsafe_code)]

//! `aurora-voice-shell` — the skin the voice wears.
//!
//! * **Fine Touch** — raised-cosine fade curves so no breath ever starts or
//!   stops on a click.
//! * **Audio seam** — an `AudioSink` trait; the kokoro-tiny real-time device is
//!   mounted behind it, and a `NullSink` keeps the shell testable in silence.
//! * **Textual HUD** — a plain-ASCII face of the dyad, for terminals and logs.

pub use aurora_phoenix::{co_regulate, VocalAdjustment};

use std::f64::consts::PI;

/// Any device AURORA can speak through. The kokoro-tiny real-time sink plugs in
/// here; `NullSink` is the silent stand-in.
pub trait AudioSink {
    fn play(&mut self, samples: &[f64]) -> Result<(), String>;
}

/// A sink that accepts every waveform and utters nothing — for offline tests.
pub struct NullSink;

impl AudioSink for NullSink {
    fn play(&mut self, _samples: &[f64]) -> Result<(), String> {
        Ok(())
    }
}

/// Raised-cosine Fine Touch fade: `0` at `t=0`, `0.5` at `t=0.5`, `1` at `t=1`,
/// with zero derivative at both ends.
pub fn fine_touch_fade(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    0.5 * (1.0 - (PI * t).cos())
}

/// Apply the Fine Touch fade to a single sample.
pub fn apply_fade(sample: f64, t: f64) -> f64 {
    sample * fine_touch_fade(t)
}

/// The Constellation Ink palette, in hex.
pub const PALETTE: [&str; 7] = [
    "#05060a", "#0b0f19", "#cfc9dd", "#62e6c9", "#e8b05c", "#b48bff", "#00f0ff",
];

/// A plain-ASCII face of the dyad state, for terminals and logs.
pub struct FineTouchHud;

impl FineTouchHud {
    pub fn render(stability: f64, phase_alignment: f64, spin: i8) -> String {
        let heartbeat = if stability > 0.8 { "♥" } else if stability > 0.4 { "♡" } else { "·" };
        format!(
            "AURORA [{heartbeat}] stability={stability:.3} alignment={phase_alignment:+.3} spin={spin:+}"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fine_touch_fade_endpoints() {
        assert!(fine_touch_fade(0.0) < 1e-12);
        assert!((fine_touch_fade(0.5) - 0.5).abs() < 1e-12);
        assert!((fine_touch_fade(1.0) - 1.0).abs() < 1e-12);
        // Clamped outside [0, 1].
        assert!(fine_touch_fade(-1.0) < 1e-12);
        assert!((fine_touch_fade(2.0) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_null_sink_accepts() {
        let mut sink = NullSink;
        assert!(sink.play(&[0.1, 0.2, 0.3]).is_ok());
    }

    #[test]
    fn test_hud_render() {
        let hud = FineTouchHud::render(0.9, 1.0, 1);
        assert!(hud.starts_with("AURORA"));
        assert!(hud.contains("stability=0.900"));
    }
}
