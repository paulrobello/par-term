# Migration Guide

Upgrade notes for par-term covering breaking configuration changes, renamed fields, and behavior shifts between significant version groups. Check the relevant section before upgrading from an older release.

## Table of Contents

- [Unreleased — Content Prettifier Removed](#unreleased--content-prettifier-removed)
- [v0.20.0 — Default Changes](#v0200--default-changes)
- [v0.25.0 — Security Hardening and Behavior Shifts](#v0250--security-hardening-and-behavior-shifts)
- [v0.25.0 — Minimum Contrast Scale Change](#v0250--minimum-contrast-scale-change)
- [v0.25.0 — Pane Padding Defaults](#v0250--pane-padding-defaults)
- [v0.27.0 — Trigger Field Renamed](#v0270--trigger-field-renamed)
- [v0.27.0 — Security-Gated Trigger Execution](#v0270--security-gated-trigger-execution)
- [v0.27.0 — Prettifier External Commands Default-Deny](#v0270--prettifier-external-commands-default-deny)
- [Related Documentation](#related-documentation)

---

## Unreleased — Content Prettifier Removed

The content prettifier feature has been removed. The `par-term-prettifier` workspace crate and all related runtime wiring have been deleted, including the settings UI tab, config/profile fields, trigger action (`type: prettify`), keybinding action (`toggle_prettifier`), and render-path substitutions.

The prettifier was an optional subsystem that reformatted terminal output (JSON, Markdown, tables) using built-in formatters or user-configured external commands. It was removed to reduce maintenance surface area; equivalent formatting can be achieved through shell aliases, pipe-through formatters (e.g., `jq`, `bat`), or the trigger system.

**Migration steps:**

1. Remove `enable_prettifier`, `content_prettifier`, and any per-profile prettifier overrides from `config.yaml`.
2. Remove `toggle_prettifier` from keybindings.
3. Remove any triggers with `type: prettify`.
4. If you relied on external prettifier commands, consider adding them as trigger `RunCommand` actions instead.

---

## v0.20.0 — Default Changes

**`tab_bar_mode` default changed from `when_multiple` to `always`.**

If you were relying on the tab bar auto-hiding when only one tab was open, add this to your config explicitly:

```yaml
tab_bar_mode: "when_multiple"
```

**`window_padding` default changed to `0.0`.**

If you preferred the previous padded look, restore it:

```yaml
window_padding: 4.0
```

---

## v0.25.0 — Security Hardening and Behavior Shifts

Several security-related defaults changed. Existing config files continue to load without error, but runtime behavior changes.

**HTTP profile URLs are now blocked by default.**

Profiles fetched from remote URLs must use HTTPS. HTTP URLs are rejected at fetch time with a warning. Update any profile `url` fields to use `https://`.

**ACP `auto_approve` now enforces `is_safe_write_path`.**

The ACP agent's automatic approval mode for file-write tools now validates that the target path falls within the user's home directory or an explicitly declared safe root. Writes to system paths are blocked even in `auto_approve` mode.

**Prettifier is disabled by default.**

The content prettifier was previously enabled for all content types. From v0.25.0 it defaults to disabled; enable it per-type in Settings → Prettifier or via `prettifier_enabled: true` in config.

---

## v0.25.0 — Minimum Contrast Scale Change

`minimum_contrast` changed from the WCAG ratio scale (1.0–21.0) to an iTerm2-compatible perceived-brightness scale (0.0–1.0).

| Old value | Meaning | New equivalent |
|-----------|---------|----------------|
| `1.0` | Disabled | `0.0` (disabled; auto-migrated on load) |
| `4.5` | WCAG AA | approximately `0.3` |
| `7.0` | WCAG AAA | approximately `0.5` |

A saved value of `1.0` is automatically migrated to `0.0` (disabled) on load. All other values are not auto-migrated — review your setting after upgrading.

The slider in Settings → Appearance is capped at `0.99`; values of `1.0` are treated as disabled.

---

## v0.25.0 — Pane Padding Defaults

Default padding values changed:

| Field | Old default | New default |
|-------|-------------|-------------|
| `pane_padding` | `4.0` px | `1.0` px |
| `window_padding` | `0.0` px | `1.0` px |

Split-pane mode now automatically adds base padding equal to half the divider width, so `pane_padding` of `0.0` is no longer needed to remove the inter-pane gap.

---

## v0.27.0 — Trigger Field Renamed

The `require_user_action` field on trigger definitions was renamed to `prompt_before_run`.

```yaml
# Before v0.27.0
triggers:
  - name: "my trigger"
    require_user_action: false

# v0.27.0 and later
triggers:
  - name: "my trigger"
    prompt_before_run: false
    i_accept_the_risk: true   # required when prompt_before_run is false
```

The old field name is accepted as a YAML alias — existing config files continue to load without modification. However, the Settings UI only shows `prompt_before_run`. Update your config to avoid confusion.

---

## v0.27.0 — Security-Gated Trigger Execution

Triggers with `prompt_before_run: false` now **require** an explicit `i_accept_the_risk: true` field. Without it, execution is blocked and an audit warning is emitted.

If your existing config has `require_user_action: false` (or the new `prompt_before_run: false`) on any trigger, add `i_accept_the_risk: true` to that trigger to restore automatic execution:

```yaml
triggers:
  - name: "auto-run trigger"
    prompt_before_run: false
    i_accept_the_risk: true
    pattern: "some pattern"
    action: ...
```

A warning banner appears in Settings → Automation when any trigger has this configuration.

---

## v0.27.0 — Prettifier External Commands Default-Deny

`ExternalCommandRenderer` (used by the content prettifier to run external formatters) now refuses execution when `allowed_commands` is empty, which is the default. This was previously a no-op.

To allow a specific external command, add it to the allowlist in your config:

```yaml
prettifier:
  allowed_commands:
    - "/usr/bin/jq"
    - "/usr/local/bin/bat"
```

Attempts to run unlisted commands are blocked with a warning in the debug log.

---

## Related Documentation

- [Config Reference](CONFIG_REFERENCE.md) — complete field reference with types and defaults
- [Automation](AUTOMATION.md) — trigger configuration and `prompt_before_run` / `i_accept_the_risk` usage
- [Assistant Panel](ASSISTANT_PANEL.md) — ACP agent configuration and `auto_approve` permissions
- [Changelog](../CHANGELOG.md) — full release history
