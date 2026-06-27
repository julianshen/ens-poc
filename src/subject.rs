//! Device identity and NATS subject construction (platform-independent).
//!
//! The agent subscribes to `notifications.device.<device-id>` where
//! `<device-id>` is the Windows Machine GUID read live from the registry
//! (spec §5). Reading the registry is OS-specific and lives behind
//! [`DeviceIdSource`]; validating the value and building the subject is pure
//! logic and lives here.

/// The NATS subject prefix all device notifications are published under.
pub const SUBJECT_PREFIX: &str = "notifications.device.";

/// A validated Windows Machine GUID, safe to embed in a NATS subject.
///
/// Stored normalised to lowercase. Guaranteed to be a canonical
/// `8-4-4-4-12` hex GUID with no braces — which also guarantees it contains
/// none of NATS's token separators or wildcards (`.`, ` `, `*`, `>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceId(String);

/// The reason a string was rejected as a device ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidDeviceId(pub String);

impl std::fmt::Display for InvalidDeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid device id: {:?}", self.0)
    }
}

impl std::error::Error for InvalidDeviceId {}

impl DeviceId {
    /// Validate and normalise a Machine GUID string (registry form, no braces).
    pub fn parse(raw: &str) -> Result<Self, InvalidDeviceId> {
        if is_canonical_guid(raw) {
            Ok(DeviceId(raw.to_ascii_lowercase()))
        } else {
            Err(InvalidDeviceId(raw.to_string()))
        }
    }

    /// The normalised GUID string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The full NATS subject this device subscribes to.
    pub fn subject(&self) -> String {
        format!("{SUBJECT_PREFIX}{}", self.0)
    }
}

/// True if `s` is a canonical `8-4-4-4-12` hex GUID (no braces).
fn is_canonical_guid(s: &str) -> bool {
    const GROUPS: [usize; 5] = [8, 4, 4, 4, 12];
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != GROUPS.len() {
        return false;
    }
    parts
        .iter()
        .zip(GROUPS)
        .all(|(part, len)| part.len() == len && part.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// A source of the local device's Machine GUID. Implemented over the Windows
/// registry in production (`#[cfg(windows)]`); mocked in tests.
pub trait DeviceIdSource {
    fn device_id(&self) -> Result<DeviceId, InvalidDeviceId>;
}

#[cfg(test)]
mod tests {
    use super::*;

    const GUID: &str = "4f3a1bc2-9d0e-4a3f-b812-1234abcd5678";

    #[test]
    fn parses_canonical_guid() {
        assert_eq!(DeviceId::parse(GUID).unwrap().as_str(), GUID);
    }

    #[test]
    fn normalises_uppercase_to_lowercase() {
        let id = DeviceId::parse("4F3A1BC2-9D0E-4A3F-B812-1234ABCD5678").unwrap();
        assert_eq!(id.as_str(), GUID);
    }

    #[test]
    fn builds_subject() {
        let id = DeviceId::parse(GUID).unwrap();
        assert_eq!(
            id.subject(),
            "notifications.device.4f3a1bc2-9d0e-4a3f-b812-1234abcd5678"
        );
    }

    #[test]
    fn rejects_empty() {
        assert!(DeviceId::parse("").is_err());
    }

    #[test]
    fn rejects_wrong_group_lengths() {
        assert!(DeviceId::parse("4f3a1bc-9d0e-4a3f-b812-1234abcd5678").is_err());
        assert!(DeviceId::parse("4f3a1bc2-9d0e-4a3f-b812-1234abcd567").is_err());
    }

    #[test]
    fn rejects_wrong_group_count() {
        assert!(DeviceId::parse("4f3a1bc2-9d0e-4a3f-1234abcd5678").is_err());
    }

    #[test]
    fn rejects_non_hex_characters() {
        assert!(DeviceId::parse("4f3a1bcz-9d0e-4a3f-b812-1234abcd5678").is_err());
    }

    #[test]
    fn rejects_braces_form() {
        let braced = "{4f3a1bc2-9d0e-4a3f-b812-1234abcd5678}";
        assert!(DeviceId::parse(braced).is_err());
    }

    #[test]
    fn rejects_subject_breaking_characters() {
        // A value containing NATS separators/wildcards must never validate.
        for bad in ["a.b", "a b", "a*b", "a>b"] {
            assert!(DeviceId::parse(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn invalid_device_id_displays() {
        let err = DeviceId::parse("nope").unwrap_err();
        assert!(format!("{err}").contains("invalid device id"));
    }
}
