//! Shader and shell integration install state.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

use crate::types::{InstallPromptState, IntegrationVersions, ShaderInstallPrompt};
use serde::{Deserialize, Serialize};

/// Install prompts and version tracking for bundled shaders and shell integration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IntegrationConfig {
    /// Shader install prompt preference
    /// - ask: Prompt user to install shaders if folder is missing/empty (default)
    /// - never: User declined, don't ask again
    /// - installed: Shaders have been installed
    #[serde(default)]
    pub shader_install_prompt: ShaderInstallPrompt,

    /// Shell integration install state
    #[serde(default)]
    pub shell_integration_state: InstallPromptState,

    /// Version tracking for integrations
    #[serde(default)]
    pub integration_versions: IntegrationVersions,
}
