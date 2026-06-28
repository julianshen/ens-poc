//! Windows Service Control Manager (SCM) runtime wrapper (OS glue; excluded
//! from coverage).
//!
//! Registers the agent with the SCM, reports Running/Stopped status, and stops
//! cleanly on a Stop control request. The service definition (name, start type,
//! restart-on-failure recovery) lives in the pure [`service`](crate::service)
//! module; this file only runs it.

#![cfg(windows)]

use std::ffi::OsString;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Notify;
use windows_service::service::{
    ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::{define_windows_service, service_dispatcher};

use crate::app;
use crate::service::AGENT_SERVICE;

const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

define_windows_service!(ffi_service_main, service_main);

/// Hand control to the SCM. Called when the process is started as a service.
pub fn run() -> windows_service::Result<()> {
    service_dispatcher::start(AGENT_SERVICE.name, ffi_service_main)
}

fn service_main(_arguments: Vec<OsString>) {
    crate::eventlog_win::init_logging();
    if let Err(err) = run_service() {
        tracing::error!(error = %err, "service exited with error");
    }
}

fn status(state: ServiceState, accepts: ServiceControlAccept, exit_code: u32) -> ServiceStatus {
    ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: state,
        controls_accepted: accepts,
        exit_code: ServiceExitCode::Win32(exit_code),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    }
}

fn run_service() -> anyhow::Result<()> {
    let shutdown = Arc::new(Notify::new());

    let handler_shutdown = shutdown.clone();
    let event_handler = move |control: ServiceControl| -> ServiceControlHandlerResult {
        match control {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                handler_shutdown.notify_one();
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(AGENT_SERVICE.name, event_handler)?;
    status_handle.set_service_status(status(
        ServiceState::Running,
        ServiceControlAccept::STOP,
        0,
    ))?;

    let runtime = app::runtime()?;
    let result = runtime.block_on(app::run_agent(
        Path::new(app::DEFAULT_CONFIG_PATH),
        async move {
            shutdown.notified().await;
        },
    ));

    // Report a non-zero exit code on failure so the SCM's restart-on-failure
    // recovery fires (install.ps1 sets the failure flag that enables recovery
    // for error stops, not just hard crashes).
    let exit_code = if result.is_ok() { 0 } else { 1 };
    status_handle.set_service_status(status(
        ServiceState::Stopped,
        ServiceControlAccept::empty(),
        exit_code,
    ))?;
    result
}
