#![forbid(unsafe_code)]
//! Phoenix uses millionths: every value v means the exact ratio v/1_000_000.
//! EWMA uses alpha=1/8, truncation <1/SCALE per update, steady error <8/SCALE.
pub const SCALE: u32 = 1_000_000;
pub const PROSODY_DIMS: usize = 8;
pub const PROSODY_LANGUAGES: [&str; PROSODY_DIMS] =
    ["jp", "zh", "en", "cant", "kr", "es", "fr", "de"];
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VocalAdjustment {
    pub pitch_ppm: u32,
    pub cadence_ppm: u32,
}
impl VocalAdjustment {
    pub const CALM: Self = Self {
        pitch_ppm: SCALE,
        cadence_ppm: SCALE,
    };
}
#[derive(Debug, Clone, Default)]
pub struct PhoenixState {
    smooth_jitter: u32,
}
impl PhoenixState {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn update(&mut self, raw_jitter: u32) {
        // EWMA with alpha=1/8: new = old + (1/8)*(input - old)
        // To avoid overflow with u32, we use u64 for intermediate calculation.
        // (7 * old + input) / 8
        let old = self.smooth_jitter as u64;
        let input = raw_jitter.min(SCALE) as u64;
        self.smooth_jitter = ((7 * old + input) / 8) as u32;
    }
    pub fn stability(&self) -> u32 {
        SCALE - self.smooth_jitter
    }
    pub fn smooth_jitter(&self) -> u32 {
        self.smooth_jitter
    }
}
pub fn marine_salience(outlier: u32, temporal_change: u32) -> u32 {
    (7 * outlier.min(SCALE) + 3 * temporal_change.min(SCALE)) / 10
}
pub fn co_regulate(stability: u32) -> VocalAdjustment {
    let damping = SCALE - 12 * (SCALE - stability.min(SCALE)) / 100;
    VocalAdjustment {
        pitch_ppm: damping,
        cadence_ppm: damping,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dynamics_are_bounded_and_converge() {
        let mut p = PhoenixState::new();
        p.update(SCALE);
        assert_eq!(p.smooth_jitter(), 125_000);
        for _ in 0..1000 {
            p.update(0);
        }
        assert_eq!(p.stability(), SCALE);
        assert_eq!(marine_salience(SCALE, 0), 700_000);
        assert_eq!(co_regulate(0).pitch_ppm, 880_000);
        assert_eq!(co_regulate(SCALE), VocalAdjustment::CALM);
    }
}
