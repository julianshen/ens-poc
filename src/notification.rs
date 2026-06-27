//! WNS XML parsing and notification-type detection.
//!
//! The agent inspects the root element of the incoming XML payload:
//! `<toast>` → toast, `<badge>` → badge, anything else → discard.
//! See `ENS-POC-Spec_1.md` §6 for the message format.

/// A badge glyph, per the spec's supported set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    None,
    Alert,
    Activity,
    Alarm,
    Available,
    Away,
    Busy,
    NewMessage,
    Paused,
    Playing,
    Unavailable,
    Error,
    Attention,
}

impl Glyph {
    /// Parse a glyph from its WNS string value, e.g. `"newMessage"`.
    pub fn from_value(s: &str) -> Option<Self> {
        Some(match s {
            "none" => Glyph::None,
            "alert" => Glyph::Alert,
            "activity" => Glyph::Activity,
            "alarm" => Glyph::Alarm,
            "available" => Glyph::Available,
            "away" => Glyph::Away,
            "busy" => Glyph::Busy,
            "newMessage" => Glyph::NewMessage,
            "paused" => Glyph::Paused,
            "playing" => Glyph::Playing,
            "unavailable" => Glyph::Unavailable,
            "error" => Glyph::Error,
            "attention" => Glyph::Attention,
            _ => return None,
        })
    }

    /// The canonical WNS string value for this glyph.
    pub fn as_value(self) -> &'static str {
        match self {
            Glyph::None => "none",
            Glyph::Alert => "alert",
            Glyph::Activity => "activity",
            Glyph::Alarm => "alarm",
            Glyph::Available => "available",
            Glyph::Away => "away",
            Glyph::Busy => "busy",
            Glyph::NewMessage => "newMessage",
            Glyph::Paused => "paused",
            Glyph::Playing => "playing",
            Glyph::Unavailable => "unavailable",
            Glyph::Error => "error",
            Glyph::Attention => "attention",
        }
    }
}

/// A badge update: either a numeric count or a glyph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Badge {
    /// Numeric value 0–99. `0` clears the badge.
    Numeric(u8),
    /// A glyph badge.
    Glyph(Glyph),
}

/// A parsed, dispatchable notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notification {
    /// Toast with its ordered `<text>` lines (first is title, rest body).
    Toast { texts: Vec<String> },
    /// Badge update.
    Badge(Badge),
}

/// Why a payload could not be turned into a [`Notification`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The XML was syntactically malformed.
    Malformed(String),
    /// The payload had no element at all.
    Empty,
    /// The root element was neither `<toast>` nor `<badge>`.
    UnknownRoot(String),
    /// A `<badge>` element had no `value` attribute.
    MissingBadgeValue,
    /// A `<badge value="...">` was neither a 0–99 number nor a known glyph.
    InvalidBadgeValue(String),
}

/// Parse a WNS XML payload into a [`Notification`], detecting the type by the
/// root element name.
pub fn parse(xml: &str) -> Result<Notification, ParseError> {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    loop {
        match reader.read_event() {
            // A self-closing root (`<toast/>` / `<badge .../>`) carries no body.
            Ok(Event::Empty(e)) => {
                return match e.local_name().as_ref() {
                    b"toast" => Ok(Notification::Toast { texts: Vec::new() }),
                    b"badge" => parse_badge(&e),
                    other => Err(ParseError::UnknownRoot(
                        String::from_utf8_lossy(other).into_owned(),
                    )),
                };
            }
            Ok(Event::Start(e)) => {
                return match e.local_name().as_ref() {
                    b"toast" => parse_toast(&mut reader),
                    // `<badge>...</badge>` — the value still lives on the open tag.
                    b"badge" => parse_badge(&e),
                    other => Err(ParseError::UnknownRoot(
                        String::from_utf8_lossy(other).into_owned(),
                    )),
                };
            }
            Ok(Event::Eof) => return Err(ParseError::Empty),
            // Skip declarations, comments, processing instructions, stray text.
            Ok(_) => continue,
            Err(e) => return Err(ParseError::Malformed(e.to_string())),
        }
    }
}

/// Read a `<toast>` body, collecting the ordered text of each `<text>` element.
fn parse_toast(reader: &mut quick_xml::Reader<&[u8]>) -> Result<Notification, ParseError> {
    use quick_xml::events::Event;

    let mut texts = Vec::new();
    let mut in_text = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.local_name().as_ref() == b"text" => in_text = true,
            Ok(Event::End(e)) if e.local_name().as_ref() == b"text" => in_text = false,
            Ok(Event::Text(t)) if in_text => {
                let s = t
                    .unescape()
                    .map_err(|e| ParseError::Malformed(e.to_string()))?;
                texts.push(s.into_owned());
            }
            Ok(Event::End(e)) if e.local_name().as_ref() == b"toast" => {
                return Ok(Notification::Toast { texts });
            }
            // Reaching EOF before `</toast>` means the payload was truncated.
            Ok(Event::Eof) => {
                return Err(ParseError::Malformed("unexpected end of input".into()));
            }
            Ok(_) => {}
            Err(e) => return Err(ParseError::Malformed(e.to_string())),
        }
    }
}

/// Interpret a `<badge>` element's `value` attribute as a number or glyph.
fn parse_badge(e: &quick_xml::events::BytesStart) -> Result<Notification, ParseError> {
    let value = badge_value(e)?;
    if let Ok(n) = value.parse::<u16>() {
        if n <= 99 {
            return Ok(Notification::Badge(Badge::Numeric(n as u8)));
        }
        return Err(ParseError::InvalidBadgeValue(value));
    }
    match Glyph::from_value(&value) {
        Some(g) => Ok(Notification::Badge(Badge::Glyph(g))),
        None => Err(ParseError::InvalidBadgeValue(value)),
    }
}

/// Extract the `value` attribute from a `<badge>` element.
fn badge_value(e: &quick_xml::events::BytesStart) -> Result<String, ParseError> {
    for attr in e.attributes() {
        let attr = attr.map_err(|e| ParseError::Malformed(e.to_string()))?;
        if attr.key.local_name().as_ref() == b"value" {
            let v = attr
                .unescape_value()
                .map_err(|e| ParseError::Malformed(e.to_string()))?;
            return Ok(v.into_owned());
        }
    }
    Err(ParseError::MissingBadgeValue)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_toast_with_title_and_body() {
        let xml = r#"<toast><visual><binding template="ToastGeneric">
            <text>Hello</text><text>This is a test notification</text>
        </binding></visual></toast>"#;
        let got = parse(xml).unwrap();
        assert_eq!(
            got,
            Notification::Toast {
                texts: vec!["Hello".into(), "This is a test notification".into()],
            }
        );
    }

    #[test]
    fn parses_toast_with_single_text() {
        let xml = r#"<toast><visual><binding template="ToastGeneric">
            <text>Only one</text></binding></visual></toast>"#;
        let got = parse(xml).unwrap();
        assert_eq!(
            got,
            Notification::Toast {
                texts: vec!["Only one".into()]
            }
        );
    }

    #[test]
    fn unescapes_toast_text_entities() {
        let xml = r#"<toast><visual><binding template="ToastGeneric">
            <text>A &amp; B &lt; C</text></binding></visual></toast>"#;
        let got = parse(xml).unwrap();
        assert_eq!(
            got,
            Notification::Toast {
                texts: vec!["A & B < C".into()]
            }
        );
    }

    #[test]
    fn parses_numeric_badge() {
        assert_eq!(
            parse(r#"<badge value="5"/>"#).unwrap(),
            Notification::Badge(Badge::Numeric(5))
        );
    }

    #[test]
    fn parses_badge_zero_as_clear() {
        assert_eq!(
            parse(r#"<badge value="0"/>"#).unwrap(),
            Notification::Badge(Badge::Numeric(0))
        );
    }

    #[test]
    fn parses_max_numeric_badge() {
        assert_eq!(
            parse(r#"<badge value="99"/>"#).unwrap(),
            Notification::Badge(Badge::Numeric(99))
        );
    }

    #[test]
    fn parses_glyph_badge() {
        assert_eq!(
            parse(r#"<badge value="alert"/>"#).unwrap(),
            Notification::Badge(Badge::Glyph(Glyph::Alert))
        );
    }

    #[test]
    fn parses_non_self_closing_badge() {
        assert_eq!(
            parse(r#"<badge value="newMessage"></badge>"#).unwrap(),
            Notification::Badge(Badge::Glyph(Glyph::NewMessage))
        );
    }

    #[test]
    fn rejects_numeric_badge_over_99() {
        assert_eq!(
            parse(r#"<badge value="100"/>"#),
            Err(ParseError::InvalidBadgeValue("100".into()))
        );
    }

    #[test]
    fn rejects_unknown_glyph() {
        assert_eq!(
            parse(r#"<badge value="sparkle"/>"#),
            Err(ParseError::InvalidBadgeValue("sparkle".into()))
        );
    }

    #[test]
    fn rejects_badge_without_value() {
        assert_eq!(parse(r#"<badge/>"#), Err(ParseError::MissingBadgeValue));
    }

    #[test]
    fn rejects_unknown_root_element() {
        assert_eq!(
            parse(r#"<tile><visual/></tile>"#),
            Err(ParseError::UnknownRoot("tile".into()))
        );
    }

    #[test]
    fn rejects_malformed_xml() {
        assert!(matches!(
            parse("<toast><visual>"),
            Err(ParseError::Malformed(_))
        ));
    }

    #[test]
    fn rejects_self_closing_unknown_root() {
        assert_eq!(
            parse(r#"<tile/>"#),
            Err(ParseError::UnknownRoot("tile".into()))
        );
    }

    #[test]
    fn rejects_malformed_before_any_element() {
        // An unterminated comment errors before a root element is ever found.
        assert!(matches!(
            parse("<!-- never closed"),
            Err(ParseError::Malformed(_))
        ));
    }

    #[test]
    fn rejects_mismatched_end_tag_in_toast_body() {
        assert!(matches!(
            parse(r#"<toast><visual></wrong></visual></toast>"#),
            Err(ParseError::Malformed(_))
        ));
    }

    #[test]
    fn reads_value_when_other_attributes_precede_it() {
        assert_eq!(
            parse(r#"<badge id="x" value="7"/>"#).unwrap(),
            Notification::Badge(Badge::Numeric(7))
        );
    }

    #[test]
    fn empty_self_closing_toast_has_no_text() {
        assert_eq!(
            parse(r#"<toast/>"#).unwrap(),
            Notification::Toast { texts: Vec::new() }
        );
    }

    #[test]
    fn rejects_empty_payload() {
        assert_eq!(parse("   "), Err(ParseError::Empty));
    }

    #[test]
    fn skips_xml_declaration_and_finds_root() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?><badge value="3"/>"#;
        assert_eq!(parse(xml).unwrap(), Notification::Badge(Badge::Numeric(3)));
    }

    #[test]
    fn glyph_value_round_trips() {
        for g in [
            Glyph::None,
            Glyph::Alert,
            Glyph::Activity,
            Glyph::Alarm,
            Glyph::Available,
            Glyph::Away,
            Glyph::Busy,
            Glyph::NewMessage,
            Glyph::Paused,
            Glyph::Playing,
            Glyph::Unavailable,
            Glyph::Error,
            Glyph::Attention,
        ] {
            assert_eq!(Glyph::from_value(g.as_value()), Some(g));
        }
    }

    #[test]
    fn unknown_glyph_value_is_none() {
        assert_eq!(Glyph::from_value("nope"), None);
    }
}
