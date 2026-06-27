# Test publisher (spec deliverable #6)

Tools for verifying toast/badge delivery end-to-end (spec section 10). All of
them publish WNS-XML to `notifications.device.<device-id>` over core NATS.

> The `<device-id>` is the target machine's Windows MachineGuid, read from
> `HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid`. Example subject:
> `notifications.device.4f3a1bc2-9d0e-4a3f-b812-1234abcd5678`
> (Each machine has its own — the agent reads it live from the registry, and
> `publish.py` reads it automatically on the local machine.)

## Prerequisites

End-to-end testing needs a **running NATS server** (single node is fine) and the
**agent running** on the target machine:

```powershell
# Receiver: run the agent in the foreground against a config file.
cargo run -- .\installer\agent.toml      # or the installed service
```

Point `agent.toml`'s `nats_url` at your test server before starting.

## Option A — Python script (`publish.py`)

```bash
pip install nats-py

python tools/publish.py toast --title "Hello" --body "This is a test"
python tools/publish.py badge --value 5
python tools/publish.py clear                  # value 0 -> clears badge
python tools/publish.py glyph --value alert
python tools/publish.py demo                   # every §10 criterion in sequence
```

Device-id is auto-read from the registry on Windows; override with
`--device-id`, and set `--server` / `--user` / `--password` to target a remote
server. See `python tools/publish.py --help`.

## Option B — NATS CLI (zero dependencies)

```bash
SUBJECT=notifications.device.<device-id>   # your target machine's MachineGuid

# Toast
nats pub $SUBJECT '<toast><visual><binding template="ToastGeneric"><text>Hello</text><text>This is a test</text></binding></visual></toast>'

# Numeric badge
nats pub $SUBJECT '<badge value="5"/>'

# Clear badge
nats pub $SUBJECT '<badge value="0"/>'

# Glyph badge
nats pub $SUBJECT '<badge value="alert"/>'
```

## What to check (spec section 10)

| Publish | Expected |
|---|---|
| toast | Toast appears within 2s |
| `badge value="5"` | Taskbar badge shows 5 |
| `badge value="0"` | Badge cleared |
| `badge value="alert"` | Alert glyph shown |
| (restart NATS, publish again) | Agent reconnects and delivers |
| malformed XML | Logged + discarded; agent keeps running |
