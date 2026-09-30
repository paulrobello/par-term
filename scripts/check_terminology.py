#!/usr/bin/env python3
"""Fail when user-facing text breaks the UX.md T1-T8 vocabulary.

The vocabulary (UX.md section 2.2, recorded in docs/DOCUMENTATION_STYLE_GUIDE.md):

- Window = an OS window. Tab = a tab in a window's tab bar (a par-mux daemon
  window is shown as a tab). Pane = a split region inside a tab.
- Session = ONLY a par-mux or tmux session. The other old meanings are renamed:
  the saved set of windows is "windows restored on launch", the closed-tab undo
  stack is "reopen closed tab", "session logging" is "output recording", and a
  running shell is a "shell" or a "tab".
- par-mux paths never say "tmux" (T8).

What is checked:

- **Rust** string literals in `src/` and `par-term-settings-ui/src/` that
  reach the user. Comments, `log::`/`debug_*!`/`tracing` lines, `#[cfg(test)]`
  items and test files are skipped, and so are identifiers such as config keys,
  action ids and `\\(session.x)` badge variables: T4 renames labels, and code
  identifiers migrate later.
- **Docs**: README.md, top-level `docs/*.md`, `docs/features/`, `docs/guides/`.
  Code spans, fenced blocks and link targets are skipped, so config keys and
  file names stay stable. Design notes (`plans/`, `superpowers/`, `research/`,
  `opus/`, `architecture/`), CHANGELOG, UX.md and AUDIT files are history, not
  user text, and are not checked.
- **par-mux-only sources** (`PAR_MUX_ONLY`) must not contain the word "tmux"
  in any user-facing literal. Sources shared with the tmux gateway pick their
  wording at runtime (`is_mux_attached()`), which a file-level scan cannot see;
  they are listed in `docs/DOCUMENTATION_STYLE_GUIDE.md` as a checklist.

Exceptions live in ALLOW below, each with a reason. An exception that no
longer matches anything is reported (not failed), so fixing the text never
turns the gate red.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

# Phrases that use "session" in a non-T4 meaning. Case-insensitive, matched
# on word boundaries against user-visible text only.
BANNED = [
    r"session[ -](?:restore|restoration|undo|logging|logs?|recording|preservation)",
    r"open (?:terminal )?sessions",
    r"session (?:state|persistence|file)",
    r"session management",
    r"copy of the session",
    r"the session was recovered",
    r"archive session",
    r"session and system information",
    r"(?:logging|recording) for all sessions",
    r"restore (?:the )?(?:previous )?session",
    r"active sessions?",
    r"shell sessions?",
    r"terminal sessions?",
    r"session ended:",
    r"session preserved",
    r"last session",
    r"when a session starts",
    r"for this session",
    r"recovered session",
    r"sessions will be terminated",
]
BANNED_RE = re.compile(r"\b(?:" + "|".join(BANNED) + r")(?!\w)", re.IGNORECASE)
TMUX_RE = re.compile(r"\btmux\b", re.IGNORECASE)

RUST_ROOTS = ["src", "par-term-settings-ui/src"]
DOC_FILES = ["README.md"]
DOC_GLOBS = ["docs/*.md", "docs/features/*.md", "docs/guides/*.md"]

# Sources that only ever run on a par-mux path (T8).
PAR_MUX_ONLY = [
    "src/mux_last_tab_ui.rs",
    "src/app/tmux_handler/notifications/mux.rs",
    "src/app/tmux_handler/notifications/mux_attach.rs",
    "src/app/tmux_handler/notifications/mux_drain.rs",
    "src/app/tmux_handler/notifications/mux_pane_exit.rs",
    "src/app/tmux_handler/notifications/mux_pane_moves.rs",
]

# (path, substring of the offending line, reason). Keep this short: every
# entry is a place the vocabulary is knowingly not applied.
ALLOW: list[tuple[str, str, str]] = [
    (
        "docs/DOCUMENTATION_STYLE_GUIDE.md",
        "",
        "the vocabulary table quotes the retired phrases on purpose",
    ),
    (
        "docs/guides/MIGRATION.md",
        "",
        "upgrade notes name the old labels so users can find the new ones",
    ),
    (
        "par-term-settings-ui/src/advanced_tab/tmux.rs",
        "tmux control mode for session management",
        "T4 meaning: tmux sessions are managed here",
    ),
    (
        "src/app/window_state/action_handlers/inspector.rs",
        "reset session state",
        "an ACP agent session, not a par-term tab or window",
    ),
    (
        "docs/API.md",
        "tmux session state",
        "T4 meaning: the tmux sync type's own description",
    ),
    (
        "docs/LOGGING.md",
        "session management",
        "T4 meaning: the TMUX log category covers tmux session management",
    ),
    (
        "docs/features/BADGES.md",
        "badge and session management",
        "iTerm2's own name for its OSC 1337 session commands",
    ),
]

SKIP_LINE_RE = re.compile(
    r"^\s*(?://|\*|/\*)|\b(?:log|tracing)::\w+!|\bdebug_(?:error|info|log|trace)!|#\[|assert"
)
STRING_RE = re.compile(r'"((?:[^"\\]|\\.)*)"')
# A literal that is a bare identifier (config key, action id, variable name).
IDENT_RE = re.compile(r"^[a-z0-9_.:\-/\\()]*$")


def is_test_path(rel: Path) -> bool:
    name = rel.name
    return (
        name == "tests.rs"
        or name.endswith("_tests.rs")
        or name.endswith("_test.rs")
        or "tests" in rel.parts[:-1]
    )


def production_lines(text: str) -> list[tuple[int, str]]:
    """Lines outside `#[cfg(test)]` items, with 1-based numbers."""
    lines = text.splitlines()
    out: list[tuple[int, str]] = []
    skip_depth: int | None = None
    depth = 0
    pending_test = False
    for i, line in enumerate(lines, 1):
        stripped = line.strip()
        if skip_depth is None and re.match(r"#\[cfg\((?:all\()?test", stripped):
            pending_test = True
            continue
        opens = line.count("{")
        closes = line.count("}")
        if pending_test and skip_depth is None:
            if opens:
                skip_depth = depth
                pending_test = False
            elif stripped.endswith(";"):
                pending_test = False
                continue
        depth += opens - closes
        if skip_depth is not None:
            if depth <= skip_depth:
                skip_depth = None
            continue
        out.append((i, line))
    return out


# Calls whose string arguments are never shown as UI text: logging macros,
# and search-only terms. A settings section's search keywords (the `&[..]`
# argument of `keyword_section(ui, title, id, &[..], ..)`) and a control's
# `.search_tag(&[..])` match what the user types, so old wording there
# deliberately still finds the setting; the section title in the same call
# is shown and stays checked.
SKIP_CALL_RE = re.compile(
    r"\b(?:log|tracing)::\w+!\s*\(|\b(?:crate::)?debug_(?:error|info|log|trace)!\s*\("
    r"|\.search_tag\s*\("
)
SEARCH_SECTION_RE = re.compile(r"\bkeyword_section(?:_with_state)?\s*\(")


def without_log_calls(lines: list[tuple[int, str]]) -> list[tuple[int, str]]:
    """Drop every line of a skipped call (possibly multi-line) and of every
    search-keyword slice."""
    out: list[tuple[int, str]] = []
    paren = 0
    in_section_call = False
    bracket = 0
    for lineno, line in lines:
        if paren > 0:
            paren += line.count("(") - line.count(")")
            continue
        if bracket > 0:
            bracket += line.count("[") - line.count("]")
            continue
        if SEARCH_SECTION_RE.search(line):
            in_section_call = True
        if in_section_call and "&[" in line:
            # The keyword slice: drop it (possibly multi-line), keep the
            # rest of the call, and stop looking for this call's slice.
            in_section_call = False
            head = line[: line.index("&[")]
            tail = line[line.index("&["):]
            bracket = max(tail.count("[") - tail.count("]"), 0)
            if STRING_RE.search(head):
                out.append((lineno, head))
            continue
        m = SKIP_CALL_RE.search(line)
        if m:
            tail = line[m.start():]
            paren = max(tail.count("(") - tail.count(")"), 0)
            head = line[: m.start()]
            if STRING_RE.search(head):
                out.append((lineno, head))
            continue
        out.append((lineno, line))
    return out


def allowed(rel: str, line: str) -> bool:
    return any(rel == path and sub in line for path, sub, _ in ALLOW)


def check_rust(violations: list[str], used: set[int]) -> None:
    for root in RUST_ROOTS:
        for path in sorted((REPO_ROOT / root).rglob("*.rs")):
            rel_path = path.relative_to(REPO_ROOT)
            if is_test_path(rel_path):
                continue
            rel = rel_path.as_posix()
            mux_only = rel in PAR_MUX_ONLY
            text = path.read_text(encoding="utf-8")
            for lineno, line in without_log_calls(production_lines(text)):
                if SKIP_LINE_RE.search(line):
                    continue
                for literal in STRING_RE.findall(line):
                    if IDENT_RE.match(literal):
                        continue
                    visible = re.sub(r"\\\(session\.[a-z_]+\)", "", literal)
                    hit = BANNED_RE.search(visible)
                    if mux_only and not hit:
                        hit = TMUX_RE.search(visible)
                    if not hit:
                        continue
                    idx = next(
                        (n for n, (p, s, _) in enumerate(ALLOW) if p == rel and s in line),
                        None,
                    )
                    if idx is not None:
                        used.add(idx)
                        continue
                    violations.append(f"{rel}:{lineno}: \"{hit.group(0)}\" in {literal!r}")


def doc_text(line: str) -> str:
    line = re.sub(r"`[^`]*`", "", line)
    line = re.sub(r"\]\([^)]*\)", "]", line)
    line = re.sub(r"<[^>]+>", "", line)
    return line


def check_docs(violations: list[str], used: set[int]) -> None:
    paths = [REPO_ROOT / f for f in DOC_FILES]
    for pattern in DOC_GLOBS:
        paths.extend(sorted(REPO_ROOT.glob(pattern)))
    for path in paths:
        rel = path.relative_to(REPO_ROOT).as_posix()
        in_fence = False
        for lineno, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if line.lstrip().startswith("```"):
                in_fence = not in_fence
                continue
            if in_fence:
                continue
            # Anchors in a table of contents mirror headings already checked.
            hit = BANNED_RE.search(doc_text(line))
            if not hit:
                continue
            idx = next(
                (n for n, (p, s, _) in enumerate(ALLOW) if p == rel and s in line),
                None,
            )
            if idx is not None:
                used.add(idx)
                continue
            violations.append(f"{rel}:{lineno}: \"{hit.group(0)}\"")


def main() -> int:
    violations: list[str] = []
    used: set[int] = set()
    check_rust(violations, used)
    check_docs(violations, used)
    for n, (path, sub, reason) in enumerate(ALLOW):
        if n not in used:
            print(f"note: unused terminology exception {path} {sub!r} ({reason})")
    if violations:
        print(f"{len(violations)} terminology violation(s) (UX.md T1-T8):")
        for v in violations:
            print(f"  {v}")
        print(
            "\n'Session' means only a par-mux or tmux session, and par-mux paths never "
            "say 'tmux'. See docs/DOCUMENTATION_STYLE_GUIDE.md (Terminology)."
        )
        return 1
    print("OK: user-facing text follows the T1-T8 vocabulary.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
