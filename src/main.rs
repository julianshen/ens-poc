//! TNS notification agent entry point.
//!
//! Two modes (Windows):
//! - `tns --service` — run under the Service Control Manager (how the installed
//!   service launches; see `install.ps1`).
//! - `tns [config-path]` — run in the foreground as a console app for local
//!   end-to-end testing. Defaults to the installed config path when omitted.
//!
//! Reusable logic lives in the library crate (`tns`); this binary is thin.

#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    use std::path::PathBuf;

    match std::env::args().nth(1).as_deref() {
        Some("--service") => {
            // Started by the SCM: hand off to the service dispatcher.
            tns::service_runtime::run().map_err(|e| anyhow::anyhow!("service dispatcher: {e}"))
        }
        arg => {
            tns::eventlog_win::init_logging();

            let config_path = arg
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(tns::app::DEFAULT_CONFIG_PATH));

            let runtime = tokio::runtime::Runtime::new()?;
            runtime.block_on(tns::app::run_agent(&config_path, async {
                let _ = tokio::signal::ctrl_c().await;
                tracing::info!("ctrl-c received");
            }))
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("The TNS notification agent runs on Windows only.");
    std::process::exit(1);
}
