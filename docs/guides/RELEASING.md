# Releasing par-term

The checklist for cutting a par-term release. Work through it in order; every
step has an owner command.

1. **Bump the version** in the root `Cargo.toml` (`par-term` package) and run
   `cargo check --workspace` to settle `Cargo.lock`.
2. **Cut the release build**: `make release` (full LTO build, ~3 min).
3. **Tag**: `git tag vX.Y.Z` (matching the new version), push the tag.
4. **Bump the installer pin (SEC-004 owner step)**: the pipe-to-shell installer
   `gh-pages/install-shell-integration.sh` downloads from a pinned release tag
   so it serves fixed reviewed content. After the new tag exists:
   `make bump-installer-ref REF=vX.Y.Z`, then commit the pin bump. `make ci`
   (and therefore `make checkall-ci`) fails on the next run after a release
   until this step runs — the gate owns the reminder.
5. **Verify the release CI** passes for the tag.
6. **Publish the installer**: the `gh-pages/` directory is what
   paulrobello.github.io/par-term serves; the pin bump commit must reach the
   published site for users to get the new pin.
7. **Announce** per project practice.
