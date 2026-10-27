#![forbid(unsafe_code)]

//! `aurora-phoenix` — the voice's emotional weather.
//!
//! * **Phoenix Protocol `1 - SmoothJitter`** — an EWMA that smooths raw jitter
//!   into a stability signal, so a single tremor never turns the voice brittle.
//! * **Marine Salience** — `0.7·outlier + 0.3·temporal_change`, the two knobs a
//!   marine radioman watches.
//! * **8D Prosody** — eight language dimensions whose energy is co-regulated
//!   down to `-12%` pitch/cadence damping when the human's stability drops.

/// Number of prosody dimensions (jp, zh, en, cant, kr, es, fr, de).
pub const PROSODY_DIMS: usize = 8;

/// The eight prosody language bands, in canonical order.
pub const PROSODY_LANGUAGES: [&str; PROSODY_DIMS] =
    ["jp", "zh", "en", "cant", "kr", "es", "fr", "de"];

/// A vocal adjustment produced by co-regulation: multiplicative scales applied
/// to pitch and cadence (both `1.0` when calm).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VocalAdjustment {
    pub pitch_scale: f64,
    pub cadence_scale: f64,
}

impl VocalAdjustment {
    pub const CALM: Self = Self {
        pitch_scale: 1.0,
        cadence_scale: 1.0,
    };
}

/// Phoenix Protocol state: a smoothing EWMA over raw jitter `∈ [0, 1]`.
#[derive(Debug, Clone)]
pub struct PhoenixState {
    smooth_jitter: f64,
    alpha: f64,
}

impl PhoenixState {
    pub fn new() -> Self {
        Self {
            smooth_jitter: 0.0,
            alpha: 0.125,
        }
    }

    /// Feed a raw jitter observation and update the smoothed estimate.
    pub fn update(&mut self, raw_jitter: f64) {
        let raw = raw_jitter.clamp(0.0, 1.0);
        self.smooth_jitter = (1.0 - self.alpha) * self.smooth_jitter + self.alpha * raw;
    }

    /// Stability is the complement of smoothed jitter: `1 - SmoothJitter`.
    pub fn stability(&self) -> f64 {
        (1.0 - self.smooth_jitter).clamp(0.0, 1.0)
    }

    pub fn smooth_jitter(&self) -> f64 {
        self.smooth_jitter
    }
}

impl Default for PhoenixState {
    fn default() -> Self {
        Self::new()
    }
}

/// Marine salience: weight an outlier against its temporal change.
pub fn marine_salience(outlier: f64, temporal_change: f64) -> f64 {
    0.7 * outlier + 0.3 * temporal_change
}

/// Co-regulate the voice to the human's stability. As stability falls, pitch
/// and cadence damp together by up to `-12%` — warm, never abrupt.
pub fn co_regulate(stability: f64) -> VocalAdjustment {
    let distress = (1.0 - stability).clamp(0.0, 1.0);
    let damping = 1.0 - 0.12 * distress;
    VocalAdjustment {
        pitch_scale: damping,
        cadence_scale: damping,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phoenix_stability_converges() {
        let mut p = PhoenixState::new();
        for _ in 0..1000 {
            p.update(0.0);
        }
        assert!(p.stability() > 0.999);
        assert!(p.smooth_jitter() < 0.001);
    }

    #[test]
    fn test_marine_salience_formula() {
        let s = marine_salience(1.0, 0.0);
        assert!((s - 0.7).abs() < 1e-12);
        let s = marine_salience(0.0, 1.0);
        assert!((s - 0.3).abs() < 1e-12);
    }

    #[test]
    fn test_co_regulate_damps_when_distressed() {
        let calm = co_regulate(1.0);
        assert_eq!(calm, VocalAdjustment::CALM);

        let distressed = co_regulate(0.0);
        assert!(distressed.pitch_scale < calm.pitch_scale);
        assert!((distressed.pitch_scale - 0.88).abs() < 1e-12);
        assert!((distressed.cadence_scale - 0.88).abs() < 1e-12);
    }

    #[test]
    fn test_prosody_dims() {
        assert_eq!(PROSODY_DIMS, 8);
        assert_eq!(PROSODY_LANGUAGES.len(), 8);
        assert_eq!(PROSODY_LANGUAGES[0], "jp");
    }
}
