//! Reconnection backoff policy (platform-independent).
//!
//! Per `ENS-POC-Spec_1.md` §7:
//! - **Initial connect:** retry every 5 seconds, up to 12 attempts (1 minute),
//!   then give up.
//! - **After disconnect:** exponential backoff 2s → 4s → 8s → 16s → 30s, with
//!   30s as the cap, retrying indefinitely until reconnected.
//!
//! No jitter at POC scale (add before production rollout).

use std::time::Duration;

/// Fixed-interval, bounded retry used for the very first connection.
#[derive(Debug, Clone)]
pub struct InitialConnectBackoff {
    attempts_made: u32,
}

impl InitialConnectBackoff {
    /// Delay between connection attempts.
    pub const INTERVAL: Duration = Duration::from_secs(5);
    /// Maximum number of attempts before giving up.
    pub const MAX_ATTEMPTS: u32 = 12;

    pub fn new() -> Self {
        Self { attempts_made: 0 }
    }

    /// The delay to wait before the next attempt, or `None` once the attempt
    /// budget is exhausted.
    pub fn next_delay(&mut self) -> Option<Duration> {
        if self.attempts_made >= Self::MAX_ATTEMPTS {
            return None;
        }
        self.attempts_made += 1;
        Some(Self::INTERVAL)
    }
}

impl Default for InitialConnectBackoff {
    fn default() -> Self {
        Self::new()
    }
}

/// Upper bound on any single reconnect delay (spec §7).
pub const RECONNECT_CAP: Duration = Duration::from_secs(30);

/// Capped exponential backoff for reconnecting after an established connection
/// drops: `2^attempt` seconds, capped at 30s. With `attempt` 1-based this gives
/// 2s, 4s, 8s, 16s, then 30s thereafter (spec §7).
///
/// This is a pure function rather than a stateful iterator because the actual
/// retry loop lives inside `async-nats` — the agent installs this as its
/// `reconnect_delay_callback`, which receives the attempt count. Reconnection
/// continues indefinitely (the cap never terminates).
pub fn reconnect_delay(attempt: u32) -> Duration {
    // 2^attempt seconds, saturating so a long outage can never overflow.
    let secs = 1u64.checked_shl(attempt).unwrap_or(u64::MAX);
    RECONNECT_CAP.min(Duration::from_secs(secs))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn initial_connect_yields_twelve_five_second_delays() {
        let mut b = InitialConnectBackoff::new();
        let delays: Vec<_> = std::iter::from_fn(|| b.next_delay()).collect();
        assert_eq!(delays, vec![secs(5); 12]);
    }

    #[test]
    fn initial_connect_total_wait_is_one_minute() {
        let mut b = InitialConnectBackoff::new();
        let total: Duration = std::iter::from_fn(|| b.next_delay()).sum();
        assert_eq!(total, secs(60));
    }

    #[test]
    fn initial_connect_stops_after_budget_exhausted() {
        let mut b = InitialConnectBackoff::new();
        for _ in 0..InitialConnectBackoff::MAX_ATTEMPTS {
            assert!(b.next_delay().is_some());
        }
        assert_eq!(b.next_delay(), None);
        // Stays exhausted on further calls.
        assert_eq!(b.next_delay(), None);
    }

    #[test]
    fn initial_connect_default_matches_new() {
        assert_eq!(
            InitialConnectBackoff::default().next_delay(),
            InitialConnectBackoff::new().next_delay()
        );
    }

    #[test]
    fn reconnect_delay_follows_exponential_then_caps() {
        // async-nats calls the callback with a 1-based attempt count.
        let seq: Vec<_> = (1..=7).map(reconnect_delay).collect();
        assert_eq!(
            seq,
            vec![
                secs(2),
                secs(4),
                secs(8),
                secs(16),
                secs(30),
                secs(30),
                secs(30)
            ]
        );
    }

    #[test]
    fn reconnect_delay_never_exceeds_cap() {
        for attempt in 0..100 {
            assert!(reconnect_delay(attempt) <= RECONNECT_CAP);
        }
    }

    #[test]
    fn reconnect_delay_saturates_on_huge_attempt() {
        // Must not panic/overflow on a shift past the integer width.
        assert_eq!(reconnect_delay(1000), RECONNECT_CAP);
    }
}
