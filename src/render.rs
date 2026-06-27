//! WNS XML rendering (platform-independent).
//!
//! The Windows notification API consumes WNS XML. After the inbound payload is
//! parsed and validated into a [`Notification`](crate::notification), the
//! Windows sink rebuilds canonical XML to hand to `Windows.UI.Notifications`.
//! Building (and escaping) that XML is pure logic and is unit-tested here; only
//! the COM call that consumes it is OS-specific.

use crate::notification::Badge;

/// Build the WNS toast XML for the given ordered text lines.
pub fn toast_xml(texts: &[String]) -> String {
    let mut out = String::from(r#"<toast><visual><binding template="ToastGeneric">"#);
    for text in texts {
        out.push_str("<text>");
        out.push_str(&escape_text(text));
        out.push_str("</text>");
    }
    out.push_str("</binding></visual></toast>");
    out
}

/// Build the WNS badge XML for the given badge value.
pub fn badge_xml(badge: &Badge) -> String {
    match badge {
        Badge::Numeric(n) => format!(r#"<badge value="{n}"/>"#),
        Badge::Glyph(g) => format!(r#"<badge value="{}"/>"#, g.as_value()),
    }
}

/// Escape the five XML predefined entities in element text content.
fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notification::{Glyph, Notification, parse};

    #[test]
    fn renders_single_text_toast() {
        assert_eq!(
            toast_xml(&["Hello".to_string()]),
            r#"<toast><visual><binding template="ToastGeneric"><text>Hello</text></binding></visual></toast>"#
        );
    }

    #[test]
    fn renders_multi_text_toast() {
        let xml = toast_xml(&["Title".to_string(), "Body".to_string()]);
        assert!(xml.contains("<text>Title</text><text>Body</text>"));
    }

    #[test]
    fn escapes_special_characters_in_toast_text() {
        let xml = toast_xml(&[r#"A & B < C > D " E ' F"#.to_string()]);
        assert!(xml.contains("A &amp; B &lt; C &gt; D &quot; E &apos; F"));
    }

    #[test]
    fn renders_numeric_badge() {
        assert_eq!(badge_xml(&Badge::Numeric(5)), r#"<badge value="5"/>"#);
        assert_eq!(badge_xml(&Badge::Numeric(0)), r#"<badge value="0"/>"#);
    }

    #[test]
    fn renders_glyph_badge() {
        assert_eq!(
            badge_xml(&Badge::Glyph(Glyph::NewMessage)),
            r#"<badge value="newMessage"/>"#
        );
    }

    #[test]
    fn rendered_toast_round_trips_through_parser() {
        let texts = vec!["Café & <tag>".to_string(), "Line 2".to_string()];
        let xml = toast_xml(&texts);
        assert_eq!(parse(&xml).unwrap(), Notification::Toast { texts });
    }

    #[test]
    fn rendered_badge_round_trips_through_parser() {
        for badge in [Badge::Numeric(42), Badge::Glyph(Glyph::Alert)] {
            let xml = badge_xml(&badge);
            assert_eq!(parse(&xml).unwrap(), Notification::Badge(badge));
        }
    }
}
