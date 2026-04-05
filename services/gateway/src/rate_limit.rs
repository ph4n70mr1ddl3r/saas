use std::time::Instant;

const MAX_TOKENS: u64 = 100;
const REFILL_RATE: u64 = 10; // tokens per second

pub struct TokenBucket {
    tokens: u64,
    last_refill: Instant,
}

impl TokenBucket {
    pub fn new() -> Self {
        Self {
            tokens: MAX_TOKENS,
            last_refill: Instant::now(),
        }
    }

    pub fn try_consume(&mut self) -> bool {
        self.refill();
        if self.tokens > 0 {
            self.tokens -= 1;
            true
        } else {
            false
        }
    }

    /// Expose last_refill for cleanup logic.
    pub fn last_refill(&self) -> Instant {
        self.last_refill
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs();
        if elapsed > 0 {
            self.tokens = (self.tokens + elapsed * REFILL_RATE).min(MAX_TOKENS);
            self.last_refill = now;
        }
    }
}
