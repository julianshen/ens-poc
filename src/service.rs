//! Windows Service definition and install parameters (platform-independent).
//!
//! Per `ENS-POC-Spec_1.md` §7 "Windows Service details":
//! - Service name `YourCoNotificationAgent`
//! - Start type **Automatic** (so it returns after a reboot — §10 criterion)
//! - **Restart on failure: Yes, after 10 seconds** (SCM relaunches the process
//!   if it dies unexpectedly)
//!
//! The values and the derived `sc.exe` recovery arguments are pure data and are
//! unit-tested here. Actually registering the service and running under the
//! Service Control Manager is OS glue (`service_runtime`, the installer script).

use std::time::Duration;

/// Service start type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartType {
    Automatic,
    Manual,
    Disabled,
}

impl StartType {
    /// The value `sc.exe create ... start= <value>` expects.
    pub fn sc_value(self) -> &'static str {
        match self {
            StartType::Automatic => "auto",
            StartType::Manual => "demand",
            StartType::Disabled => "disabled",
        }
    }

    /// The value PowerShell `New-Service -StartupType <value>` expects.
    pub fn powershell_value(self) -> &'static str {
        match self {
            StartType::Automatic => "Automatic",
            StartType::Manual => "Manual",
            StartType::Disabled => "Disabled",
        }
    }
}

/// Everything needed to install and recover the agent service.
#[derive(Debug, Clone)]
pub struct ServiceSpec {
    pub name: &'static str,
    pub display_name: &'static str,
    pub start_type: StartType,
    /// How long the SCM waits before relaunching a crashed process.
    pub restart_delay: Duration,
    /// Window of stability after which the failure counter resets.
    pub reset_period: Duration,
}

/// The agent's service definition (spec §7).
pub const AGENT_SERVICE: ServiceSpec = ServiceSpec {
    name: "YourCoNotificationAgent",
    display_name: "Acme Notification Agent",
    start_type: StartType::Automatic,
    restart_delay: Duration::from_secs(10),
    reset_period: Duration::from_secs(86_400),
};

impl ServiceSpec {
    /// Arguments to `sc.exe` that configure restart-on-failure recovery:
    /// `sc failure <name> reset= <seconds> actions= restart/<milliseconds>`.
    ///
    /// Returned as discrete argv tokens (no shell quoting) so they can be passed
    /// straight to a process spawn or `& sc.exe @args` in PowerShell.
    pub fn sc_failure_args(&self) -> Vec<String> {
        vec![
            "failure".into(),
            self.name.into(),
            "reset=".into(),
            self.reset_period.as_secs().to_string(),
            "actions=".into(),
            format!("restart/{}", self.restart_delay.as_millis()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_service_matches_spec() {
        assert_eq!(AGENT_SERVICE.name, "YourCoNotificationAgent");
        assert_eq!(AGENT_SERVICE.start_type, StartType::Automatic);
        assert_eq!(AGENT_SERVICE.restart_delay, Duration::from_secs(10));
    }

    #[test]
    fn restart_failure_args_use_ten_second_delay() {
        // sc.exe expects the restart delay in milliseconds: 10s => 10000.
        assert_eq!(
            AGENT_SERVICE.sc_failure_args(),
            vec![
                "failure".to_string(),
                "YourCoNotificationAgent".to_string(),
                "reset=".to_string(),
                "86400".to_string(),
                "actions=".to_string(),
                "restart/10000".to_string(),
            ]
        );
    }

    #[test]
    fn start_type_maps_to_sc_values() {
        assert_eq!(StartType::Automatic.sc_value(), "auto");
        assert_eq!(StartType::Manual.sc_value(), "demand");
        assert_eq!(StartType::Disabled.sc_value(), "disabled");
    }

    #[test]
    fn start_type_maps_to_powershell_values() {
        assert_eq!(StartType::Automatic.powershell_value(), "Automatic");
        assert_eq!(StartType::Manual.powershell_value(), "Manual");
        assert_eq!(StartType::Disabled.powershell_value(), "Disabled");
    }
}
