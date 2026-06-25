# Audit Remediation Report

> **Project**: par-term (GPU-accelerated terminal emulator)
> **Audit Date**: 2026-06-25
> **Remediation Date**: 2026-06-25
> **Severity Filter Applied**: high (Critical + High, including Phase 2 architecture)
> **Branch**: `fix/audit-remediation-2026-06-25`
> **Commits**: `3b5d87f1` → `0d96d3b3` → `b3c05566` → `d7dff61f`

---

## Execution Summary

| Phase | Status | Agent | Targeted | Resolved | Partial | Deferred | Manual |
|-------|--------|-------|:--------:|:--------:|:-------:|:--------:|:------:|
| 1 — Critical Security | ✅ | fix-security | 2 | 2 | 0 | 0 | 0 |
| 2 — Critical Architecture | ✅ | fix-architecture | 3 | 1 | 0 | 2 | 0 |
| 3a — High Security | ✅ | fix-security | 7 | 7 | 0 | 0 | 0 |
| 3b — High Architecture | ✅ | fix-architecture | 5 | 2 | 1 | 2 | 0 |
| 3c — High Code Quality | ✅ | fix-code-quality | 5 | 2 | 1 | 2 | 0 |
| 3d — High Documentation | ✅ | fix-documentation | 6 | 4 | 1 | 0 | 1 |
| 4 — Verification | ✅ | orchestrator (+fmt/+test) | — | — | — | — | — |

**Overall**: of 28 targeted issues — **18 Resolved**, **3 Partial**, **6 Deferred (documented)**, **1 requires manual action** (DOC-003 tag push). SEC-006 was reworked to **opt-in / fail-open** so it no longer breaks ACP — see Resolved.

> **Note on scope**: The audit's auto-generated Phase 2 table labeled ARC-001/ARC-002 as "Critical Architecture (Blocking)," but the audit's *own* Remediation Roadmap files both under "Long-term (Backlog)," and ARC-002 is explicitly "blocked on a field-by-field borrow audit." These were therefore attempted pragmatically and deferred with concrete next-step plans rather than force-solved. A prior `Pragmatic Code Quality Audit Remediation` vault lesson and the `Arc<Config>` migration note informed this posture.

---

## Resolved Issues ✅

### Security
- **[SEC-001]** `write_file_safe` bypasses sensitive-path blocklist — `par-term-acp/src/fs_ops.rs:153` — added `check_path_allowed(path)?` before `create_dir_all`, mirroring the sibling read functions. A `bypassPermissions` agent can no longer overwrite `~/.ssh/authorized_keys`, `~/.aws/`, `/etc/`, etc.
- **[SEC-002]** `ssh_extra_args` SSH flag injection — `par-term-config/src/profile_types/profile.rs:465` — tokenized with `shell_words::split` and filtered by `filter_ssh_extra_args`, dropping denied flags (`-A -D -R -L -W -w`) and options (`ProxyCommand`, `LocalCommand`, `StrictHostKeyChecking`, `UserKnownHostsFile`, `ForwardAgent`, …). Field doc comment rewritten. (`shell-words` added to `par-term-config/Cargo.toml`.)
- **[SEC-003]** Screenshot fallback path exfiltration — `par-term-mcp/src/tools/screenshot.rs` — `validate_fallback_path()` canonicalizes and require the path live under the system temp dir or the par-term app-data dir.
- **[SEC-004]** Agent TOML `[env]` linker injection — `par-term-acp/src/agent.rs:314` — `is_dynamic_linker_env_key()` drops any `LD_*` / `DYLD_*` key before `envs()`.
- **[SEC-005]** MCP `config_update` arbitrary keys — `par-term-mcp/src/tools/config_update.rs` — explicit `ALLOWED_CONFIG_KEYS` allowlist (17 cosmetic/rendering keys); rejects unknown + all security-sensitive keys (incl. `bypassPermissions`).
- **[SEC-007]** SSH host leading-hyphen injection — `par-term-config/src/profile_types/profile.rs:469` — `is_safe_ssh_host()` rejects leading `-` and embedded `\n`/`\r`/`\0`; wired into `validate()` (warning) and `ssh_command_args()` (defensive `None` return).
- **[SEC-008]** Self-update installs on missing SHA256 — `par-term-update/src/binary_ops.rs:181` — `verify_download()` now returns `Err` on `None` checksum, matching the shader installer's hard-gate policy. Test updated.
- **[SEC-009]** OSC 8 URL scheme not validated — `src/url_detection/render.rs:49` — `ALLOWED_URL_SCHEMES` (`http`/`https`/`mailto`) checked before `open::that`; `file://`/`ftp://`/`data:` rejected, bare `host:port` still allowed.
- **[SEC-006]** MCP stdin authentication — `par-term-mcp/src/lib.rs` — **reworked to opt-in / fail-open**. The initial implementation auto-generated a random session token that no client knew, then rejected every `tools/list`/`tools/call` (`-32001`) — which would have broken all ACP tool calls (par-term doesn't spawn the MCP server; the agent host does, so nothing could supply the token). `resolve_auth_token()` now returns `Some` only when `PAR_TERM_MCP_AUTH_TOKEN` is explicitly set; otherwise auth is fully DISABLED and the server behaves exactly as before. The hardening remains available for operators who set the env var and configure their agent host to forward `_meta.parTermAuthToken`. 6 tests incl. the auth-disabled default.

### Architecture
- **[ARC-003]** `par-term-config` optional `wgpu` layer violation — moved the three wgpu conversion helpers (`VsyncMode`/`PowerPreference`/`ImageScalingMode`) into a new `par-term-render/src/wgpu_conversions.rs` via narrow extension traits (call-site method names unchanged); removed the optional `wgpu` dep + `wgpu-types` feature from `par-term-config` and all 3 dependent Cargo.tomls. Layer-1 → Layer-3 layering now clean.
- **[ARC-006]** `par-term-input` single 654-line file — split into `clipboard.rs` / `key_encoding.rs` / `modifiers.rs`; `lib.rs` slimmed 654→73 lines. Public API byte-identical.
- **[ARC-008]** Glyph rasterization triplicated — `par-term-render/src/renderer/render_passes.rs` pane title-bar text now routes through the shared `CellRenderer::get_or_rasterize_glyph()` helper instead of inlining the cache-check/rasterize/upload/LRU sequence. Behavior-preserving by construction; does not touch the untested `pane_render/` path (QA-013). (Audit's site map was partly stale — two sites were already migrated.)

### Code Quality
- **[QA-001]** `\(date)` incorrect date arithmetic — `par-term-config/src/snippets.rs:960` — replaced naive integer math with `chrono::Local::now().format("%Y-%m-%d")`. Regression test strengthened to assert valid `YYYY-MM-DD` (month 1-12, day 1-31). (`chrono` added to `par-term-config`.)
- **[QA-004]** Six `unreachable!()` in the render path — `src/app/render_pipeline/egui_submit.rs` — `DemoteSnapshot::ChooseDirection` destructured once; wrong-variant falls through to the existing `Idle`/`PickTab`/`PickPane` arms (graceful skip) instead of panicking mid-frame. Zero `unreachable!()` remain.

### Documentation
- **[DOC-001]** CLAUDE.md version stale — `CLAUDE.md:12` — `0.30.12` → `0.33.1`; added a `cut-release` `sed` one-liner comment so the line stays in sync.
- **[DOC-002]** Broken README TOC anchor — `README.md:19` — `#whats-new-in-03012` → `#whats-new-in-0330`; added a release-checklist TOC-anchor note to CONTRIBUTING.
- **[DOC-004]** CHANGELOG non-conforming headings — `CHANGELOG.md` — renamed 13× `### Bug Fixes` → `### Fixed`; deduplicated the `### Added`/`### Fixed` sections in `[0.32.0]` (15 bullets preserved, 0 lost); softened the "Keep a Changelog" declaration to document the historical-heading deviation honestly rather than force-normalizing ~22 entries (would collide).
- **[DOC-005]** Linux deps inconsistent — `CONTRIBUTING.md` now mirrors README's complete list (added GTK3/Wayland/ALSA + Fedora/Arch sections); README marked canonical.

---

## Partially Fixed 🔶

- **[ARC-004]** Dual logging systems — converted the 2 hottest per-frame `log::trace!` calls in the root render loop (`gpu_submit.rs:444`, `tab_snapshot.rs:176`) to `crate::debug_trace!`. The remaining ~90 root-crate + ~595 sub-crate `log::` calls are deferred to the `tracing` migration (multi-day; sub-crates correctly use `log::` per their own convention).
- **[QA-003]** `parse_shader_controls` 660-line function — extracted the 5 identical capacity-check arms into a shared `check_and_push_capacity_warning()` helper. The 10 repetitive parsing match arms were deferred (each uses `continue` 2-5× for error recovery; safe extraction needs a control-flow enum + test gate).
- **[DOC-006]** `docs/API.md` coverage — strengthened the drift warning + added a "Coverage and validation" section documenting that `par-term-settings-ui`/`par-term-render` sections are intentionally non-exhaustive (blocked on ARC-001/002). Full rewrite + `make doc-check` gate deferred.

---

## Deferred (documented, with plans) ⏭️

- **[ARC-001]** Root-crate monolith — multi-day extraction the audit files under Long-term Backlog. `src/badge.rs` verified leaf-decoupled (only `par-term-config` + `egui` deps) but a single-file extraction is a scope judgment excluded this pass. Recommended order: `par-term-ui` (13 dialog modules) → `par-term-badge` (`badge.rs`+`progress_bar.rs`) → `par-term-session` (blocked on WindowState via `session/capture.rs`).
- **[ARC-002]** `WindowState` god object (93 impl blocks, 7,704 lines) — `TmuxSubsystem` extraction entangled (tmux_handler methods reach into config/tab/UI/tmux_state together; chicken-and-egg with ARC-007). Recommended order: `WindowInfrastructure` first → `SelectionSubsystem` → `TmuxSubsystem` last (after ARC-007 or a `cargo expand` borrow audit of all 25+ tmux_handler methods).
- **[ARC-005]** Config struct (1,529 lines / ~235 fields) — same as QA-002. No round-trip serde test exists to gate a behavior-preserving extraction, and the vault `Arc<Config>` lesson warns the mutation sites cascade. Recommended first extraction: `ProgressBarConfig` (9 clean `progress_bar_*` fields; 43 sites, mutation isolated to `progress_bar_tab.rs`). **Write a round-trip test first.**
- **[ARC-007]** `EventHandler` trait — hard-blocked by ARC-002 (cannot be implemented on sub-handlers until WindowState fields move). Deferral already documented in `src/traits.rs:184-198`.
- **[QA-002]** = ARC-005 (skipped to avoid a two-agent conflict on `config_struct/mod.rs`; folded into ARC-005 above).
- **[QA-005]** `snippets.rs` `CustomActionConfig` enum — the remaining 8-arm reference getters are the zero-alloc pattern (can't delegate to `base()` which returns by value); the full `CustomAction{base,kind}` shape needs a hand-written custom `Deserialize`/`Serialize` (serde can't combine `tag`+`flatten`) touching 89 call sites across 7 files in 3 crates. Vault lesson: document-and-defer.

---

## Requires Manual Intervention 🔧

### [DOC-003] `v0.33.1` git tag not pushed
- **Why**: Pushing a tag is an outward-facing action; held back for explicit confirmation. The `0.33.1` release commit is `847630ff` (HEAD has since moved past it, so the tag must target `847630ff`, not HEAD).
- **Commands**:
  ```bash
  git tag v0.33.1 847630ff
  git push origin v0.33.1
  ```
- **Impact if left undone**: Homebrew SHA formula, the self-update checker, and `git describe`-based version detection all remain broken against `0.33.1`.
- **Effort**: small.

---

## Verification Results

| Check | Result |
|-------|--------|
| Format (`cargo fmt --check`) | ✅ Pass |
| Lint (`cargo clippy`) | ✅ Pass (0 warnings) |
| Type Check (`cargo check --workspace`) | ✅ Pass |
| Tests | ✅ Pass (full suite, including the updated `ssh_integration` + 6 SEC-006 tests incl. the auth-disabled default + strengthened QA-001 test) |

`make checkall` → **"All quality checks passed!"** (exit 0). No regressions.

One pre-existing test (`test_profile_ssh_command_args`) was updated: it previously asserted that a dangerous `-o StrictHostKeyChecking=no` passed through to the SSH argv — i.e. it encoded the exact insecure behavior SEC-002 now blocks. It now mixes a safe option (forwarded) with the dangerous one (filtered) to assert both halves of the SEC-002 policy.

---

## Files Changed (35 remediation files + AUDIT.md + this report)

**New files (4):** `par-term-render/src/wgpu_conversions.rs` (ARC-003); `par-term-input/src/{clipboard,key_encoding,modifiers}.rs` (ARC-006).
**Modified (31):** security — `par-term-acp/{src/fs_ops.rs,src/agent.rs}`, `par-term-mcp/{src/lib.rs,src/tools/screenshot.rs,src/tools/config_update.rs}`, `par-term-config/src/profile_types/profile.rs`, `par-term-update/src/binary_ops.rs`, `src/url_detection/render.rs`; architecture — `par-term-render/{src/lib.rs,src/cell_renderer/mod.rs,src/cell_renderer/surface.rs,src/graphics_renderer.rs,src/renderer/render_passes.rs,Cargo.toml}`, `par-term-config/{Cargo.toml,src/types/rendering.rs}`, `par-term-input/src/lib.rs`, `par-term-settings-ui/Cargo.toml`, root `Cargo.toml`/`Cargo.lock`, `src/app/render_pipeline/{egui_submit.rs,gpu_submit.rs,tab_snapshot.rs}`; quality — `par-term-config/src/{snippets.rs,shader_controls.rs}`; docs — `CLAUDE.md`, `README.md`, `CONTRIBUTING.md`, `CHANGELOG.md`, `docs/API.md`; test — `tests/ssh_integration.rs`.

Cumulative: **37 files changed, +2204 / −824** (incl. `AUDIT.md` and this report, both tracked on the branch).

---

## Next Steps

1. **Action the manual item** above: push `v0.33.1` (DOC-003). SEC-006 is opt-in and no longer blocks ACP; enabling it is optional hardening (set `PAR_TERM_MCP_AUTH_TOKEN` + configure the agent host) if desired.
2. **Begin the highest-ROI deferred work** when capacity allows: ARC-002 `WindowInfrastructure` extraction (unblocks ARC-007), then ARC-005 `ProgressBarConfig` (write a round-trip test first), then ARC-001 `par-term-ui`.
3. **Re-run `/audit`** to regenerate AUDIT.md against the current state — the 17 resolved issues should drop off and the deferred items will re-surface with updated line numbers.
4. Consider a follow-up remediation pass for the **Medium/Low** issues (SEC-010–023, ARC-009–020, QA-006–021, DOC-007–021) — many are small and safe.
