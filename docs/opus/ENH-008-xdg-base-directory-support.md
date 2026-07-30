# ENH-008 — Honor `XDG_CONFIG_HOME` (and decide about the other XDG variables)

> **Impact**: medium · **Effort**: medium · **Source**: AUDIT.md **DOC-005** (High) — the *behavior-change*
> branch of that finding
>
> ⚠️ **This is a product decision, not a cleanup.** It changes where existing users' configs are found. Confirm
> the decision before implementing.

## Goal

Make par-term actually follow the XDG Base Directory specification its documentation already claims it follows —
or, if that is rejected, close the item deliberately rather than leaving it open.

## Current State

`docs/guides/ENVIRONMENT_VARIABLES.md:88` states par-term "follows the XDG Base Directory specification for
configuration and data storage on Linux and macOS", and `:92-96` documents five variables:
`XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_STATE_HOME`, `XDG_CACHE_HOME`, `XDG_RUNTIME_DIR`.

**None of the five is read for path resolution.** `par-term-config/src/config/persistence.rs:186-190` hardcodes:

```rust
dirs::home_dir().join(".config").join("par-term").join("config.yaml")
```

The only `XDG_CONFIG_HOME` reads anywhere in the workspace are:
- `par-term-config/src/config/env_vars.rs:45` — an entry in the 30-variable pass-through **allowlist** (so the
  variable is forwarded to child processes), not path resolution.
- `src/shell_integration_installer.rs:248` — locating shell RC files only.

`XDG_DATA_HOME`, `XDG_STATE_HOME`, `XDG_CACHE_HOME`, and `XDG_RUNTIME_DIR` are read **nowhere**.

The comment at `persistence.rs:185` reveals exactly how this drifted: *"Use XDG convention on all platforms"* —
par-term follows the default **path** (`~/.config/par-term/`) but not the **specification** (which says to honor
the variable when set). Someone wrote the path correctly and the docs then over-claimed.

**Real impact**: on Linux and in dotfile-managed setups a non-default `XDG_CONFIG_HOME` is common. Those users
edit a config par-term never reads and have no way to discover why their settings do nothing.

**Relationship to DOC-005**: that finding fixes the *documentation* to describe reality, which is the honest
short-term move and should land in the remediation cycle. This item is the other branch. **If this item is
implemented, DOC-005's doc change must be re-reverted** — coordinate, and do not run them concurrently.

Relevant recent context: commit `a7089017` added `src/config_migration.rs` with `migrate_legacy_config_dir` for a
one-time macOS config-directory move, and `docs/guides/MIGRATION.md` has an `Unreleased` entry for it. So the
repo already has both the machinery and the documentation pattern this change needs.

## The Decision to Make First

Three defensible options. Pick one explicitly:

| Option | Behavior | Cost |
|---|---|---|
| **A. Honor `XDG_CONFIG_HOME` only** | Config path becomes `${XDG_CONFIG_HOME:-~/.config}/par-term/`. Other variables stay undocumented/unused. | Low. Covers the actual user complaint. **Recommended.** |
| **B. Full XDG compliance** | Config, data (sessions/profiles/arrangements), state (logs/history), and cache each move to their proper root. | High. Relocates five categories of user data; needs migration for each. |
| **C. Reject** | Keep fixed `~/.config/par-term/`. DOC-005's doc fix is the whole resolution; close this item. | Zero. Legitimate — a fixed path is simpler and macOS users do not expect XDG. |

**Recommendation: A.** It resolves the real failure (a user's `XDG_CONFIG_HOME` being ignored) at a fraction of
B's risk. B relocates session, profile, arrangement, log, and history files — five migrations, each able to lose
user data — for a spec-purity benefit few users will notice. If B is chosen, stage it one category per release.

The rest of this plan assumes **A**, with notes for B.

## Implementation (Option A)

### Step 1 — Centralize the path resolution

`persistence.rs:186-190` is not the only place a config-adjacent path is built. Find them all first:

```bash
grep -rn 'join(".config")\|home_dir()' --include='*.rs' src/ par-term-*/src | grep -v test
```

Add one resolver in `par-term-config/src/config/persistence.rs`:

```rust
fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())          // spec: relative values MUST be ignored
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".config"))
        .join("par-term")
}
```

The `is_absolute` filter is required by the specification and is the most commonly missed detail — a relative
value must be treated as unset, not joined.

Route every discovered site through it. Note the shader directory (`~/.config/par-term/shaders/`) and anything
else under the config root must follow, or shaders silently stop loading for XDG users.

### Step 2 — Decide the platform scope

The spec is a freedesktop (Linux) standard. macOS convention is `~/Library/Application Support/`, but par-term
already deliberately uses `~/.config/par-term/` on macOS — commit `a7089017` *migrated to* that path on purpose.

**Recommendation**: honor `XDG_CONFIG_HOME` on **both** Linux and macOS when set, keeping `~/.config/par-term/`
as the default on both. That preserves `a7089017`'s intent while respecting an explicit user override, and it
matches what the docs already promise. Windows is unaffected (`%APPDATA%\par-term\`).

### Step 3 — Handle the migration case

A user who currently has `~/.config/par-term/config.yaml` **and** sets `XDG_CONFIG_HOME` would, after this change,
suddenly get an empty config — a silent regression that looks like data loss.

Reuse the existing pattern in `src/config_migration.rs`:

1. If the resolved XDG path has no `config.yaml` but `~/.config/par-term/config.yaml` exists, either migrate it
   (one-time copy, matching `migrate_legacy_config_dir`'s approach) or load from the legacy path and log a clear
   one-time warning naming both paths.
2. **Prefer the warning over an automatic move** here: unlike `a7089017`'s case, both paths are legitimate, and
   silently relocating a user's config out from under their dotfile manager is worse than telling them.
3. Never write to the new location until the user acts, so the operation is reversible.

### Step 4 — Update the documentation to match the new reality

`docs/guides/ENVIRONMENT_VARIABLES.md:88-96` — describe what is now true: `XDG_CONFIG_HOME` is honored when set
to an absolute path; the other four are **not** used (Option A) and should be **removed from the doc**, not left
documented-but-inert, which is the original defect. Add the migration note to `docs/guides/MIGRATION.md`.

Also update `docs/CONFIG_REFERENCE.md` and `CLAUDE.md`'s Configuration section, both of which state the config
location.

### Step 5 — Tests

Add to `par-term-config/tests/`:
- `XDG_CONFIG_HOME` set absolute → resolver returns `$XDG_CONFIG_HOME/par-term`
- set relative → **ignored**, falls back to `~/.config/par-term`
- set empty → treated as unset
- unset → `~/.config/par-term`
- legacy config present + XDG set → migration/warning path fires

⚠️ **These tests mutate the environment.** `par-term-config`'s sibling tests had exactly this problem —
`env::set_var` racing `getenv` (AUDIT.md **SEC-007**), and commit `979ecd11` fixed the config half by resolving
substitution through a **lookup instead of the environment**. Follow that precedent: make the resolver accept an
injected lookup (`impl Fn(&str) -> Option<OsString>`) so tests need no `set_var` at all. Do not add new
`env::set_var` calls to this crate.

## Files to Touch

| File | Change |
|---|---|
| `par-term-config/src/config/persistence.rs:185-190` | the resolver; injected lookup for testability |
| `src/config_migration.rs` | legacy-path detection + warning |
| any other `join(".config")` sites found in Step 1 | route through the resolver |
| `par-term-config/tests/xdg_paths.rs` | **new** — resolver tests, no `set_var` |
| `docs/guides/ENVIRONMENT_VARIABLES.md:88-96` | describe real behavior; remove the four unused variables |
| `docs/guides/MIGRATION.md` | migration note |
| `docs/CONFIG_REFERENCE.md`, `CLAUDE.md` | config-location statements |

## Verification

```bash
cargo test -p par-term-config
make checkall
```

End-to-end, which is what actually proves it:

```bash
mkdir -p /tmp/xdg/par-term && cp ~/.config/par-term/config.yaml /tmp/xdg/par-term/
XDG_CONFIG_HOME=/tmp/xdg ./target/dev-release/par-term --dump-config --exit-after 3   # reads /tmp/xdg
XDG_CONFIG_HOME=relative/path ./target/dev-release/par-term --dump-config --exit-after 3  # ignored → ~/.config
./target/dev-release/par-term --dump-config --exit-after 3                            # unchanged default
```

Confirm specifically: shaders in `$XDG_CONFIG_HOME/par-term/shaders/` load; an existing `~/.config` user with
`XDG_CONFIG_HOME` set sees the warning rather than an empty config; Windows behavior is untouched.

## Rollback

Single-commit revert of the resolver restores the hardcoded path. Because Step 3 never writes to the new location
unprompted, no user data has moved and rollback is clean.

Risks:

1. **Silent config loss** for existing users who set `XDG_CONFIG_HOME` — the reason Step 3 exists and why the
   warning is preferred over an automatic move. This is the main hazard.
2. **Partial adoption** — honoring the variable for `config.yaml` but not for shaders, sessions, or logs, so a
   user's data splits across two trees. Step 1's exhaustive grep is what prevents it; do not skip it.
3. **DOC-005 conflict** — if DOC-005's doc fix has landed saying "does not consult XDG variables", this change
   makes that text wrong again. Sequence them and update the doc in the same PR as the behavior change.
4. **New `env::set_var` in tests** would reintroduce SEC-007's UB. Use the injected lookup.
