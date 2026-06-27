//! Windows platform glue (OS-specific; excluded from coverage).
//!
//! This is the thin shell around the tested core: it implements the
//! [`NotificationSink`](crate::dispatch::NotificationSink),
//! [`DeviceIdSource`](crate::subject::DeviceIdSource) and
//! [`AumidRegistrar`](crate::aumid::AumidRegistrar) traits using real Windows
//! APIs. All formatting/validation logic it relies on lives in (and is tested
//! by) the pure modules — here we only make the COM and registry calls.

#![cfg(windows)]

use winreg::RegKey;
use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE};

use windows::Data::Xml::Dom::XmlDocument;
use windows::UI::Notifications::{
    BadgeNotification, BadgeUpdateManager, ToastNotification, ToastNotificationManager,
};
use windows::core::HSTRING;

use crate::aumid::{AumidRegistrar, AumidRegistration};
use crate::dispatch::{NotificationSink, SinkError};
use crate::notification::Badge;
use crate::render;
use crate::subject::{DeviceId, DeviceIdSource, InvalidDeviceId};

/// Registry path holding the per-machine cryptographic GUID (spec §5).
const MACHINE_GUID_KEY: &str = r"SOFTWARE\Microsoft\Cryptography";
const MACHINE_GUID_VALUE: &str = "MachineGuid";

fn sink_err(e: impl std::fmt::Display) -> SinkError {
    SinkError(e.to_string())
}

/// A [`NotificationSink`] backed by `Windows.UI.Notifications`.
pub struct WindowsSink {
    aumid: HSTRING,
}

impl WindowsSink {
    pub fn new(aumid: &str) -> Self {
        Self {
            aumid: HSTRING::from(aumid),
        }
    }

    fn load_xml(payload: &str) -> Result<XmlDocument, SinkError> {
        let doc = XmlDocument::new().map_err(sink_err)?;
        doc.LoadXml(&HSTRING::from(payload)).map_err(sink_err)?;
        Ok(doc)
    }
}

impl NotificationSink for WindowsSink {
    fn show_toast(&mut self, xml: &str) -> Result<(), SinkError> {
        // Pass the toast XML through unchanged so rich templates (buttons,
        // inputs, images) reach Windows intact.
        let doc = Self::load_xml(xml)?;
        let toast = ToastNotification::CreateToastNotification(&doc).map_err(sink_err)?;
        let notifier =
            ToastNotificationManager::CreateToastNotifierWithId(&self.aumid).map_err(sink_err)?;
        notifier.Show(&toast).map_err(sink_err)
    }

    fn update_badge(&mut self, badge: &Badge) -> Result<(), SinkError> {
        let doc = Self::load_xml(&render::badge_xml(badge))?;
        let notification = BadgeNotification::CreateBadgeNotification(&doc).map_err(sink_err)?;
        let updater = BadgeUpdateManager::CreateBadgeUpdaterForApplicationWithId(&self.aumid)
            .map_err(sink_err)?;
        updater.Update(&notification).map_err(sink_err)
    }
}

/// Reads the device's Machine GUID from the registry (spec §5).
pub struct RegistryDeviceIdSource;

impl DeviceIdSource for RegistryDeviceIdSource {
    fn device_id(&self) -> Result<DeviceId, InvalidDeviceId> {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let raw: String = hklm
            .open_subkey_with_flags(MACHINE_GUID_KEY, KEY_READ)
            .and_then(|k| k.get_value(MACHINE_GUID_VALUE))
            .map_err(|e| InvalidDeviceId(format!("reading MachineGuid: {e}")))?;
        DeviceId::parse(raw.trim())
    }
}

/// Writes AUMID registration to `HKLM` (requires admin; spec §7).
pub struct RegistryAumidRegistrar;

impl AumidRegistrar for RegistryAumidRegistrar {
    fn register(&self, reg: &AumidRegistration) -> Result<(), String> {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let (key, _) = hklm
            .create_subkey_with_flags(reg.subkey_path(), KEY_WRITE)
            .map_err(|e| format!("creating AUMID key: {e}"))?;
        for (name, value) in reg.values() {
            key.set_value(name, &value)
                .map_err(|e| format!("setting {name}: {e}"))?;
        }
        Ok(())
    }
}
