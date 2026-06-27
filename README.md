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
| `tools/` | `publish.py` test publisher + usage docs ([tools/README.md](tools/README.md)) |

The code is split so all logic (XML parse/render, type detection, badge
validation, reconnect backoff, config, subject building, service/AUMID data)
lives in OS-independent modules tested to >90%; only the COM/registry/network
calls are Windows-specific. See [CLAUDE.md](CLAUDE.md#module-map).

## Build & test

The Rust toolchain lives at `C:\Users\julia\.cargo\bin`; prepend it to PATH in
each shell first (`$env:Path += ";$env:USERPROFILE\.cargo\bin"`).

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
System, **restart 10s after a crash**), registers the AUMID and the Event Log
source, and starts it. Remove with `.\installer\uninstall.ps1`.

For local foreground testing without the service:

```powershell
cargo run -- .\installer\agent.toml      # Ctrl-C to stop
```

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

Feature-complete against the spec deliverables (§9). The remaining work is
environmental: stand up a NATS server, install on a test machine, and walk the
§10 success criteria (toast/badge appear, badge clear, glyph, reconnect after
NATS restart, bad-XML resilience, reboot survival). The `agent.ico` is a
placeholder pending real branding.
