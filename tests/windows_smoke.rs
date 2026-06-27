//! Manual smoke tests that touch real Windows APIs (registry + notifications).
//! Ignored by default because they read the machine registry and pop on-screen
//! notifications. Run explicitly:
//!
//!   cargo test --test windows_smoke -- --ignored --nocapture
#![cfg(windows)]

use tns::dispatch::NotificationSink;
use tns::notification::{Badge, Glyph};
use tns::platform::{RegistryDeviceIdSource, WindowsSink};
use tns::subject::DeviceIdSource;

#[test]
#[ignore = "reads the real registry; run with --ignored"]
fn reads_real_machine_guid() {
    let id = RegistryDeviceIdSource
        .device_id()
        .expect("should read MachineGuid from registry");
    println!("device-id = {}", id.as_str());
    assert_eq!(
        id.subject(),
        format!("notifications.device.{}", id.as_str())
    );
}

#[test]
#[ignore = "shows real toast/badge via Windows.UI.Notifications; run with --ignored"]
fn shows_toast_and_badge() {
    let mut sink = WindowsSink::new("YourCo.NotificationAgent");
    sink.show_toast(&["TNS smoke test".into(), "Toast path works".into()])
        .expect("show_toast should reach Windows.UI.Notifications");
    sink.update_badge(&Badge::Numeric(7))
        .expect("numeric badge");
    sink.update_badge(&Badge::Glyph(Glyph::Alert))
        .expect("glyph badge");
    sink.update_badge(&Badge::Numeric(0)).expect("clear badge");
}
