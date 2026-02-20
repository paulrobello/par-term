# Hourly Version Check + Status Bar Update Widget

**Date**: 2026-02-19
**Status**: Approved

## Overview

Add an `Hourly` update check frequency option and a status bar widget that appears when a new version is available. Clicking the widget opens a dedicated update dialog.

## 1. Hourly Frequency Option

- Add `Hourly` variant to `UpdateCheckFrequency` enum in `par-term-config/src/types.rs`
- Maps to 3600 seconds interval
- Appears in Settings Advanced tab dropdown
- The existing 1-hour rate limit in `UpdateChecker` already prevents rapid re-checks

## 2. Status Bar Update Widget

- New `WidgetId::UpdateAvailable` variant in `par-term-config/src/status_bar.rs`
- Only visible when an update is available (hidden otherwise)
- Displays indicator text like `"Update: v0.20.0"` with up-arrow or similar
- Positioned in the **Right** section by default, enabled by default
- Reads `last_update_result` from `WindowManager` to determine visibility

## 3. Click-to-Open Update Dialog

- Clicking the widget opens a dedicated egui update dialog (modal overlay)
- Dialog contents:
  - Version info (current vs available)
  - Release notes (if available)
  - Link to GitHub release page
  - **Install** button (for standalone/bundle installs)
  - **Skip Version** button
  - **Dismiss** button
- For Homebrew/Cargo installs, shows the appropriate command instead of Install button
- Reuses existing update/install logic from `window_manager.rs`

## 4. Data Flow

```
WindowManager::handle_periodic_update_check()
  -> UpdateChecker::check_now() (respects Hourly/Daily/Weekly/Monthly)
  -> last_update_result stored on WindowManager
  -> StatusBar reads result, shows/hides UpdateAvailable widget
  -> User clicks widget -> sets show_update_dialog flag
  -> WindowManager renders egui update dialog overlay
  -> Dialog reuses existing install/skip logic
```

## 5. Configuration Changes

| Field | Change |
|-------|--------|
| `UpdateCheckFrequency` | Add `Hourly` variant |
| `WidgetId` | Add `UpdateAvailable` variant |
| Default status bar config | Include `UpdateAvailable` widget (right section, enabled) |

## 6. Files Affected

- `par-term-config/src/types.rs` - Add `Hourly` to enum
- `par-term-config/src/status_bar.rs` - Add `UpdateAvailable` widget ID + default config
- `par-term-update/src/update_checker.rs` - Handle `Hourly` interval
- `par-term-settings-ui/src/advanced_tab.rs` - Add `Hourly` to dropdown
- `par-term-settings-ui/src/sidebar.rs` - Add search keywords
- `src/status_bar/widgets.rs` - Render update widget text
- `src/status_bar/mod.rs` - Wire up update result to widget context
- `src/app/window_manager.rs` - Handle widget click, render update dialog, schedule hourly checks
