//! Rich / interactive toast demo: inline reply box, action buttons, a
//! selection dropdown, and avatar/hero images — the full WNS toast vocabulary
//! that Teams/Slack-style notifications use.
//!
//!   cargo run --example notify_rich -- <aumid>   (default "TNS.SmokeDemo")
//!
//! NOTE: this goes BEYOND what the POC agent itself parses/renders. The agent
//! (per spec) only handles a 2-text ToastGeneric subset, and toast button
//! *callbacks* require a COM activator that the spec explicitly defers. Here the
//! controls render and are clickable, but handling a click (e.g. processing the
//! typed reply) would need that activator. This example talks to
//! `Windows.UI.Notifications` directly to showcase the capability.
//!
//! Requires the AUMID to be registered with a Start Menu shortcut (see
//! tools/New-AumidShortcut.ps1) or the toasts are silently dropped.

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use std::thread::sleep;
    use std::time::Duration;

    use windows::Data::Xml::Dom::XmlDocument;
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
    use windows::core::HSTRING;

    let aumid = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "TNS.SmokeDemo".into());

    // file:/// URIs for the bundled demo images (forward slashes required).
    let base = env!("CARGO_MANIFEST_DIR").replace('\\', "/");
    let avatar = format!("file:///{base}/tools/demo-assets/avatar.png");
    let hero = format!("file:///{base}/tools/demo-assets/hero.png");

    // 1) Teams-style chat with an inline reply box + Reply/Like buttons + avatar.
    let chat = format!(
        r#"<toast>
  <visual>
    <binding template="ToastGeneric">
      <image placement="appLogoOverride" hint-crop="circle" src="{avatar}"/>
      <text>Alex Chen</text>
      <text>Can you review the deploy PR before standup?</text>
    </binding>
  </visual>
  <actions>
    <input id="reply" type="text" placeHolderContent="Type a reply…"/>
    <action content="Reply" arguments="action=reply&amp;to=alex" hint-inputId="reply"/>
    <action content="Like 👍" arguments="action=like"/>
  </actions>
</toast>"#
    );

    // 2) Slack-style channel post with a hero image + two action buttons.
    let post = format!(
        r#"<toast>
  <visual>
    <binding template="ToastGeneric">
      <text>#engineering — Maria</text>
      <text>Prod deploy is green and rolling out 🎉</text>
      <image placement="hero" src="{hero}"/>
    </binding>
  </visual>
  <actions>
    <action content="View build" arguments="action=view"/>
    <action content="Dismiss" arguments="action=dismiss"/>
  </actions>
</toast>"#
    );

    // 3) Reminder scenario (stays until dismissed) with a selection dropdown.
    let reminder = r#"<toast scenario="reminder">
  <visual>
    <binding template="ToastGeneric">
      <text>Standup in 15 minutes</text>
      <text>Daily engineering sync</text>
    </binding>
  </visual>
  <actions>
    <input id="snooze" type="selection" defaultInput="15">
      <selection id="5" content="5 minutes"/>
      <selection id="15" content="15 minutes"/>
      <selection id="60" content="1 hour"/>
    </input>
    <action content="Snooze" arguments="action=snooze" hint-inputId="snooze"/>
    <action content="Join" arguments="action=join"/>
  </actions>
</toast>"#
        .to_string();

    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(&aumid))?;
    for (label, xml) in [("chat", chat), ("post", post), ("reminder", reminder)] {
        let doc = XmlDocument::new()?;
        doc.LoadXml(&HSTRING::from(xml))?;
        let toast = ToastNotification::CreateToastNotification(&doc)?;
        notifier.Show(&toast)?;
        println!("shown: {label}");
        sleep(Duration::from_millis(1800));
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("This demo runs on Windows only.");
}
