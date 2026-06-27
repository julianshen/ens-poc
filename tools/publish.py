#!/usr/bin/env python3
"""Test publisher for the TNS notification agent (spec deliverable #6).

Publishes WNS-XML notifications to `notifications.device.<device-id>` over core
NATS, so you can verify toast/badge delivery end-to-end (spec section 10).

Setup:
    pip install nats-py

Examples (device-id auto-read from the registry on Windows):
    python publish.py toast --title "Hello" --body "This is a test"
    python publish.py badge --value 5
    python publish.py clear                 # badge value 0 -> clears the badge
    python publish.py glyph --value alert
    python publish.py demo                   # runs through every §10 criterion

Target a specific machine / server / credentials:
    python publish.py --server nats://nats.internal:4222 \\
                      --user agent --password changeme \\
                      --device-id 4f3a1bc2-9d0e-4a3f-b812-1234abcd5678 \\
                      toast --title Hi --body There

Zero-dependency alternative using the NATS CLI:
    nats pub notifications.device.<device-id> \\
      '<toast><visual><binding template="ToastGeneric"><text>Hi</text></binding></visual></toast>'
"""

import argparse
import asyncio
import sys
from xml.sax.saxutils import escape

SUBJECT_PREFIX = "notifications.device."

GLYPHS = {
    "none", "alert", "activity", "alarm", "available", "away", "busy",
    "newMessage", "paused", "playing", "unavailable", "error", "attention",
}


def read_machine_guid():
    """Return the local Windows Machine GUID, or None if unavailable."""
    try:
        import winreg
    except ImportError:
        return None
    try:
        with winreg.OpenKey(
            winreg.HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Cryptography"
        ) as key:
            value, _ = winreg.QueryValueEx(key, "MachineGuid")
            return value.strip()
    except OSError:
        return None


def toast_xml(title, body):
    texts = [f"<text>{escape(t)}</text>" for t in (title, body) if t is not None]
    return (
        '<toast><visual><binding template="ToastGeneric">'
        + "".join(texts)
        + "</binding></visual></toast>"
    )


def badge_xml(value):
    return f'<badge value="{escape(str(value))}"/>'


def build_payloads(args):
    """Return a list of (label, xml) tuples for the chosen command."""
    if args.command == "toast":
        return [("toast", toast_xml(args.title, args.body))]
    if args.command == "badge":
        if not 0 <= args.value <= 99:
            sys.exit("badge --value must be 0-99")
        return [(f"badge {args.value}", badge_xml(args.value))]
    if args.command == "clear":
        return [("badge clear", badge_xml(0))]
    if args.command == "glyph":
        if args.value not in GLYPHS:
            sys.exit(f"unknown glyph {args.value!r}; one of: {', '.join(sorted(GLYPHS))}")
        return [(f"glyph {args.value}", badge_xml(args.value))]
    if args.command == "demo":
        return [
            ("toast", toast_xml("TNS demo", "Toast delivery works")),
            ("badge 5", badge_xml(5)),
            ("glyph alert", badge_xml("alert")),
            ("badge clear", badge_xml(0)),
        ]
    sys.exit("no command given; try --help")


async def run(args):
    import nats

    device_id = args.device_id or read_machine_guid()
    if not device_id:
        sys.exit("could not determine device-id; pass --device-id explicitly")
    subject = SUBJECT_PREFIX + device_id

    options = {"servers": [args.server]}
    if args.user:
        options["user"] = args.user
        options["password"] = args.password

    nc = await nats.connect(**options)
    try:
        for label, xml in build_payloads(args):
            await nc.publish(subject, xml.encode("utf-8"))
            await nc.flush()
            print(f"published {label} -> {subject}")
            if args.command == "demo":
                await asyncio.sleep(args.delay)
    finally:
        await nc.close()


def parse_args(argv):
    parser = argparse.ArgumentParser(
        description="Publish test notifications to the TNS agent.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument("--server", default="nats://127.0.0.1:4222")
    parser.add_argument("--user")
    parser.add_argument("--password")
    parser.add_argument("--device-id", help="default: local Machine GUID (Windows)")
    parser.add_argument(
        "--delay", type=float, default=2.0, help="seconds between demo messages"
    )

    sub = parser.add_subparsers(dest="command", required=True)

    p_toast = sub.add_parser("toast", help="send a toast")
    p_toast.add_argument("--title", default="Test notification")
    p_toast.add_argument("--body", default="Hello from the TNS test publisher")

    p_badge = sub.add_parser("badge", help="send a numeric badge (0-99)")
    p_badge.add_argument("--value", type=int, required=True)

    sub.add_parser("clear", help="clear the badge (value 0)")

    p_glyph = sub.add_parser("glyph", help="send a glyph badge")
    p_glyph.add_argument("--value", required=True)

    sub.add_parser("demo", help="run through every §10 success criterion")

    return parser.parse_args(argv)


if __name__ == "__main__":
    asyncio.run(run(parse_args(sys.argv[1:])))
