//! When to try again after a connection drops. Five quick attempts
//! (1, 2, 4, 8 and 16 s), then — without ever giving up — a check every 30 s
//! whether the console is on, so audio comes back by itself after the console
//! was off or the Wi-Fi was down for a while.

use std::time::Duration;

/// Consecutive attempts at 1, 2, 4, 8 and 16 s.
pub const QUICK_ATTEMPTS: u32 = 5;
/// After that, how often to check whether the console is on.
pub const SLOW_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, PartialEq)]
pub enum Wait {
    /// Try again after this delay.
    Quick(Duration),
    /// Wait for the console: check every `SLOW_INTERVAL`, reconnect when it is on.
    Slow,
}

/// `failures`: consecutive failures including this one (1 for the first).
pub fn wait(failures: u32) -> Wait {
    match failures { 1..=QUICK_ATTEMPTS => Wait::Quick(Duration::from_secs(1 << (failures - 1))), _ => Wait::Slow }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_schedule() {
        let quick: Vec<u64> = (1..=5).map(|n| match wait(n) { Wait::Quick(d) => d.as_secs(), Wait::Slow => 0 }).collect();
        assert_eq!(quick, vec![1, 2, 4, 8, 16]);
        // And then, never giving up.
        for n in [6, 7, 100, u32::MAX] { assert_eq!(wait(n), Wait::Slow) }
    }
}
