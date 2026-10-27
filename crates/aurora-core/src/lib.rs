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

/// `8b-is/wupz` 32B frame gate (`0x62`), ABI version (`1`), `qt:55` (`0x55`), and fold `0xe1034e575c371ab0`.
pub const WUPZ_GATE_0X62: u8 = 0x62;
pub const WUPZ_FRAME_ABI_V1: u8 = 0x01;
pub const WUPZ_QUANT_QT55: u8 = 0x55;
pub const WUPZ_FOLD_0XE1034E575C371AB0: u64 = 0xe103_4e57_5c37_1ab0;

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

    /// Canonical 32-byte little-endian wire encoding for `8b-is/wupz` folding & sealing.
    #[inline]
    pub const fn to_wupz_bytes(self) -> [u8; 32] {
        let f = self.frequency_hz.to_le_bytes();
        let t = self.timestamp_us.to_le_bytes();
        let a = self.amplitude_ppm.to_le_bytes();
        let p = self.phase_turns_ppm.to_le_bytes();
        let d = self.dyad_coherence.to_le_bytes();
        [
            f[0],
            f[1],
            f[2],
            f[3],
            f[4],
            f[5],
            f[6],
            f[7],
            t[0],
            t[1],
            t[2],
            t[3],
            t[4],
            t[5],
            t[6],
            t[7],
            a[0],
            a[1],
            a[2],
            a[3],
            p[0],
            p[1],
            p[2],
            p[3],
            self.spin.as_i8() as u8,
            self.kintsugi_scar_gold,
            d[0],
            d[1],
            0,
            0,
            0,
            0,
        ]
    }
}

/// `8b-is/wupz` 4-way unrolled 64-byte XOR fold (`wupz_xor_fold` parity).
pub fn wupz_xor_fold(bytes: &[u8]) -> u64 {
    let mut a0 = 0u64;
    let mut a1 = 0u64;
    let mut a2 = 0u64;
    let mut a3 = 0u64;
    let mut i: usize = 0;
    while i + 64 <= bytes.len() {
        a0 ^= u64::from_le_bytes([
            bytes[i],
            bytes[i + 1],
            bytes[i + 2],
            bytes[i + 3],
            bytes[i + 4],
            bytes[i + 5],
            bytes[i + 6],
            bytes[i + 7],
        ]) ^ u64::from_le_bytes([
            bytes[i + 32],
            bytes[i + 33],
            bytes[i + 34],
            bytes[i + 35],
            bytes[i + 36],
            bytes[i + 37],
            bytes[i + 38],
            bytes[i + 39],
        ]);
        a1 ^= u64::from_le_bytes([
            bytes[i + 8],
            bytes[i + 9],
            bytes[i + 10],
            bytes[i + 11],
            bytes[i + 12],
            bytes[i + 13],
            bytes[i + 14],
            bytes[i + 15],
        ]) ^ u64::from_le_bytes([
            bytes[i + 40],
            bytes[i + 41],
            bytes[i + 42],
            bytes[i + 43],
            bytes[i + 44],
            bytes[i + 45],
            bytes[i + 46],
            bytes[i + 47],
        ]);
        a2 ^= u64::from_le_bytes([
            bytes[i + 16],
            bytes[i + 17],
            bytes[i + 18],
            bytes[i + 19],
            bytes[i + 20],
            bytes[i + 21],
            bytes[i + 22],
            bytes[i + 23],
        ]) ^ u64::from_le_bytes([
            bytes[i + 48],
            bytes[i + 49],
            bytes[i + 50],
            bytes[i + 51],
            bytes[i + 52],
            bytes[i + 53],
            bytes[i + 54],
            bytes[i + 55],
        ]);
        a3 ^= u64::from_le_bytes([
            bytes[i + 24],
            bytes[i + 25],
            bytes[i + 26],
            bytes[i + 27],
            bytes[i + 28],
            bytes[i + 29],
            bytes[i + 30],
            bytes[i + 31],
        ]) ^ u64::from_le_bytes([
            bytes[i + 56],
            bytes[i + 57],
            bytes[i + 58],
            bytes[i + 59],
            bytes[i + 60],
            bytes[i + 61],
            bytes[i + 62],
            bytes[i + 63],
        ]);
        i += 64;
    }
    let mut acc = (a0 ^ a1) ^ (a2 ^ a3);
    while i + 8 <= bytes.len() {
        acc ^= u64::from_le_bytes([
            bytes[i],
            bytes[i + 1],
            bytes[i + 2],
            bytes[i + 3],
            bytes[i + 4],
            bytes[i + 5],
            bytes[i + 6],
            bytes[i + 7],
        ]);
        i += 8;
    }
    while i < bytes.len() {
        acc ^= u64::from(bytes[i]);
        i += 1;
    }
    acc
}

/// `8b-is/wupz` 4-way unrolled 64-bit POPCNT (`wupz_popcount` parity).
pub fn wupz_popcount(bytes: &[u8]) -> u64 {
    let mut c0 = 0u64;
    let mut c1 = 0u64;
    let mut c2 = 0u64;
    let mut c3 = 0u64;
    let mut i: usize = 0;
    while i + 32 <= bytes.len() {
        c0 += u64::from(
            u64::from_le_bytes([
                bytes[i],
                bytes[i + 1],
                bytes[i + 2],
                bytes[i + 3],
                bytes[i + 4],
                bytes[i + 5],
                bytes[i + 6],
                bytes[i + 7],
            ])
            .count_ones(),
        );
        c1 += u64::from(
            u64::from_le_bytes([
                bytes[i + 8],
                bytes[i + 9],
                bytes[i + 10],
                bytes[i + 11],
                bytes[i + 12],
                bytes[i + 13],
                bytes[i + 14],
                bytes[i + 15],
            ])
            .count_ones(),
        );
        c2 += u64::from(
            u64::from_le_bytes([
                bytes[i + 16],
                bytes[i + 17],
                bytes[i + 18],
                bytes[i + 19],
                bytes[i + 20],
                bytes[i + 21],
                bytes[i + 22],
                bytes[i + 23],
            ])
            .count_ones(),
        );
        c3 += u64::from(
            u64::from_le_bytes([
                bytes[i + 24],
                bytes[i + 25],
                bytes[i + 26],
                bytes[i + 27],
                bytes[i + 28],
                bytes[i + 29],
                bytes[i + 30],
                bytes[i + 31],
            ])
            .count_ones(),
        );
        i += 32;
    }
    let mut count = (c0 + c1) + (c2 + c3);
    while i < bytes.len() {
        count += u64::from(bytes[i].count_ones());
        i += 1;
    }
    count
}

/// `8b-is/wupz` SWAR 64-bit byte-diff counter (`wupz_byte_diff` parity).
pub fn wupz_byte_diff(a: &[u8], b: &[u8]) -> u64 {
    let n = a.len().min(b.len());
    let mut count = 0u64;
    let mut i: usize = 0;
    while i + 8 <= n {
        let wa = u64::from_le_bytes([
            a[i],
            a[i + 1],
            a[i + 2],
            a[i + 3],
            a[i + 4],
            a[i + 5],
            a[i + 6],
            a[i + 7],
        ]);
        let wb = u64::from_le_bytes([
            b[i],
            b[i + 1],
            b[i + 2],
            b[i + 3],
            b[i + 4],
            b[i + 5],
            b[i + 6],
            b[i + 7],
        ]);
        let d = wa ^ wb;
        if d != 0 {
            let msb = ((d & 0x7f7f_7f7f_7f7f_7f7f) + 0x7f7f_7f7f_7f7f_7f7f) | d;
            count += u64::from((msb & 0x8080_8080_8080_8080).count_ones());
        }
        i += 8;
    }
    while i < n {
        if a[i] != b[i] {
            count += 1;
        }
        i += 1;
    }
    count
}

/// `8b-is/wupz` 32-byte `0x62`-gated frame seal (`sealFrame32B` parity).
pub fn seal_wupz_frame_32b(raw: &[u8], packed: &[u8], gate: u8) -> [u8; 32] {
    let mut frame = [0u8; 32];
    frame[0..8].copy_from_slice(&wupz_xor_fold(raw).to_le_bytes());
    frame[8..16].copy_from_slice(&wupz_xor_fold(packed).to_le_bytes());
    frame[16..20].copy_from_slice(&(raw.len() as u32).to_le_bytes());
    frame[20..24].copy_from_slice(&(packed.len() as u32).to_le_bytes());
    frame[24] = gate;
    frame[25] = WUPZ_FRAME_ABI_V1;
    let prefix = wupz_xor_fold(&frame[0..26]).to_le_bytes();
    frame[26..32].copy_from_slice(&prefix[0..6]);
    frame
}

/// `8b-is/wupz` 32-byte frame verifier using `wupz_byte_diff`.
pub fn verify_wupz_frame_32b(frame: &[u8; 32], raw: &[u8], packed: &[u8], gate: u8) -> bool {
    if frame[24] != gate || frame[25] != WUPZ_FRAME_ABI_V1 {
        return false;
    }
    let expected = seal_wupz_frame_32b(raw, packed, gate);
    wupz_byte_diff(frame, &expected) == 0
}

/// Pack a 64-bit fold signature into disjoint `BFP-64` bitplanes partitioned by `qt`.
#[inline]
pub const fn pack_fold_bfp64(fold: u64, qt: u8) -> (u64, u64) {
    let mask = (qt as u64).wrapping_mul(0x0101_0101_0101_0101);
    (fold & mask, fold & !mask)
}

/// Unpack and verify a 64-bit fold signature from `BFP-64` bitplanes at quantization `qt`.
#[inline]
pub const fn unpack_fold_bfp64(pos_plane: u64, neg_plane: u64, qt: u8) -> Kintsugi<u64> {
    if (pos_plane & neg_plane) != 0 {
        return Err(KintsugiError::InvalidData);
    }
    let mask = (qt as u64).wrapping_mul(0x0101_0101_0101_0101);
    if (pos_plane & !mask) != 0 || (neg_plane & mask) != 0 {
        return Err(KintsugiError::InvalidData);
    }
    Ok(pos_plane | neg_plane)
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

        let tuple = WaveMemoryTuple::new(
            W_BAND_HZ,
            800_000,
            250_000,
            1_730_000_000,
            TernarySpin::Resonance,
        )
        .unwrap();
        let raw = tuple.to_wupz_bytes();
        let (pos, neg) = pack_fold_bfp64(WUPZ_FOLD_0XE1034E575C371AB0, WUPZ_QUANT_QT55);
        assert_eq!(
            unpack_fold_bfp64(pos, neg, WUPZ_QUANT_QT55),
            Ok(WUPZ_FOLD_0XE1034E575C371AB0)
        );
        let mut packed = [0u8; 16];
        packed[0..8].copy_from_slice(&pos.to_le_bytes());
        packed[8..16].copy_from_slice(&neg.to_le_bytes());
        let mut seal = seal_wupz_frame_32b(&raw, &packed, WUPZ_GATE_0X62);
        assert!(verify_wupz_frame_32b(&seal, &raw, &packed, WUPZ_GATE_0X62));
        assert!(wupz_popcount(&seal) > 0);
        seal[31] ^= 0x01;
        assert!(!verify_wupz_frame_32b(&seal, &raw, &packed, WUPZ_GATE_0X62));
    }
}
