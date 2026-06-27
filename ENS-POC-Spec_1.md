# Enterprise Notification Service — POC Spec

**Version:** 0.1 (MVP)  
**Status:** Draft  
**Scope:** Proof of Concept

---

## 1. Goal

Validate that a NATS-based push notification system can deliver toast and badge
notifications to Windows desktops via a lightweight Rust agent, using the WNS XML
message format as the payload contract.

---

## 2. In Scope (MVP)

| Area | Decision |
|---|---|
| Transport | Core NATS pub/sub only (no JetStream) |
| Notification types | Toast and Badge only |
| Platform | Windows 10/11 x64 |
| Agent runtime | Rust, runs as a Windows Service |
| Message format | WNS-compatible XML (subset) |
| Auth | NATS username/password (NKey deferred to production) |
| Deployment | Manual install on test machines |

## 3. Out of Scope (POC)

- JetStream / offline message queuing
- Tile notifications
- Raw notifications
- Toast button click callbacks (COM activator)
- Multi-region / leaf node topology
- MDM / Group Policy deployment
- Delivery receipts / acknowledgement tracking
- Web or mobile clients

---

## 4. Architecture

```
Publisher (any app / test script)
        |
        | NATS publish
        | subject: notifications.device.<device-id>
        | payload: WNS XML
        v
  NATS Server (single node, self-hosted)
        |
        | core pub/sub
        v
  Rust Agent (Windows Service, per device)
        |
        | parse XML → detect toast or badge
        v
  Windows.UI.Notifications API
        |
        v
  Notification shown to user
```

No message persistence. If the device is offline when a message is published,
the message is dropped. Acceptable for POC.

---

## 5. NATS Subject Design

```
notifications.device.<device-id>
```

`device-id` is the Windows Machine GUID read from:
```
HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid
```

Example:
```
notifications.device.4f3a1bc2-9d0e-4a3f-b812-1234abcd5678
```

### Publishing a notification (test script)

```bash
nats pub notifications.device.4f3a1bc2-9d0e-4a3f-b812-1234abcd5678 \
  '<toast><visual><binding template="ToastGeneric">
     <text>Hello</text><text>This is a test notification</text>
   </binding></visual></toast>'
```

---

## 6. Message Format

Reuses WNS XML. The agent detects the notification type by the root element name.

### Toast

```xml
<toast>
  <visual>
    <binding template="ToastGeneric">
      <text>Title text</text>
      <text>Body text</text>
    </binding>
  </visual>
</toast>
```

### Badge (numeric)

```xml
<badge value="5"/>
```

Value 1–99. Value `0` clears the badge.

### Badge (glyph)

```xml
<badge value="alert"/>
```

Supported glyphs: `none`, `alert`, `activity`, `alarm`, `available`, `away`,
`busy`, `newMessage`, `paused`, `playing`, `unavailable`, `error`, `attention`.

### Type detection logic

The agent inspects the root XML element:

| Root element | Action |
|---|---|
| `<toast>` | Show toast notification |
| `<badge>` | Update badge |
| anything else | Log warning and discard |

---

## 7. Rust Agent

### Responsibilities

1. On install: register AUMID in registry
2. On start: read device ID and connect to NATS
3. Subscribe to `notifications.device.<device-id>`
4. On message: parse XML → dispatch to Windows notification API
5. On NATS disconnect: reconnect with backoff

### AUMID Registration (install-time, requires admin)

Registry key:
```
HKLM\SOFTWARE\Classes\AppUserModelId\YourCo.NotificationAgent
  DisplayName  = "Acme Notification Agent"
  IconUri      = "C:\Program Files\YourCo\agent.ico"
```

### Configuration file

Stored at: `C:\Program Files\YourCo\agent.toml`

```toml
nats_url   = "nats://nats.internal.yourco.com:4222"
nats_user  = "agent"
nats_pass  = "changeme"
aumid      = "YourCo.NotificationAgent"
```

`device-id` is always read live from the registry — not stored in config.

### Reconnection policy

```
Initial connect:    retry every 5 seconds, up to 12 attempts (1 minute)
After disconnect:   exponential backoff: 2s, 4s, 8s, 16s, 30s (cap)
```

No jitter needed at POC scale. Add jitter before production rollout.

### Windows Service details

| Property | Value |
|---|---|
| Service name | `YourCoNotificationAgent` |
| Start type | Automatic |
| Runs as | Local System |
| Restart on failure | Yes, after 10 seconds |

---

## 8. Key Crates

```toml
async-nats          = "0.35"
tokio               = { version = "1", features = ["full"] }
windows             = { version = "0.58", features = [
                        "UI_Notifications", "Data_Xml_Dom" ] }
windows-service     = "0.7"
quick-xml           = "0.36"
tracing             = "0.1"
tracing-subscriber  = "0.3"
anyhow              = "1"
```

---

## 9. Deliverables

| # | Deliverable | Notes |
|---|---|---|
| 1 | NATS server running on a test VM | Single node, no clustering |
| 2 | Rust agent binary + installer PowerShell script | Manual deploy to 2–3 test machines |
| 3 | AUMID registration at install time | Via PowerShell or agent self-register |
| 4 | Toast delivery working end-to-end | Verified on test machines |
| 5 | Badge delivery working end-to-end | Verified on test machines |
| 6 | Test publisher script | `nats pub` CLI or small Python script |
| 7 | Basic logging | Writes to Windows Event Log |

---

## 10. Success Criteria

| Criterion | Pass condition |
|---|---|
| Toast delivery | Toast appears on screen within 2 seconds of publish |
| Badge delivery | Badge number updates correctly on taskbar icon |
| Badge clear | `value="0"` removes the badge |
| Glyph badge | `value="alert"` shows glyph correctly |
| Reconnect | Agent recovers automatically after NATS restart |
| Bad XML | Invalid payload is logged and discarded; agent keeps running |
| Service restart | Agent survives machine reboot and reconnects |

---

## 11. Open Questions

| # | Question | Owner |
|---|---|---|
| 1 | Which NATS server version to standardize on? | Infra |
| 2 | Is Local System sufficient or does agent need a dedicated service account? | Security |
| 3 | How are device IDs enrolled? Manual list or auto-register on first connect? | Platform |
| 4 | What app owns the AUMID — the agent, or each individual app? | Arch |
| 5 | Logging destination — Windows Event Log only, or also file? | Ops |

---

## 12. Not Decided (defer to production)

- JetStream for offline message delivery
- Per-device NKey authentication
- Multi-region leaf node topology  
- Toast click callback / COM activator
- Group / user-level subjects
- Delivery acknowledgement database
