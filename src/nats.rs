//! NATS connection lifecycle and message loop (network glue; excluded from
//! coverage).
//!
//! This wires the tested pieces together: it uses
//! [`InitialConnectBackoff`](crate::backoff::InitialConnectBackoff) and
//! [`ReconnectBackoff`](crate::backoff::ReconnectBackoff) for the retry timing
//! (spec §7) and [`dispatch`](crate::dispatch::dispatch) for each message. It
//! owns no formatting or validation logic of its own.

use anyhow::{Context, anyhow};
use futures::StreamExt;

use crate::backoff::{InitialConnectBackoff, reconnect_delay};
use crate::config::Config;
use crate::dispatch::{NotificationSink, dispatch};
use crate::subject::DeviceId;

/// Open a single NATS connection. Once established, `async-nats` reconnects and
/// re-subscribes automatically and indefinitely on a dropped connection; we
/// supply the spec §7 exponential backoff via its `reconnect_delay_callback`.
/// `retry_on_initial_connect` is left off so the *first* connect fails fast and
/// `connect_initial` can drive the bounded 5s×12 retry itself.
async fn connect_once(config: &Config) -> Result<async_nats::Client, async_nats::ConnectError> {
    async_nats::ConnectOptions::new()
        .user_and_password(config.nats_user.clone(), config.nats_pass.clone())
        .max_reconnects(None) // reconnect forever (spec §7)
        .reconnect_delay_callback(|attempts| reconnect_delay(attempts as u32))
        .connect(config.nats_url.as_str())
        .await
}

/// Initial connect: retry every 5s up to 12 attempts, then give up (spec §7).
async fn connect_initial(config: &Config) -> anyhow::Result<async_nats::Client> {
    let mut backoff = InitialConnectBackoff::new();
    loop {
        match connect_once(config).await {
            Ok(client) => return Ok(client),
            Err(e) => match backoff.next_delay() {
                Some(delay) => {
                    tracing::warn!(error = %e, ?delay, "initial connect failed; retrying");
                    tokio::time::sleep(delay).await;
                }
                None => {
                    return Err(anyhow!(
                        "giving up after {} attempts: {e}",
                        InitialConnectBackoff::MAX_ATTEMPTS
                    ));
                }
            },
        }
    }
}

/// Run the agent: connect, subscribe to the device subject, and dispatch every
/// message to `sink`. `async-nats` transparently re-establishes the connection
/// and subscription on transient disconnects (spec §7/§10), so this loop only
/// ends if the client is closed unrecoverably — which is reported as an error
/// so the supervisor (SCM) can restart the process.
pub async fn run(
    config: &Config,
    device_id: &DeviceId,
    sink: &mut impl NotificationSink,
) -> anyhow::Result<()> {
    let subject = device_id.subject();
    let client = connect_initial(config)
        .await
        .context("initial connection failed")?;

    let mut sub = client
        .subscribe(subject.clone())
        .await
        .map_err(|e| anyhow!("subscribe to {subject} failed: {e}"))?;
    tracing::info!(%subject, "subscribed; awaiting notifications");

    while let Some(msg) = sub.next().await {
        let payload = String::from_utf8_lossy(msg.payload.as_ref());
        match dispatch(&payload, sink) {
            Ok(kind) => tracing::info!(?kind, bytes = msg.payload.len(), "delivered"),
            Err(err) => tracing::warn!(%err, "dropping message"),
        }
    }

    Err(anyhow!(
        "NATS subscription closed; connection unrecoverable"
    ))
}
