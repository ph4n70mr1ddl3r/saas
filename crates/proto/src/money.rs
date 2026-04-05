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
    type Output = Result<Self, String>;
    fn add(self, rhs: Self) -> Self::Output {
        if self.currency != rhs.currency {
            return Err(format!("Cannot add Money with different currencies: {} != {}", self.currency, rhs.currency));
        }
        let amount = self.amount_cents.checked_add(rhs.amount_cents)
            .ok_or_else(|| format!("Money overflow: {} + {}", self.amount_cents, rhs.amount_cents))?;
        Ok(Self { amount_cents: amount, currency: self.currency })
    }
}

impl std::ops::Sub for Money {
    type Output = Result<Self, String>;
    fn sub(self, rhs: Self) -> Self::Output {
        if self.currency != rhs.currency {
            return Err(format!("Cannot subtract Money with different currencies: {} != {}", self.currency, rhs.currency));
        }
        let amount = self.amount_cents.checked_sub(rhs.amount_cents)
            .ok_or_else(|| format!("Money underflow: {} - {}", self.amount_cents, rhs.amount_cents))?;
        Ok(Self { amount_cents: amount, currency: self.currency })
    }
}
