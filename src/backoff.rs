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

/// Capped exponential backoff used after an established connection drops.
#[derive(Debug, Clone)]
pub struct ReconnectBackoff {
    step: u32,
}

impl ReconnectBackoff {
    /// Upper bound on any single backoff delay.
    pub const CAP: Duration = Duration::from_secs(30);

    pub fn new() -> Self {
        Self { step: 0 }
    }

    /// The delay to wait before the next reconnect attempt. Grows 2s → 4s → 8s
    /// → 16s → 30s and then stays at the cap. Never terminates.
    pub fn next_delay(&mut self) -> Duration {
        // 2^(step+1) seconds, saturating at the cap. `step` saturates too so a
        // long-running disconnect can never overflow the shift.
        let exponent = self.step.saturating_add(1).min(16);
        let secs = 1u64 << exponent;
        self.step = self.step.saturating_add(1);
        Self::CAP.min(Duration::from_secs(secs))
    }

    /// Reset the sequence after a successful reconnect, so a later disconnect
    /// starts again from 2s.
    pub fn reset(&mut self) {
        self.step = 0;
    }
}

impl Default for ReconnectBackoff {
    fn default() -> Self {
        Self::new()
    }
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
    fn reconnect_follows_exponential_then_caps() {
        let mut b = ReconnectBackoff::new();
        let seq: Vec<_> = (0..7).map(|_| b.next_delay()).collect();
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
    fn reconnect_never_exceeds_cap() {
        let mut b = ReconnectBackoff::new();
        for _ in 0..100 {
            assert!(b.next_delay() <= ReconnectBackoff::CAP);
        }
    }

    #[test]
    fn defaults_match_new() {
        assert_eq!(
            InitialConnectBackoff::default().next_delay(),
            InitialConnectBackoff::new().next_delay()
        );
        assert_eq!(
            ReconnectBackoff::default().next_delay(),
            ReconnectBackoff::new().next_delay()
        );
    }

    #[test]
    fn reconnect_reset_restarts_from_two_seconds() {
        let mut b = ReconnectBackoff::new();
        let _ = b.next_delay(); // 2
        let _ = b.next_delay(); // 4
        let _ = b.next_delay(); // 8
        b.reset();
        assert_eq!(b.next_delay(), secs(2));
        assert_eq!(b.next_delay(), secs(4));
    }
}
