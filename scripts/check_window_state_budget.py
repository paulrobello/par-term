#!/usr/bin/env python3
"""Fail the build when the WindowState hub grows past its frozen budget.

`WindowState` (`src/app/window_state/mod.rs`) is the app layer's hub type.
AUDIT.md ARC-001 (audit 2026-10-30) measured its `impl WindowState` surface
growing 37% in three months (89 to 122 raw grep hits) while a remediation item
asking for decomposition sat open. Awareness did not stop the growth, so this gate
enforces a budget instead.

Two numbers are frozen:

- IMPL_BLOCK_CEILING: inherent `impl WindowState { ... }` blocks anywhere under
  `src/`, including path-qualified headers such as
  `impl crate::app::window_state::WindowState {`. Trait impls (`impl Drop for
  WindowState`) are not counted: the trait bounds what they can add.
- FIELD_CEILING: fields declared directly on `struct WindowState`. Fields of
  the extracted sub-structs (`EguiState`, `FocusState`, `ShaderState`, ...) are
  deliberately NOT capped -- they are where new state is supposed to go.

Both ceilings were frozen at the counts measured on 2026-10-30 with zero
headroom: any headroom is permitted hub growth, which is what ARC-001 exists to
stop. The ceilings only move down. When a refactor drops a count below its
ceiling, the gate reports "lower the ceiling" and still passes, so fixing the
hub never turns the build red; lower the constant in the same commit.

Interaction with `make check-line-counts`: splitting an 800-line
`impl WindowState` file into two files adds a block and costs budget even
though the hub did not grow. That pressure is intended. Move the methods onto
a sub-struct behind the facade instead of opening another `impl WindowState`.

Counting is textual over comment- and string-stripped source, so a scratch
file under `src/` counts even if no module declares it.
"""

from __future__ import annotations

import os
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from check_line_counts import strip_literals  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parent.parent
SRC_DIR = REPO_ROOT / "src"
HUB_FILE = SRC_DIR / "app" / "window_state" / "mod.rs"

# Frozen 2026-10-30 (AUDIT.md ARC-001). Lower these as the hub shrinks; never raise them.
IMPL_BLOCK_CEILING = 117
FIELD_CEILING = 48

GOOD_EXAMPLES = (
    "src/app/window_state/frame_state.rs (a sub-struct that owns its fields and methods)",
    "src/app/window_state/search_highlight.rs (free functions over a params struct)",
)

INHERENT_IMPL_RE = re.compile(
    r"\bimpl\s*(?:<[^>{]*>)?\s+(?:[A-Za-z_][A-Za-z0-9_]*\s*::\s*)*WindowState\s*\{"
)
STRUCT_RE = re.compile(r"\bstruct\s+WindowState\b[^{;]*\{")
FIELD_RE = re.compile(r"^\s*(?:pub(?:\s*\([^)]*\))?\s+)?([a-z_][a-z0-9_]*)\s*:(?!:)")


def stripped(path: Path) -> str:
    return "\n".join(strip_literals(path.read_text(encoding="utf-8", errors="replace")))


def count_impl_blocks() -> dict[str, int]:
    per_file: dict[str, int] = {}
    for dirpath, dirnames, filenames in os.walk(SRC_DIR):
        dirnames.sort()
        for filename in sorted(filenames):
            if not filename.endswith(".rs"):
                continue
            path = Path(dirpath) / filename
            text = path.read_text(encoding="utf-8", errors="replace")
            if "WindowState" not in text:
                continue
            hits = len(INHERENT_IMPL_RE.findall(stripped(path)))
            if hits:
                per_file[path.relative_to(REPO_ROOT).as_posix()] = hits
    return per_file


def count_hub_fields() -> int:
    code = stripped(HUB_FILE)
    m = STRUCT_RE.search(code)
    if not m:
        print(f"FAIL: `struct WindowState` not found in {HUB_FILE.relative_to(REPO_ROOT)}")
        sys.exit(2)
    depth = 1
    fields = 0
    for line in code[m.end():].split("\n"):
        if depth == 1 and FIELD_RE.match(line):
            fields += 1
        depth += line.count("{") - line.count("}")
        if depth <= 0:
            break
    return fields


def main() -> int:
    per_file = count_impl_blocks()
    impl_blocks = sum(per_file.values())
    fields = count_hub_fields()

    print(
        f"WindowState impl blocks: {impl_blocks} across {len(per_file)} file(s) "
        f"(ceiling {IMPL_BLOCK_CEILING})"
    )
    print(f"WindowState fields:      {fields} (ceiling {FIELD_CEILING})")

    failures: list[str] = []
    if impl_blocks > IMPL_BLOCK_CEILING:
        failures.append(
            f"{impl_blocks} inherent `impl WindowState` blocks exceed the ceiling of "
            f"{IMPL_BLOCK_CEILING} (+{impl_blocks - IMPL_BLOCK_CEILING})."
        )
    if fields > FIELD_CEILING:
        failures.append(
            f"{fields} fields on `struct WindowState` exceed the ceiling of "
            f"{FIELD_CEILING} (+{fields - FIELD_CEILING})."
        )

    for name, measured, ceiling in (
        ("IMPL_BLOCK_CEILING", impl_blocks, IMPL_BLOCK_CEILING),
        ("FIELD_CEILING", fields, FIELD_CEILING),
    ):
        if measured < ceiling:
            print(
                f"note: {name} can drop to {measured}; lower it in "
                f"scripts/check_window_state_budget.py to lock in the gain."
            )

    if failures:
        print("\nFAIL: the WindowState hub budget (AUDIT.md ARC-001) is exceeded:")
        for failure in failures:
            print(f"  {failure}")
        print("\nFiles with the most `impl WindowState` blocks:")
        for key, count in sorted(per_file.items(), key=lambda item: (-item[1], item[0]))[:8]:
            print(f"  {key}: {count}")
        print(
            "\nDo not add state or behavior to WindowState directly. Put it in a "
            "sub-struct in src/app/window_state/ that owns its fields and methods, "
            "hold it as one WindowState field, and call it through that field. "
            f"Examples: {'; '.join(GOOD_EXAMPLES)}. Lifecycle rules are in "
            "docs/architecture/STATE_LIFECYCLE.md. A file split for "
            "check-line-counts must move methods onto a sub-struct rather than "
            "open another `impl WindowState`. Never raise the ceilings."
        )
        return 1

    print("\nOK: WindowState is within its frozen budget.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
