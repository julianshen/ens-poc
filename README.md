# TNS — Enterprise Notification Service (POC)

A NATS-based push notification system that delivers Windows **toast** and
**badge** notifications to desktops via a lightweight Rust agent running as a
Windows Service. A publisher sends WNS-XML over core NATS; the agent detects the
type, and renders it through `Windows.UI.Notifications`.

```
Publisher → NATS (core pub/sub) → Rust agent (Windows Service) → Windows.UI.Notifications
            subject: notifications.device.<MachineGuid>
            payload: WNS XML (<toast …> or <badge …>)
```

See [ENS-POC-Spec_1.md](ENS-POC-Spec_1.md) for the full specification and
[CLAUDE.md](CLAUDE.md) for the architecture and module map.

## How it works

- **Subject** — the agent subscribes to `notifications.device.<device-id>`,
  where `<device-id>` is the Windows Machine GUID read **live** from
  `HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid` (never stored in config).
- **Type detection** — by XML root element: `<toast>` → toast, `<badge>` →
  badge, anything else → logged and discarded.
- **Toast passthrough** — a valid `<toast>` is validated then forwarded to
  Windows **verbatim**, so rich templates (action buttons, inputs, images,
  scenarios) survive. The agent does not re-render toasts down to text.
- **Badges** — numeric `1–99` (`0` clears) or a glyph (`alert`, `newMessage`, …).
- **No persistence** — if the device is offline at publish time the message is
  dropped (acceptable for the POC).
- **Resilience** — malformed/unknown payloads are logged and discarded; the
  agent keeps running (spec §10).

## Layout

| Path | What |
|---|---|
| `src/` | Agent: a pure, unit-tested core + a thin Windows/NATS shell |
| `installer/` | `install.ps1` / `uninstall.ps1`, `agent.toml` sample, `agent.ico` |
| `tools/` | `publish.py` test publisher, `New-AumidShortcut.ps1`, demo image assets ([tools/README.md](tools/README.md)) |
| `templates/` | Kitchen-sink WNS XML template library (IM, CI/CD, news, approval, …) + [docs](templates/README.md) |
| `examples/` | Runnable demos + NATS publishers (see [Examples](#examples)) |
| `tests/` | `windows_smoke.rs` + `nats_integration.rs` — `--ignored` tests over the real OS / a live broker |

The code is split so all logic (XML parse/render, type detection, badge
validation, reconnect backoff, config, subject building, service/AUMID/event-log
data) lives in OS-independent modules tested to >90%; only the COM/registry/
network calls are Windows-specific. See [CLAUDE.md](CLAUDE.md#module-map).

## Build & test

If `cargo` is not already on your PATH (e.g. a rustup install under
`%USERPROFILE%\.cargo\bin`), prepend it in each shell first:
`$env:Path += ";$env:USERPROFILE\.cargo\bin"`.

```powershell
cargo build --release            # release binary -> target/release/tns.exe (~1.8 MB)
cargo test                       # unit tests (the ~70 pure-logic tests)
cargo clippy --all-targets -- -D warnings
```

### Coverage (gate: >90% of the pure core)

The OS/network glue (`platform`, `nats`, `app`, `service_runtime`,
`eventlog_win`) makes real COM/registry/network calls and can't be unit-tested,
so it's excluded from the gate:

```powershell
cargo llvm-cov --lib --ignore-filename-regex "(platform|nats|app|service_runtime|eventlog_win)\.rs$" --fail-under-lines 90 --summary-only
```

The pure core sits at ~99% lines.

### Tests that need a real environment (ignored by default)

```powershell
# Real Windows registry read + on-screen toast/badge (pops notifications):
cargo test --test windows_smoke -- --ignored --nocapture

# NATS round-trip publish -> dispatch (needs a broker on 127.0.0.1:4222):
cargo test --test nats_integration -- --ignored
```

`nats_integration` covers a numeric badge, a toast, a **rich toast with controls
surviving the round-trip**, and a **mixed stream** (valid + malformed + unknown
+ out-of-range) asserting bad payloads are dropped while valid ones still deliver
in order.

## Run end-to-end over NATS

`nats-server` is a single self-contained binary. Download it from the
[official releases](https://github.com/nats-io/nats-server/releases), then:

```powershell
# 1. Start a broker with the agent's credentials (plaintext is fine for the POC).
.\nats-server.exe --user agent --pass changeme --addr 127.0.0.1 --port 4222

# 2. Point a config at it (or edit installer/agent.toml). agent.toml needs:
#      nats_url  = "nats://127.0.0.1:4222"
#      nats_user = "agent"
#      nats_pass = "changeme"
#      aumid     = "TNS.SmokeDemo"   # an AUMID with a Start Menu shortcut (see below)

# 3. Run the agent in the foreground (Ctrl-C to stop).
cargo run -- .\my-agent.toml

# 4. Publish to it (reads this machine's device-id automatically):
cargo run --example nats_publish -- toast      # or: badge | rich | demo
cargo run --example send_templates             # the whole template library
```

This was verified end-to-end against `nats-server` 2.14.2: published messages
drive real toasts, and the agent **recovers automatically after a broker
restart** (see [Reconnection](#reconnection)).

## Install as a Windows service (on a test machine, as Administrator)

```powershell
.\installer\install.ps1 -BinaryPath .\target\release\tns.exe -ConfigSource .\installer\agent.toml
```

Installs the service `YourCoNotificationAgent`:

- **Automatic** start (returns after a reboot) running as **Local System**.
- **Restart-on-failure**: `sc failure … restart/10000` **plus** `sc failureflag 1`
  so the SCM relaunches the agent ~10s after a crash *or* a non-zero exit (the
  agent reports exit code 1 when it fails to start/connect).
- Registers the **AUMID** (registry key) **and a Start Menu shortcut carrying it**
  via `tools/New-AumidShortcut.ps1` — **required**, or Windows silently drops
  toasts (`Show()` returns Ok but nothing appears).
- Registers the **Event Log** source (logging goes to the Windows Application log
  in service mode, stderr in console mode).

Ship `tools/New-AumidShortcut.ps1` alongside the installer. Remove everything
with `.\installer\uninstall.ps1`.

## See the notifications (no NATS needed)

The demos drive the real Windows notification path directly. Register the AUMID
with a Start Menu shortcut first (HKCU works without admin):

```powershell
$k = "HKCU:\Software\Classes\AppUserModelId\TNS.SmokeDemo"
New-Item -Path $k -Force | Out-Null
New-ItemProperty -Path $k -Name DisplayName -Value "TNS Notifications" -Force | Out-Null
.\tools\New-AumidShortcut.ps1 -ShortcutPath "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\TNS Notifications.lnk" `
  -TargetPath "$PWD\target\debug\examples\notify_demo.exe" -Aumid TNS.SmokeDemo
```

## Examples

| Example | Path | What it does |
|---|---|---|
| `notify_demo` | direct COM | Chat-style toasts + badge via `WindowsSink` |
| `notify_rich` | direct COM | Inline reply, buttons, selection dropdown, avatar/hero images |
| `nats_publish` | over NATS | Publish `toast` / `badge` / `rich` / `demo` to the agent |
| `nats_gallery` | over NATS | Local + remote images, and clickable protocol-link buttons |
| `send_templates` | over NATS | Send the `templates/` library (`-- list`, or a name substring) |

```powershell
cargo run --example notify_rich              # no NATS
cargo run --example nats_gallery             # needs broker + agent
cargo run --example send_templates -- cicd   # send CI/CD templates over NATS
```

**Activation caveat:** `activationType="protocol"` buttons and the toast body
**open URLs** in the browser (and `activationType="system"` Snooze/Dismiss work)
without any extra code. But `foreground`/`background` buttons (Reply, Approve)
render and are clickable, yet *processing* a click needs a COM activator — which
the spec explicitly defers.

## Template library

[templates/](templates/README.md) holds 13 self-contained WNS XML payloads
covering real scenarios (instant message, CI/CD success/failure, news, approval
request, incoming call, reminder, progress, system alert, rich media, local
images, badges) and nearly the full toast vocabulary. They're plain data files —
publishable with `send_templates`, the `nats` CLI, or `publish.py`.

## Observability (optional)

Set either field in `agent.toml` to light up the corresponding layer on the
agent's `tracing` pipeline (omit to disable — each is a no-op when unset):

```toml
sentry_dsn    = "https://<key>@<org>.ingest.sentry.io/<project>"   # error/event reporting
otel_endpoint = "http://collector.internal:4318"                  # OTLP/HTTP trace export
```

Sentry captures errors/events (and panics); OpenTelemetry exports spans over
OTLP/HTTP (protobuf) to a collector. Both flush on shutdown.

## Footprint

- **Binary:** ~3.8 MB release (~1.8 MB without the observability stack —
  sentry/otel/reqwest/prost add ~2 MB). `opt-level="z"`, LTO, `strip`,
  `panic="abort"`.
- **Memory:** ~2–3 MB private working set; ~7 threads idle (a 2-worker Tokio
  runtime). Larger working-set figures after the first toast are the shared
  WinRT/COM notification subsystem mapping in, not the agent's own memory.
- The `async-nats` dependency hard-requires a rustls crypto provider even for
  plaintext, so a ~1–2 MB TLS stack is linked in unavoidably (TLS itself is
  unused; NKey/TLS are deferred per spec).

## Reconnection

- **Initial connect** (spec §7): retry every 5s, up to 12 attempts, then give up.
  Driven by `connect_initial` (`backoff::InitialConnectBackoff`).
- **After a disconnect:** `async-nats` reconnects and re-subscribes itself,
  indefinitely; the agent supplies the spec's exponential backoff
  (2 → 4 → 8 → 16 → 30s cap) as its `reconnect_delay_callback`
  (`backoff::reconnect_delay`). Verified by restarting a live broker — reconnect
  attempts space out exponentially and delivery resumes after recovery.

## Status

Feature-complete against the spec deliverables (§9), and exercised end-to-end:

- ✅ Unit + integration tests green; toast/badge delivered over a live NATS
  broker; rich/interactive templates pass through intact.
- ✅ Automatic recovery after a NATS restart (spec §10), with the spec backoff.
- ✅ Visible toast/badge once the AUMID has a Start Menu shortcut.

Open items:

- **Local System / session 0:** a service running as Local System lives in
  session 0 and can't show interactive toasts to the logged-in user; console
  mode (user session) does. Service→user-session delivery is unresolved.
- **Reboot-survival** of the installed service (Automatic start is configured;
  not yet walked on a real machine).
- `agent.ico` is a placeholder pending real branding.
