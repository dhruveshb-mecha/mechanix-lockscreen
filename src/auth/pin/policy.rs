use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct Policy {
    pub pin_len: usize,
    pub max_attempts: u32,
    pub lockout: Duration,
}

impl Policy {
    pub const PIN_LEN: usize = 4;

    pub const fn new() -> Self {
        Self {
            pin_len: Self::PIN_LEN,
            max_attempts: 5,
            lockout: Duration::from_secs(30),
        }
    }
}

impl Default for Policy {
    fn default() -> Self {
        Self::new()
    }
}
