// Mouse event handling split into logical sub-modules.
//
// Sub-modules:
//   clipboard_image_guard  — clipboard image preservation during clicks
//   coords                 — pixel-to-cell coordinate conversion and file drop
//   mouse_button           — handle_mouse_button and left/middle/right dispatch
//   mouse_move             — handle_mouse_move (URL hover, drag selection, divider drag)
//   mouse_tracking         — try_send_mouse_event, active_terminal_mouse_tracking_enabled_at
//   mouse_wheel            — handle_mouse_wheel, set_scroll_target, drag_scrollbar_to

mod clipboard_image_guard;
mod coords;
mod mouse_button;
mod mouse_move;
mod mouse_tracking;
mod mouse_wheel;
