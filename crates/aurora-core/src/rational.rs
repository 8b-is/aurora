//! Canonical checked p/q arithmetic. Zero denominators and overflow are errors.
use std::cmp::Ordering;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArithmeticError {
    ZeroDenominator,
    Overflow,
    InvalidDomain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rational {
    p: i64,
    q: u64,
}

impl Rational {
    pub const ZERO: Self = Self { p: 0, q: 1 };
    pub const ONE: Self = Self { p: 1, q: 1 };
    pub const fn integer(p: i64) -> Self {
        Self { p, q: 1 }
    }
    pub fn new(p: i64, q: u64) -> Result<Self, ArithmeticError> {
        Self::wide(i128::from(p), u128::from(q))
    }
    fn wide(p: i128, q: u128) -> Result<Self, ArithmeticError> {
        if q == 0 {
            return Err(ArithmeticError::ZeroDenominator);
        }
        let (mut a, mut b) = (p.unsigned_abs(), q);
        while b != 0 {
            (a, b) = (b, a % b);
        }
        let magnitude = p.unsigned_abs() / a;
        let signed = if p < 0 {
            -(i128::try_from(magnitude).map_err(|_| ArithmeticError::Overflow)?)
        } else {
            i128::try_from(magnitude).map_err(|_| ArithmeticError::Overflow)?
        };
        Ok(Self {
            p: i64::try_from(signed).map_err(|_| ArithmeticError::Overflow)?,
            q: u64::try_from(q / a).map_err(|_| ArithmeticError::Overflow)?,
        })
    }
    pub const fn numerator(self) -> i64 {
        self.p
    }
    pub const fn denominator(self) -> u64 {
        self.q
    }
    pub fn checked_add(self, rhs: Self) -> Result<Self, ArithmeticError> {
        let p = (i128::from(self.p) * i128::from(rhs.q))
            .checked_add(i128::from(rhs.p) * i128::from(self.q))
            .ok_or(ArithmeticError::Overflow)?;
        Self::wide(p, u128::from(self.q) * u128::from(rhs.q))
    }
    pub fn checked_sub(self, rhs: Self) -> Result<Self, ArithmeticError> {
        let p = (i128::from(self.p) * i128::from(rhs.q))
            .checked_sub(i128::from(rhs.p) * i128::from(self.q))
            .ok_or(ArithmeticError::Overflow)?;
        Self::wide(p, u128::from(self.q) * u128::from(rhs.q))
    }
    pub fn checked_mul(self, rhs: Self) -> Result<Self, ArithmeticError> {
        Self::wide(
            i128::from(self.p) * i128::from(rhs.p),
            u128::from(self.q) * u128::from(rhs.q),
        )
    }
    pub fn checked_div(self, rhs: Self) -> Result<Self, ArithmeticError> {
        let p = i128::from(self.p) * i128::from(rhs.q);
        Self::wide(
            if rhs.p < 0 { -p } else { p },
            u128::from(self.q) * u128::from(rhs.p.unsigned_abs()),
        )
    }
    /// Explicit fixed-denominator projection, truncated toward zero. Error < 1/scale.
    pub fn scaled(self, scale: u64) -> Result<i64, ArithmeticError> {
        if scale == 0 {
            return Err(ArithmeticError::ZeroDenominator);
        }
        i64::try_from(i128::from(self.p) * i128::from(scale) / i128::from(self.q))
            .map_err(|_| ArithmeticError::Overflow)
    }
    /// Zeno Paradox 1 (The Dichotomy): partial sum S_k = sum_{n=1..=k} (1/2)^n = (2^k - 1) / 2^k.
    pub fn dichotomy_partial_sum(steps: u32) -> Result<Self, ArithmeticError> {
        if steps == 0 {
            return Ok(Self::ZERO);
        }
        if steps >= 62 {
            return Err(ArithmeticError::Overflow);
        }
        let denom = 1_u64.checked_shl(steps).ok_or(ArithmeticError::Overflow)?;
        let numer = i64::try_from(denom - 1).map_err(|_| ArithmeticError::Overflow)?;
        Self::new(numer, denom)
    }
    /// Closed-form convergent geometric series limit S = a / (1 - r) for |r| < 1.
    pub fn geometric_limit(first_term: Self, ratio: Self) -> Result<Self, ArithmeticError> {
        let abs_ratio = Self::new(ratio.p.abs(), ratio.q)?;
        if abs_ratio >= Self::ONE {
            return Err(ArithmeticError::InvalidDomain);
        }
        let denom = Self::ONE.checked_sub(ratio)?;
        first_term.checked_div(denom)
    }
    /// Zeno Paradox 2 (Achilles and the Tortoise): T = d0 / (v_a - v_t) for v_a > v_t >= 0.
    pub fn achilles_catchup_time(
        head_start: Self,
        v_achilles: Self,
        v_tortoise: Self,
    ) -> Result<Self, ArithmeticError> {
        if head_start < Self::ZERO || v_tortoise < Self::ZERO || v_achilles <= v_tortoise {
            return Err(ArithmeticError::InvalidDomain);
        }
        let relative_v = v_achilles.checked_sub(v_tortoise)?;
        head_start.checked_div(relative_v)
    }
    /// Zeno Paradox 2 (Achilles overtake distance): D = (v_a * d0) / (v_a - v_t).
    pub fn achilles_overtake_distance(
        head_start: Self,
        v_achilles: Self,
        v_tortoise: Self,
    ) -> Result<Self, ArithmeticError> {
        let time = Self::achilles_catchup_time(head_start, v_achilles, v_tortoise)?;
        v_achilles.checked_mul(time)
    }
    /// Zeno Paradox 3 (The Arrow): instantaneous velocity quotient dx / dt for dt > 0.
    pub fn arrow_velocity(dx: Self, dt: Self) -> Result<Self, ArithmeticError> {
        if dt <= Self::ZERO {
            return Err(ArithmeticError::InvalidDomain);
        }
        dx.checked_div(dt)
    }
    /// Zeno Paradox 4 (The Stadium / Moving Rows): relative velocity v_B - v_C across frames.
    pub fn stadium_relative_velocity(v_row_b: Self, v_row_c: Self) -> Result<Self, ArithmeticError> {
        v_row_b.checked_sub(v_row_c)
    }
}
impl Ord for Rational {
    fn cmp(&self, rhs: &Self) -> Ordering {
        (i128::from(self.p) * i128::from(rhs.q)).cmp(&(i128::from(rhs.p) * i128::from(self.q)))
    }
}
impl PartialOrd for Rational {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}
impl fmt::Display for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.p, self.q)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checked_extremes_and_reduction() {
        assert_eq!(Rational::new(2, 4).unwrap().to_string(), "1/2");
        assert_eq!(
            Rational::new(i64::MIN, 1 << 63).unwrap(),
            Rational::integer(-1)
        );
        assert_eq!(
            Rational::integer(i64::MAX).checked_add(Rational::ONE),
            Err(ArithmeticError::Overflow)
        );
        assert_eq!(
            Rational::ONE.checked_div(Rational::ZERO),
            Err(ArithmeticError::ZeroDenominator)
        );
        let third = Rational::new(1, 3).unwrap();
        assert_eq!(
            third
                .checked_add(third)
                .unwrap()
                .checked_add(third)
                .unwrap(),
            Rational::ONE
        );
        assert_eq!(third.scaled(1_000_000).unwrap(), 333_333);
        assert!(Rational::new(i64::MIN, u64::MAX).unwrap() < Rational::ZERO);
    }

    #[test]
    fn zeno_four_paradoxes_exact_rational_resolution() {
        // 1. The Dichotomy: S_k = (2^k - 1)/2^k and closed-form limit S = (1/2) / (1 - 1/2) = 1/1.
        let half = Rational::new(1, 2).unwrap();
        assert_eq!(
            Rational::dichotomy_partial_sum(4).unwrap(),
            Rational::new(15, 16).unwrap()
        );
        // At k = 60, binary64 mantissas have already collapsed to 1, whereas p/q preserves 1/2^60 gap.
        let s60 = Rational::dichotomy_partial_sum(60).unwrap();
        assert!(s60 < Rational::ONE);
        assert_eq!(
            Rational::ONE.checked_sub(s60).unwrap(),
            Rational::new(1, 1_u64 << 60).unwrap()
        );
        assert_eq!(
            Rational::geometric_limit(half, half).unwrap(),
            Rational::ONE
        );

        // 2. Achilles and the Tortoise: d0 = 100, v_A = 10, v_T = 1 => T = 100/9, D = 1000/9.
        let d0 = Rational::integer(100);
        let v_a = Rational::integer(10);
        let v_t = Rational::integer(1);
        assert_eq!(
            Rational::achilles_catchup_time(d0, v_a, v_t).unwrap(),
            Rational::new(100, 9).unwrap()
        );
        assert_eq!(
            Rational::achilles_overtake_distance(d0, v_a, v_t).unwrap(),
            Rational::new(1000, 9).unwrap()
        );

        // 3. The Arrow: instantaneous quotient dx/dt remains invariant across arbitrary refinement.
        let v_coarse =
            Rational::arrow_velocity(Rational::integer(30), Rational::integer(10)).unwrap();
        let v_fine = Rational::arrow_velocity(
            Rational::new(3, 1_000_000).unwrap(),
            Rational::new(1, 1_000_000).unwrap(),
        )
        .unwrap();
        assert_eq!(v_coarse, Rational::integer(3));
        assert_eq!(v_fine, v_coarse);

        // 4. The Stadium (Moving Rows): B (+v) vs C (-v) yields 2v relative to stationary A (0).
        let v = Rational::new(7, 3).unwrap();
        let neg_v = Rational::new(-7, 3).unwrap();
        assert_eq!(
            Rational::stadium_relative_velocity(v, neg_v).unwrap(),
            Rational::new(14, 3).unwrap()
        );
    }
}
