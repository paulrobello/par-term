# Audit Remediation Report

> **Project**: par-term
> **Audit Date**: 2026-10-30 (audited commit `80357bf0`)
> **Remediation Date**: 2026-10-30
> **Severity Filter Applied**: all
> **Plan Source**: AUDIT.md `## Remediation Plan` (AUDIT-REMEDIATION-PLAN.md was present but stale — July cycle — and was ignored; deleted at wrap-up)
> **Implementation Model**: Opus 5 (all fix agents)
> **Integration**: branch `fix/audit-remediation`, base `6f314174` (local main); six isolated agent worktrees merged sequentially

---

## Execution Summary

| Phase | Status | Agent | Issues Targeted | Resolved | Partial | Disposition/Manual |
|-------|--------|-------|----------------|----------|---------|--------------------|
| 1 — SEC-003 (promoted) | ✅ | fix-security | 1 | 1 | 0 | 0 |
| 2 — ARC-001 (promoted) | ✅ | fix-architecture | 1 | 1 | 0 | 0 |
| 3a — Security (remaining) | ✅ | fix-security | 7 | 5 | 0 | 2 (verified upstream / confirmed acceptable) |
| 3b — Architecture (remaining) | ✅ | fix-architecture ×2 | 10 | 5 | 4 | 1 (ARC-010 managed-debt disposition) |
| 3c — Code Quality | ✅ | fix-code-quality ×2 | 5 | 4 | 1 | 0 |
| 3d — Documentation | ✅ | fix-documentation | 8 | 7 | 0 | 1 (DOC-007 informational, no action) |
| 4 — Verification | see below | — | — | — | — | — |

**Overall**: 31 issues — 23 resolved, 5 closed with documented deviations (residue filed as follow-up cards), 3 disposition-only (no code change per audit), 31/31 have a recorded outcome. No issue was left unaddressed.

---

## Resolved Issues ✅

### Security
- **[SEC-003]** Agent-installable GPU shader chain — `a4fa1d09` — `custom_shader`/`cursor_shader` values from agents must name an existing file inside the canonicalized shaders directory (symlink-safe, validated in `Config::validate_agent_shader_path`, enforced at the app's agent-config boundary and early in the MCP tool); the config directory is no longer an auto-approved agent write root (only `.config-update.json` is). Residual (first-use confirmation) filed as a follow-up card.
- **[SEC-001]** Fail-open auto-approval classifier — `ea1756c1` — exact `mcp__par-term-config__<tool>` allowlist replaces the namespace `contains()`; unknown par-term MCP tools escalate. Tests cover unknown names, near-misses, and spoofed namespaces.
- **[SEC-002]** `send_keys_literal` quoting — `873cb828` — text quoted `'\''`-style; `\n`/`\r`/NUL rejected with `InvalidInput` pointing at `send_keys_hex`. Round-trip tests against the pinned core `parse_command`.
- **[SEC-004]** Unpinned installers — `8628cb30` — `gh-pages/install-shell-integration.sh` pins `REF="v0.50.0"`. Follow-up card: the REF bump needs a release-process owner.
- **[SEC-005]** Plugin revision pinning — `b57bf8cb` — installed commit recorded at `add`; `update_fetch` refuses when HEAD moved outside the plugin update; `update_apply` lands exactly the reviewed commit, once. Pre-pinning installs get trust-once-on-record with a logged warning.
- **[ARC-011]** (reassigned for file-conflict) `86c405f9` — TWIN header comments on both `install_shaders.sh` copies.
- **[SEC-006]** Daemon socket perms — no code (client-only crate, correct). Hardening verified present in pinned core 0.73 (`ipc.rs`: 0o600 socket, per-UID 0700 dir, euid peer check, Windows owner-only ACL); upstream verification card closed with that evidence.
- **[SEC-007]** tmp-dir TOCTOU — no code; confirmed acceptable as documented in-code (`SAFE_PATH_CHECK_LOCK` + `is_safe_write_path` docs).

### Architecture
- **[ARC-001]** WindowState hub budget — `c7ebaf1d` — `make check-window-state-budget` (in `make ci`) fails on growth past frozen ceilings: 117 inherent `impl WindowState` blocks / 48 struct fields (audit's 122/46 were grep noise — comment lines and two full-path headers). Ceilings only go down.
- **[ARC-002]** Event-dispatch complexity — `592bb1e2` — the remedy's named target `about_to_wait` fully refactored: declarative 8-row `PollTask` table + ordered phase functions + explicit queue-priority contract; wake-reduction contract preserved; decision count 101 → 4 (local proxy). **Partial**: `handle_key_event` (94), `handle_window_event` (84), `apply_renderer_config` (78) untouched — residue card filed with the pattern to follow.
- **[ARC-003]** Mux notification subsystem — `4dbb2fab` + `d9c1a0ba` — `mux.rs` 6,896 → 854 lines; tests split into 11 feature-grouped sibling files (84/84 tests pass, bodies unchanged); typed seam (`DirectEvent`/`EventBatch`/`EventSource`/`DirectEffects` in `event_dispatch.rs`) with both dispatch entries (`mux_drain`, `polling`) migrated. Roster/telemetry/held-pane handling remains app-side — internal migration rides the "Route every terminal through mux" board card, per the audit roadmap.
- **[ARC-004]** Duplicated HTTP stack — `b1e96234` — `par-term-update/src/http.rs` is the single validated-HTTP surface; callers own their `HostPolicy` allowlists; `src/http.rs` is a thin binding. Side-effect hardening: updater validation errors now redact URL query/userinfo (**flagged for owner review** — security-adjacent behavior change, in the ARC-004 commit).
- **[ARC-005]** Dedup pairs — `f07921b6` — four of five pairs share one helper (`start_refresh_task` with the tab-only child-death redraw preserved as `LivenessPolling` parameter; `macos_major_version` in `macos_ffi.rs`; `has_shader_files` on `par_term_config::Config`; `show_shader_error_panel` free function). **Deviation**: `output_with_timeout` pair stays duplicated (no clean shared home without a new util crate); loud TWIN cross-references added on both sides.
- **[ARC-006]** Data-as-code — `c85b2650` — copy-mode motions became three predicate-parameterized scanners with characterization tests written first (CJK rows, clamping, word classes). Theme-constructor dedup skipped — the audit itself judged it low-value optional.
- **[ARC-007]** SettingsUI tracking — `6bf90614` — advisory (warn-only) section in the budget script: 16 impl blocks / 134 fields, measured like ARC-001.
- **[ARC-008]** Nightly tests — `6bf90614` — `nightly-test` cron job in `ci.yml` runs `cargo test --workspace` on Linux/macOS/Windows with pinned toolchain SHAs; push CI still lint-only. actionlint clean.
- **[ARC-009]** Makefile quartet — `6bf90614` — `check-all` is a deprecated alias of `checkall`; three entry points remain; `ci` drops the redundant `check-all` clippy pass; CLAUDE.md + CONTRIBUTING.md updated.
- **[ARC-010]** 800-line crowding — disposition-only: confirmed managed; gate is the monitor (0 over cap, 79 in warn band).

### Code Quality
- **[QA-001]** Key-code mappers — `f9547160`, `e6cc8c0a`, `9c7ad932`, `c209b660` — all six functions converted to lookup tables; exhaustive family tests (74 physical-key + 71 recorder mappings; 894-line copy-mode golden transcript; 11,111 generated agent histories). Measured complexity: `parse_physical_key_code` 75→1, `handle_copy_mode_key` 78→2, `update_post_render_state` 58→28, `process_agent_messages_tick` 62→46. **Deviation**: `sync_script_running_state` only 74→67 — residue (plus a pinned always-false skill-failure scan) filed as follow-up cards.
- **[QA-002]** Typed errors — `ca687084`, `1a0a39ac` — `IpcError` (9 variants) in par-term-mcp replacing `Result<_, String>`; `TerminalError` (6 variants, thiserror, core errors as sources) in par-term-terminal replacing anyhow. Per the TODO's priority order. Breaking API for the published crates — version bumps land at release.
- **[QA-003]** Zero-caller methods — `1f71f39b` + `f07921b6` — per-item verification (workspace grep, parsight impact, crates.io reverse deps: sole external dependent pins `^0.8`, current 0.16). Removed: `find_by_tag`, `filter_by_tags`, `get_valid_parents`, private `has_ancestor`, and (orchestrator follow-up) `ResolvedCursorShaderConfig::for_shader`. Kept with evidence: `TriggerRateLimiter::new`, `resolve_tmux_path`, `MetadataCache::with_shaders_dir`, `JsonRpcClient::new`, the two shader-metadata parsers (documented pub-API). `checked_shader_*` helpers `#[doc(hidden)]`.
- **[QA-004]** Silent discards — `9d023969` — 15 config-watcher + 8 fs_tools discard sites now log warnings; resync.rs deliberately unchanged (typed-error or documented best-effort paths).
- **[QA-005]** Mutex poisoning — `e83e8464` — recovery applied. Finding: both audit-cited sites were TEST code; production PTY-write/trigger paths hardened anyway (`pane_write.rs:106,404`, `triggers/mod.rs:926`).

### Documentation
- **[DOC-001 + DOC-003 + DOC-008]** — `5c823484` — `## [Unreleased]` with the unreleased features, signing fix, core-pin story (0.66.1→0.73.0 — corrected from the brief; 0.68 came after the tag); compare link advanced to `v0.50.0...HEAD`; MUX.md "Inline graphics and file drops" section written from the shipped code (upload limits verified against 0.73 constants); MIGRATION.md v0.49.0 section.
- **[DOC-002]** — `55ec7186` — SECURITY.md repointed: dispatcher at `src/app/window_manager/scripting/mod.rs`, trigger event traced to `par_term_emu_core_rust::terminal::TerminalEvent::TriggerMatched`; stale heading renamed; `require_user_action`→`prompt_before_run`; `write_to_pty()`→`TerminalManager::write_str`. One known inaccuracy left stated in the report: the RunCommand "optional consent" pseudo-code step has no implementation.
- **[DOC-004]** — ssh half `9e9cc56c` (7.5%→13.0% doc lines, `-D warnings` + doctests clean); mcp half `5ea31d88` (11.0%→15.1%, builds clean with `-W missing_docs`).
- **[DOC-005]** — `64e02fed` — font-guide path fixed + sibling stale path repointed.
- **[DOC-006]** — `b91846ae` — 172 parsight `doc-path:` waiver markers across 34 files; 17 heading lines intentionally unmarked (anchor-slug preservation). Upstream suggestion filed on parsight (frozen-history set for plain `docs/plans`).
- **[DOC-007]** — no action; lychee exclusions verified still in place.
- Accuracy pass across all of the above: `1700ad71`.

---

## Requires Manual Intervention 🔧

1. **Review the ARC-004 redaction side-effect** (`b1e96234`): self-update validation errors now hide query string/userinfo. This is a tightening, but it is a security-adjacent behavior change flagged per policy. Estimated effort: small (read the commit).
2. **Publish order at next release**: `par-term-config`, `par-term-update`, `par-term-mcp`, `par-term-terminal` all carry public-API changes; bump in the CLAUDE.md Layer order before publishing `par-term`. Estimated effort: medium (established checklist).
3. **Design decision — SEC-003 residual** (card filed): first-use confirmation (D4b-style hash ledger) for agent-written shaders. Estimated effort: medium.
4. **Design decision — installer REF bump ownership** (card filed): release flow must advance `gh-pages/install-shell-integration.sh` `REF` per release or resolve the latest tag dynamically. Estimated effort: small.

Follow-up cards filed during the run (backlog): remaining three dispatchers (ARC-002 residue), skill-failure scan always-false, pane-refresh child-death asymmetry, file-drop on pre-0.68 daemon fallback, parsight frozen-history for `docs/plans`.

---

## Verification Results

- `make checkall` (local, integrated tree): ✅ exit 0 — 66 test suites ok, 0 failures
- `make checkall-ci` (local): ⚠️ 1411 passed / 1 failed — `mux_sessions_tests::workspaces_create_switch_rename_kill_end_to_end`, the known load-sensitive daemon-timing flake family; passed in isolation twice (0.90 s, 0.86 s) and the identical suite was green in the `checkall` run minutes earlier. Local load was 24–46 (threshold 15), so the gate was re-run on MiniMac (load 1.4, ARM64, rustc 1.99.1, booked window `01a18e8dd23a75d2b785545833b5ff62`).
- `make checkall-ci` (MiniMac, booked window, load 1.5 → 11.9): ✅ exit 0 — all parity legs passed, cargo-deny clean, 66 suites / 0 failures; the previously flaky `workspaces_create_switch_rename_kill_end_to_end` passed, and the full lib suite ran in 16.22 s (vs 70 s under local load), confirming the load-sensitivity diagnosis
- Per-issue validation: every closed card's change verified on the merged tree by inspection (grep/read) and agent-reported targeted tests before closure; this final global gate is the safety net.

---

## Files Changed

Full list per phase (see commit messages for per-issue attribution):

- **Security**: `par-term-acp/src/permissions.rs`, `par-term-config/src/config/path_validation.rs`, `par-term-mcp/src/lib.rs`, `par-term-mcp/src/tools/config_update.rs`, `par-term-mcp/src/tools/mod.rs`, `par-term-mux/src/client.rs`, `par-term-scripting/src/plugin_git.rs`, `gh-pages/install-shell-integration.sh`, `gh-pages/install-shaders.sh`, `install_shaders.sh`, `docs/ASSISTANT_PANEL.md`
- **Gates/tooling**: `scripts/check_window_state_budget.py` (new), `Makefile`, `.github/workflows/ci.yml`, `CLAUDE.md`, `CONTRIBUTING.md`, `src/app/window_state/mod.rs`
- **Dispatch/mux**: `src/app/handler/window_state_impl/about_to_wait.rs` + new `about_to_wait/` submodule, `src/app/tmux_handler/notifications/mux.rs` (split), 11 new `mux_*_tests.rs` files, `event_dispatch.rs` (new), `mux_drain.rs`, `polling.rs`
- **HTTP/dedup**: `src/http.rs`, `par-term-update/src/http.rs`, `src/shader_installer.rs`, `src/tab/refresh_task.rs`, `src/tab/mod.rs`, `src/pane/types/pane_ops.rs`, `src/macos_space.rs`, `src/macos_blur.rs`, `src/macos_ffi.rs`, `par-term-config/src/url_policy.rs`, `par-term-settings-ui/src/shader_editor.rs`, `par-term-settings-ui/src/cursor_shader_editor.rs`, `src/copy_mode/motion.rs`
- **Quality**: `par-term-keybindings/src/platform.rs`, `src/app/copy_mode/handler.rs` + `handler_tests.rs` (new) + `mod.rs`, `par-term-settings-ui/src/input_tab/keybindings.rs`, `src/app/window_manager/scripting/mod.rs`, `src/app/window_state/agent_messages.rs` + `agent_messages_tests.rs` (new), `src/app/render_pipeline/post_render.rs`, `src/app/window_state/config_watchers.rs`, `par-term-acp/src/fs_tools.rs`, `src/app/triggers/mod.rs`, `src/app/tmux_handler/pane_write.rs`
- **Crates**: `par-term-mcp/src/error.rs` (new), `par-term-terminal/src/error.rs` (new), both crates' `Cargo.toml`, `Cargo.lock`, `par-term-config/src/profile_types/matchers.rs`, `par-term-config/src/shader_config.rs`, `par-term-render/src/error.rs`, `src/process_timeout.rs`
- **Docs**: `CHANGELOG.md`, `docs/features/MUX.md`, `docs/features/PLUGINS.md`, `docs/guides/MIGRATION.md`, `par-term-scripting/SECURITY.md`, `scripts/README.md`, `docs/API.md`, `docs/plans/**` (waiver markers), `par-term-ssh/src/**` (doc comments)

---

## Next Steps

1. Review the four Manual Intervention items above.
2. Re-run `/audit` on the merged result for a fresh AUDIT.md.
3. The wrap-up (pending owner confirmation): merge `fix/audit-remediation` into local `main`, delete consumed audit artifacts, remove agent worktrees. Push remains a separate explicit decision.
