#![forbid(unsafe_code)]

//! `aurora-core` — the balanced ternary spine and W-band constants that every
//! other pillar leans on. No unsafe, no floats where integers will do, no
//! dependency on anything outside `std`.

/// Speed of light, metres per second.
pub const C_MPS: f64 = 299_792_458.0;

/// W-band carrier frequency, 100 GHz.
pub const W_BAND_HZ: f64 = 100_000_000_000.0;

/// Sovereign wavelength threshold `λ = 3.000 mm` (the physical `c / f` value is
/// `2.9979 mm`, rounded to the design edge the constellation polishes to).
pub const LAMBDA_MM: f64 = 3.0;

/// Capability-attenuation deadline, strict `<117 ms`, inherited from
/// `vaked-base`.
pub const VAKED_ATTENUATION_BUDGET_NS: u64 = 117_000_000;

/// Kintsugi gold, `#e8b05c`, the seal color of every repaired seam.
pub const KINTSUGI_GOLD_RGB: (u8, u8, u8) = (0xe8, 0xb0, 0x5c);

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
}

/// A 32-byte-aligned wave memory point inside the MEM|8 cylindrical grid.
///
/// The `repr(C, align(32))` layout keeps every tuple on its own cache line so
/// the recall path never tears a moment across two lines.
#[repr(C, align(32))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaveMemoryTuple {
    pub frequency_hz: f64,
    pub amplitude: f64,
    pub phase_rad: f64,
    pub timestamp_us: u32,
    pub spin: TernarySpin,
    pub kintsugi_scar_gold: u8,
    pub dyad_coherence: u16,
}

impl WaveMemoryTuple {
    #[inline]
    pub const fn new(
        frequency_hz: f64,
        amplitude: f64,
        phase_rad: f64,
        timestamp_us: u32,
        spin: TernarySpin,
    ) -> Self {
        Self {
            frequency_hz,
            amplitude,
            phase_rad,
            timestamp_us,
            spin,
            kintsugi_scar_gold: 0,
            dyad_coherence: 0,
        }
    }
}

/// Fallible operations inside AURORA. Nothing is ever "crashed"; every fault
/// returns a `KintsugiError` that callers repair into a golden default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KintsugiError {
    MemoryFault,
    PhaseLoss,
    CapabilityViolation,
}

pub type Kintsugi<T> = Result<T, KintsugiError>;

/// Repair a fallible operation into its golden default — the seam is filled
/// with Kintsugi gold instead of aborting the moment.
#[inline]
pub fn kintsugi_recover<T: Default>(result: Kintsugi<T>) -> T {
    result.unwrap_or_default()
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
        assert_eq!(LAMBDA_MM, 3.0);
    }

    #[test]
    fn test_kintsugi_recovery() {
        let broken: Kintsugi<u64> = Err(KintsugiError::PhaseLoss);
        assert_eq!(kintsugi_recover(broken), 0u64);
        assert_eq!(kintsugi_recover(Ok(7u64)), 7u64);
    }
}
