//! Demo / smoke harness: show chat-style (Teams/Slack-like) toasts through the
//! agent's real `WindowsSink`, plus a badge. Requires the AUMID to be
//! registered first (see the HKCU/HKLM AppUserModelId key) so the toasts
//! actually display.
//!
//!   cargo run --example notify_demo -- <aumid>
//!
//! Defaults to "TNS.SmokeDemo".

#[cfg(windows)]
fn main() {
    use std::thread::sleep;
    use std::time::Duration;

    use tns::dispatch::NotificationSink;
    use tns::notification::Badge;
    use tns::platform::WindowsSink;
    use tns::render::toast_xml;

    let aumid = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "TNS.SmokeDemo".into());
    let mut sink = WindowsSink::new(&aumid);

    // Teams-style: sender name as the title, message as the body.
    sink.show_toast(&toast_xml(&[
        "Alex Chen".into(),
        "Can you review the deploy PR before standup?".into(),
    ]))
    .expect("show Teams-style toast");
    sleep(Duration::from_millis(1200));

    // Slack-style: channel/sender as the title, message as the body.
    sink.show_toast(&toast_xml(&[
        "#engineering — Maria".into(),
        "Prod deploy is green and rolling out. Nice work all!".into(),
    ]))
    .expect("show Slack-style toast");
    sleep(Duration::from_millis(1200));

    // Unread badge, like a chat app's taskbar count.
    sink.update_badge(&Badge::Numeric(2)).expect("set badge");

    println!("Sent 2 chat-style toasts + badge under AUMID '{aumid}'.");
}

#[cfg(not(windows))]
fn main() {
    eprintln!("This demo runs on Windows only.");
}
