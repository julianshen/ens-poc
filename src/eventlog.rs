//! Windows Event Log severity mapping (platform-independent).
//!
//! Spec deliverable §9 #7 is "basic logging → Windows Event Log". The pure
//! decision here is how a `tracing` level maps to a Windows `EVENTLOG_*` event
//! type; the actual `ReportEventW` call is OS glue (`eventlog_win`). Keeping the
//! mapping here means it is unit-tested without the Win32 API.

use tracing::Level;

/// Event source name registered in the Application log (matches the service
/// name in [`crate::service`]).
pub const EVENT_SOURCE: &str = "YourCoNotificationAgent";

// Windows `EVENTLOG_*` event types (winnt.h).
pub const EVENTLOG_ERROR_TYPE: u16 = 0x0001;
pub const EVENTLOG_WARNING_TYPE: u16 = 0x0002;
pub const EVENTLOG_INFORMATION_TYPE: u16 = 0x0004;

/// Map a `tracing` level to the Windows Event Log event type.
pub fn eventlog_type(level: &Level) -> u16 {
    match *level {
        Level::ERROR => EVENTLOG_ERROR_TYPE,
        Level::WARN => EVENTLOG_WARNING_TYPE,
        // INFO, DEBUG, TRACE all surface as informational events.
        _ => EVENTLOG_INFORMATION_TYPE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_maps_to_error_type() {
        assert_eq!(eventlog_type(&Level::ERROR), EVENTLOG_ERROR_TYPE);
    }

    #[test]
    fn warn_maps_to_warning_type() {
        assert_eq!(eventlog_type(&Level::WARN), EVENTLOG_WARNING_TYPE);
    }

    #[test]
    fn info_and_below_map_to_information_type() {
        assert_eq!(eventlog_type(&Level::INFO), EVENTLOG_INFORMATION_TYPE);
        assert_eq!(eventlog_type(&Level::DEBUG), EVENTLOG_INFORMATION_TYPE);
        assert_eq!(eventlog_type(&Level::TRACE), EVENTLOG_INFORMATION_TYPE);
    }

    #[test]
    fn event_source_matches_service_name() {
        assert_eq!(EVENT_SOURCE, crate::service::AGENT_SERVICE.name);
    }
}
