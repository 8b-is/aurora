#![forbid(unsafe_code)]
//! PCM16 audio, rational HUD metrics, and an integer smoothstep fade.
//! Smoothstep replaces the former cosine curve; it is exact on its polynomial
//! before explicit fixed-denominator truncation, not an exact cosine.
pub use aurora_phoenix::{VocalAdjustment, co_regulate};
const SCALE: u32 = 1_000_000;
pub trait AudioSink {
    fn play(&mut self, samples: &[i16]) -> Result<(), String>;
}
pub struct NullSink;
impl AudioSink for NullSink {
    fn play(&mut self, _samples: &[i16]) -> Result<(), String> {
        Ok(())
    }
}
pub fn fine_touch_fade(t: u32) -> u32 {
    let t = u128::from(t.min(SCALE));
    let s = u128::from(SCALE);
    // Smoothstep polynomial: 3t^2 - 2t^3.
    // To scale to [0, SCALE], we use (3t^2s - 2t^3) / s^2
    // which is equivalent to (t^2 * (3s - 2t)) / s^2
    // We use u128 to prevent overflow during intermediate products.
    ((t * t * (3 * s - 2 * t)) / (s * s)) as u32
}
pub fn apply_fade(sample: i16, t: u32) -> i16 {
    (i64::from(sample) * i64::from(fine_touch_fade(t)) / i64::from(SCALE)) as i16
}
pub const PALETTE: [&str; 7] = [
    "#05060a", "#0b0f19", "#cfc9dd", "#62e6c9", "#e8b05c", "#b48bff", "#00f0ff",
];
pub struct FineTouchHud;
impl FineTouchHud {
    pub fn render(stability: u32, phase_alignment: i64, spin: i8) -> String {
        format!(
            "AURORA stability={}/{} alignment={}/{} spin={:+}",
            stability.min(SCALE),
            SCALE,
            phase_alignment.clamp(-i64::from(SCALE), i64::from(SCALE)),
            SCALE,
            spin
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fade_is_monotone_and_preserves_pcm_limits() {
        assert_eq!(fine_touch_fade(0), 0);
        assert_eq!(fine_touch_fade(SCALE / 2), SCALE / 2);
        assert_eq!(apply_fade(i16::MIN, SCALE), i16::MIN);
        assert_eq!(apply_fade(i16::MAX, SCALE), i16::MAX);
        for t in (0..SCALE).step_by(1000) {
            assert!(fine_touch_fade(t) <= fine_touch_fade(t + 1000));
        }
    }
}
