//! Assistant sections (Assistant & Agents tab). Placement is set in
//! [`crate::layout`].
//!
//! - Panel settings (enabled, width, scope, view mode)
//! - Agent settings (default agent, auto-launch, auto-context)
//! - Custom Agent definitions (identity, run commands, env vars)
//! - Permission settings (auto-approve / yolo mode, terminal access, screenshot access)
//! - Prompt library
//!
//! | File | Contents |
//! |------|----------|
//! | `context_section.rs` | Panel section + Agent section (scope, view mode, auto-context) |
//! | `agent_config_section.rs` | Custom Agents section + Permissions section |
//! | `prompt_library.rs` | Prompt Library section |

pub(crate) mod agent_config_section;
pub(crate) mod context_section;
pub(crate) mod prompt_library;
mod state;

pub use state::AiInspectorTabState;
