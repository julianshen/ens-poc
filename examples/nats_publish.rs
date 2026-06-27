//! Publish test notifications to the agent over NATS — exercises the full
//! publisher → NATS → agent → toast path.
//!
//!   cargo run --example nats_publish -- [toast|badge|demo]
//!
//! Defaults: server nats://127.0.0.1:4222, user agent / changeme, and the
//! device-id read live from this machine's registry (so it targets the agent
//! running on the same box).

#[cfg(windows)]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use bytes::Bytes;

    use tns::notification::{Badge, Glyph};
    use tns::platform::RegistryDeviceIdSource;
    use tns::render::{badge_xml, toast_xml};
    use tns::subject::DeviceIdSource;

    let what = std::env::args().nth(1).unwrap_or_else(|| "demo".into());

    let device = RegistryDeviceIdSource
        .device_id()
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let subject = device.subject();

    let client = async_nats::ConnectOptions::new()
        .user_and_password("agent".into(), "changeme".into())
        .connect("nats://127.0.0.1:4222")
        .await?;

    let messages: Vec<(String, String)> = match what.as_str() {
        "toast" => vec![(
            "toast".into(),
            toast_xml(&["Alex Chen".into(), "Can you review the deploy PR?".into()]),
        )],
        "badge" => vec![("badge 5".into(), badge_xml(&Badge::Numeric(5)))],
        _ => vec![
            (
                "toast".into(),
                toast_xml(&[
                    "Alex Chen".into(),
                    "Can you review the deploy PR before standup?".into(),
                ]),
            ),
            ("badge 3".into(), badge_xml(&Badge::Numeric(3))),
            ("glyph alert".into(), badge_xml(&Badge::Glyph(Glyph::Alert))),
        ],
    };

    for (label, xml) in messages {
        client
            .publish(subject.clone(), Bytes::from(xml.into_bytes()))
            .await?;
        println!("published {label} -> {subject}");
    }
    client.flush().await?;
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("This publisher reads the device-id from the Windows registry; Windows only.");
}
