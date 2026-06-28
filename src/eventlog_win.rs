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

/// Guards for observability exporters that must outlive the program: the Sentry
/// client (flushes on drop) and the OpenTelemetry tracer provider (flushed via
/// its `Drop` here). Hold the returned value in `main` / the service until exit.
#[must_use = "drop the guards at program end so Sentry/OTel flush their queues"]
pub struct LoggingGuards {
    _sentry: Option<sentry::ClientInitGuard>,
    otel: Option<opentelemetry_sdk::trace::SdkTracerProvider>,
}

impl Drop for LoggingGuards {
    fn drop(&mut self) {
        if let Some(provider) = &self.otel {
            let _ = provider.shutdown();
        }
    }
}

/// Build an OTLP/HTTP tracer provider for `endpoint` (e.g. `http://host:4318`).
/// Uses a blocking HTTP client so the batch exporter runs on its own thread and
/// needs no Tokio runtime.
fn build_otel_provider(
    endpoint: &str,
) -> Result<opentelemetry_sdk::trace::SdkTracerProvider, opentelemetry_otlp::ExporterBuildError> {
    use opentelemetry_otlp::WithExportConfig;

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_endpoint(endpoint)
        .with_protocol(opentelemetry_otlp::Protocol::HttpBinary)
        .build()?;

    Ok(opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(
            opentelemetry_sdk::Resource::builder()
                .with_service_name("tns-agent")
                .build(),
        )
        .build())
}

/// Initialise logging and observability from `config`: stderr (console) + the
/// Windows event log, plus Sentry error reporting and OpenTelemetry tracing
/// when `config.sentry_dsn` / `config.otel_endpoint` are set (each is a no-op
/// otherwise). Call once at startup and keep the returned [`LoggingGuards`].
pub fn init_logging(config: &crate::config::Config) -> LoggingGuards {
    use opentelemetry::trace::TracerProvider as _;
    use tracing_subscriber::Layer;

    // Sentry: holding the guard keeps the client alive (and flushes on drop).
    let sentry_guard = config.sentry_dsn.as_ref().map(|dsn| {
        sentry::init((
            dsn.as_str(),
            sentry::ClientOptions {
                release: sentry::release_name!(),
                ..Default::default()
            },
        ))
    });
    let sentry_layer = sentry_guard.as_ref().map(|_| sentry_tracing::layer());

    // OpenTelemetry: an OTLP/HTTP span exporter, if an endpoint is configured.
    let otel_provider = config.otel_endpoint.as_ref().and_then(|endpoint| {
        build_otel_provider(endpoint)
            .map_err(|e| eprintln!("OpenTelemetry init failed: {e}"))
            .ok()
    });
    let otel_layer = otel_provider
        .as_ref()
        .map(|p| tracing_opentelemetry::layer().with_tracer(p.tracer("tns")));

    let eventlog = EventLogLayer::new().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::filter::LevelFilter::INFO)
        .with(tracing_subscriber::fmt::layer())
        .with(eventlog)
        .with(sentry_layer.map(|l| l.boxed()))
        .with(otel_layer.map(|l| l.boxed()))
        .init();

    LoggingGuards {
        _sentry: sentry_guard,
        otel: otel_provider,
    }
}
