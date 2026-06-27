//! End-to-end NATS → dispatch integration test.
//!
//! Verifies the transport leg the unit tests skip: a message published to NATS
//! is received on the device subject and routed through `dispatch` to a sink.
//! Uses a mock sink (no Windows needed), so it is cross-platform.
//!
//! Requires a running NATS server with the agent credentials. Ignored by
//! default; run with:
//!
//!   cargo test --test nats_integration -- --ignored --nocapture
//!
//! Override the server with NATS_URL (default nats://127.0.0.1:4222).

use std::time::Duration;

use bytes::Bytes;
use futures::StreamExt;

use tns::dispatch::{Dispatched, NotificationSink, SinkError, dispatch};
use tns::notification::{Badge, Glyph};

#[derive(Default)]
struct RecordingSink {
    /// Raw toast XML, exactly as it arrived (passed through verbatim).
    toasts: Vec<String>,
    badges: Vec<Badge>,
}

impl NotificationSink for RecordingSink {
    fn show_toast(&mut self, xml: &str) -> Result<(), SinkError> {
        self.toasts.push(xml.to_string());
        Ok(())
    }
    fn update_badge(&mut self, badge: &Badge) -> Result<(), SinkError> {
        self.badges.push(badge.clone());
        Ok(())
    }
}

async fn connect() -> async_nats::Client {
    let url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".into());
    async_nats::ConnectOptions::new()
        .user_and_password("agent".into(), "changeme".into())
        .connect(url)
        .await
        .expect("connect to NATS (is the server running with agent/changeme?)")
}

/// Receive the next message on `sub` (with a timeout) and route it to `sink`.
async fn next_dispatched(sub: &mut async_nats::Subscriber, sink: &mut RecordingSink) -> Dispatched {
    let msg = tokio::time::timeout(Duration::from_secs(5), sub.next())
        .await
        .expect("timed out waiting for NATS message")
        .expect("subscription closed unexpectedly");
    dispatch(&String::from_utf8_lossy(&msg.payload), sink).expect("dispatch should succeed")
}

#[tokio::test]
#[ignore = "requires a running NATS server (agent/changeme @ 127.0.0.1:4222)"]
async fn badge_published_over_nats_reaches_dispatch() {
    let client = connect().await;
    let subject = "notifications.device.integration-test";
    let mut sub = client.subscribe(subject).await.expect("subscribe");

    client
        .publish(subject, Bytes::from_static(br#"<badge value="7"/>"#))
        .await
        .expect("publish");
    client.flush().await.expect("flush");

    let mut sink = RecordingSink::default();
    let kind = next_dispatched(&mut sub, &mut sink).await;

    assert_eq!(kind, Dispatched::Badge);
    assert_eq!(sink.badges, vec![Badge::Numeric(7)]);
    assert!(sink.toasts.is_empty());
}

#[tokio::test]
#[ignore = "requires a running NATS server (agent/changeme @ 127.0.0.1:4222)"]
async fn toast_published_over_nats_reaches_dispatch() {
    let client = connect().await;
    let subject = "notifications.device.integration-test-toast";
    let mut sub = client.subscribe(subject).await.expect("subscribe");

    let payload = r#"<toast><visual><binding template="ToastGeneric">
        <text>Alex Chen</text><text>Review the PR?</text></binding></visual></toast>"#;
    client
        .publish(subject, Bytes::copy_from_slice(payload.as_bytes()))
        .await
        .expect("publish");
    client.flush().await.expect("flush");

    let mut sink = RecordingSink::default();
    let kind = next_dispatched(&mut sub, &mut sink).await;

    assert_eq!(kind, Dispatched::Toast);
    assert_eq!(sink.toasts.len(), 1);
    assert!(sink.toasts[0].contains("<text>Alex Chen</text>"));
    assert!(sink.toasts[0].contains("<text>Review the PR?</text>"));
}

/// A rich toast with interactive controls (inline reply input + action
/// buttons) published over NATS must reach the sink with the controls intact —
/// the agent passes the toast XML through verbatim rather than re-rendering it.
#[tokio::test]
#[ignore = "requires a running NATS server (agent/changeme @ 127.0.0.1:4222)"]
async fn rich_toast_controls_survive_the_round_trip() {
    let client = connect().await;
    let subject = "notifications.device.integration-rich";
    let mut sub = client.subscribe(subject).await.expect("subscribe");

    let rich = r#"<toast>
  <visual><binding template="ToastGeneric">
    <text>Alex Chen</text><text>Review the deploy PR?</text>
  </binding></visual>
  <actions>
    <input id="reply" type="text" placeHolderContent="Type a reply…"/>
    <action content="Reply" arguments="action=reply" hint-inputId="reply"/>
    <action content="Like" arguments="action=like"/>
  </actions>
</toast>"#;
    client
        .publish(subject, Bytes::copy_from_slice(rich.as_bytes()))
        .await
        .expect("publish");
    client.flush().await.expect("flush");

    let mut sink = RecordingSink::default();
    let kind = next_dispatched(&mut sub, &mut sink).await;

    assert_eq!(kind, Dispatched::Toast);
    let got = &sink.toasts[0];
    // The whole template survived: text, the input, and both action buttons.
    assert!(got.contains("<input id=\"reply\""), "input survived: {got}");
    assert!(got.contains("content=\"Reply\""), "reply button survived");
    assert!(got.contains("content=\"Like\""), "like button survived");
    assert!(
        got.contains("hint-inputId=\"reply\""),
        "inline-reply wiring survived"
    );
}

/// A realistic mixed stream: valid toasts and badges interleaved with
/// malformed XML, an unknown root, and an out-of-range badge. Mirrors the
/// agent's loop (`Ok` → delivered, `Err` → dropped) and asserts the bad
/// payloads are discarded without disturbing the valid ones that follow
/// (spec §10 "Bad XML … agent keeps running"), preserving order.
#[tokio::test]
#[ignore = "requires a running NATS server (agent/changeme @ 127.0.0.1:4222)"]
async fn mixed_stream_delivers_valid_and_discards_invalid_in_order() {
    let client = connect().await;
    let subject = "notifications.device.integration-mixed";
    let mut sub = client.subscribe(subject).await.expect("subscribe");

    // (label, payload, is_valid)
    let stream: &[(&str, &[u8], bool)] = &[
        (
            "toast",
            br#"<toast><visual><binding template="ToastGeneric"><text>Quarterly report</text><text>Numbers are in</text></binding></visual></toast>"#,
            true,
        ),
        ("badge 9", br#"<badge value="9"/>"#, true),
        ("glyph alert", br#"<badge value="alert"/>"#, true),
        ("clear", br#"<badge value="0"/>"#, true),
        ("malformed", b"<toast><visual>", false),
        ("unknown root", br#"<tile/>"#, false),
        ("badge 100 (out of range)", br#"<badge value="100"/>"#, false),
        // After three bad messages, a valid one must still be delivered.
        ("badge 42", br#"<badge value="42"/>"#, true),
    ];

    for (_, payload, _) in stream {
        client
            .publish(subject, Bytes::copy_from_slice(payload))
            .await
            .expect("publish");
    }
    client.flush().await.expect("flush");

    let mut sink = RecordingSink::default();
    let mut delivered = 0usize;
    let mut dropped = 0usize;
    for _ in 0..stream.len() {
        let msg = tokio::time::timeout(Duration::from_secs(5), sub.next())
            .await
            .expect("timed out waiting for NATS message")
            .expect("subscription closed unexpectedly");
        match dispatch(&String::from_utf8_lossy(&msg.payload), &mut sink) {
            Ok(_) => delivered += 1,
            Err(_) => dropped += 1,
        }
    }

    let expected_valid = stream.iter().filter(|(_, _, ok)| *ok).count();
    assert_eq!(delivered, expected_valid, "all valid messages delivered");
    assert_eq!(
        dropped,
        stream.len() - expected_valid,
        "all bad messages dropped"
    );

    // Valid messages arrived and were routed in publish order.
    assert_eq!(sink.toasts.len(), 1);
    assert!(sink.toasts[0].contains("<text>Quarterly report</text>"));
    assert!(sink.toasts[0].contains("<text>Numbers are in</text>"));
    assert_eq!(
        sink.badges,
        vec![
            Badge::Numeric(9),
            Badge::Glyph(Glyph::Alert),
            Badge::Numeric(0),
            Badge::Numeric(42),
        ]
    );
}
