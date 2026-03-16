---
description: Bump version, update docs, sanity check and deploy project
---

- Ensure project is using latest published version of core library
- Run 'make pre-commit' fix all issues
- Bump version update changelog and readme

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
  par-term-prettifier   → par-term-config
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
   - Bump the `version` field in the subcrate's own `Cargo.toml` (patch for bug fixes, minor for features)
   - Update all crates that depend on it to reference the new version in their `[dependencies]` section

4. **Workspace Dependencies**:
   The root `Cargo.toml` has a `[workspace.dependencies]` table that centralizes all shared external dependency versions. When upgrading external deps:
   - Update the version ONLY in the root `[workspace.dependencies]` table — subcrates inherit via `dep.workspace = true`
   - Do NOT edit individual subcrate `Cargo.toml` files for external dep version bumps
   - If a subcrate needs an extra feature on a workspace dep, it uses `dep = { workspace = true, features = ["extra"] }`
   - Internal workspace crate references (par-term-config, par-term-fonts, etc.) are still path deps with explicit version fields — these DO need updating per step 3

5. Run `cargo check --workspace` to verify all version references are correct

This ensures crates.io publishes have the correct versions with all changes, preventing build failures like missing type exports.

Use docs/DOCUMENTATION_STYLE_GUIDE.md to update all docs/ and or create new docs for all the changes since last release. Cleanup and summarize changelog for this releaswe.

commit and push all changes

- Run 'make deploy' to trigger cicd deployment
- Monitor cicd every 5 minutes for issues and fix any found and re-trigger deploy

## Final Release Summary

Once the CI/CD run completes successfully, output a release summary in this exact format:

**vX.Y.Z release complete.** Summary:

| Job | Result |
|-----|--------|
| Preflight Checks | ✓ |
| Publish to crates.io | ✓ |
| Build — Linux x86_64 | ✓ |
| Build — Linux ARM64 | ✓ |
| Build — macOS x86_64 | ✓ |
| Build — macOS ARM64 | ✓ |
| Build — Windows x86_64 | ✓ |
| Create GitHub Release | ✓ |
| Publish Homebrew Cask | ✓ |

**What shipped:**
- Bullet-point list of all Added features from the [Unreleased] changelog section
- Bullet-point list of all Fixed items from the [Unreleased] changelog section

Use the CHANGELOG.md [Unreleased] entries (before they were moved to the release section) as the source for "What shipped". Keep each bullet concise — one line per feature/fix.
