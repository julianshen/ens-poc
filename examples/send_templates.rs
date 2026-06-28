//! Send the `templates/` library to the agent over NATS for testing.
//!
//!   cargo run --example send_templates           # send every template
//!   cargo run --example send_templates -- list    # list template names
//!   cargo run --example send_templates -- cicd    # send only matching names
//!
//! Reads each `.xml` from `templates/`, substitutes `__ASSETS__` with a file://
//! URI to `tools/demo-assets`, and publishes it to this machine's device
//! subject. Requires a running NATS server and the subscribed agent.

#[cfg(windows)]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use std::path::Path;
    use std::time::Duration;

    use bytes::Bytes;

    use tns::platform::RegistryDeviceIdSource;
    use tns::subject::DeviceIdSource;

    let manifest = env!("CARGO_MANIFEST_DIR");
    let templates_dir = Path::new(manifest).join("templates");
    let assets = format!("file:///{}/tools/demo-assets", manifest.replace('\\', "/"));

    // All .xml templates, sorted by file name for a stable order.
    let mut templates: Vec<std::path::PathBuf> = std::fs::read_dir(&templates_dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "xml"))
        .collect();
    templates.sort();

    let filter = std::env::args().nth(1);
    if filter.as_deref() == Some("list") {
        for p in &templates {
            println!("{}", p.file_stem().unwrap().to_string_lossy());
        }
        return Ok(());
    }

    let device = RegistryDeviceIdSource
        .device_id()
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let subject = device.subject();

    let client = async_nats::ConnectOptions::new()
        .user_and_password("agent".into(), "changeme".into())
        .connect("nats://127.0.0.1:4222")
        .await?;

    let mut sent = 0usize;
    for path in &templates {
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        if filter
            .as_ref()
            .is_some_and(|want| !stem.contains(want.as_str()))
        {
            continue;
        }
        let xml = std::fs::read_to_string(path)?.replace("__ASSETS__", &assets);
        client
            .publish(subject.clone(), Bytes::from(xml.clone().into_bytes()))
            .await?;
        client.flush().await?;
        println!("sent {stem} ({} bytes)", xml.len());
        sent += 1;
        tokio::time::sleep(Duration::from_millis(2500)).await;
    }

    println!("done — {sent} template(s) -> {subject}");
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("This sender reads the device-id from the Windows registry; Windows only.");
}
