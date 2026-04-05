use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Money {
    pub amount_cents: i64,
    pub currency: String,
}

impl Money {
    pub fn new(amount_cents: i64, currency: &str) -> Self {
        Self { amount_cents, currency: currency.to_string() }
    }

    pub fn usd(amount_cents: i64) -> Self {
        Self::new(amount_cents, "USD")
    }

    pub fn zero() -> Self {
        Self::usd(0)
    }
}

impl std::ops::Add for Money {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        assert_eq!(self.currency, rhs.currency,
            "Cannot add Money with different currencies: {} != {}", self.currency, rhs.currency);
        Self { amount_cents: self.amount_cents + rhs.amount_cents, currency: self.currency }
    }
}

impl std::ops::Sub for Money {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        assert_eq!(self.currency, rhs.currency,
            "Cannot subtract Money with different currencies: {} != {}", self.currency, rhs.currency);
        Self { amount_cents: self.amount_cents - rhs.amount_cents, currency: self.currency }
    }
}
