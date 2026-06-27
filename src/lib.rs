//! Enterprise Notification Service — platform-independent core.
//!
//! This crate holds the OS-agnostic logic (WNS XML parsing, type detection,
//! badge validation) so it can be unit-tested on any platform. Windows-only
//! glue (notifications, registry, service) lives behind traits elsewhere.

pub mod aumid;
pub mod backoff;
pub mod config;
pub mod dispatch;
pub mod eventlog;
pub mod nats;
pub mod notification;
pub mod render;
pub mod service;
pub mod subject;

#[cfg(windows)]
pub mod app;
#[cfg(windows)]
pub mod eventlog_win;
#[cfg(windows)]
pub mod platform;
#[cfg(windows)]
pub mod service_runtime;
