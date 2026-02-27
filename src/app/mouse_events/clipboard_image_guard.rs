use crate::app::window_state::{ClipboardImageClickGuard, PreservedClipboardImage, WindowState};
use std::sync::Arc;
use winit::event::{ElementState, MouseButton};

impl WindowState {
    pub(crate) fn begin_clipboard_image_click_guard(
        &mut self,
        button: MouseButton,
        state: ElementState,
    ) {
        if button != MouseButton::Left || state != ElementState::Pressed {
            return;
        }

        // Only guard plain clicks; modified clicks are usually intentional actions.
        let mods = self.input_handler.modifiers.state();
        if mods.alt_key() || mods.control_key() || mods.shift_key() || mods.super_key() {
            self.clipboard_image_click_guard = None;
            return;
        }

        let press_position = self
            .tab_manager
            .active_tab()
            .map(|t| t.mouse.position)
            .unwrap_or((0.0, 0.0));

        let mut clipboard = match arboard::Clipboard::new() {
            Ok(clipboard) => clipboard,
            Err(_) => {
                self.clipboard_image_click_guard = None;
                return;
            }
        };

        match clipboard.get_image() {
            Ok(image) => {
                self.clipboard_image_click_guard = Some(ClipboardImageClickGuard {
                    image: PreservedClipboardImage {
                        width: image.width,
                        height: image.height,
                        bytes: image.bytes.into_owned(),
                    },
                    press_position,
                    suppress_terminal_mouse_click: self
                        .active_terminal_mouse_tracking_enabled_at(press_position),
                });
                crate::debug_log!("MOUSE", "Armed clipboard image click guard");
            }
            Err(_) => {
                self.clipboard_image_click_guard = None;
            }
        }
    }

    pub(crate) fn finish_clipboard_image_click_guard(
        &mut self,
        button: MouseButton,
        state: ElementState,
    ) {
        if button != MouseButton::Left || state != ElementState::Released {
            return;
        }

        let Some(guard) = self.clipboard_image_click_guard.take() else {
            return;
        };

        let release_position = self
            .tab_manager
            .active_tab()
            .map(|t| t.mouse.position)
            .unwrap_or((0.0, 0.0));

        let dx = release_position.0 - guard.press_position.0;
        let dy = release_position.1 - guard.press_position.1;
        const CLICK_RESTORE_THRESHOLD_PX: f64 = 6.0;
        if (dx * dx + dy * dy) > CLICK_RESTORE_THRESHOLD_PX * CLICK_RESTORE_THRESHOLD_PX {
            return;
        }

        let _ = self.restore_clipboard_image_if_missing(&guard.image);

        let runtime = Arc::clone(&self.runtime);
        let image = guard.image.clone();
        runtime.spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(80)).await;
            if let Ok(mut clipboard) = arboard::Clipboard::new()
                && clipboard.get_image().is_err()
            {
                let _ = clipboard.set_image(arboard::ImageData {
                    width: image.width,
                    height: image.height,
                    bytes: std::borrow::Cow::Owned(image.bytes),
                });
            }
        });
    }

    fn restore_clipboard_image_if_missing(&self, image: &PreservedClipboardImage) -> bool {
        let mut clipboard = match arboard::Clipboard::new() {
            Ok(clipboard) => clipboard,
            Err(_) => return false,
        };

        if clipboard.get_image().is_ok() {
            return true;
        }

        match clipboard.set_image(arboard::ImageData {
            width: image.width,
            height: image.height,
            bytes: std::borrow::Cow::Owned(image.bytes.clone()),
        }) {
            Ok(()) => {
                crate::debug_log!("MOUSE", "Restored clipboard image after plain click");
                true
            }
            Err(_) => false,
        }
    }

    pub(crate) fn should_suppress_terminal_mouse_click_for_image_guard(
        &self,
        button: MouseButton,
        _state: ElementState,
        mouse_position: (f64, f64),
    ) -> bool {
        if button != MouseButton::Left {
            return false;
        }

        let Some(guard) = &self.clipboard_image_click_guard else {
            return false;
        };
        if !guard.suppress_terminal_mouse_click {
            return false;
        }

        let mods = self.input_handler.modifiers.state();
        if mods.alt_key() || mods.control_key() || mods.shift_key() || mods.super_key() {
            return false;
        }

        // When mouse tracking is active the click is forwarded to the PTY application, not
        // consumed as a local text selection.  There is no risk of the clipboard image being
        // overwritten by a selection, so let the click through.
        // finish_clipboard_image_click_guard will still restore the image afterwards if the PTY
        // app happened to change the clipboard (e.g. a TUI doing an internal copy).
        // Without this early return, plain clicks in tmux are swallowed here, so pane-switching
        // by clicking never reaches tmux.
        if self.active_terminal_mouse_tracking_enabled_at(mouse_position) {
            return false;
        }

        let dx = mouse_position.0 - guard.press_position.0;
        let dy = mouse_position.1 - guard.press_position.1;
        const CLICK_RESTORE_THRESHOLD_PX: f64 = 6.0;
        // Suppress the full press/release pair if this started as a plain click-like gesture.
        (dx * dx + dy * dy) <= CLICK_RESTORE_THRESHOLD_PX * CLICK_RESTORE_THRESHOLD_PX
    }

    pub(crate) fn maybe_forward_guarded_terminal_mouse_press_on_drag(
        &mut self,
        position: (f64, f64),
    ) {
        let should_release_guard = self
            .clipboard_image_click_guard
            .as_ref()
            .is_some_and(|guard| {
                if !guard.suppress_terminal_mouse_click {
                    return false;
                }
                let dx = position.0 - guard.press_position.0;
                let dy = position.1 - guard.press_position.1;
                const CLICK_RESTORE_THRESHOLD_PX: f64 = 6.0;
                (dx * dx + dy * dy) > CLICK_RESTORE_THRESHOLD_PX * CLICK_RESTORE_THRESHOLD_PX
            });

        if !should_release_guard {
            return;
        }

        if let Some(guard) = &mut self.clipboard_image_click_guard {
            guard.suppress_terminal_mouse_click = false;
        }

        if self.try_send_mouse_event(0, true) {
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.mouse.button_pressed = true;
            }
            crate::debug_log!(
                "MOUSE",
                "Released image clipboard click guard on drag; forwarded delayed mouse press"
            );
        } else {
            crate::debug_log!(
                "MOUSE",
                "Released image clipboard click guard on drag; terminal mouse tracking unavailable"
            );
        }
    }
}
