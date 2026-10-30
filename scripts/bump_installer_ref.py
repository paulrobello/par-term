#!/usr/bin/env python3
"""Bump the gh-pages installer's pinned release REF to a given tag.

Verifies, before touching the pin, that the files the installer downloads
exist at the new tag (par-term card 01a18e7070). Run as part of every
release cut: make bump-installer-ref REF=vX.Y.Z
"""

import argparse
import re
import subprocess
import sys
from pathlib import Path

INSTALLER = Path("gh-pages/install-shell-integration.sh")
REF_LINE = re.compile(r'^REF="([^"]+)"', re.MULTILINE)
TAG_RE = re.compile(r"^v(\d+)\.(\d+)\.(\d+)$")
# The installer downloads, from <tag>/shell_integration/: the per-shell
# integration scripts plus the pt-dl / pt-ul / pt-imgcat transfer utils.
REQUIRED_PATHS = (
    "shell_integration/par_term_shell_integration.bash",
    "shell_integration/par_term_shell_integration.zsh",
    "shell_integration/par_term_shell_integration.fish",
    "shell_integration/pt-dl",
    "shell_integration/pt-ul",
    "shell_integration/pt-imgcat",
)


def run_git(*args: str) -> str:
    result = subprocess.run(["git", *args], capture_output=True, text=True)
    if result.returncode != 0:
        print(f"error: git {' '.join(args)} failed: {result.stderr.strip()}", file=sys.stderr)
        sys.exit(1)
    return result.stdout


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ref", required=True, help="release tag to pin (vX.Y.Z)")
    args = parser.parse_args()
    ref = args.ref
    if not TAG_RE.match(ref):
        print(f"error: {ref!r} is not a vX.Y.Z release tag", file=sys.stderr)
        return 1

    run_git("cat-file", "-e", f"{ref}^{{commit}}")
    missing = []
    for path in REQUIRED_PATHS:
        out = run_git("ls-tree", ref, "--name-only", "--", path)
        if not out.strip():
            missing.append(path)
    if missing:
        print(
            "error: the installer downloads these files, and they do not exist at "
            + ref
            + ":\n  "
            + "\n  ".join(missing),
            file=sys.stderr,
        )
        return 1

    text = INSTALLER.read_text(encoding="utf-8")
    new_text, count = REF_LINE.subn(f'REF="{ref}"', text, count=1)
    if count != 1:
        print(f'error: no REF="..." line in {INSTALLER}', file=sys.stderr)
        return 1
    INSTALLER.write_text(new_text, encoding="utf-8")
    print(f"installer pin moved to {ref}")
    print("next: commit gh-pages/install-shell-integration.sh as part of the release")
    return 0


if __name__ == "__main__":
    sys.exit(main())
