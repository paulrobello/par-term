#!/usr/bin/env python3
"""Fail the build when a production Rust file grows past the size CLAUDE.md sets.

CLAUDE.md: "Keep files under 500 lines; refactor files exceeding 800 lines."
500 is a target, so it warns. 800 is the refactor threshold, so it fails.

Only production lines count. A `#[cfg(test)]` item -- module, function, `use`,
or `impl` block -- is subtracted wherever it appears in the file, not just as a
trailing test module. Cutting at the first `#[cfg(test)]` instead would report
`par-term-acp/src/agents.rs` (a test-bearing file) as 2
production lines and exempt it permanently, which is the failure mode that kills
a check like this: a false negative nobody ever sees.

Exemptions live in `.line-count-exempt`, one `path: reason` per line. An
exemption for a file that no longer needs one is reported but never fails --
otherwise the gate turns red the moment somebody fixes a file.
"""

from __future__ import annotations

import os
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
EXEMPT_FILE = REPO_ROOT / ".line-count-exempt"

FAIL_THRESHOLD = 800
WARN_THRESHOLD = 500

SKIP_DIRS = {".git", "target", "node_modules", "tests", "benches", "examples"}

CFG_TEST_RE = re.compile(r"#\[cfg\((?![^)]*\bnot\(test\))[^)]*\btest\b")


def is_test_path(rel: Path) -> bool:
    """Test and benchmark sources are exempt as a class, not per-file."""
    name = rel.name
    return (
        name == "tests.rs"
        or name.endswith("_tests.rs")
        or name.endswith("_test.rs")
        or "tests" in rel.parts[:-1]
        or "benches" in rel.parts[:-1]
        or "examples" in rel.parts[:-1]
    )


def strip_literals(text: str) -> list[str]:
    """Blank out comments and string/char literals, preserving line structure.

    Brace counting has to ignore a `}` inside a doc comment or inside the GLSL
    and WGSL source embedded in this repo's test fixtures, or the extent of a
    `#[cfg(test)] mod` is computed wrong.
    """
    out: list[list[str]] = [[]]

    def emit(ch: str) -> None:
        if ch == "\n":
            out.append([])
        else:
            out[-1].append(ch)

    i = 0
    n = len(text)
    while i < n:
        ch = text[i]

        # Raw string: r"..." / r#"..."# / br#"..."#
        m = re.match(r'(?:b?r)(#*)"', text[i:])
        if m and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")):
            hashes = m.group(1)
            terminator = '"' + hashes
            end = text.find(terminator, i + m.end())
            end = n if end == -1 else end + len(terminator)
            for c in text[i:end]:
                emit(" " if c != "\n" else "\n")
            i = end
            continue

        if ch == '"':
            i += 1
            emit(" ")
            while i < n:
                if text[i] == "\\":
                    emit(" ")
                    i += 1
                    if i < n:
                        emit(" " if text[i] != "\n" else "\n")
                        i += 1
                    continue
                if text[i] == '"':
                    emit(" ")
                    i += 1
                    break
                emit(" " if text[i] != "\n" else "\n")
                i += 1
            continue

        if ch == "'":
            # Char literal, but `'a` is a lifetime. A char literal is at most
            # `'\u{1F600}'`, so bound the lookahead.
            m = re.match(r"'(?:\\.|\\u\{[0-9a-fA-F]+\}|[^\\'])'", text[i:])
            if m:
                for _ in range(m.end()):
                    emit(" ")
                i += m.end()
                continue
            emit(ch)
            i += 1
            continue

        if text.startswith("//", i):
            while i < n and text[i] != "\n":
                emit(" ")
                i += 1
            continue

        if text.startswith("/*", i):
            depth = 0
            while i < n:
                if text.startswith("/*", i):
                    depth += 1
                    emit(" ")
                    emit(" ")
                    i += 2
                    continue
                if text.startswith("*/", i):
                    depth -= 1
                    emit(" ")
                    emit(" ")
                    i += 2
                    if depth == 0:
                        break
                    continue
                emit(text[i] if text[i] == "\n" else " ")
                i += 1
            continue

        emit(ch)
        i += 1

    return ["".join(line) for line in out]


def production_lines(path: Path) -> int:
    text = path.read_text(encoding="utf-8", errors="replace")

    # Most files have no test code at all, and the literal-stripping scan below
    # is by far the expensive part of this check. Skipping it when the marker is
    # absent cannot hide anything: the fast path only fires when there is no
    # `#[cfg(` in the file to find.
    if "#[cfg(" not in text:
        return len(text.splitlines())

    code = strip_literals(text)
    # `split("\n")` on a trailing-newline file yields a phantom final element;
    # drop it so the count matches `wc -l`.
    if code and code[-1] == "":
        code.pop()

    excluded: set[int] = set()
    i = 0
    total = len(code)
    while i < total:
        if not CFG_TEST_RE.search(code[i].strip()):
            i += 1
            continue

        depth = 0
        opened = False
        j = i
        while j < total:
            line = code[j]
            depth += line.count("{") - line.count("}")
            if "{" in line:
                opened = True
            if opened and depth <= 0:
                break
            if not opened and ";" in line and (j > i or ";" in line.split("]", 1)[-1]):
                break
            j += 1
        excluded.update(range(i, min(j, total - 1) + 1))
        i = j + 1

    return total - len(excluded)


def load_exemptions() -> dict[str, str]:
    exemptions: dict[str, str] = {}
    if not EXEMPT_FILE.exists():
        return exemptions
    for lineno, line in enumerate(
        EXEMPT_FILE.read_text(encoding="utf-8").splitlines(), start=1
    ):
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if ":" not in stripped:
            print(
                f"{EXEMPT_FILE.name}:{lineno}: entry needs a `path: reason`, got {stripped!r}",
                file=sys.stderr,
            )
            sys.exit(2)
        path, reason = stripped.split(":", 1)
        reason = reason.strip()
        if not reason:
            print(
                f"{EXEMPT_FILE.name}:{lineno}: exemption for {path.strip()} has no reason",
                file=sys.stderr,
            )
            sys.exit(2)
        exemptions[path.strip()] = reason
    return exemptions


def main() -> int:
    exemptions = load_exemptions()

    violations: list[tuple[str, int, str]] = []
    warnings: list[tuple[str, int]] = []
    exempt_hits: set[str] = set()

    # Prune rather than filter: `target/` holds tens of thousands of vendored
    # .rs files and walking it costs seconds on every run, locally and in CI.
    files = []
    for dirpath, dirnames, filenames in os.walk(REPO_ROOT):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
        for filename in filenames:
            if not filename.endswith(".rs"):
                continue
            path = Path(dirpath) / filename
            rel = path.relative_to(REPO_ROOT)
            if is_test_path(rel):
                continue
            files.append((rel, path))
    files.sort()

    for rel, path in files:
        count = production_lines(path)
        key = rel.as_posix()
        if count > FAIL_THRESHOLD:
            if key in exemptions:
                exempt_hits.add(key)
                print(f"exempt  {key}: {count} lines ({exemptions[key]})")
            else:
                violations.append((key, count, ""))
        elif count > WARN_THRESHOLD:
            warnings.append((key, count))
            if key in exemptions:
                exempt_hits.add(key)
        elif key in exemptions:
            exempt_hits.add(key)

    if warnings:
        print(
            f"\n{len(warnings)} file(s) over the {WARN_THRESHOLD}-line target "
            f"(warning only, largest first):"
        )
        for key, count in sorted(warnings, key=lambda item: -item[1])[:10]:
            print(f"  {key}: {count} lines")
        if len(warnings) > 10:
            print(f"  ... and {len(warnings) - 10} more")

    stale = sorted(set(exemptions) - exempt_hits)
    if stale:
        print("\nExemptions that are no longer needed (remove them):")
        for key in stale:
            print(f"  {key}")

    if violations:
        print(
            f"\nFAIL: {len(violations)} production file(s) exceed the "
            f"{FAIL_THRESHOLD}-line refactor threshold from CLAUDE.md:"
        )
        for key, count, _ in sorted(violations, key=lambda item: -item[1]):
            print(f"  {key}: {count} production lines (limit {FAIL_THRESHOLD})")
        print(
            "\nSplit the file, or add it to .line-count-exempt with a reason "
            "if it is a generated table or has no natural seam."
        )
        return 1

    print(
        f"\nOK: {len(files)} production file(s) checked, none over "
        f"{FAIL_THRESHOLD} lines."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
