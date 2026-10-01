//! The leader's pure half: decisions, timing, and the table, without a
//! window.

use super::table::{self, TableKey};
use super::which_key::{self, WhichKeyInputs};
use super::*;
use crate::pane::NavigationDirection;
use winit::keyboard::{Key, ModifiersState, NamedKey};

fn timing(timeout_ms: u64, delay_ms: u64) -> LeaderTiming {
    LeaderTiming {
        timeout: Duration::from_millis(timeout_ms),
        overlay_delay: Duration::from_millis(delay_ms),
    }
}

fn press() -> PressFacts {
    PressFacts {
        pressed: true,
        ..PressFacts::default()
    }
}

fn leader() -> PressFacts {
    PressFacts {
        is_leader: true,
        ..press()
    }
}

fn key(k: TableKey) -> PressFacts {
    PressFacts {
        table_key: Some(k),
        ..press()
    }
}

fn ch(c: char) -> PressFacts {
    key(TableKey::Char(c))
}

#[test]
fn the_leader_chord_arms_and_nothing_else_does() {
    assert_eq!(
        decide(false, &leader(), false, false),
        Some(LeaderStep::Arm(ArmedBy::Leader))
    );
    assert_eq!(decide(false, &ch('c'), false, false), None, "c types c");
    let release = PressFacts {
        pressed: false,
        ..leader()
    };
    assert_eq!(decide(false, &release, false, false), None);
    let held = PressFacts {
        os_repeat: true,
        ..leader()
    };
    assert_eq!(decide(false, &held, false, false), None);
}

#[test]
fn the_tmux_prefix_arms_the_same_machine() {
    let prefix = PressFacts {
        is_tmux_prefix: true,
        ..press()
    };
    assert_eq!(
        decide(false, &prefix, false, true),
        Some(LeaderStep::Arm(ArmedBy::TmuxPrefix))
    );
    // Pressed again it is the literal prefix, as in tmux.
    assert_eq!(
        decide(true, &prefix, false, true),
        Some(LeaderStep::Literal)
    );
}

#[test]
fn armed_releases_modifiers_and_auto_repeat_are_swallowed() {
    for facts in [
        PressFacts {
            pressed: false,
            ..ch('c')
        },
        PressFacts {
            modifier_only: true,
            ..press()
        },
        PressFacts {
            os_repeat: true,
            ..leader()
        },
    ] {
        assert_eq!(
            decide(true, &facts, false, false),
            Some(LeaderStep::Swallow),
            "{facts:?}"
        );
    }
}

#[test]
fn leader_twice_is_the_literal_chord_and_escape_cancels() {
    assert_eq!(
        decide(true, &leader(), false, false),
        Some(LeaderStep::Literal)
    );
    let escape = PressFacts {
        escape: true,
        ..press()
    };
    assert_eq!(
        decide(true, &escape, false, false),
        Some(LeaderStep::Cancel)
    );
}

#[test]
fn the_five_criterion_keys_run_their_actions() {
    for (c, action) in [
        ('c', "new_tab"),
        ('n', "next_tab"),
        ('p', "prev_tab"),
        ('x', "close_pane"),
        ('z', "toggle_pane_zoom"),
    ] {
        assert_eq!(
            decide(true, &ch(c), false, false),
            Some(LeaderStep::Run {
                action,
                repeat: false
            }),
            "{c}"
        );
    }
}

#[test]
fn repeat_keys_stay_armed_and_others_do_not() {
    let arrow = key(TableKey::Arrow {
        dir: NavigationDirection::Left,
        shift: false,
    });
    assert_eq!(
        decide(true, &arrow, false, false),
        Some(LeaderStep::Run {
            action: "navigate_pane_left",
            repeat: true
        })
    );
    assert_eq!(
        decide(true, &ch('o'), false, false),
        Some(LeaderStep::Run {
            action: "next_pane",
            repeat: true
        })
    );
    let swap = key(TableKey::Arrow {
        dir: NavigationDirection::Up,
        shift: true,
    });
    assert_eq!(
        decide(true, &swap, false, false),
        Some(LeaderStep::Run {
            action: "swap_pane_up",
            repeat: true
        })
    );
}

#[test]
fn an_unbound_key_disarms_and_says_so() {
    assert_eq!(
        decide(true, &ch('y'), false, false),
        Some(LeaderStep::Unbound)
    );
    // A follow-up with Ctrl/Alt/Cmd is a different key (no table key).
    assert_eq!(
        decide(true, &press(), false, false),
        Some(LeaderStep::Unbound)
    );
}

#[test]
fn vim_keys_are_opt_in_and_move_last_tab_to_tab() {
    assert_eq!(
        decide(true, &ch('l'), false, false),
        Some(LeaderStep::Run {
            action: "last_tab",
            repeat: false
        })
    );
    assert_eq!(
        decide(true, &ch('l'), true, false),
        Some(LeaderStep::Run {
            action: "navigate_pane_right",
            repeat: true
        })
    );
    assert_eq!(
        decide(true, &ch('H'), true, false),
        Some(LeaderStep::Run {
            action: "swap_pane_left",
            repeat: true
        })
    );
    assert_eq!(
        decide(true, &key(TableKey::Tab), true, false),
        Some(LeaderStep::Run {
            action: "last_tab",
            repeat: false
        })
    );
    assert_eq!(
        decide(true, &key(TableKey::Tab), false, false),
        Some(LeaderStep::Unbound)
    );
}

#[test]
fn a_gateway_tab_hands_tmux_its_own_keys_and_keeps_par_term_ui() {
    // tmux's table: new window, splits, pane focus, zoom, kill.
    for c in ['c', 'x', 'z', '%', '"', 'o', ';', 'd', '&', '!'] {
        assert!(
            matches!(
                decide(true, &ch(c), false, true),
                Some(LeaderStep::Tmux { .. })
            ),
            "{c} runs in tmux"
        );
    }
    // Tab navigation stays par-term's: par-term does not follow tmux's
    // current window, so tmux's next-window would leave the tab put.
    for (c, action) in [
        ('n', "next_tab"),
        ('p', "prev_tab"),
        ('l', "last_tab"),
        ('3', "switch_to_tab_3"),
    ] {
        assert_eq!(
            decide(true, &ch(c), false, true),
            Some(LeaderStep::Run {
                action,
                repeat: false
            }),
            "{c}"
        );
    }
    assert_eq!(
        decide(true, &ch('0'), false, true),
        Some(LeaderStep::Unbound),
        "0 selects tmux window 0, which no tab follows"
    );
    // par-term UI keys stay par-term's (tmux's versions draw nowhere).
    for (c, action) in [
        (':', "toggle_command_palette"),
        ('?', "toggle_help"),
        ('w', "toggle_tree_picker"),
        ('s', "toggle_session_picker"),
        ('q', "select_pane_hint"),
        (',', "rename_tab"),
    ] {
        assert_eq!(
            decide(true, &ch(c), false, true),
            Some(LeaderStep::Run {
                action,
                repeat: false
            }),
            "{c}"
        );
    }
    // Shift+Arrow has no tmux key: it stays par-term's swap.
    let swap = key(TableKey::Arrow {
        dir: NavigationDirection::Right,
        shift: true,
    });
    assert!(matches!(
        decide(true, &swap, false, true),
        Some(LeaderStep::Run {
            action: "swap_pane_right",
            ..
        })
    ));
    // A key only tmux knows still reaches tmux.
    assert_eq!(
        decide(true, &ch('('), false, true),
        Some(LeaderStep::Tmux {
            key: TableKey::Char('('),
            repeat: false
        })
    );
    // Arrows keep their repeat flag through tmux.
    let arrow = key(TableKey::Arrow {
        dir: NavigationDirection::Down,
        shift: false,
    });
    assert!(matches!(
        decide(true, &arrow, false, true),
        Some(LeaderStep::Tmux { repeat: true, .. })
    ));
}

#[test]
fn letter_case_comes_from_shift_not_the_logical_key() {
    let none = ModifiersState::empty();
    let shift = ModifiersState::SHIFT;
    // The ui-test injector delivers an upper-case letter without Shift.
    assert_eq!(
        TableKey::from_key(&Key::Character("C".into()), none),
        Some(TableKey::Char('c'))
    );
    assert_eq!(
        TableKey::from_key(&Key::Character("e".into()), shift),
        Some(TableKey::Char('E'))
    );
    // Punctuation is the glyph as typed, Shift or not.
    assert_eq!(
        TableKey::from_key(&Key::Character("%".into()), shift),
        Some(TableKey::Char('%'))
    );
    assert_eq!(
        TableKey::from_key(&Key::Named(NamedKey::ArrowLeft), shift),
        Some(TableKey::Arrow {
            dir: NavigationDirection::Left,
            shift: true
        })
    );
    for mods in [
        ModifiersState::CONTROL,
        ModifiersState::ALT,
        ModifiersState::SUPER,
    ] {
        assert_eq!(TableKey::from_key(&Key::Character("c".into()), mods), None);
    }
}

#[test]
fn the_overlay_comes_after_its_delay_and_the_arm_ends_at_the_timeout() {
    let t0 = Instant::now();
    let mut state = LeaderState::default();
    state.arm(t0, ArmedBy::Leader, timing(2000, 400));
    assert!(state.is_armed());
    assert!(!state.overlay_due(t0 + Duration::from_millis(399)));
    assert!(state.overlay_due(t0 + Duration::from_millis(400)));
    assert_eq!(state.next_wake(t0), Some(t0 + Duration::from_millis(400)));
    assert_eq!(
        state.next_wake(t0 + Duration::from_millis(500)),
        Some(t0 + Duration::from_millis(2000))
    );
    assert!(!state.expired(t0 + Duration::from_millis(1999)));
    assert!(state.expired(t0 + Duration::from_millis(2000)));

    // A repeat key restarts the timeout from its own press.
    state.refresh(t0 + Duration::from_millis(1500), timing(2000, 400));
    assert!(!state.expired(t0 + Duration::from_millis(3000)));
    assert!(state.expired(t0 + Duration::from_millis(3500)));

    state.disarm();
    assert!(!state.is_armed());
    assert_eq!(state.next_wake(t0), None);
}

#[test]
fn an_empty_or_bad_leader_key_disables_the_leader() {
    let mut state = LeaderState::default();
    assert!(state.combo_for("").is_none());
    assert!(state.combo_for("   ").is_none());
    assert!(state.combo_for("Ctrl+Ctrl").is_none());
    assert!(state.combo_for("Cmd+B").is_some());
}

#[test]
fn every_table_action_is_dispatchable() {
    let dispatchable: Vec<&str> = crate::app::input_events::keybinding_actions::ACTION_HANDLERS
        .iter()
        .map(|(id, _)| *id)
        .chain(
            crate::app::input_events::keybinding_display_actions::DISPLAY_ACTION_HANDLERS
                .iter()
                .map(|(id, _)| *id),
        )
        .collect();
    for binding in table::BASE_TABLE.iter().chain(table::VIM_TABLE) {
        assert!(
            dispatchable.contains(&binding.action),
            "leader {:?} names {}, which nothing dispatches",
            binding.key,
            binding.action
        );
    }
}

/// The leader owns its chord ahead of the registry, so an action the
/// Settings table advertises on the leader chord would never run (D3 moved
/// the background-shader toggle off Ctrl+Shift+B for this reason).
#[test]
fn no_advertised_chord_is_the_default_leader() {
    use par_term_keybindings::parser::parse_key_combo;
    let leader = parse_key_combo(&par_term_config::defaults::leader_key())
        .expect("leader parses")
        .platform_normalized();
    let clashes: Vec<&str> = par_term_settings_ui::input_tab::actions_table::AVAILABLE_ACTIONS
        .iter()
        .filter(|(_, _, chord)| {
            chord
                .and_then(|c| parse_key_combo(c).ok())
                .is_some_and(|c| c.platform_normalized() == leader)
        })
        .map(|(action, _, _)| *action)
        .collect();
    assert!(
        clashes.is_empty(),
        "AVAILABLE_ACTIONS advertises the leader chord for {clashes:?}"
    );
}

#[test]
fn every_key_in_each_table_is_unique() {
    for (name, tbl) in [("base", table::BASE_TABLE), ("vim", table::VIM_TABLE)] {
        let mut seen = Vec::new();
        for binding in tbl {
            assert!(
                !seen.contains(&binding.key),
                "{name} table binds {:?} twice",
                binding.key
            );
            seen.push(binding.key);
        }
    }
    // The vim table replaces base keys rather than duplicating them.
    let live = table::entries(true);
    let mut keys: Vec<TableKey> = live.iter().map(|b| b.key).collect();
    let before = keys.len();
    keys.dedup();
    assert_eq!(keys.len(), before);
    assert!(
        live.iter()
            .any(|b| b.key == TableKey::Char('l') && b.action == "navigate_pane_right")
    );
    assert!(
        !live
            .iter()
            .any(|b| b.action == "last_tab" && b.key == TableKey::Char('l'))
    );
}

#[test]
fn the_which_key_lists_live_chords_and_follows_a_rebind() {
    let inputs = WhichKeyInputs {
        vim_keys: false,
        tmux_tab: false,
        armed_by: ArmedBy::Leader,
        leader_chord: "Cmd+B".to_string(),
    };
    let chord_of = |action: &str| (action == "new_tab").then(|| "Cmd+T".to_string());
    let wk = which_key::build(&inputs, chord_of, |a| a.to_string());
    let row = |k: &str| wk.rows.iter().find(|r| r.key == k).expect(k).clone();
    assert_eq!(row("c").chord.as_deref(), Some("Cmd+T"));
    assert_eq!(row("z").action_id, "toggle_pane_zoom");
    assert!(row("o").repeat);
    assert_eq!(wk.rows.len(), table::BASE_TABLE.len());
    assert!(wk.title.contains("Cmd+B"));

    let rebound = which_key::build(
        &inputs,
        |action: &str| (action == "new_tab").then(|| "Ctrl+Alt+N".to_string()),
        |a| a.to_string(),
    );
    assert_eq!(
        rebound
            .rows
            .iter()
            .find(|r| r.key == "c")
            .unwrap()
            .chord
            .as_deref(),
        Some("Ctrl+Alt+N"),
        "the overlay reads the live registry, never a static chord"
    );
}

#[test]
fn a_gateway_which_key_marks_tmux_rows_and_adds_tmux_only_keys() {
    let wk = which_key::build(
        &WhichKeyInputs {
            vim_keys: false,
            tmux_tab: true,
            armed_by: ArmedBy::TmuxPrefix,
            leader_chord: "Cmd+B".to_string(),
        },
        |_| None,
        |a| a.to_string(),
    );
    let row = |k: &str| wk.rows.iter().find(|r| r.key == k).expect(k).clone();
    assert!(row("c").via_tmux);
    assert!(!row(":").via_tmux);
    assert!(row("(").via_tmux, "tmux's session keys stay listed");
    assert!(row("[").via_tmux);
    assert_eq!(wk.title, "tmux prefix");
}
