# TNS — Enterprise Notification Service (POC)

A NATS-based push notification system that delivers Windows **toast** and
**badge** notifications to desktops via a lightweight Rust agent running as a
Windows Service. A publisher sends WNS-XML over core NATS; the agent parses it
and renders it through `Windows.UI.Notifications`.

```
Publisher → NATS (core pub/sub) → Rust agent (Windows Service) → Windows.UI.Notifications
```

See [ENS-POC-Spec_1.md](ENS-POC-Spec_1.md) for the full specification and
[CLAUDE.md](CLAUDE.md) for the architecture and module map.

## Layout

| Path | What |
|---|---|
| `src/` | Agent: a pure, unit-tested core + a thin Windows/NATS shell |
| `installer/` | `install.ps1` / `uninstall.ps1`, `agent.toml` sample, `agent.ico` |
| `tools/` | `publish.py` test publisher, `New-AumidShortcut.ps1`, demo assets ([tools/README.md](tools/README.md)) |
| `examples/` | Runnable notification demos (`notify_demo`, `notify_rich`) |
| `tests/` | `windows_smoke.rs` — `--ignored` smoke tests over the real OS APIs |

The code is split so all logic (XML parse/render, type detection, badge
validation, reconnect backoff, config, subject building, service/AUMID data)
lives in OS-independent modules tested to >90%; only the COM/registry/network
calls are Windows-specific. See [CLAUDE.md](CLAUDE.md#module-map).

## Build & test

If `cargo` is not already on your PATH (e.g. a rustup install under
`%USERPROFILE%\.cargo\bin`), prepend it in each shell first:
`$env:Path += ";$env:USERPROFILE\.cargo\bin"`.

```powershell
cargo build --release            # release binary -> target/release/tns.exe
cargo test                       # unit tests
cargo clippy --all-targets -- -D warnings
cargo llvm-cov --lib --ignore-filename-regex "(platform|nats|app|service_runtime|eventlog_win)\.rs$" --fail-under-lines 90 --summary-only
```

## Install (on a test machine, as Administrator)

```powershell
# Edit installer/agent.toml first: set nats_url / nats_user / nats_pass.
.\installer\install.ps1 -BinaryPath .\target\release\tns.exe -ConfigSource .\installer\agent.toml
```

This installs the service `YourCoNotificationAgent` (Automatic start, Local
System, **restart 10s after a crash**), registers the AUMID **and a Start Menu
shortcut carrying it** (required — without the shortcut Windows silently drops
toasts), registers the Event Log source, and starts it. Remove with
`.\installer\uninstall.ps1`. Ship `tools/New-AumidShortcut.ps1` alongside the
installer.

For local foreground testing without the service:

```powershell
cargo run -- .\installer\agent.toml      # Ctrl-C to stop
```

## See the notifications (no NATS needed)

The example demos drive the real Windows notification path directly. They need
the AUMID registered with a Start Menu shortcut first (HKCU works without admin):

```powershell
$k = "HKCU:\Software\Classes\AppUserModelId\TNS.SmokeDemo"
New-Item -Path $k -Force | Out-Null
New-ItemProperty -Path $k -Name DisplayName -Value "TNS Notifications" -Force | Out-Null
.\tools\New-AumidShortcut.ps1 -ShortcutPath "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\TNS Notifications.lnk" `
  -TargetPath "$PWD\target\debug\examples\notify_demo.exe" -Aumid TNS.SmokeDemo

cargo run --example notify_demo    # chat-style toasts + badge
cargo run --example notify_rich    # inline reply, buttons, dropdown, images
```

`notify_rich` shows controls beyond the agent's spec'd 2-text subset; acting on a
button click would need a COM activator (deferred by spec).

## Publish a test notification

Requires a running NATS server. With the [NATS CLI](https://github.com/nats-io/natscli):

```bash
nats pub notifications.device.<device-id> '<badge value="5"/>'
```

Or the Python publisher (`pip install nats-py`):

```bash
python tools/publish.py demo     # runs through every success criterion
```

Find `<device-id>` (the Windows MachineGuid) at
`HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid`, or let `publish.py` read it
automatically. Full options and the verification checklist are in
[tools/README.md](tools/README.md).

## Status

Feature-complete against the spec deliverables (§9). Verified by smoke tests on
Windows: the binary boots, reads the real device-id, retries on a dead NATS with
the spec'd backoff, and the `Windows.UI.Notifications` path delivers a visible
toast + badge once the AUMID has a Start Menu shortcut (see
[CLAUDE.md](CLAUDE.md) "Toast delivery requirements").

Open items:

- **End-to-end against a live NATS server** — stand one up, install on a test
  machine, and walk the remaining §10 criteria (reconnect after NATS restart,
  reboot survival).
- **Local System / session 0:** a service running as Local System can't show
  interactive toasts to the logged-in user; console mode (user session) does.
  Delivering from the service to the user session is unresolved.
- `agent.ico` is a placeholder pending real branding.
