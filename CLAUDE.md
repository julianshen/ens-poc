# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`tns` is the **Enterprise Notification Service (ENS) POC** — a NATS-based push
notification system that delivers Windows toast and badge notifications to
desktops via a lightweight Rust agent running as a Windows Service. The agent
subscribes to NATS, receives WNS-compatible XML payloads, and renders them
through `Windows.UI.Notifications`.

The authoritative requirements live in [ENS-POC-Spec_1.md](ENS-POC-Spec_1.md).
**Read it before implementing anything** — message format, subject design,
reconnection policy, and success criteria are all specified there and changes
should be checked against it.

## Module map

The crate is split **lib + bin**. The library (`tns`) holds all reusable logic;
`main.rs` is thin wiring. Pure modules are unit-tested to >90%; OS/network glue
is excluded from coverage (see below) and verified manually per spec §10.

| Module | Kind | Responsibility |
|---|---|---|
| [notification.rs](src/notification.rs) | pure | Parse WNS XML → `Notification`, detect type by root element |
| [render.rs](src/render.rs) | pure | Build WNS XML from a `Notification` (inverse of `notification`; round-trip tested) |
| [dispatch.rs](src/dispatch.rs) | pure | `NotificationSink` trait + route a parsed payload to it; discard bad input |
| [backoff.rs](src/backoff.rs) | pure | `InitialConnectBackoff` (5s×12) + `reconnect_delay()` (§7) |
| [config.rs](src/config.rs) | pure | Load `agent.toml` (`device-id` is **not** here) |
| [subject.rs](src/subject.rs) | pure | `DeviceId` GUID validation + NATS subject; `DeviceIdSource` trait |
| [aumid.rs](src/aumid.rs) | pure | AUMID registry key/value data; `AumidRegistrar` trait |
| [service.rs](src/service.rs) | pure | Service definition + `sc.exe` restart-on-failure args (§7) |
| [eventlog.rs](src/eventlog.rs) | pure | `tracing` level → Windows `EVENTLOG_*` type mapping (§9 #7) |
| [platform.rs](src/platform.rs) | **OS glue** `#[cfg(windows)]` | Real `NotificationSink`/`DeviceIdSource`/`AumidRegistrar` over `Windows.UI.Notifications` + registry |
| [nats.rs](src/nats.rs) | **net glue** | Connect/subscribe/reconnect loop, driving `backoff` + `dispatch` |
| [app.rs](src/app.rs) | **OS glue** `#[cfg(windows)]` | Bootstrap: config → device ID → sink → run loop, with cancellation |
| [service_runtime.rs](src/service_runtime.rs) | **OS glue** `#[cfg(windows)]` | `windows-service` SCM wrapper (Running/Stop, runs `app`) |
| [eventlog_win.rs](src/eventlog_win.rs) | **OS glue** `#[cfg(windows)]` | `tracing` layer → `ReportEventW`; `init_logging()` (stderr + Event Log) |

`main.rs` dispatches: `tns --service` → SCM runtime; `tns [config]` → console.

The seam: pure modules define traits (`NotificationSink`, `DeviceIdSource`,
`AumidRegistrar`); `platform.rs` implements them with real Windows calls and
tests use mocks. `platform.rs`/`nats.rs` carry **no formatting or validation
logic of their own** — they only make the COM/registry/network calls.

### Service install (built)

[installer/install.ps1](installer/install.ps1) registers the service per §7:
Automatic start (reboot survival), Local System, and **restart-on-failure after
10s** (`sc.exe failure ... actions= restart/10000`). It also registers the
AUMID. The recovery values mirror `ServiceSpec::sc_failure_args` in
[service.rs](src/service.rs) — **keep them in sync**.
[installer/uninstall.ps1](installer/uninstall.ps1) reverses it.

### Test publisher (built)

[tools/publish.py](tools/publish.py) publishes toast/badge test messages to a
device subject (auto-reads the local MachineGuid). [tools/README.md](tools/README.md)
documents it alongside zero-dependency `nats pub` CLI commands and the §10
verification checklist. Payloads mirror the round-trip-tested [render.rs](src/render.rs).

### Toast delivery requirements (learned from smoke testing)

- **A Start Menu shortcut carrying the AUMID is mandatory.** An unpackaged Win32
  app cannot raise toasts on the registry `AppUserModelId` key alone —
  `Show()` returns `Ok` but the toast is silently dropped. `install.ps1` creates
  the shortcut via [tools/New-AumidShortcut.ps1](tools/New-AumidShortcut.ps1)
  (sets `System.AppUserModel.ID` on the `.lnk`). Verified: the toast only
  reached the notification center after the shortcut existed.
- **Local System / session 0 caveat (open):** a service running as Local System
  lives in session 0 and **cannot display interactive toasts** to the logged-in
  user. The console mode (`tns <config>`, running in the user session) shows
  toasts correctly. Delivering from the service to the user session needs a
  user-session helper or a different service account — not yet addressed.

### Toast passthrough (rich templates)

Toasts are forwarded to Windows **verbatim**: `dispatch` uses `parse()` only for
type detection (`<toast>` vs `<badge>`) and malformed-rejection, then hands the
**original XML** to `NotificationSink::show_toast(xml)`. So rich templates —
inline reply inputs, action buttons, selection dropdowns, hero/avatar images —
pass through intact (verified end-to-end over NATS in
[tests/nats_integration.rs](tests/nats_integration.rs)). `render::toast_xml` is
now a publisher-side helper (builds a simple toast from text); the agent no
longer re-renders. Badges still go the typed route.

Controls **render and are clickable**, but *handling* a click (processing a
typed reply, running "Snooze") needs a COM activator — explicitly deferred by
the spec. [examples/notify_rich.rs](examples/notify_rich.rs) shows the full
vocabulary directly; `nats_publish -- rich` drives it through the agent.

### Not yet built

- `installer/agent.ico` is a generated placeholder (blue "N"); swap in real
  branding before release.
- **End-to-end manual verification** on a real machine against a NATS server
  (the §10 success criteria) — see [README.md](README.md) and [tools/README.md](tools/README.md).

## End-to-end flow

```
Publisher → NATS (core pub/sub, no JetStream) → Rust agent → Windows.UI.Notifications
```

- **Subject:** `notifications.device.<device-id>` where `<device-id>` is the
  Windows Machine GUID read **live** from
  `HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid` (never stored in config).
- **No persistence:** if the device is offline at publish time, the message is
  dropped. This is intentional for the POC.
- **Type detection is by XML root element**, not a type field:
  `<toast>` → show toast, `<badge>` → update badge, anything else → log warning
  and discard (the agent must keep running on bad input — this is a success
  criterion, not just defensive coding).

## Architectural boundaries that matter

The single most important design constraint for development: **separate the
platform-independent logic from the Windows API calls.**

- **Parsing + type detection + badge value validation** (1–99, `0` clears,
  glyph set: `none`, `alert`, `activity`, `alarm`, `available`, `away`, `busy`,
  `newMessage`, `paused`, `playing`, `unavailable`, `error`, `attention`) is
  pure logic with no OS dependency. Keep it in its own module so it can be
  unit-tested on **any** platform without the Windows APIs.
- **Windows-only surface** — `Windows.UI.Notifications`, registry reads
  (MachineGuid, AUMID registration), Windows Service lifecycle, Event Log —
  must be isolated behind traits/interfaces so the pure logic never depends on
  it. Gate Windows-only code with `#[cfg(windows)]` and test the boundary with
  mock implementations of those traits.

This split is what makes the >90% coverage target achievable: the testable core
is platform-independent, and the thin Windows shell is mocked.

## Key crates (from the spec — declare these in Cargo.toml as needed)

```toml
async-nats          = "0.35"
tokio               = { version = "1", features = ["full"] }
windows             = { version = "0.58", features = ["UI_Notifications", "Data_Xml_Dom"] }
windows-service     = "0.7"
quick-xml           = "0.36"
tracing             = "0.1"
tracing-subscriber  = "0.3"
anyhow              = "1"
```

Note `Cargo.toml` uses `edition = "2024"`.

## Commands

The Rust toolchain (cargo/rustc 1.96, clippy, rustfmt) may be installed under
`%USERPROFILE%\.cargo\bin` but **not on the PATH** that Claude Code's shells
inherit. Prepend it first in each PowerShell session:

```powershell
$env:Path += ";$env:USERPROFILE\.cargo\bin"
```

```bash
cargo build                      # debug build
cargo build --release            # release build (deliverable binary)
cargo run                        # run the agent locally
cargo test                       # run all tests
cargo test <name>                # run tests matching a substring
cargo test --lib <module>::      # run one module's unit tests
cargo test -- --nocapture        # show stdout/println from tests
cargo clippy --all-targets -- -D warnings   # lint (treat warnings as errors)
cargo fmt                        # format
```

### Smoke tests (real OS, manual)

[tests/windows_smoke.rs](tests/windows_smoke.rs) holds `#[ignore]`d tests that
touch real Windows APIs (registry read + on-screen notifications), so they stay
out of the normal suite. Run on a Windows box:

```bash
cargo test --test windows_smoke -- --ignored --nocapture
```

`show_toast` returning `Ok` proves the COM path is wired correctly, but a toast
only *displays* once the AUMID is registered (`install.ps1`). Also useful: run
the binary against an unreachable NATS to confirm boot + device-id + backoff:
`cargo run -- <config-with-dead-nats>`.

For a *visible* demo, register an AUMID (HKCU works without admin) and run the
chat-style ([examples/notify_demo.rs](examples/notify_demo.rs)) demo:

```powershell
$k = "HKCU:\Software\Classes\AppUserModelId\TNS.SmokeDemo"
New-Item -Path $k -Force | Out-Null
New-ItemProperty -Path $k -Name DisplayName -Value "TNS Notifications" -Force | Out-Null
cargo run --example notify_demo -- TNS.SmokeDemo
```

### Coverage (target: >90%)

Uses `cargo-llvm-cov` (already installed, with the `llvm-tools-preview`
component). **The gate excludes the OS/network glue** — `platform.rs` and
`nats.rs` make real COM/registry/network calls that can't be unit-tested, so
counting them would dilute and misrepresent the metric. Run:

```bash
# The >90% gate over the pure core (this is the command to keep green):
cargo llvm-cov --lib --ignore-filename-regex "(platform|nats|app|service_runtime|eventlog_win)\.rs$" \
  --fail-under-lines 90 --summary-only

cargo llvm-cov --lib --ignore-filename-regex "(platform|nats|app|service_runtime|eventlog_win)\.rs$" --html
```

The pure core sits at ~99% lines. `platform.rs`/`nats.rs` are verified manually
against the spec §10 success criteria (toast/badge appear, reconnect after NATS
restart), not by unit tests. When adding a new pure module, it falls under the
gate automatically; when adding OS glue, extend the ignore regex.

## Working style for this repo

- **TDD is required: red → green → refactor.** Write a failing test first, watch
  it fail, write the minimum code to pass, watch it pass, then refactor. Do not
  add production code without a failing test driving it.
- Derive test cases directly from the spec's **Success Criteria** (§10) and
  **Message Format** (§6): badge clear on `value="0"`, glyph rendering, invalid
  XML discarded without crashing, reconnect after NATS restart, etc.
- The **reconnection policy is exact** and is unit-tested as pure logic in
  `backoff.rs`: the **initial** connect retries every 5s up to 12 attempts then
  gives up (`connect_initial` drives this loop). **After** an established
  connection drops, `async-nats` reconnects and re-subscribes itself,
  indefinitely — the agent supplies the spec backoff (2→4→8→16→30s) as its
  `reconnect_delay_callback` rather than looping manually. (Confirmed by a live
  broker restart: the manual approach with `max_reconnects(Some(0))` did NOT
  disable async-nats's internal reconnection, so that code was dead.) No jitter
  at POC scale.
- Config is read from `agent.toml` (`nats_url`, `nats_user`, `nats_pass`,
  `aumid`); `device-id` is **not** in config — it is always read live from the
  registry.
