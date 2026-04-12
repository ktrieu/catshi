use std::{
    fmt::Display,
    ops::{Add, Mul, Neg, Sub},
};

// Currency in our system - symbol YP. Stored as integer 1000ths (bips), so a value of 1000 = 1.00 YP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, sqlx::Type)]
#[sqlx(transparent)]
pub struct Currency(i64);

const BIPS_PER_YP: i64 = 1000;

impl Currency {
    pub fn from_instrument_price(price: f32) -> Self {
        // All instrument prices are a probability [0, 1], and we price a 100% contract as 1 YP.
        // So we need to multiply our price by 100 before rounding to an integer.

        let rounded = (price * BIPS_PER_YP as f32).round_ties_even().trunc();

        Self(rounded as i64)
    }

    pub fn as_instrument_price(&self) -> f32 {
        // Convert from currency to a fractional number of 100% instruments.

        self.0 as f32 / BIPS_PER_YP as f32
    }

    pub const fn new_yp(yp: i64) -> Self {
        Currency(yp * BIPS_PER_YP)
    }
}

impl Display for Currency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // We'll manually apply the negative sign later.
        let positive = self.0.abs();
        // For precision let's not use floats to downscale and use integer arithmetic instead.
        let fractional = positive % BIPS_PER_YP;

        let yp = (positive - fractional) / BIPS_PER_YP;

        let neg_sign = if self.0 < 0 { "-" } else { "" };

        write!(f, "{}{}.{:03}yp", neg_sign, yp, fractional)
    }
}

impl Add for Currency {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Currency(self.0 + rhs.0)
    }
}

impl Sub for Currency {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Currency(self.0 - rhs.0)
    }
}

impl Neg for Currency {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Currency(-self.0)
    }
}

impl Mul<f32> for Currency {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        let raw = self.0 as f32 * rhs;

        Currency(raw.round_ties_even() as i64)
    }
}

impl Mul<i64> for Currency {
    type Output = Self;

    fn mul(self, rhs: i64) -> Self::Output {
        let raw = self.0 * rhs;

        Currency(raw)
    }
}

impl From<Currency> for i64 {
    fn from(value: Currency) -> Self {
        value.0
    }
}

impl From<i64> for Currency {
    fn from(value: i64) -> Self {
        Currency(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_instrument_price() {
        assert_eq!(Currency::from_instrument_price(0.500).0, 500);
        assert_eq!(Currency::from_instrument_price(1.0).0, 1000);
        assert_eq!(Currency::from_instrument_price(0.001).0, 1);
    }

    #[test]
    fn test_as_instrument_price() {
        assert_eq!(Currency::from(1500).as_instrument_price(), 1.5);
    }

    #[test]
    fn test_new_yp() {
        assert_eq!(Currency::new_yp(12).0, 12000);
    }

    #[test]
    fn test_formatting() {
        assert_eq!(Currency::from(1000).to_string(), "1.000yp");
        assert_eq!(Currency::from(1500).to_string(), "1.500yp");
        assert_eq!(Currency::from(-1200).to_string(), "-1.200yp");
        assert_eq!(Currency::from(35001).to_string(), "35.001yp");
    }
}
