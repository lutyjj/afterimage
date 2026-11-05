use std::time::{SystemTime, UNIX_EPOCH};

/// The time the server acts at, so that expiry is testable.
pub trait Clock: Send + Sync + 'static {
    /// Seconds since the Unix epoch.
    fn now(&self) -> i64;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| {
                i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
            })
    }
}
