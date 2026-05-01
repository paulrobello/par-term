//! Conversion functions from `par-term-config` types to `par-term-emu-core-rust` types.
//!
//! These functions live in the terminal crate (which depends on both
//! `par-term-config` and `par-term-emu-core-rust`), keeping the foundation
//! config crate free of the emulation-core dependency (ARC-003).
//!
//! The orphan rule prevents `impl From<ForeignA> for ForeignB`, so these are
//! standalone functions. Callers should `use par_term_terminal::conversion::*`
//! or import individual functions by name.

use par_term_config::{
    AmbiguousWidth, NormalizationForm, RestartPolicy, SplitPaneCommand, TriggerActionConfig,
    TriggerSplitDirection, TriggerSplitTarget, UnicodeVersion,
};

/// Convert a config-layer `RestartPolicy` into the emu-core equivalent.
pub fn to_core_restart_policy(
    value: RestartPolicy,
) -> par_term_emu_core_rust::coprocess::RestartPolicy {
    match value {
        RestartPolicy::Never => par_term_emu_core_rust::coprocess::RestartPolicy::Never,
        RestartPolicy::Always => par_term_emu_core_rust::coprocess::RestartPolicy::Always,
        RestartPolicy::OnFailure => par_term_emu_core_rust::coprocess::RestartPolicy::OnFailure,
    }
}

/// Convert a config-layer `TriggerActionConfig` into the emu-core `TriggerAction`.
pub fn to_core_trigger_action(
    value: TriggerActionConfig,
) -> par_term_emu_core_rust::terminal::TriggerAction {
    use par_term_emu_core_rust::terminal::TriggerAction;
    match value {
        TriggerActionConfig::Highlight {
            fg,
            bg,
            duration_ms,
        } => TriggerAction::Highlight {
            fg: fg.map(|c| (c[0], c[1], c[2])),
            bg: bg.map(|c| (c[0], c[1], c[2])),
            duration_ms,
        },
        TriggerActionConfig::Notify { title, message } => TriggerAction::Notify { title, message },
        TriggerActionConfig::MarkLine { label, color } => TriggerAction::MarkLine {
            label,
            color: color.map(|c| (c[0], c[1], c[2])),
        },
        TriggerActionConfig::SetVariable { name, value } => {
            TriggerAction::SetVariable { name, value }
        }
        TriggerActionConfig::RunCommand { command, args } => {
            TriggerAction::RunCommand { command, args }
        }
        TriggerActionConfig::PlaySound { sound_id, volume } => {
            TriggerAction::PlaySound { sound_id, volume }
        }
        TriggerActionConfig::SendText { text, delay_ms } => {
            TriggerAction::SendText { text, delay_ms }
        }
        TriggerActionConfig::SplitPane {
            direction,
            command,
            focus_new_pane,
            target,
            split_percent: _,
        } => {
            let core_direction = to_core_trigger_split_direction(direction);
            let core_command = command.map(to_core_trigger_split_command);
            let core_target = to_core_trigger_split_target(target);
            TriggerAction::SplitPane {
                direction: core_direction,
                command: core_command,
                focus_new_pane,
                target: core_target,
            }
        }
    }
}

/// Convert a config-layer `TriggerSplitDirection` into the emu-core equivalent.
pub fn to_core_trigger_split_direction(
    value: TriggerSplitDirection,
) -> par_term_emu_core_rust::terminal::TriggerSplitDirection {
    match value {
        TriggerSplitDirection::Horizontal => {
            par_term_emu_core_rust::terminal::TriggerSplitDirection::Horizontal
        }
        TriggerSplitDirection::Vertical => {
            par_term_emu_core_rust::terminal::TriggerSplitDirection::Vertical
        }
    }
}

/// Convert a config-layer `TriggerSplitTarget` into the emu-core equivalent.
pub fn to_core_trigger_split_target(
    value: TriggerSplitTarget,
) -> par_term_emu_core_rust::terminal::TriggerSplitTarget {
    match value {
        TriggerSplitTarget::Active => par_term_emu_core_rust::terminal::TriggerSplitTarget::Active,
        TriggerSplitTarget::Source => par_term_emu_core_rust::terminal::TriggerSplitTarget::Source,
    }
}

/// Convert a config-layer `SplitPaneCommand` into the emu-core `TriggerSplitCommand`.
pub fn to_core_trigger_split_command(
    value: SplitPaneCommand,
) -> par_term_emu_core_rust::terminal::TriggerSplitCommand {
    match value {
        SplitPaneCommand::SendText { text, delay_ms } => {
            par_term_emu_core_rust::terminal::TriggerSplitCommand::SendText { text, delay_ms }
        }
        SplitPaneCommand::InitialCommand { command, args } => {
            par_term_emu_core_rust::terminal::TriggerSplitCommand::InitialCommand { command, args }
        }
    }
}

/// Convert a config-layer `UnicodeVersion` into the emu-core equivalent.
pub fn to_core_unicode_version(value: UnicodeVersion) -> par_term_emu_core_rust::UnicodeVersion {
    match value {
        UnicodeVersion::Unicode9 => par_term_emu_core_rust::UnicodeVersion::Unicode9,
        UnicodeVersion::Unicode10 => par_term_emu_core_rust::UnicodeVersion::Unicode10,
        UnicodeVersion::Unicode11 => par_term_emu_core_rust::UnicodeVersion::Unicode11,
        UnicodeVersion::Unicode12 => par_term_emu_core_rust::UnicodeVersion::Unicode12,
        UnicodeVersion::Unicode13 => par_term_emu_core_rust::UnicodeVersion::Unicode13,
        UnicodeVersion::Unicode14 => par_term_emu_core_rust::UnicodeVersion::Unicode14,
        UnicodeVersion::Unicode15 => par_term_emu_core_rust::UnicodeVersion::Unicode15,
        UnicodeVersion::Unicode15_1 => par_term_emu_core_rust::UnicodeVersion::Unicode15_1,
        UnicodeVersion::Unicode16 => par_term_emu_core_rust::UnicodeVersion::Unicode16,
        UnicodeVersion::Auto => par_term_emu_core_rust::UnicodeVersion::Auto,
    }
}

/// Convert a config-layer `AmbiguousWidth` into the emu-core equivalent.
pub fn to_core_ambiguous_width(value: AmbiguousWidth) -> par_term_emu_core_rust::AmbiguousWidth {
    match value {
        AmbiguousWidth::Narrow => par_term_emu_core_rust::AmbiguousWidth::Narrow,
        AmbiguousWidth::Wide => par_term_emu_core_rust::AmbiguousWidth::Wide,
    }
}

/// Convert a config-layer `NormalizationForm` into the emu-core equivalent.
pub fn to_core_normalization_form(
    value: NormalizationForm,
) -> par_term_emu_core_rust::NormalizationForm {
    match value {
        NormalizationForm::None => par_term_emu_core_rust::NormalizationForm::None,
        NormalizationForm::NFC => par_term_emu_core_rust::NormalizationForm::NFC,
        NormalizationForm::NFD => par_term_emu_core_rust::NormalizationForm::NFD,
        NormalizationForm::NFKC => par_term_emu_core_rust::NormalizationForm::NFKC,
        NormalizationForm::NFKD => par_term_emu_core_rust::NormalizationForm::NFKD,
    }
}
