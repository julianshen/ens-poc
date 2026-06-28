//! Publish a gallery of complex toasts over NATS to the running agent —
//! controls, local images, remote (downloaded) images, and clickable links.
//!
//!   cargo run --example nats_gallery
//!
//! Each toast is published to this machine's device subject with a short gap so
//! they appear one at a time. The agent forwards the toast XML verbatim, so the
//! full templates render. Requires the agent running against a local NATS
//! server, and the AUMID registered with a Start Menu shortcut.
//!
//! Note: the "Open …" buttons and the toast body use `activationType="protocol"`
//! — these open a URL in the default browser and work WITHOUT a COM activator
//! (unlike foreground/background button callbacks, which the spec defers).

#[cfg(windows)]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use std::time::Duration;

    use bytes::Bytes;

    use tns::platform::RegistryDeviceIdSource;
    use tns::subject::DeviceIdSource;

    let device = RegistryDeviceIdSource
        .device_id()
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let subject = device.subject();

    // Local image assets, as file:/// URIs.
    let base = env!("CARGO_MANIFEST_DIR").replace('\\', "/");
    let avatar = format!("file:///{base}/tools/demo-assets/avatar.png");
    let hero = format!("file:///{base}/tools/demo-assets/hero.png");

    let repo = "https://github.com/julianshen/ens-poc";

    let gallery: Vec<(&str, String)> = vec![
        // 1) Local images: circular app-logo override + hero banner.
        (
            "local images",
            format!(
                r#"<toast>
  <visual><binding template="ToastGeneric">
    <image placement="appLogoOverride" hint-crop="circle" src="{avatar}"/>
    <text>Alex Chen</text>
    <text>Local images — avatar as app logo, hero banner below.</text>
    <image placement="hero" src="{hero}"/>
    <text placement="attribution">via TNS · local file images</text>
  </binding></visual>
</toast>"#
            ),
        ),
        // 2) Remote images: Windows downloads these from the web at render time.
        (
            "remote images",
            r#"<toast>
  <visual><binding template="ToastGeneric">
    <image placement="appLogoOverride" hint-crop="circle" src="https://picsum.photos/id/237/96/96"/>
    <text>Maria · #engineering</text>
    <text>Remote images downloaded by Windows from picsum.photos.</text>
    <image src="https://picsum.photos/id/180/360/180"/>
    <text placement="attribution">remote https images</text>
  </binding></visual>
</toast>"#
                .to_string(),
        ),
        // 3) Links: protocol activation opens a URL in the browser (no activator).
        (
            "links",
            format!(
                r#"<toast launch="{repo}" activationType="protocol">
  <visual><binding template="ToastGeneric">
    <text>Deploy succeeded ✅</text>
    <text>ens-poc · main · build #128 — click the toast or a button to open a link.</text>
    <text placement="attribution">click to open the repo</text>
  </binding></visual>
  <actions>
    <action content="Open repo" activationType="protocol" arguments="{repo}"/>
    <action content="View Actions" activationType="protocol" arguments="{repo}/actions"/>
  </actions>
</toast>"#
            ),
        ),
        // 4) Kitchen sink: local avatar + remote inline image + inputs +
        //    selection dropdown + a reply button + a protocol link button.
        (
            "kitchen sink",
            format!(
                r#"<toast launch="{repo}" activationType="protocol">
  <visual><binding template="ToastGeneric">
    <image placement="appLogoOverride" hint-crop="circle" src="{avatar}"/>
    <text>Alex Chen · #engineering</text>
    <text>Release candidate is ready — review and choose an action.</text>
    <image src="https://picsum.photos/id/1067/360/180"/>
    <text placement="attribution">via TNS Notifications</text>
  </binding></visual>
  <actions>
    <input id="reply" type="text" placeHolderContent="Reply to Alex…"/>
    <input id="decision" type="selection" defaultInput="ship">
      <selection id="ship" content="Ship it"/>
      <selection id="hold" content="Hold"/>
      <selection id="later" content="Review later"/>
    </input>
    <action content="Reply" arguments="action=reply" hint-inputId="reply"/>
    <action content="Open PR" activationType="protocol" arguments="{repo}"/>
  </actions>
</toast>"#
            ),
        ),
    ];

    let client = async_nats::ConnectOptions::new()
        .user_and_password("agent".into(), "changeme".into())
        .connect("nats://127.0.0.1:4222")
        .await?;

    for (label, xml) in gallery {
        client
            .publish(subject.clone(), Bytes::from(xml.into_bytes()))
            .await?;
        client.flush().await?;
        println!("published: {label} -> {subject}");
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("This gallery reads the device-id from the Windows registry; Windows only.");
}
