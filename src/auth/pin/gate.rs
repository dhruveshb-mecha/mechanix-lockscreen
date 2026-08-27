use std::time::{Duration, Instant};

/// Brute-force throttle: counts failed attempts and locks out after too many.
#[derive(Debug, Clone)]
pub struct Gate {
    max_attempts: u32,
    lockout: Duration,
    attempts: u32,
    locked_until: Option<Instant>,
}

impl Gate {
    pub fn new(max_attempts: u32, lockout: Duration) -> Self {
        Self {
            max_attempts,
            lockout,
            attempts: 0,
            locked_until: None,
        }
    }

    pub fn attempts(&self) -> u32 {
        self.attempts
    }

    pub fn lockout_remaining(&self, now: Instant) -> Option<u32> {
        match self.locked_until {
            Some(until) if now < until => {
                let remaining = until.saturating_duration_since(now);
                Some(remaining.as_secs() as u32 + u32::from(remaining.subsec_millis() > 0))
            }
            _ => None,
        }
    }

    pub fn tick(&mut self, now: Instant) -> bool {
        if let Some(until) = self.locked_until
            && now >= until
        {
            self.locked_until = None;
            self.attempts = 0;
            return true;
        }
        false
    }

    pub fn failed(&mut self, now: Instant) -> Option<Duration> {
        if self.lockout_remaining(now).is_some() {
            return None;
        }
        self.tick(now);
        self.attempts += 1;
        if self.attempts >= self.max_attempts {
            self.attempts = 0;
            self.locked_until = Some(now + self.lockout);
            Some(self.lockout)
        } else {
            None
        }
    }

    pub fn succeeded(&mut self) {
        self.attempts = 0;
        self.locked_until = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gate() -> Gate {
        Gate::new(3, Duration::from_secs(30))
    }

    #[test]
    fn failures_until_lockout() {
        let mut g = gate();
        let t0 = Instant::now();
        assert_eq!(g.failed(t0), None);
        assert_eq!(g.failed(t0), None);
        assert_eq!(g.attempts(), 2);
        assert_eq!(g.failed(t0), Some(Duration::from_secs(30)));
        assert_eq!(g.attempts(), 0);
        assert_eq!(g.lockout_remaining(t0), Some(30));
    }

    #[test]
    fn lockout_expires() {
        let mut g = gate();
        let t0 = Instant::now();
        for _ in 0..3 {
            g.failed(t0);
        }
        let t1 = t0 + Duration::from_secs(31);
        assert!(g.tick(t1));
        assert_eq!(g.lockout_remaining(t1), None);
        assert_eq!(g.attempts(), 0);
    }

    #[test]
    fn lockout_remaining_counts_up() {
        let mut g = gate();
        let t0 = Instant::now();
        for _ in 0..3 {
            g.failed(t0);
        }
        assert_eq!(g.lockout_remaining(t0), Some(30));
        assert_eq!(g.lockout_remaining(t0 + Duration::from_secs(29)), Some(1));
        assert_eq!(g.lockout_remaining(t0 + Duration::from_secs(30)), None);
    }

    #[test]
    fn failed_during_lockout_is_ignored() {
        let mut g = gate();
        let t0 = Instant::now();
        for _ in 0..3 {
            g.failed(t0);
        }
        assert_eq!(g.failed(t0 + Duration::from_secs(1)), None);
        assert_eq!(g.attempts(), 0);
        assert_eq!(g.lockout_remaining(t0 + Duration::from_secs(1)), Some(29));
    }
}
