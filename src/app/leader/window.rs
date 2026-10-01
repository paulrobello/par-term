//! `WindowState` side of the leader: build [`PressFacts`] from a key
//! press, ask [`decide`], and apply the step. Both key entry points — the
//! winit handler and the `--ui-test` chord injector — call
//! [`WindowState::handle_leader_press`], so a ui-test drives the real path.

use super::table::{self, TableKey};
use super::{ArmedBy, LeaderStep, LeaderTiming, PressFacts, decide};
use crate::app::window_state::WindowState;
use std::time::Instant;
use winit::keyboard::{Key, NamedKey, PhysicalKey};

/// One key press as the leader's entry point takes it. winit's `KeyEvent`
/// cannot be built outside winit, so the injector and the tests pass the
/// fields a real event carries.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LeaderPress<'a> {
    pub(crate) logical: &'a Key,
    pub(crate) physical: PhysicalKey,
    pub(crate) pressed: bool,
    pub(crate) os_repeat: bool,
}

impl<'a> LeaderPress<'a> {
    pub(crate) fn from_event(event: &'a winit::event::KeyEvent) -> Self {
        Self {
            logical: &event.logical_key,
            physical: event.physical_key,
            pressed: event.state == winit::event::ElementState::Pressed,
            os_repeat: event.repeat,
        }
    }

    /// A first press of `logical`.
    #[cfg(test)]
    pub(crate) fn pressed(logical: &'a Key) -> Self {
        Self {
            logical,
            physical: PhysicalKey::Unidentified(winit::keyboard::NativeKeyCode::Unidentified),
            pressed: true,
            os_repeat: false,
        }
    }
}

impl WindowState {
    /// Give one key press to the leader. Returns the step taken when the
    /// leader owned the press (the caller stops there), `None` when normal
    /// key handling continues. Reads the modifier state from
    /// `input_handler`, which both callers have updated for this press.
    pub(crate) fn handle_leader_press(&mut self, press: LeaderPress<'_>) -> Option<LeaderStep> {
        let now = Instant::now();
        // The timer may not have ticked yet: an arm past its deadline must
        // not swallow the next key.
        if self.leader.expired(now) {
            self.disarm_leader();
        }

        let config = self.config.load();
        let modifiers = self.input_handler.modifiers;
        let tmux_tab = self.leader_tmux_tab();
        let modifier_only = matches!(
            press.logical,
            Key::Named(
                NamedKey::Shift
                    | NamedKey::Control
                    | NamedKey::Alt
                    | NamedKey::Super
                    | NamedKey::Meta
            )
        );
        let is_leader = !modifier_only
            && self
                .leader
                .combo_for(&config.input.leader_key)
                .is_some_and(|combo| {
                    par_term_keybindings::KeybindingMatcher::from_key_fields_with_remapping(
                        press.logical,
                        press.physical,
                        &modifiers,
                        &config.input.modifier_remapping,
                    )
                    .matches_with_physical_preference(combo, config.input.use_physical_keys)
                });
        let is_tmux_prefix = tmux_tab
            && !modifier_only
            && self
                .tmux_state
                .tmux_prefix_key
                .as_ref()
                .is_some_and(|prefix| prefix.matches(press.logical, modifiers.state()));
        let facts = PressFacts {
            pressed: press.pressed,
            os_repeat: press.os_repeat,
            modifier_only,
            escape: matches!(press.logical, Key::Named(NamedKey::Escape)),
            is_leader,
            is_tmux_prefix,
            table_key: TableKey::from_key(press.logical, modifiers.state()),
        };
        let vim_keys = config.input.leader_vim_keys;
        let timing = LeaderTiming::from_config(&config.input);
        drop(config);

        let step = decide(self.leader.is_armed(), &facts, vim_keys, tmux_tab)?;
        self.apply_leader_step(step, press, &facts, now, timing);
        Some(step)
    }

    /// Whether leader keys tmux's prefix table owns go to tmux: a tmux
    /// gateway is connected and the active tab is its gateway or one of
    /// its window tabs. A local tab opened beside a gateway keeps
    /// par-term's actions, so leader `x` there never kills a tmux pane.
    pub(crate) fn leader_tmux_tab(&self) -> bool {
        self.config.load().tmux.tmux_enabled
            && self.is_tmux_connected()
            && self
                .tab_manager
                .active_tab()
                .is_some_and(|tab| tab.tmux.tmux_gateway_active || tab.tmux.tmux_pane_id.is_some())
    }

    fn apply_leader_step(
        &mut self,
        step: LeaderStep,
        press: LeaderPress<'_>,
        facts: &PressFacts,
        now: Instant,
        timing: LeaderTiming,
    ) {
        match step {
            LeaderStep::Arm(by) => {
                crate::debug_info!("LEADER", "armed by {by:?}");
                self.leader.arm(now, by, timing);
                self.overlay_state.which_key = None;
                self.focus_state.needs_redraw = true;
                self.request_redraw();
            }
            LeaderStep::Swallow => {}
            LeaderStep::Cancel => self.disarm_leader(),
            LeaderStep::Literal => {
                self.disarm_leader();
                self.send_leader_literal(press);
            }
            LeaderStep::Unbound => {
                self.disarm_leader();
                let what = facts
                    .table_key
                    .map_or_else(|| "that key".to_string(), TableKey::label);
                self.show_toast(format!("Leader: {what} is not bound"));
            }
            LeaderStep::Run { action, repeat } => {
                // Disarm first: an action that opens a mode (resize, pane
                // letters) must own the next key, not the leader.
                self.after_leader_key(repeat, now, timing);
                crate::debug_info!("LEADER", "running {action}");
                self.execute_keybinding_action(action);
            }
            LeaderStep::Tmux { key, repeat } => {
                self.after_leader_key(repeat, now, timing);
                let vim_keys = self.config.load().input.leader_vim_keys;
                if let Some(tmux_key) = table::tmux_key(key, vim_keys) {
                    self.run_tmux_prefix_key(&tmux_key);
                }
            }
        }
    }

    fn after_leader_key(&mut self, repeat: bool, now: Instant, timing: LeaderTiming) {
        if repeat {
            self.leader.refresh(now, timing);
        } else {
            self.disarm_leader();
        }
    }

    /// Disarm and drop the which-key overlay.
    pub(crate) fn disarm_leader(&mut self) {
        if !self.leader.is_armed() && self.overlay_state.which_key.is_none() {
            return;
        }
        self.leader.disarm();
        self.overlay_state.which_key = None;
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    /// The leader pressed twice (K8): the chord goes to the pane as the
    /// bytes the terminal encoder gives it — what the key sends with the
    /// leader off — through the typed-input route (par-mux and tmux panes
    /// via the daemon or gateway, local panes to their PTY).
    fn send_leader_literal(&mut self, press: LeaderPress<'_>) {
        let (modify_other_keys, application_cursor) = self.focused_key_modes();
        let input = par_term_input::KeyInput {
            logical_key: press.logical.clone(),
            physical_key: press.physical,
            state: winit::event::ElementState::Pressed,
        };
        match self.input_handler.handle_key_input_with_mode(
            &input,
            modify_other_keys,
            application_cursor,
        ) {
            Some(bytes) => self.send_typed_bytes(bytes),
            None => log::debug!("Leader literal: the chord has no terminal encoding"),
        }
    }

    /// The focused pane's key-encoding modes, or the tab's cached ones
    /// under lock contention (the key handler's priority order).
    fn focused_key_modes(&self) -> (u8, bool) {
        let Some(tab) = self.tab_manager.active_tab() else {
            return (0, false);
        };
        if let Some(pane) = tab.pane_manager().and_then(|pm| pm.focused_pane())
            && let Ok(term) = pane.terminal.try_read()
        {
            return (term.modify_other_keys_mode(), term.application_cursor());
        }
        let (modify_other_keys, application_cursor, _) = tab.read_or_cached_modes();
        (modify_other_keys, application_cursor)
    }

    /// Per-frame leader upkeep from `about_to_wait`: expire a timed-out
    /// arm, show the which-key overlay once its delay passes, and return
    /// when the event loop must next wake for it.
    pub(crate) fn tick_leader(&mut self, now: Instant) -> Option<Instant> {
        if self.leader.expired(now) {
            self.disarm_leader();
            return None;
        }
        if self.leader.overlay_due(now) && self.overlay_state.which_key.is_none() {
            self.overlay_state.which_key = Some(self.build_which_key());
            self.focus_state.needs_redraw = true;
        }
        self.leader.next_wake(now)
    }

    /// The which-key snapshot for the live table and the live registry.
    pub(crate) fn build_which_key(&self) -> super::WhichKey {
        let catalog = crate::command_palette::catalog::build_catalog();
        let label_of = |action: &str| {
            catalog
                .iter()
                .find(|entry| entry.action_id == action)
                .map_or_else(
                    || crate::command_palette::catalog::humanize(action),
                    |entry| entry.label.clone(),
                )
        };
        let leader_chord = self
            .keybinding_leader_chord()
            .unwrap_or_else(|| "leader".to_string());
        super::which_key::build(
            &super::which_key::WhichKeyInputs {
                vim_keys: self.config.load().input.leader_vim_keys,
                tmux_tab: self.leader_tmux_tab(),
                armed_by: self.leader.armed_by().unwrap_or(ArmedBy::Leader),
                leader_chord,
            },
            |action| self.live_chord_hint(action),
            label_of,
        )
    }

    /// The configured leader chord, spelled the way the palette spells
    /// chords.
    fn keybinding_leader_chord(&self) -> Option<String> {
        let source = self.config.load().input.leader_key.clone();
        par_term_keybindings::parser::parse_key_combo(source.trim())
            .ok()
            .map(|combo| {
                crate::command_palette::catalog::chord_display(&combo.platform_normalized())
            })
    }
}
