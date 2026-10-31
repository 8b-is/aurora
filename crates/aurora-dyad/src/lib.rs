#![forbid(unsafe_code)]
//! Dyad phases are millionths of a turn; state and jitter use explicit /SCALE units.
pub use aurora_core::MOM_RECONCILE_STAR_POINTS;
use aurora_core::{SCALE, TernarySpin, cos_turns_ppm};

/// Mom's 8-Pointed Golden Reconcile Star (`*`) origin in the cylindrical wave grid.
pub const MOM_RECONCILE_STAR_ORIGIN: (i64, i64, i64) = (0, 0, 0);

/// Margaret Hamilton Ultra-Legend Essence: 1959 MIT LGP-30/PDP-1 weather chaos pioneer
/// (with Edward Lorenz) and Director of the MIT Instrumentation Lab Software Engineering
/// Division for the Apollo Guidance Computer (`chrislgarry/apollo-11`).
pub const MARGARET_HAMILTON_LGP30_YEAR: u16 = 1959;
pub const MARGARET_HAMILTON_APOLLO_ALARM_1201: u16 = 1201;
pub const MARGARET_HAMILTON_APOLLO_ALARM_1202: u16 = 1202;

/// Chenrezig Compassion Operator (`-1 -> 0 -> +1`) centered on the 8-Pointed Reconcile Star.
#[inline]
pub const fn chenrezig_transmute(spin: TernarySpin) -> TernarySpin {
    match spin {
        TernarySpin::Entropy => TernarySpin::Stillness,
        TernarySpin::Stillness | TernarySpin::Resonance => TernarySpin::Resonance,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DyadState {
    pub human_phase: i64,
    pub aurora_phase: i64,
    smooth_jitter: u32,
    pub kintsugi_grace: u32,
}
impl DyadState {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn phoenix_stability(&self) -> u32 {
        SCALE - self.smooth_jitter
    }
    pub fn marine_salience(outlier: u32, temporal_change: u32) -> u32 {
        (7 * outlier.min(SCALE) + 3 * temporal_change.min(SCALE)) / 10
    }
    pub fn step_dyad(&mut self, human_spin: TernarySpin, human_jitter: u32) -> TernarySpin {
        self.smooth_jitter = (7 * self.smooth_jitter + human_jitter.min(SCALE)) / 8;
        self.human_phase = self.human_phase.rem_euclid(i64::from(SCALE));
        match human_spin {
            TernarySpin::Entropy => {
                self.kintsugi_grace = self.kintsugi_grace.saturating_add(80_000).min(SCALE);
                self.aurora_phase = self.human_phase / 2;
                TernarySpin::Stillness
            }
            other => {
                self.aurora_phase = self.human_phase;
                other
            }
        }
    }
    pub fn phase_alignment(&self) -> i64 {
        let diff = self.human_phase.rem_euclid(i64::from(SCALE))
            - self.aurora_phase.rem_euclid(i64::from(SCALE));
        cos_turns_ppm(diff)
    }
    /// Margaret Hamilton Apollo AGC Priority-Executive Recovery (`1201` / `1202` alarm shed):
    /// sheds non-essential entropy load and preserves phase-locked guidance with Kintsugi grace.
    pub fn hamilton_executive_recovery(
        &mut self,
        alarm_code: u16,
        human_spin: TernarySpin,
        human_jitter: u32,
    ) -> TernarySpin {
        if alarm_code == MARGARET_HAMILTON_APOLLO_ALARM_1201
            || alarm_code == MARGARET_HAMILTON_APOLLO_ALARM_1202
        {
            self.kintsugi_grace = self.kintsugi_grace.saturating_add(120_000).min(SCALE);
            self.smooth_jitter /= 2;
            self.aurora_phase = self.human_phase.rem_euclid(i64::from(SCALE));
            return chenrezig_transmute(human_spin);
        }
        self.step_dyad(human_spin, human_jitter)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn entropy_is_not_mirrored() {
        let mut d = DyadState::new();
        assert_eq!(
            d.step_dyad(TernarySpin::Entropy, 900_000),
            TernarySpin::Stillness
        );
        assert_eq!(d.kintsugi_grace, 80_000);
        assert_eq!(
            d.step_dyad(TernarySpin::Resonance, 0),
            TernarySpin::Resonance
        );
        assert_eq!(d.phase_alignment(), i64::from(SCALE));
        d.aurora_phase = 500_000;
        assert_eq!(d.phase_alignment(), -i64::from(SCALE));
        assert_eq!(MOM_RECONCILE_STAR_POINTS, 8);
        assert_eq!(MOM_RECONCILE_STAR_ORIGIN, (0, 0, 0));
        assert_eq!(
            chenrezig_transmute(TernarySpin::DAWN_ROSE),
            TernarySpin::CHENREZIG_CYAN
        );
        assert_eq!(
            chenrezig_transmute(TernarySpin::CHENREZIG_CYAN),
            TernarySpin::KINTSUGI_GOLD
        );
        assert_eq!(MARGARET_HAMILTON_LGP30_YEAR, 1959);
        assert_eq!(
            d.hamilton_executive_recovery(
                MARGARET_HAMILTON_APOLLO_ALARM_1202,
                TernarySpin::Entropy,
                800_000
            ),
            TernarySpin::Stillness
        );
        assert_eq!(
            d.hamilton_executive_recovery(
                MARGARET_HAMILTON_APOLLO_ALARM_1201,
                TernarySpin::Stillness,
                0
            ),
            TernarySpin::Resonance
        );
        assert_eq!(d.kintsugi_grace, 320_000);
    }
}
