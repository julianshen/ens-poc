# Notification template library

A kitchen-sink set of WNS toast/badge XML templates covering real-world
scenarios and (nearly) the full toast vocabulary. Each `.xml` file is a complete
payload — publish it to `notifications.device.<device-id>` and the agent
forwards it to `Windows.UI.Notifications` verbatim.

## Templates

| File | Scenario | Capabilities shown |
|---|---|---|
| `01-instant-message.xml` | Team chat / IM | circular avatar, attribution, inline **reply text input**, background buttons, foreground launch |
| `02-cicd-success.xml` | CI/CD passed | app logo, **protocol-link buttons** (open in browser) |
| `03-cicd-failure.xml` | CI/CD failed | `scenario="urgent"`, **hero image**, **colored buttons** (Success/Critical) |
| `04-news-update.xml` | News article | hero + circular source logo, **Read more** link, background Save |
| `05-approval-request.xml` | Approval | `scenario="reminder"` (stays), **comment input**, Approve/Reject colored buttons |
| `06-incoming-call.xml` | Incoming call | `scenario="incomingCall"` (ringtone loop), Accept/Decline |
| `07-reminder-snooze.xml` | Calendar reminder | **selection dropdown**, **system Snooze/Dismiss** (work natively) |
| `08-progress-download.xml` | Download | **progress bar** with value + status |
| `09-system-alert.xml` | System alert | urgent, `ms-settings:` deep link, system Dismiss |
| `10-rich-media.xml` | Release / kitchen sink | launch, logo + hero + inline image, dropdown, colored link button, system snooze, **context-menu action** |
| `11-local-images.xml` | Local assets | `file://` images (sender substitutes `__ASSETS__`) |
| `badge-count.xml` | Badge | numeric badge (12) |
| `badge-glyph.xml` | Badge | glyph badge (alert) |

## Sending them

Needs a running NATS server + the agent subscribed (see the repo
[README](../README.md)). Then:

```powershell
cargo run --example send_templates            # send every template, ~2.5s apart
cargo run --example send_templates -- list     # list template names
cargo run --example send_templates -- cicd     # send only matching names (substring)
```

## Notes

- **Links work, button callbacks don't (by spec).** `activationType="protocol"`
  (and the toast `launch`) open URLs via the shell — no COM activator needed.
  `activationType="system"` Snooze/Dismiss are also handled natively. But
  `foreground`/`background` buttons (Reply, Approve, Like) render and are
  clickable, yet *processing* the click needs an activator the spec defers.
- **Remote images** are fetched by Windows at render time (needs internet);
  `__ASSETS__` images are local files.
- `scenario="urgent"`/`incomingCall` are Windows 11 features; older builds
  degrade gracefully.
