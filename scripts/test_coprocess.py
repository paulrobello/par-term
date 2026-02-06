#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# ///
"""Test coprocess for par-term automation feature.

Run with: uv run scripts/test_coprocess.py [--mode MODE]

Modes:
  echo      - Echo stdin lines back with a prefix (default)
  filter    - Filter stdin for lines matching a pattern (like grep)
  transform - Transform stdin lines (uppercase, reverse, etc.)
  log       - Log all stdin to a file with timestamps
  periodic  - Ignore stdin, emit periodic heartbeat messages
  counter   - Count lines received on stdin, report periodically
  alert     - Watch stdin for keywords and emit alert lines

When used as a par-term coprocess with "copy_terminal_output" enabled,
the script receives all terminal output on stdin. Its stdout is read
by par-term's coprocess manager as line-buffered text.

Examples:
  # Echo mode: prefixes each line
  uv run scripts/test_coprocess.py --mode echo

  # Filter mode: only passes through lines containing "error" (case-insensitive)
  uv run scripts/test_coprocess.py --mode filter --pattern "error"

  # Log mode: writes timestamped terminal output to a file
  uv run scripts/test_coprocess.py --mode log --logfile /tmp/par_term_coproc.log

  # Periodic mode: emits a heartbeat every N seconds
  uv run scripts/test_coprocess.py --mode periodic --interval 5

  # Alert mode: watches for keywords and emits alerts
  uv run scripts/test_coprocess.py --mode alert --keywords "error,fail,panic"
"""

from __future__ import annotations

import argparse
import re
import sys
import threading
import time
from datetime import datetime, timezone
from pathlib import Path


def mode_echo(args: argparse.Namespace) -> None:
    """Echo each stdin line back with a prefix."""
    prefix = args.prefix
    for line in sys.stdin:
        line = line.rstrip("\n")
        sys.stdout.write(f"[{prefix}] {line}\n")
        sys.stdout.flush()


def mode_filter(args: argparse.Namespace) -> None:
    """Pass through only lines matching a regex pattern."""
    pattern = re.compile(args.pattern, re.IGNORECASE if args.ignore_case else 0)
    for line in sys.stdin:
        line = line.rstrip("\n")
        if pattern.search(line):
            sys.stdout.write(f"{line}\n")
            sys.stdout.flush()


def mode_transform(args: argparse.Namespace) -> None:
    """Transform each line (uppercase, reverse, strip-ansi)."""
    ansi_re = re.compile(r"\x1b\[[0-9;]*[a-zA-Z]")
    for line in sys.stdin:
        line = line.rstrip("\n")
        # Strip ANSI escape sequences first
        clean = ansi_re.sub("", line)
        match args.transform:
            case "upper":
                out = clean.upper()
            case "lower":
                out = clean.lower()
            case "reverse":
                out = clean[::-1]
            case "strip":
                out = clean
            case _:
                out = clean
        sys.stdout.write(f"{out}\n")
        sys.stdout.flush()


def mode_log(args: argparse.Namespace) -> None:
    """Log all stdin to a timestamped file."""
    logfile = Path(args.logfile).expanduser()
    logfile.parent.mkdir(parents=True, exist_ok=True)
    sys.stdout.write(f"Logging to {logfile}\n")
    sys.stdout.flush()
    with logfile.open("a", encoding="utf-8") as f:
        f.write(f"\n--- Session started {datetime.now(timezone.utc).isoformat()} ---\n")
        for line in sys.stdin:
            ts = datetime.now(timezone.utc).strftime("%H:%M:%S.%f")[:-3]
            f.write(f"[{ts}] {line}")
            f.flush()
        f.write(f"--- Session ended {datetime.now(timezone.utc).isoformat()} ---\n")


def mode_periodic(args: argparse.Namespace) -> None:
    """Emit periodic heartbeat messages, ignore stdin."""
    interval = args.interval
    count = 0

    # Drain stdin in background so the process doesn't block
    def drain_stdin() -> None:
        try:
            for _ in sys.stdin:
                pass
        except Exception:
            pass

    t = threading.Thread(target=drain_stdin, daemon=True)
    t.start()

    try:
        while True:
            count += 1
            ts = datetime.now(timezone.utc).strftime("%H:%M:%S")
            sys.stdout.write(f"[heartbeat #{count}] {ts}\n")
            sys.stdout.flush()
            time.sleep(interval)
    except KeyboardInterrupt:
        pass


def mode_counter(args: argparse.Namespace) -> None:
    """Count stdin lines and report periodically."""
    interval = args.interval
    count = 0
    lock = threading.Lock()

    def report() -> None:
        while True:
            time.sleep(interval)
            with lock:
                sys.stdout.write(f"[counter] {count} lines received\n")
                sys.stdout.flush()

    t = threading.Thread(target=report, daemon=True)
    t.start()

    try:
        for _ in sys.stdin:
            with lock:
                count += 1
    except KeyboardInterrupt:
        pass
    finally:
        sys.stdout.write(f"[counter] final: {count} lines\n")
        sys.stdout.flush()


def mode_alert(args: argparse.Namespace) -> None:
    """Watch stdin for keywords and emit alert lines."""
    keywords = [k.strip().lower() for k in args.keywords.split(",") if k.strip()]
    ansi_re = re.compile(r"\x1b\[[0-9;]*[a-zA-Z]")
    if not keywords:
        sys.stderr.write("No keywords specified\n")
        sys.exit(1)
    sys.stdout.write(f"[alert] Watching for: {', '.join(keywords)}\n")
    sys.stdout.flush()
    for line in sys.stdin:
        clean = ansi_re.sub("", line).rstrip("\n").lower()
        for kw in keywords:
            if kw in clean:
                ts = datetime.now(timezone.utc).strftime("%H:%M:%S")
                sys.stdout.write(f"[ALERT {ts}] matched '{kw}': {line.rstrip()}\n")
                sys.stdout.flush()
                break


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Test coprocess for par-term automation",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "--mode",
        choices=["echo", "filter", "transform", "log", "periodic", "counter", "alert"],
        default="echo",
        help="Operating mode (default: echo)",
    )
    parser.add_argument("--prefix", default="COPROC", help="Prefix for echo mode")
    parser.add_argument("--pattern", default="error", help="Regex pattern for filter mode")
    parser.add_argument("--ignore-case", action="store_true", help="Case-insensitive filter")
    parser.add_argument(
        "--transform",
        choices=["upper", "lower", "reverse", "strip"],
        default="strip",
        help="Transform type for transform mode",
    )
    parser.add_argument(
        "--logfile",
        default="/tmp/par_term_coproc.log",
        help="Log file path for log mode",
    )
    parser.add_argument(
        "--interval",
        type=float,
        default=5.0,
        help="Interval in seconds for periodic/counter modes",
    )
    parser.add_argument(
        "--keywords",
        default="error,fail,panic,warning",
        help="Comma-separated keywords for alert mode",
    )

    args = parser.parse_args()

    modes = {
        "echo": mode_echo,
        "filter": mode_filter,
        "transform": mode_transform,
        "log": mode_log,
        "periodic": mode_periodic,
        "counter": mode_counter,
        "alert": mode_alert,
    }

    sys.stdout.write(f"[coprocess] started in '{args.mode}' mode\n")
    sys.stdout.flush()

    try:
        modes[args.mode](args)
    except (BrokenPipeError, KeyboardInterrupt):
        pass
    finally:
        sys.stdout.write("[coprocess] exiting\n")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
