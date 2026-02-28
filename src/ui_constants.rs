//! Named constants for UI layout dimensions.
//!
//! Centralising these values makes future DPI scaling and theming easier.
//! Only UI layout constants (dimensions, sizes, spacing) belong here.
//! Animation speeds, colour values, and algorithm parameters live elsewhere.

// ---------------------------------------------------------------------------
// Clipboard History UI  (src/clipboard_history_ui.rs)
// ---------------------------------------------------------------------------

/// Default / initial width of the Clipboard History window.
pub const CLIPBOARD_WINDOW_DEFAULT_WIDTH: f32 = 400.0;
/// Default / initial height of the Clipboard History window.
pub const CLIPBOARD_WINDOW_DEFAULT_HEIGHT: f32 = 250.0;
/// Maximum height allowed for the Clipboard History window.
pub const CLIPBOARD_WINDOW_MAX_HEIGHT: f32 = 350.0;

// ---------------------------------------------------------------------------
// Command History UI  (src/command_history_ui.rs)
// ---------------------------------------------------------------------------

/// Default / initial width of the Command History Search window.
pub const CMD_HISTORY_WINDOW_DEFAULT_WIDTH: f32 = 500.0;
/// Default / initial height of the Command History Search window.
pub const CMD_HISTORY_WINDOW_DEFAULT_HEIGHT: f32 = 350.0;
/// Maximum height allowed for the Command History Search window.
pub const CMD_HISTORY_WINDOW_MAX_HEIGHT: f32 = 450.0;

// ---------------------------------------------------------------------------
// Paste Special UI  (src/paste_special_ui.rs)
// ---------------------------------------------------------------------------

/// Default / initial width of the Paste Special window.
pub const PASTE_SPECIAL_WINDOW_DEFAULT_WIDTH: f32 = 500.0;
/// Default / initial height of the Paste Special window.
pub const PASTE_SPECIAL_WINDOW_DEFAULT_HEIGHT: f32 = 400.0;
/// Maximum height of the transformation list scroll area inside Paste Special.
pub const PASTE_SPECIAL_TRANSFORMS_MAX_HEIGHT: f32 = 250.0;

// ---------------------------------------------------------------------------
// SSH Quick Connect UI  (src/ssh_connect_ui.rs)
// ---------------------------------------------------------------------------

/// Minimum width for the SSH Quick Connect dialog.
pub const SSH_CONNECT_DIALOG_MIN_WIDTH: f32 = 350.0;
/// Maximum width for the SSH Quick Connect dialog.
pub const SSH_CONNECT_DIALOG_MAX_WIDTH: f32 = 500.0;
/// Minimum height for the SSH Quick Connect dialog.
pub const SSH_CONNECT_DIALOG_MIN_HEIGHT: f32 = 300.0;
/// Maximum height for the SSH Quick Connect dialog.
pub const SSH_CONNECT_DIALOG_MAX_HEIGHT: f32 = 500.0;
/// Inner margin of the SSH Quick Connect frame.
pub const SSH_CONNECT_INNER_MARGIN: f32 = 16.0;
/// Height of each host row button in the SSH Quick Connect list.
pub const SSH_CONNECT_HOST_ROW_HEIGHT: f32 = 28.0;
/// Height of the search bar in the SSH Quick Connect dialog.
pub const SSH_CONNECT_SEARCH_BAR_HEIGHT: f32 = 24.0;
/// Space reserved below the host list for the cancel/hint bar.
pub const SSH_CONNECT_LIST_BOTTOM_RESERVE: f32 = 100.0;

// ---------------------------------------------------------------------------
// Profile Modal UI  (src/profile_modal_ui.rs)
// ---------------------------------------------------------------------------

/// Width of the Profile management modal dialog.
pub const PROFILE_MODAL_WIDTH: f32 = 550.0;
/// Height of the Profile management modal dialog.
pub const PROFILE_MODAL_HEIGHT: f32 = 580.0;
/// Width of the SSH port text-edit field inside the profile form.
pub const PROFILE_SSH_PORT_FIELD_WIDTH: f32 = 60.0;
/// Maximum height of the icon picker scroll area in the profile form.
pub const PROFILE_ICON_PICKER_MAX_HEIGHT: f32 = 300.0;
/// Minimum width of the icon picker panel.
pub const PROFILE_ICON_PICKER_MIN_WIDTH: f32 = 280.0;

// ---------------------------------------------------------------------------
// Help UI  (src/help_ui.rs)
// ---------------------------------------------------------------------------

/// Default width of the Help window.
pub const HELP_WINDOW_DEFAULT_WIDTH: f32 = 550.0;
/// Default height of the Help window.
pub const HELP_WINDOW_DEFAULT_HEIGHT: f32 = 600.0;

// ---------------------------------------------------------------------------
// Shader Install UI  (src/shader_install_ui.rs)
// ---------------------------------------------------------------------------

/// Default width of the Shader Pack Available dialog.
pub const SHADER_INSTALL_DIALOG_WIDTH: f32 = 450.0;
/// Inner margin of the Shader Install dialog frame.
pub const SHADER_INSTALL_INNER_MARGIN: f32 = 20.0;
/// Uniform width of action buttons (Yes/Never/Later/OK) in Shader Install.
pub const SHADER_INSTALL_BUTTON_WIDTH: f32 = 120.0;
/// Height of action buttons in the Shader Install dialog.
pub const SHADER_INSTALL_BUTTON_HEIGHT: f32 = 32.0;

// ---------------------------------------------------------------------------
// Integrations / Update UI  (src/integrations_ui.rs)
// ---------------------------------------------------------------------------

/// Default width of the Integrations / update dialog.
pub const INTEGRATIONS_DIALOG_WIDTH: f32 = 500.0;
/// Inner margin of the Integrations dialog frame.
pub const INTEGRATIONS_INNER_MARGIN: f32 = 24.0;
/// Base width for action buttons in the Integrations dialog.
pub const INTEGRATIONS_BUTTON_WIDTH: f32 = 130.0;
/// Height of action buttons in the Integrations dialog.
pub const INTEGRATIONS_BUTTON_HEIGHT: f32 = 32.0;
/// Width of the dismiss "OK" button in the Integrations dialog.
pub const INTEGRATIONS_OK_BUTTON_WIDTH: f32 = 120.0;

// ---------------------------------------------------------------------------
// tmux Session Picker UI  (src/tmux_session_picker_ui.rs)
// ---------------------------------------------------------------------------

/// Default width of the tmux Session Picker window.
pub const TMUX_PICKER_WINDOW_DEFAULT_WIDTH: f32 = 400.0;
/// Default height of the tmux Session Picker window.
pub const TMUX_PICKER_WINDOW_DEFAULT_HEIGHT: f32 = 350.0;
/// Maximum height of the session list scroll area in the tmux Picker.
pub const TMUX_PICKER_LIST_MAX_HEIGHT: f32 = 200.0;

// ---------------------------------------------------------------------------
// File Transfers overlay  (src/app/file_transfers.rs)
// ---------------------------------------------------------------------------

/// Minimum width of the File Transfers overlay window.
pub const FILE_TRANSFERS_MIN_WIDTH: f32 = 250.0;
/// Anchor offset (negative = from right/bottom edge) for the File Transfers window.
pub const FILE_TRANSFERS_ANCHOR_OFFSET: f32 = 10.0;

// ---------------------------------------------------------------------------
// Tab Bar UI  (src/tab_bar_ui/mod.rs)
// ---------------------------------------------------------------------------

/// Horizontal spacing between tabs.
pub const TAB_SPACING: f32 = 4.0;
/// Left padding before the first tab in the horizontal tab bar.
pub const TAB_LEFT_PADDING: f32 = 2.0;
/// Width of each scroll navigation button.
pub const TAB_SCROLL_BTN_WIDTH: f32 = 24.0;
/// Width of the "new tab" (+) button (without chevron).
pub const TAB_NEW_BTN_BASE_WIDTH: f32 = 28.0;
/// Corner rounding radius for horizontal tab tiles.
pub const TAB_ROUNDING: f32 = 4.0;
/// Shrink margin applied to a tab rect before drawing (horizontal, vertical).
pub const TAB_DRAW_SHRINK_X: f32 = 2.0;
pub const TAB_DRAW_SHRINK_Y: f32 = 1.0;
/// Horizontal content padding inside each tab (left+right, top+bottom).
pub const TAB_CONTENT_PAD_X: f32 = 8.0;
pub const TAB_CONTENT_PAD_Y: f32 = 2.0;
/// Width of the active-tab indicator bar (left edge vertical stripe).
pub const TAB_ACTIVE_INDICATOR_WIDTH: f32 = 3.0;
/// Size (width and height) of the close button in the horizontal tab bar.
pub const TAB_CLOSE_BTN_SIZE_H: f32 = 16.0;
/// Margin between the close button and the right edge of the tab.
pub const TAB_CLOSE_BTN_MARGIN: f32 = 4.0;
/// Size (width and height) of the close button in the vertical tab bar.
pub const TAB_CLOSE_BTN_SIZE_V: f32 = 20.0;
/// Width reserved for the hotkey label (e.g. "⌘1") in the vertical tab bar.
pub const TAB_HOTKEY_LABEL_WIDTH: f32 = 26.0;
/// Padding between content and edges in the vertical tab context menu.
pub const TAB_CONTEXT_PADDING: f32 = 12.0;
/// Width of the drop-zone diamond indicator drawn during tab drag.
pub const TAB_DROP_DIAMOND_SIZE: f32 = 4.0;
/// Minimum width of the "New Tab" profile picker window.
pub const TAB_NEW_PROFILE_MENU_WIDTH: f32 = 200.0;
/// Anchor offset for the "New Tab" profile picker window.
pub const TAB_NEW_PROFILE_MENU_OFFSET_X: f32 = -4.0;
pub const TAB_NEW_PROFILE_MENU_OFFSET_Y: f32 = 4.0;
/// Minimum width of the tab context menu.
pub const TAB_CONTEXT_MENU_MIN_WIDTH: f32 = 160.0;
/// Height of each item row in the tab context menu.
pub const TAB_CONTEXT_MENU_ITEM_HEIGHT: f32 = 24.0;
/// Width of the rename text-edit inside the tab context menu.
pub const TAB_RENAME_EDIT_WIDTH: f32 = 140.0;
/// Width of the hex colour edit inside the tab context menu.
pub const TAB_COLOR_HEX_EDIT_WIDTH: f32 = 60.0;
/// Minimum width of the icon picker panel inside the tab context menu.
pub const TAB_ICON_PICKER_MIN_WIDTH: f32 = 280.0;
/// Maximum height of the icon picker scroll area inside the tab context menu.
pub const TAB_ICON_PICKER_MAX_HEIGHT: f32 = 300.0;
/// Size of icon glyphs rendered inside the icon picker.
pub const TAB_ICON_PICKER_GLYPH_SIZE: f32 = 16.0;
/// Size of the colour-swatch buttons in the tab context menu.
pub const TAB_COLOR_SWATCH_SIZE: f32 = 18.0;
/// Corner radius of colour-swatch buttons in the tab context menu.
pub const TAB_COLOR_SWATCH_ROUNDING: f32 = 2.0;

// ---------------------------------------------------------------------------
// AI Inspector Panel  (src/ai_inspector/panel.rs)
// ---------------------------------------------------------------------------

/// Minimum width of the AI Inspector panel.
pub const AI_PANEL_MIN_WIDTH: f32 = 200.0;
/// Maximum width of the AI Inspector panel as a fraction of the viewport width.
pub const AI_PANEL_MAX_WIDTH_RATIO: f32 = 0.5;
/// Width of the resize drag handle for the AI Inspector panel.
pub const AI_PANEL_RESIZE_HANDLE_WIDTH: f32 = 8.0;
/// Inner margin of the AI Inspector panel frame.
pub const AI_PANEL_INNER_MARGIN: f32 = 8.0;
/// Margin subtracted from panel width to obtain inner content width
/// (accounts for 8 px margins each side + 1 px border stroke each side).
pub const AI_PANEL_INNER_INSET: f32 = 18.0;
/// Margin subtracted from viewport height to obtain inner panel height.
pub const AI_PANEL_HEIGHT_INSET: f32 = 18.0;
/// Minimum height of the command-capture scroll area inside the AI panel.
pub const AI_PANEL_CMD_SCROLL_MIN_HEIGHT: f32 = 100.0;
/// Maximum height of the command-capture scroll area inside the AI panel.
pub const AI_PANEL_CMD_SCROLL_MAX_HEIGHT: f32 = 300.0;
/// Minimum height of the chat message scroll area.
pub const AI_PANEL_CHAT_MIN_HEIGHT: f32 = 50.0;
/// Base height of the chat input row (single line).
pub const AI_PANEL_CHAT_INPUT_BASE_HEIGHT: f32 = 20.0;
/// Per-additional-line height increment in the chat input.
pub const AI_PANEL_CHAT_INPUT_LINE_HEIGHT: f32 = 14.0;
/// Width reserved for the Send + Clear buttons next to the chat input.
pub const AI_PANEL_CHAT_BUTTON_WIDTH: f32 = 60.0;
/// Corner rounding for message / card frames in the AI Inspector.
pub const AI_PANEL_CARD_ROUNDING: f32 = 4.0;

// ---------------------------------------------------------------------------
// Confirmation Dialogs  (close_confirmation_ui, quit_confirmation_ui)
// ---------------------------------------------------------------------------

/// Title font size for confirmation dialog headings.
pub const CONFIRM_DIALOG_TITLE_SIZE: f32 = 18.0;
/// Body command-label font size in the close-confirmation dialog.
pub const CONFIRM_DIALOG_CMD_SIZE: f32 = 14.0;

// ---------------------------------------------------------------------------
// FPS Overlay  (src/app/window_state/render_pipeline.rs)
// ---------------------------------------------------------------------------

/// Horizontal anchor offset of the FPS overlay (negative = from right edge).
pub const FPS_OVERLAY_OFFSET_X: f32 = -30.0;
/// Vertical anchor offset of the FPS overlay (from top edge).
pub const FPS_OVERLAY_OFFSET_Y: f32 = 10.0;
/// Corner rounding of the FPS overlay background frame.
pub const FPS_OVERLAY_ROUNDING: f32 = 4.0;

// ---------------------------------------------------------------------------
// Mouse / interaction thresholds
// ---------------------------------------------------------------------------

/// Minimum squared pixel distance a mouse must travel from press position before
/// a click-and-hold is treated as a drag selection. Prevents accidental selections
/// from trackpad tap-to-click jitter.
///
/// Note: compared against `dx*dx + dy*dy` (squared distance) to avoid a sqrt.
pub const DRAG_THRESHOLD_PX: f64 = 8.0;

/// Maximum pixel distance a click-and-release may travel while still being
/// considered a plain click (vs. a drag). Used by the clipboard-image click
/// guard to decide whether to restore the clipboard after a release.
///
/// Note: compared against `dx*dx + dy*dy` (squared distance) to avoid a sqrt.
pub const CLICK_RESTORE_THRESHOLD_PX: f64 = 6.0;

/// Hit-test radius (in physical pixels) for scrollbar mark tooltips.
/// Mouse must be within this many pixels of a mark's centre to show the tooltip.
pub const SCROLLBAR_MARK_HIT_RADIUS_PX: f32 = 8.0;

// ---------------------------------------------------------------------------
// Animation / timing
// ---------------------------------------------------------------------------

/// Duration of the visual-bell screen flash in milliseconds.
/// Shared by the pane, tab, and render-pipeline flash logic.
pub const VISUAL_BELL_FLASH_DURATION_MS: u128 = 150;
