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

use crate::backoff::{InitialConnectBackoff, ReconnectBackoff};
use crate::config::Config;
use crate::dispatch::{NotificationSink, dispatch};
use crate::subject::DeviceId;

/// Open a single NATS connection with reconnection disabled, so a dropped
/// connection surfaces as the end of the subscription stream and we drive the
/// reconnect backoff ourselves.
async fn connect_once(config: &Config) -> Result<async_nats::Client, async_nats::ConnectError> {
    async_nats::ConnectOptions::new()
        .user_and_password(config.nats_user.clone(), config.nats_pass.clone())
        .max_reconnects(Some(0))
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

/// Reconnect after a disconnect: exponential backoff, retrying forever (spec §7).
async fn reconnect(config: &Config) -> async_nats::Client {
    let mut backoff = ReconnectBackoff::new();
    loop {
        let delay = backoff.next_delay();
        tracing::warn!(?delay, "connection lost; reconnecting after delay");
        tokio::time::sleep(delay).await;
        match connect_once(config).await {
            Ok(client) => return client,
            Err(e) => tracing::warn!(error = %e, "reconnect attempt failed"),
        }
    }
}

/// Run the agent: connect, subscribe to the device subject, and dispatch every
/// message to `sink`, reconnecting automatically on disconnect. Returns only if
/// the bounded initial connect gives up.
pub async fn run(
    config: &Config,
    device_id: &DeviceId,
    sink: &mut impl NotificationSink,
) -> anyhow::Result<()> {
    let subject = device_id.subject();
    let mut client = connect_initial(config)
        .await
        .context("initial connection failed")?;

    loop {
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

        // Stream ended => the connection dropped. Reconnect and re-subscribe.
        client = reconnect(config).await;
        tracing::info!("reconnected");
    }
}
