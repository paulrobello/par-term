---
description: Bump version, update docs, sanity check and deploy project
---
ensure project is using latest published version of core library
run 'make pre-commit' fix all issues
bump version update changelog and readme

**IMPORTANT: Subcrate Version Bumping**
Before committing, check if any workspace subcrates have changes since their last published version:
1. Run `git log --oneline <last-tag>..HEAD -- par-term-*/` to find changes in each subcrate
2. For each subcrate with changes:
   - Bump the version in the subcrate's `Cargo.toml` (patch for bug fixes, minor for features)
   - Update all crates that depend on it to reference the new version
3. Common dependency chains to update:
   - `par-term-config` → all other crates depend on it
   - `par-term-fonts` → `par-term-render`, `par-term-settings-ui`
   - `par-term-terminal` → `par-term-tmux`
4. Run `cargo check` to verify all version references are correct

This ensures crates.io publishes have the correct versions with all changes, preventing build failures like missing type exports.

use docs/DOCUMENTATION_STYLE_GUIDE.md to update all docs/ and or create new docs for all the changes since last release
commit and push all changes
run 'make deploy' to trigger cicd deployment
