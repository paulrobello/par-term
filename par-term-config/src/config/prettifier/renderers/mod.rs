//! Per-renderer configuration types for the Content Prettifier system.
//!
//! Each renderer (Markdown, JSON, YAML, TOML, XML, CSV, Diff, Log, SQL, Stack Trace,
//! Diagrams) has its own config type here, along with profile-level override types.
//!
//! # Sub-modules
//!
//! - [`toggle`] — Basic [`RendererToggle`] / [`RendererToggleOverride`] types shared by all renderers
//! - [`diff_log`] — [`DiffRendererConfig`] (diff renderer with side-by-side option)
//! - [`diagrams`] — [`DiagramRendererConfig`] (diagram renderer with engine selection)
//! - [`collection`] — [`RenderersConfig`] and [`RenderersConfigOverride`] (aggregate collections)
//! - [`custom`] — [`CustomRendererConfig`], [`FormatDetectionRulesConfig`], [`UserDetectionRule`], [`RuleOverride`]

mod collection;
mod custom;
mod diagrams;
mod diff_log;
mod toggle;

// Re-export everything so external callers can use flat paths like
// `config::prettifier::RenderersConfig` without knowing the sub-module layout.
pub use collection::{RenderersConfig, RenderersConfigOverride};
pub use custom::{
    CustomRendererConfig, FormatDetectionRulesConfig, RuleOverride, UserDetectionRule,
};
pub use diagrams::DiagramRendererConfig;
pub use diff_log::DiffRendererConfig;
pub use toggle::{RendererToggle, RendererToggleOverride};

// ---------------------------------------------------------------------------
// Default value functions (shared across sub-modules via pub(super))
// ---------------------------------------------------------------------------

pub(super) fn default_true() -> bool {
    true
}

pub(super) fn default_priority() -> i32 {
    50
}

pub(super) fn default_diagrams_priority() -> i32 {
    55
}

pub(super) fn default_rule_weight() -> f32 {
    0.3
}

pub(super) fn default_rule_scope() -> String {
    "any_line".to_string()
}
