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
}
