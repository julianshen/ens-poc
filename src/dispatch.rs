//! Notification dispatch — the seam between the platform-independent core and
//! the OS notification API.
//!
//! [`dispatch`] parses an incoming WNS XML payload and routes it to a
//! [`NotificationSink`]. The real sink (Windows) lives behind `#[cfg(windows)]`
//! elsewhere; tests drive a mock sink so the full parse-and-route flow is
//! exercised without the OS.
//!
//! Bad payloads return an error so the caller can log and discard them while
//! the agent keeps running (spec §10, "Bad XML").

use crate::notification::{Badge, Notification, ParseError, parse};

/// A target that can render notifications. Implemented by the Windows backend
/// in production and by a mock in tests.
pub trait NotificationSink {
    /// Show a toast with the given ordered text lines.
    fn show_toast(&mut self, texts: &[String]) -> Result<(), SinkError>;
    /// Update (or clear) the badge.
    fn update_badge(&mut self, badge: &Badge) -> Result<(), SinkError>;
}

/// An error from the underlying notification backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SinkError(pub String);

impl std::fmt::Display for SinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for SinkError {}

/// What a successfully dispatched payload turned out to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dispatched {
    Toast,
    Badge,
}

/// Why a payload was not delivered.
#[derive(Debug)]
pub enum DispatchError {
    /// The payload could not be parsed — discard it (the agent keeps running).
    Parse(ParseError),
    /// The payload was valid but the backend failed to render it.
    Sink(SinkError),
}

impl std::fmt::Display for DispatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DispatchError::Parse(e) => write!(f, "discarding invalid payload: {e:?}"),
            DispatchError::Sink(e) => write!(f, "backend failed to render notification: {e}"),
        }
    }
}

impl std::error::Error for DispatchError {}

/// Parse a WNS XML payload and route it to `sink`.
pub fn dispatch(xml: &str, sink: &mut impl NotificationSink) -> Result<Dispatched, DispatchError> {
    match parse(xml).map_err(DispatchError::Parse)? {
        Notification::Toast { texts } => {
            sink.show_toast(&texts).map_err(DispatchError::Sink)?;
            Ok(Dispatched::Toast)
        }
        Notification::Badge(badge) => {
            sink.update_badge(&badge).map_err(DispatchError::Sink)?;
            Ok(Dispatched::Badge)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notification::Glyph;

    /// A test double that records what it was asked to render, and can be told
    /// to simulate a backend failure.
    #[derive(Default)]
    struct RecordingSink {
        toasts: Vec<Vec<String>>,
        badges: Vec<Badge>,
        fail: bool,
    }

    impl NotificationSink for RecordingSink {
        fn show_toast(&mut self, texts: &[String]) -> Result<(), SinkError> {
            if self.fail {
                return Err(SinkError("toast backend down".into()));
            }
            self.toasts.push(texts.to_vec());
            Ok(())
        }

        fn update_badge(&mut self, badge: &Badge) -> Result<(), SinkError> {
            if self.fail {
                return Err(SinkError("badge backend down".into()));
            }
            self.badges.push(badge.clone());
            Ok(())
        }
    }

    #[test]
    fn routes_toast_to_sink() {
        let mut sink = RecordingSink::default();
        let xml = r#"<toast><visual><binding template="ToastGeneric">
            <text>Hi</text><text>Body</text></binding></visual></toast>"#;
        let out = dispatch(xml, &mut sink).unwrap();
        assert_eq!(out, Dispatched::Toast);
        assert_eq!(
            sink.toasts,
            vec![vec!["Hi".to_string(), "Body".to_string()]]
        );
        assert!(sink.badges.is_empty());
    }

    #[test]
    fn routes_numeric_badge_to_sink() {
        let mut sink = RecordingSink::default();
        let out = dispatch(r#"<badge value="5"/>"#, &mut sink).unwrap();
        assert_eq!(out, Dispatched::Badge);
        assert_eq!(sink.badges, vec![Badge::Numeric(5)]);
        assert!(sink.toasts.is_empty());
    }

    #[test]
    fn routes_glyph_badge_to_sink() {
        let mut sink = RecordingSink::default();
        dispatch(r#"<badge value="alert"/>"#, &mut sink).unwrap();
        assert_eq!(sink.badges, vec![Badge::Glyph(Glyph::Alert)]);
    }

    #[test]
    fn unknown_root_is_discarded_without_touching_sink() {
        let mut sink = RecordingSink::default();
        let err = dispatch(r#"<tile/>"#, &mut sink).unwrap_err();
        assert!(matches!(
            err,
            DispatchError::Parse(ParseError::UnknownRoot(_))
        ));
        assert!(sink.toasts.is_empty() && sink.badges.is_empty());
    }

    #[test]
    fn malformed_xml_is_discarded() {
        let mut sink = RecordingSink::default();
        let err = dispatch("<toast><visual>", &mut sink).unwrap_err();
        assert!(matches!(
            err,
            DispatchError::Parse(ParseError::Malformed(_))
        ));
    }

    #[test]
    fn sink_failure_is_reported() {
        let mut sink = RecordingSink {
            fail: true,
            ..Default::default()
        };
        let err = dispatch(r#"<badge value="3"/>"#, &mut sink).unwrap_err();
        assert!(matches!(err, DispatchError::Sink(_)));
    }

    #[test]
    fn agent_keeps_running_across_mixed_messages() {
        // A bad message between two good ones must not stop later delivery.
        let mut sink = RecordingSink::default();
        let _ = dispatch(r#"<badge value="1"/>"#, &mut sink);
        let _ = dispatch("garbage", &mut sink); // discarded
        let _ = dispatch(r#"<badge value="2"/>"#, &mut sink);
        assert_eq!(sink.badges, vec![Badge::Numeric(1), Badge::Numeric(2)]);
    }

    #[test]
    fn errors_render_for_logging() {
        let mut sink = RecordingSink::default();
        let parse_err = dispatch("<tile/>", &mut sink).unwrap_err();
        assert!(format!("{parse_err}").contains("discarding invalid payload"));

        let mut failing = RecordingSink {
            fail: true,
            ..Default::default()
        };
        let sink_err = dispatch(r#"<badge value="1"/>"#, &mut failing).unwrap_err();
        assert!(format!("{sink_err}").contains("backend failed"));
    }
}
