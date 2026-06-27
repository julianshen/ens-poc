//! Agent bootstrap shared by the console and service entry points (OS glue;
//! excluded from coverage).
//!
//! Wires the tested pieces together — config, registry device ID, Windows sink,
//! NATS loop — and runs until either the loop returns or `shutdown` fires.

#![cfg(windows)]

use std::future::Future;
use std::path::Path;

use anyhow::anyhow;

use crate::config::Config;
use crate::nats;
use crate::platform::{RegistryDeviceIdSource, WindowsSink};
use crate::subject::DeviceIdSource;

/// Default config location when none is given (spec §7).
pub const DEFAULT_CONFIG_PATH: &str = r"C:\Program Files\YourCo\agent.toml";

/// Load configuration, resolve the device ID, and run the agent until the NATS
/// loop ends (only on bounded-initial-connect give-up) or `shutdown` resolves.
pub async fn run_agent(
    config_path: &Path,
    shutdown: impl Future<Output = ()>,
) -> anyhow::Result<()> {
    let config = Config::from_path(config_path).map_err(|e| anyhow!("{e}"))?;

    let device_id = RegistryDeviceIdSource
        .device_id()
        .map_err(|e| anyhow!("{e}"))?;
    tracing::info!(device_id = device_id.as_str(), "resolved device id");

    let mut sink = WindowsSink::new(&config.aumid);

    tokio::select! {
        result = nats::run(&config, &device_id, &mut sink) => result,
        _ = shutdown => {
            tracing::info!("shutdown requested; stopping agent");
            Ok(())
        }
    }
}
