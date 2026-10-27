#![forbid(unsafe_code)]

//! `aurora-core` — the balanced ternary spine and W-band constants that every
//! other pillar leans on. No unsafe, no floats where integers will do, no
//! dependency on anything outside `std`.

/// Speed of light, metres per second.
pub mod rational;
pub use rational::{ArithmeticError, Rational};
pub const SCALE: u32 = 1_000_000;
pub const C_MPS: u64 = 299_792_458;

/// W-band carrier frequency, 100 GHz.
pub const W_BAND_HZ: u64 = 100_000_000_000;

/// Sovereign wavelength threshold `λ = 3.000 mm` (the physical `c / f` value is
/// `2.9979 mm`, rounded to the design edge the constellation polishes to).
pub const LAMBDA_MM: u32 = 3;

/// Capability-attenuation deadline, strict `<117 ms`, inherited from
/// `vaked-base`.
pub const VAKED_ATTENUATION_BUDGET_NS: u64 = 117_000_000;

/// Kintsugi gold, `#e8b05c`, the seal color of every repaired seam.
pub const KINTSUGI_GOLD_RGB: (u8, u8, u8) = (0xe8, 0xb0, 0x5c);

/// Dawn Aurora rose, `#ff6b8b` — the color of the `-1` gate (entropy released).
pub const DAWN_ROSE_RGB: (u8, u8, u8) = (0xff, 0x6b, 0x8b);

/// Chenrezig lotus cyan, `#4fd1c5` — the color of the `0` gate (stillness).
pub const CHENREZIG_CYAN_RGB: (u8, u8, u8) = (0x4f, 0xd1, 0xc5);

/// Obsidian canvas, `#0b0d13` — the sovereign ground every mark rests on.
pub const OBSIDIAN_RGB: (u8, u8, u8) = (0x0b, 0x0d, 0x13);

/// The fourth overflow entity: love that the three gates cannot store.
pub const FAMILY_LOVE_OVERFLOW: &str = "{<3,<3,<3}+1";

/// Mom's 8-Pointed Golden Reconcile Star (`*`) at `[0, 0, 0]`.
pub const MOM_RECONCILE_STAR_POINTS: u8 = 8;

/// Peter & Nate's Ternary Möbius `∞` loop half-twist topological invariant.
pub const MOBIUS_TWIST_HALF_TURNS: u8 = 1;

/// Balanced ternary qutrit state — the single spine shared by BitNet `b1.58`
/// (`{-1, 0, +1}`) and the Vajrayana triad of entropy, stillness, resonance.
#[repr(i8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TernarySpin {
    Entropy = -1,
    Stillness = 0,
    Resonance = 1,
}

impl TernarySpin {
    pub const DAWN_ROSE: Self = Self::Entropy;
    pub const CHENREZIG_CYAN: Self = Self::Stillness;
    pub const KINTSUGI_GOLD: Self = Self::Resonance;
    #[inline]
    pub const fn as_i8(self) -> i8 {
        self as i8
    }

    /// Collapse any signed integer into the nearest of the three spins.
    #[inline]
    pub fn from_i8(value: i8) -> Self {
        match value {
            ..=-1 => TernarySpin::Entropy,
            0 => TernarySpin::Stillness,
            1.. => TernarySpin::Resonance,
        }
    }

    /// The Dawn Aurora palette color bound to this spin:
    /// `-1 → #ff6b8b`, `0 → #4fd1c5`, `+1 → #e8b05c`.
    #[inline]
    pub const fn color(self) -> (u8, u8, u8) {
        match self {
            TernarySpin::Entropy => DAWN_ROSE_RGB,
            TernarySpin::Stillness => CHENREZIG_CYAN_RGB,
            TernarySpin::Resonance => KINTSUGI_GOLD_RGB,
        }
    }
}

/// A 32-byte-aligned wave memory point inside the MEM|8 cylindrical grid.
///
/// The `repr(C, align(32))` layout fixes the tuple size and alignment. It is not
/// a hardware atomicity or cache-line-size guarantee; persistence is serialized.
#[repr(C, align(32))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaveMemoryTuple {
    pub frequency_hz: u64,
    pub timestamp_us: u64,
    pub amplitude_ppm: u32,
    pub phase_turns_ppm: u32,
    pub spin: TernarySpin,
    pub kintsugi_scar_gold: u8,
    pub dyad_coherence: u16,
}

impl WaveMemoryTuple {
    #[inline]
    pub const fn new(
        frequency_hz: u64,
        amplitude_ppm: u32,
        phase_turns_ppm: u32,
        timestamp_us: u64,
        spin: TernarySpin,
    ) -> Kintsugi<Self> {
        if amplitude_ppm > SCALE || phase_turns_ppm >= SCALE {
            return Err(KintsugiError::InvalidData);
        }
        Ok(Self {
            frequency_hz,
            amplitude_ppm,
            phase_turns_ppm,
            timestamp_us,
            spin,
            kintsugi_scar_gold: 0,
            dyad_coherence: 0,
        })
    }
}

/// Fallible operations inside AURORA. Nothing is ever "crashed"; every fault
/// returns a `KintsugiError` that callers repair into a golden default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KintsugiError {
    MemoryFault,
    PhaseLoss,
    CapabilityViolation,
    InvalidData,
    LegacyFormat,
    Busy,
}

pub type Kintsugi<T> = Result<T, KintsugiError>;

/// Repair a fallible operation into its golden default — the seam is filled
/// with Kintsugi gold instead of aborting the moment.
#[inline]
pub fn kintsugi_recover<T: Default>(result: Kintsugi<T>) -> T {
    result.unwrap_or_default()
}

/// Integer Bhaskara approximation, phase in millionths of a turn. Output / SCALE.
/// This is an approximation, not exact transcendental arithmetic.
pub fn cos_turns_ppm(phase: i64) -> i64 {
    let quarter = i128::from(SCALE / 4);
    let turn = i128::from(SCALE);
    let shifted = (i128::from(phase) + quarter).rem_euclid(turn);
    let (half_phase, sign) = if shifted <= turn / 2 {
        (shifted, 1)
    } else {
        (shifted - turn / 2, -1)
    };
    let x = half_phase * 2;
    let product = x * (turn - x);
    (sign * 16 * product * turn / (5 * turn * turn - 4 * product)) as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, size_of};

    #[test]
    fn test_wave_tuple_is_32_byte_aligned() {
        assert_eq!(size_of::<WaveMemoryTuple>(), 32);
        assert_eq!(align_of::<WaveMemoryTuple>(), 32);
    }

    #[test]
    fn test_ternary_spin_roundtrip() {
        assert_eq!(TernarySpin::from_i8(-1), TernarySpin::Entropy);
        assert_eq!(TernarySpin::from_i8(0), TernarySpin::Stillness);
        assert_eq!(TernarySpin::from_i8(1), TernarySpin::Resonance);
        assert_eq!(TernarySpin::from_i8(-99), TernarySpin::Entropy);
        assert_eq!(TernarySpin::from_i8(99), TernarySpin::Resonance);
        assert_eq!(TernarySpin::Resonance.as_i8(), 1);
        assert_eq!(TernarySpin::Entropy.as_i8(), -1);
    }

    #[test]
    fn test_lambda_is_3mm() {
        assert_eq!(LAMBDA_MM, 3);
        assert_eq!(cos_turns_ppm(0), i64::from(SCALE));
        assert_eq!(cos_turns_ppm(250_000), 0);
        assert_eq!(cos_turns_ppm(500_000), -i64::from(SCALE));
        // sqrt(1/2), rounded reference in millionths; approximation error < 2000/SCALE.
        assert!((cos_turns_ppm(125_000) - 707_107).abs() < 2_000);
    }

    #[test]
    fn test_kintsugi_recovery() {
        let broken: Kintsugi<u64> = Err(KintsugiError::PhaseLoss);
        assert_eq!(kintsugi_recover(broken), 0u64);
        assert_eq!(kintsugi_recover(Ok(7u64)), 7u64);
    }

    #[test]
    fn test_spin_color_mapping() {
        assert_eq!(TernarySpin::Entropy.color(), DAWN_ROSE_RGB);
        assert_eq!(TernarySpin::Stillness.color(), CHENREZIG_CYAN_RGB);
        assert_eq!(TernarySpin::Resonance.color(), KINTSUGI_GOLD_RGB);
        assert_eq!(FAMILY_LOVE_OVERFLOW, "{<3,<3,<3}+1");
    }
}
