//! Snippet variable substitution and insertion engine.
//!
//! This module provides:
//! - Variable substitution for snippets (built-in and custom variables)
//! - Session variable access (hostname, username, path, job, etc.)
//! - Snippet text processing with \(variable) syntax
//! - Integration with the terminal for text insertion

use crate::badge::SessionVariables;
use crate::config::snippets::BuiltInVariable;
use regex::Regex;
use std::collections::HashMap;

/// Error type for snippet substitution failures.
#[derive(Debug, Clone)]
pub enum SubstitutionError {
    /// Variable name is empty or invalid
    InvalidVariable(String),
    /// Variable is not defined (built-in or custom)
    UndefinedVariable(String),
}

impl std::fmt::Display for SubstitutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidVariable(name) => write!(f, "Invalid variable name: {}", name),
            Self::UndefinedVariable(name) => write!(f, "Undefined variable: {}", name),
        }
    }
}

impl std::error::Error for SubstitutionError {}

/// Result type for substitution operations.
pub type SubstitutionResult<T> = Result<T, SubstitutionError>;

/// Variable substitution engine for snippets.
///
/// Substitutes variables in the format \(variable_name) with their values.
/// Supports both built-in variables (date, time, hostname, etc.) and custom
/// variables defined per-snippet.
pub struct VariableSubstitutor {
    /// Regex pattern to match \(variable) syntax
    pattern: Regex,
}

impl VariableSubstitutor {
    /// Create a new variable substitutor.
    pub fn new() -> Self {
        // Match \(variable_name) where variable_name is alphanumeric + underscore + dot
        // Dot allows session.hostname style variables
        let pattern = Regex::new(r"\\\(([a-zA-Z_][a-zA-Z0-9_.]*)\)").unwrap();

        Self { pattern }
    }

    /// Substitute all variables in the given text.
    ///
    /// # Arguments
    /// * `text` - The text containing variables to substitute
    /// * `custom_vars` - Custom variables defined for this snippet
    ///
    /// # Returns
    /// The text with all variables replaced by their values.
    pub fn substitute(
        &self,
        text: &str,
        custom_vars: &HashMap<String, String>,
    ) -> SubstitutionResult<String> {
        self.substitute_with_session(text, custom_vars, None)
    }

    /// Substitute all variables in the given text, including session variables.
    ///
    /// # Arguments
    /// * `text` - The text containing variables to substitute
    /// * `custom_vars` - Custom variables defined for this snippet
    /// * `session_vars` - Optional session variables (hostname, path, job, etc.)
    ///
    /// # Returns
    /// The text with all variables replaced by their values.
    pub fn substitute_with_session(
        &self,
        text: &str,
        custom_vars: &HashMap<String, String>,
        session_vars: Option<&SessionVariables>,
    ) -> SubstitutionResult<String> {
        let mut result = text.to_string();

        // Find all variable placeholders
        for cap in self.pattern.captures_iter(text) {
            let full_match = cap.get(0).unwrap().as_str();
            let var_name = cap.get(1).unwrap().as_str();

            // Resolve the variable value
            let value = self.resolve_variable_with_session(var_name, custom_vars, session_vars)?;

            // Replace the placeholder with the value
            result = result.replace(full_match, &value);
        }

        Ok(result)
    }

    /// Resolve a single variable to its value, including session variables.
    fn resolve_variable_with_session(
        &self,
        name: &str,
        custom_vars: &HashMap<String, String>,
        session_vars: Option<&SessionVariables>,
    ) -> SubstitutionResult<String> {
        // Check custom variables first (highest priority)
        if let Some(value) = custom_vars.get(name) {
            return Ok(value.clone());
        }

        // Check session variables (second priority)
        if let Some(value) = session_vars.and_then(|session| session.get(name)) {
            return Ok(value);
        }

        // Check built-in variables (third priority)
        if let Some(builtin) = BuiltInVariable::parse(name) {
            return Ok(builtin.resolve());
        }

        // Variable not found
        Err(SubstitutionError::UndefinedVariable(name.to_string()))
    }

    /// Check if text contains any variables.
    pub fn has_variables(&self, text: &str) -> bool {
        self.pattern.is_match(text)
    }

    /// Extract all variable names from the text.
    pub fn extract_variables(&self, text: &str) -> Vec<String> {
        self.pattern
            .captures_iter(text)
            .map(|cap| cap.get(1).unwrap().as_str().to_string())
            .collect()
    }
}

impl Default for VariableSubstitutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_substitute_builtin_variables() {
        let substitutor = VariableSubstitutor::new();
        let custom_vars = HashMap::new();

        // Test date variable (should produce something like YYYY-MM-DD)
        let result = substitutor
            .substitute("Today is \\(date)", &custom_vars)
            .unwrap();
        assert!(result.starts_with("Today is "));
        assert!(!result.contains("\\(date)"));

        // Test user variable
        let result = substitutor
            .substitute("User: \\(user)", &custom_vars)
            .unwrap();
        assert!(result.starts_with("User: "));
        assert!(!result.contains("\\(user)"));
    }

    #[test]
    fn test_substitute_custom_variables() {
        let substitutor = VariableSubstitutor::new();
        let mut custom_vars = HashMap::new();
        custom_vars.insert("name".to_string(), "Alice".to_string());
        custom_vars.insert("project".to_string(), "par-term".to_string());

        let result = substitutor
            .substitute("Hello \\(name), welcome to \\(project)!", &custom_vars)
            .unwrap();

        assert_eq!(result, "Hello Alice, welcome to par-term!");
    }

    #[test]
    fn test_substitute_mixed_variables() {
        let substitutor = VariableSubstitutor::new();
        let mut custom_vars = HashMap::new();
        custom_vars.insert("greeting".to_string(), "Hello".to_string());

        let result = substitutor
            .substitute("\\(greeting) \\(user), today is \\(date)", &custom_vars)
            .unwrap();

        assert!(result.starts_with("Hello "));
        assert!(!result.contains("\\("));
    }

    #[test]
    fn test_undefined_variable() {
        let substitutor = VariableSubstitutor::new();
        let custom_vars = HashMap::new();

        let result = substitutor.substitute("Value: \\(undefined)", &custom_vars);

        assert!(result.is_err());
        match result.unwrap_err() {
            SubstitutionError::UndefinedVariable(name) => assert_eq!(name, "undefined"),
            _ => panic!("Expected UndefinedVariable error"),
        }
    }

    #[test]
    fn test_has_variables() {
        let substitutor = VariableSubstitutor::new();

        assert!(substitutor.has_variables("Hello \\(user)"));
        assert!(!substitutor.has_variables("Hello world"));
    }

    #[test]
    fn test_extract_variables() {
        let substitutor = VariableSubstitutor::new();

        let vars = substitutor.extract_variables("Hello \\(user), today is \\(date)");
        assert_eq!(vars, vec!["user", "date"]);
    }

    #[test]
    fn test_no_variables() {
        let substitutor = VariableSubstitutor::new();
        let custom_vars = HashMap::new();

        let result = substitutor
            .substitute("Just plain text with no variables", &custom_vars)
            .unwrap();

        assert_eq!(result, "Just plain text with no variables");
    }

    #[test]
    fn test_empty_custom_vars() {
        let substitutor = VariableSubstitutor::new();
        let custom_vars = HashMap::new();

        let result = substitutor
            .substitute("User: \\(user), Path: \\(path)", &custom_vars)
            .unwrap();

        // Should successfully substitute built-in variables
        assert!(result.contains("User:"));
        assert!(result.contains("Path:"));
        assert!(!result.contains("\\("));
    }

    #[test]
    fn test_duplicate_variables() {
        let substitutor = VariableSubstitutor::new();
        let mut custom_vars = HashMap::new();
        custom_vars.insert("name".to_string(), "Alice".to_string());

        let result = substitutor
            .substitute("\\(name) and \\(name) again", &custom_vars)
            .unwrap();

        assert_eq!(result, "Alice and Alice again");
    }

    #[test]
    fn test_escaped_backslash() {
        let substitutor = VariableSubstitutor::new();
        let custom_vars = HashMap::new();

        // Test that \( is the variable syntax, not just an escaped paren
        let result = substitutor
            .substitute("Use \\(user) for the username", &custom_vars)
            .unwrap();

        assert!(!result.contains("\\("));
        assert!(!result.contains("\\)"));
    }
}
