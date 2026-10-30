#!/usr/bin/env python3
"""Fail when the gh-pages installer's pinned REF is older than the latest release tag.

SEC-004 pinned gh-pages/install-shell-integration.sh to a release tag so the
pipe-to-shell installer serves fixed reviewed content. The pin must move with
every release; this gate owns the reminder (par-term card 01a18e7070,
"Installer REF pin needs a release-process bump owner"). Runs in `make ci`,
so the next `make ci` after a release fails until the pin is bumped.
"""

import re
import subprocess
import sys
from pathlib import Path

INSTALLER = Path("gh-pages/install-shell-integration.sh")
REF_LINE = re.compile(r'^REF="([^"]+)"', re.MULTILINE)
TAG_RE = re.compile(r"^v(\d+)\.(\d+)\.(\d+)$")


def tag_key(tag: str) -> tuple[int, int, int]:
    match = TAG_RE.match(tag)
    assert match is not None  # callers filter to TAG_RE matches first
    return int(match.group(1)), int(match.group(2)), int(match.group(3))


def main() -> int:
    if not INSTALLER.exists():
        print(f"error: {INSTALLER} not found; the installer pin gate needs it", file=sys.stderr)
        return 1
    match = REF_LINE.search(INSTALLER.read_text(encoding="utf-8"))
    if not match:
        print(f'error: no REF="..." line in {INSTALLER}', file=sys.stderr)
        return 1
    pin = match.group(1)
    if not TAG_RE.match(pin):
        print(f"error: installer pin {pin!r} is not a vX.Y.Z release tag", file=sys.stderr)
        return 1

    tags = subprocess.run(
        ["git", "tag", "-l", "v*"], capture_output=True, text=True, check=True
    ).stdout.split()
    release_tags = [t for t in tags if TAG_RE.match(t)]
    if not release_tags:
        # Checkout without release tags (e.g. a shallow CI clone): nothing to enforce.
        return 0
    latest = max(release_tags, key=tag_key)
    if tag_key(latest) > tag_key(pin):
        print(
            f"error: installer pin is {pin} but release {latest} is out; "
            f"bump the pin with: make bump-installer-ref REF={latest}",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
