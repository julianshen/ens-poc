//! Agent configuration (platform-independent).
//!
//! Loaded from `agent.toml` (spec §7). Note that `device-id` is deliberately
//! **not** part of the config — it is always read live from the registry at
//! runtime, never stored here.

use std::path::Path;

use serde::Deserialize;

/// Static configuration read from `agent.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Config {
    /// NATS server URL, e.g. `nats://nats.internal.yourco.com:4222`.
    pub nats_url: String,
    /// NATS username.
    pub nats_user: String,
    /// NATS password.
    pub nats_pass: String,
    /// Application User Model ID used for notification registration.
    pub aumid: String,
    /// Optional Sentry DSN. When set, errors/events are reported to Sentry.
    #[serde(default)]
    pub sentry_dsn: Option<String>,
    /// Optional OTLP/HTTP endpoint (e.g. `http://collector:4318`). When set,
    /// traces are exported to an OpenTelemetry collector.
    #[serde(default)]
    pub otel_endpoint: Option<String>,
}

/// Why a config could not be loaded.
#[derive(Debug)]
pub enum ConfigError {
    /// The file could not be read.
    Io(std::io::Error),
    /// The file contents were not valid TOML / were missing fields.
    Parse(toml::de::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "reading config: {e}"),
            ConfigError::Parse(e) => write!(f, "parsing config: {e}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    /// Parse a config from a TOML string.
    pub fn from_toml(s: &str) -> Result<Self, ConfigError> {
        toml::from_str(s).map_err(ConfigError::Parse)
    }

    /// Read and parse a config from a file on disk.
    pub fn from_path(path: &Path) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path).map_err(ConfigError::Io)?;
        Self::from_toml(&contents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"
        nats_url  = "nats://nats.internal.yourco.com:4222"
        nats_user = "agent"
        nats_pass = "changeme"
        aumid     = "YourCo.NotificationAgent"
    "#;

    #[test]
    fn parses_all_fields_from_toml() {
        let cfg = Config::from_toml(VALID).unwrap();
        assert_eq!(
            cfg,
            Config {
                nats_url: "nats://nats.internal.yourco.com:4222".into(),
                nats_user: "agent".into(),
                nats_pass: "changeme".into(),
                aumid: "YourCo.NotificationAgent".into(),
                // Optional observability fields default to None when absent.
                sentry_dsn: None,
                otel_endpoint: None,
            }
        );
    }

    #[test]
    fn parses_optional_observability_fields() {
        let toml = format!(
            "{VALID}\n            sentry_dsn = \"https://k@sentry.example/1\"\n            otel_endpoint = \"http://collector:4318\"\n"
        );
        let cfg = Config::from_toml(&toml).unwrap();
        assert_eq!(
            cfg.sentry_dsn.as_deref(),
            Some("https://k@sentry.example/1")
        );
        assert_eq!(cfg.otel_endpoint.as_deref(), Some("http://collector:4318"));
    }

    #[test]
    fn missing_required_field_is_an_error() {
        let toml = r#"
            nats_url  = "nats://x:4222"
            nats_user = "agent"
            nats_pass = "changeme"
        "#; // aumid omitted
        assert!(matches!(
            Config::from_toml(toml),
            Err(ConfigError::Parse(_))
        ));
    }

    #[test]
    fn malformed_toml_is_an_error() {
        assert!(matches!(
            Config::from_toml("nats_url = = ="),
            Err(ConfigError::Parse(_))
        ));
    }

    #[test]
    fn unknown_extra_fields_are_ignored() {
        let toml = format!("{VALID}\n            extra = \"ignored\"\n");
        assert!(Config::from_toml(&toml).is_ok());
    }

    #[test]
    fn display_renders_parse_error() {
        let err = Config::from_toml("not valid").unwrap_err();
        assert!(format!("{err}").starts_with("parsing config:"));
    }

    #[test]
    fn from_path_reads_a_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("tns_test_config_ok.toml");
        std::fs::write(&path, VALID).unwrap();
        let cfg = Config::from_path(&path).unwrap();
        assert_eq!(cfg.nats_user, "agent");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn from_path_missing_file_is_io_error() {
        let path = std::env::temp_dir().join("tns_test_config_does_not_exist.toml");
        let _ = std::fs::remove_file(&path);
        let err = Config::from_path(&path).unwrap_err();
        assert!(matches!(err, ConfigError::Io(_)));
        assert!(format!("{err}").starts_with("reading config:"));
    }
}
