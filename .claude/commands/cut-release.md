---
description: Bump version, update docs, sanity check and deploy project
---
ensure project is using latest published version of core library
run 'make pre-commit' fix all issues
bump version update changelog and readme

**IMPORTANT: Subcrate Version Bumping**
Before committing, check if any workspace subcrates have changes since their last published version:
1. Run `git log --oneline <last-tag>..HEAD -- par-term-*/` to find changes in each subcrate
2. For each subcrate with changes, bump in dependency order:

```
Layer 0 — No internal deps (bump in any order):
  par-term-acp
  par-term-ssh
  par-term-mcp

Layer 1 — Foundation (bump before anything that depends on it):
  par-term-config
    └── depends on: (none, only external par-term-emu-core-rust)

Layer 2 — Depend on par-term-config only (bump after Layer 1):
  par-term-fonts        → par-term-config
  par-term-input        → par-term-config
  par-term-keybindings  → par-term-config
  par-term-scripting    → par-term-config
  par-term-settings-ui  → par-term-config
  par-term-terminal     → par-term-config
  par-term-tmux         → par-term-config
  par-term-update       → par-term-config

Layer 3 — Depend on Layer 2 crates (bump after Layer 2):
  par-term-render       → par-term-config, par-term-fonts

Layer 4 — Root crate (bump last):
  par-term              → all of the above
```

3. For each bumped subcrate:
   - Bump the version in the subcrate's `Cargo.toml` (patch for bug fixes, minor for features)
   - Update all crates that depend on it to reference the new version
4. Run `cargo check` to verify all version references are correct

This ensures crates.io publishes have the correct versions with all changes, preventing build failures like missing type exports.

use docs/DOCUMENTATION_STYLE_GUIDE.md to update all docs/ and or create new docs for all the changes since last release
commit and push all changes
run 'make deploy' to trigger cicd deployment
