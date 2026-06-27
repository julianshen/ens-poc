//! AUMID (Application User Model ID) registration data (platform-independent).
//!
//! Install-time, the agent registers its AUMID so Windows attributes toasts
//! and badges to it (spec §7). The registry key path and named values are pure
//! strings built here and unit-tested; the actual privileged registry write is
//! OS-specific and lives behind [`AumidRegistrar`].
//!
//! ```text
//! HKLM\SOFTWARE\Classes\AppUserModelId\<aumid>
//!   DisplayName = "<display_name>"
//!   IconUri     = "<icon_uri>"
//! ```

/// The registry subkey (under `HKLM`) where AUMIDs are registered.
pub const APP_USER_MODEL_ID_ROOT: &str = r"SOFTWARE\Classes\AppUserModelId";

/// The data needed to register one AUMID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AumidRegistration {
    pub aumid: String,
    pub display_name: String,
    pub icon_uri: String,
}

impl AumidRegistration {
    /// The full registry subkey path (relative to `HKLM`) for this AUMID.
    pub fn subkey_path(&self) -> String {
        format!(r"{APP_USER_MODEL_ID_ROOT}\{}", self.aumid)
    }

    /// The named string values to write under [`Self::subkey_path`], in order.
    pub fn values(&self) -> [(&'static str, &str); 2] {
        [
            ("DisplayName", self.display_name.as_str()),
            ("IconUri", self.icon_uri.as_str()),
        ]
    }
}

/// Performs the privileged registry write to register an AUMID. Implemented
/// over the Windows registry in production (`#[cfg(windows)]`); mocked in tests.
pub trait AumidRegistrar {
    fn register(&self, reg: &AumidRegistration) -> Result<(), String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> AumidRegistration {
        AumidRegistration {
            aumid: "YourCo.NotificationAgent".into(),
            display_name: "Acme Notification Agent".into(),
            icon_uri: r"C:\Program Files\YourCo\agent.ico".into(),
        }
    }

    #[test]
    fn builds_subkey_path_under_app_user_model_id() {
        assert_eq!(
            sample().subkey_path(),
            r"SOFTWARE\Classes\AppUserModelId\YourCo.NotificationAgent"
        );
    }

    #[test]
    fn exposes_display_name_and_icon_values() {
        let reg = sample();
        assert_eq!(
            reg.values(),
            [
                ("DisplayName", "Acme Notification Agent"),
                ("IconUri", r"C:\Program Files\YourCo\agent.ico"),
            ]
        );
    }
}
