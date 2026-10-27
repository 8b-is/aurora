#![forbid(unsafe_code)]

//! `aurora-dharma-quantum` — the protective and consolidating layer.
//!
//! * **Mahākāla** — the capability firewall. A granted capability must cover the
//!   requested one, and every attenuation stays under the strict `117 ms`
//!   budget. Violations are counted, never silent.
//! * **Chenrezig** — the compassionate transformer: `-1 → 0 → +1`.
//! * **Vajrasattva** — the nightly consolidator that distills the wave grid,
//!   returning a purity fraction and the accumulated Kintsugi gold.
//! * **qwave** — a 2×2 density-matrix Von Neumann entropy and state fidelity,
//!   the `λ = 3 mm` entanglement substrate.

use aurora_core::{
    ArithmeticError, Rational, SCALE, TernarySpin, VAKED_ATTENUATION_BUDGET_NS, WaveMemoryTuple,
};
use std::time::Duration;

/// The qwave entanglement wavelength, kept at the `3 mm` design threshold.
pub const WAVELENGTH_MM: u32 = 3;

/// Mahākāla capability firewall with a hard attenuation budget.
#[derive(Debug, Clone)]
pub struct MahakalaGuard {
    budget_ns: u64,
    spent_ns: u64,
    violations: u64,
}

impl MahakalaGuard {
    pub fn new(budget_ns: u64) -> Self {
        Self {
            budget_ns,
            spent_ns: 0,
            violations: 0,
        }
    }

    /// The sovereign default: strict `<117 ms`.
    pub fn default_budget() -> Self {
        Self::new(VAKED_ATTENUATION_BUDGET_NS)
    }

    /// Returns `true` when the requested capability is covered by the granted
    /// one (`requested & !granted == 0`) and the attenuation has not blown the
    /// budget. Otherwise records a violation and returns `false`.
    pub fn permits(&mut self, requested: u64, granted: u64, elapsed: Duration) -> bool {
        if requested & !granted != 0 {
            self.violations = self.violations.saturating_add(1);
            return false;
        }
        let nanos = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        self.spent_ns = self.spent_ns.saturating_add(nanos);
        if self.spent_ns >= self.budget_ns {
            self.violations = self.violations.saturating_add(1);
            return false;
        }
        true
    }

    pub fn violations(&self) -> u64 {
        self.violations
    }

    pub fn spent_ns(&self) -> u64 {
        self.spent_ns
    }
}

/// Chenrezig resonator: a spin transformer that lifts `-1 → 0 → +1` and counts
/// every `om mani padme hum`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ChenrezigResonator {
    pub manis: u64,
}

impl ChenrezigResonator {
    pub fn transmute(&mut self, spin: TernarySpin) -> TernarySpin {
        match spin {
            TernarySpin::Entropy => {
                self.manis += 1;
                TernarySpin::Stillness
            }
            TernarySpin::Stillness => {
                self.manis += 1;
                TernarySpin::Resonance
            }
            TernarySpin::Resonance => TernarySpin::Resonance,
        }
    }
}

/// Vajrasattva consolidator: distills a batch of memories into `(purity, gold)`.
#[derive(Debug, Clone, Default)]
pub struct VajrasattvaConsolidator {
    pub cycles: u64,
}

impl VajrasattvaConsolidator {
    pub fn consolidate(&mut self, memories: &[WaveMemoryTuple]) -> (Rational, u64) {
        self.cycles += 1;
        let n = memories.len().max(1) as u64;
        let resonant = memories
            .iter()
            .filter(|m| m.spin != TernarySpin::Entropy)
            .count() as i64;
        let gold = memories
            .iter()
            .map(|m| m.kintsugi_scar_gold as u64)
            .sum::<u64>();
        (
            Rational::new(resonant, n).expect("bounded memory count"),
            gold,
        )
    }
}

/// Von Neumann entropy of a real symmetric 2×2 density matrix
/// `ρ = [[a, b], [b, 1-a]]` (trace 1). Closed-form eigenvalues
/// `λ± = 0.5(1 ± sqrt((2a-1)^2 + 4b^2))`.
pub fn von_neumann_entropy_2x2(a: u32, b: i32) -> Result<Rational, ArithmeticError> {
    if a > SCALE || b.unsigned_abs() > SCALE / 2 {
        return Err(ArithmeticError::InvalidDomain);
    }
    let s = i64::from(SCALE);
    let delta_squared = (2 * i64::from(a) - s).pow(2) + 4 * i64::from(b).pow(2);
    if delta_squared > s * s {
        return Err(ArithmeticError::InvalidDomain);
    }
    let delta = (delta_squared as u64).isqrt() as i64;
    let plus = (s + delta) / 2;
    // Integer log approximation via range reduction and the atanh series.
    fn shannon(mut x: i64) -> i64 {
        if x == 0 {
            return 0;
        }
        let original = x;
        let s = i64::from(SCALE);
        let mut shifts = 0;
        while x < s / 2 {
            x *= 2;
            shifts += 1;
        }
        let y = (x - s) * s / (x + s);
        let yy = y * y / s;
        let mut term = y;
        let mut sum = y;
        for denominator in (3..=31).step_by(2) {
            term = term * yy / s;
            sum += term / denominator;
        }
        let log = 2 * sum - shifts * 693_147;
        -original * log / s
    }
    Rational::new(shannon(plus) + shannon(s - plus), u64::from(SCALE))
}

/// Fidelity `|⟨ψ|φ⟩|²` between two real 2-component state vectors.
pub fn fidelity(psi: [Rational; 2], phi: [Rational; 2]) -> Result<Rational, ArithmeticError> {
    let inner = psi[0]
        .checked_mul(phi[0])?
        .checked_add(psi[1].checked_mul(phi[1])?)?;
    inner.checked_mul(inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ghost_in_the_shell_vaked_attenuation() {
        let mut guard = MahakalaGuard::default_budget();
        // Granted covers requested, well within budget.
        assert!(guard.permits(2, 2, Duration::from_micros(50)));
        // Requested exceeds granted → Cap(v) ⊄ Cap(u).
        assert!(!guard.permits(3, 2, Duration::from_micros(10)));
        assert!(guard.violations() >= 1);
    }

    #[test]
    fn test_budget_exhaustion() {
        let mut guard = MahakalaGuard::new(100); // 100 ns budget
        assert!(!guard.permits(1, 1, Duration::from_nanos(100)));
        assert!(guard.violations() >= 1);
        let mut guard = MahakalaGuard::default_budget();
        assert!(!guard.permits(1, 2, Duration::ZERO));
    }

    #[test]
    fn test_chenrezig_transmutes_upward() {
        let mut c = ChenrezigResonator::default();
        assert_eq!(c.transmute(TernarySpin::Entropy), TernarySpin::Stillness);
        assert_eq!(c.transmute(TernarySpin::Stillness), TernarySpin::Resonance);
        assert_eq!(c.transmute(TernarySpin::Resonance), TernarySpin::Resonance);
        assert_eq!(c.manis, 2);
    }

    #[test]
    fn test_pure_state_has_zero_entropy() {
        // ρ = |0⟩⟨0|
        assert_eq!(von_neumann_entropy_2x2(SCALE, 0).unwrap(), Rational::ZERO);
        // ρ = |+⟩⟨+| = [[0.5,0.5],[0.5,0.5]]
        assert_eq!(
            von_neumann_entropy_2x2(500_000, 500_000).unwrap(),
            Rational::ZERO
        );
    }

    #[test]
    fn test_maximally_mixed_entropy_is_ln2() {
        // ρ = I/2
        let s = von_neumann_entropy_2x2(500_000, 0)
            .unwrap()
            .scaled(u64::from(SCALE))
            .unwrap();
        assert!((s - 693_147).abs() < 10);
        assert!(von_neumann_entropy_2x2(SCALE, 1).is_err());
    }

    #[test]
    fn test_fidelity_identical_is_one() {
        assert_eq!(
            fidelity(
                [Rational::ONE, Rational::ZERO],
                [Rational::ONE, Rational::ZERO]
            )
            .unwrap(),
            Rational::ONE
        );
        assert_eq!(
            fidelity(
                [Rational::ONE, Rational::ZERO],
                [Rational::ZERO, Rational::ONE]
            )
            .unwrap(),
            Rational::ZERO
        );
    }
}
