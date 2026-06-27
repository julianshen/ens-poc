//! Windows Event Log `tracing` layer and logging setup (OS glue; excluded from
//! coverage).
//!
//! Forwards every `tracing` event to the Windows Application event log via
//! `ReportEventW`, using the pure level→type mapping from
//! [`crate::eventlog`]. Also keeps an stderr layer so console runs stay
//! readable.

#![cfg(windows)]

use std::fmt::Write as _;

use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::prelude::*;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Security::PSID;
use windows::Win32::System::EventLog::{
    DeregisterEventSource, REPORT_EVENT_TYPE, RegisterEventSourceW, ReportEventW,
};
use windows::core::{HSTRING, PCWSTR};

use crate::eventlog::{EVENT_SOURCE, eventlog_type};

/// A `tracing` layer that writes events to the Windows Application event log.
///
/// The registered source handle is stored as an `isize` so the layer is
/// `Send + Sync` without wrapping a raw pointer; it is rebuilt into a `HANDLE`
/// per call.
pub struct EventLogLayer {
    handle: isize,
}

impl EventLogLayer {
    /// Register the event source. Fails if the registry/source is unavailable.
    pub fn new() -> windows::core::Result<Self> {
        let source = HSTRING::from(EVENT_SOURCE);
        let handle = unsafe { RegisterEventSourceW(PCWSTR::null(), &source)? };
        Ok(Self {
            handle: handle.0 as isize,
        })
    }
}

impl Drop for EventLogLayer {
    fn drop(&mut self) {
        let handle = HANDLE(self.handle as *mut core::ffi::c_void);
        unsafe {
            let _ = DeregisterEventSource(handle);
        }
    }
}

impl<S: Subscriber> Layer<S> for EventLogLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut message = String::new();
        event.record(&mut MessageVisitor(&mut message));
        if message.is_empty() {
            return;
        }

        let wtype = REPORT_EVENT_TYPE(eventlog_type(event.metadata().level()));
        let text = HSTRING::from(message);
        let strings = [PCWSTR(text.as_ptr())];
        let handle = HANDLE(self.handle as *mut core::ffi::c_void);
        unsafe {
            // Event id 1 / category 0: generic agent message (no message DLL).
            let _ = ReportEventW(
                handle,
                wtype,
                0,
                1,
                PSID(core::ptr::null_mut()),
                0,
                Some(&strings),
                None,
            );
        }
    }
}

/// Flattens a tracing event's fields into a single human-readable line.
struct MessageVisitor<'a>(&'a mut String);

impl Visit for MessageVisitor<'_> {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(self.0, "{value:?}");
        } else {
            let _ = write!(self.0, " {}={value:?}", field.name());
        }
    }
}

/// Initialise logging: stderr (for console use) plus the Windows event log when
/// the source can be registered. Safe to call once at startup.
pub fn init_logging() {
    let eventlog = EventLogLayer::new().ok();
    tracing_subscriber::registry()
        .with(tracing_subscriber::filter::LevelFilter::INFO)
        .with(tracing_subscriber::fmt::layer())
        .with(eventlog)
        .init();
}
