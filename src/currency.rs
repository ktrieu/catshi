use std::{
    fmt::Display,
    ops::{Add, Mul, Neg, Sub},
};

// Currency in our system - symbol YP. Stored as integer 100ths (bips), so a value of 100 = 1.00 YP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, sqlx::Type)]
#[sqlx(transparent)]
pub struct Currency(i64);

const BIPS_PER_YP: i64 = 100;

impl Currency {
    pub fn from_instrument_price(price: f32) -> Self {
        // All instrument prices are a probability [0, 1], and we price a 100% contract as 1 YP.
        // So we need to multiply our price by 100 before rounding to an integer.

        let rounded = (price * BIPS_PER_YP as f32).round_ties_even().trunc();

        Self(rounded as i64)
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

        write!(f, "{}{}.{}yp", neg_sign, yp, fractional)
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
