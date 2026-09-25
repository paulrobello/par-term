//! Session environment over the command tier.
//!
//! The daemon outlives its clients, so a pane otherwise inherits whatever
//! environment the daemon was spawned with. par-term hands the daemon its
//! own shell environment (`build_shell_env`) as the session's env map:
//! `new-session -e NAME=VALUE` when it creates the session (the first pane
//! spawns inside that command, so a later `set-environment` would miss it)
//! and `set-environment -t $N NAME VALUE` per variable on reattach. Panes
//! spawned afterwards see the map; panes already running do not (tmux
//! semantics).
//!
//! Values can be secrets (`shell_env` tokens), so nothing here logs a
//! value or a reply body — the daemon's own parse errors echo the full
//! `NAME=VALUE` back.

use crate::client::MuxSessionClient;
use std::io;

/// Single-quote `value` for the daemon's bounded quoting grammar: the
/// quote closes, an escaped `'` follows, and the quote reopens (`'\''`).
pub fn quote_env_value(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

/// Whether `name`/`value` can travel on the line-framed wire at all. A
/// newline would end the command mid-value; the daemon refuses NUL, `=`
/// in a name, and an empty name.
pub fn is_wire_safe(name: &str, value: &str) -> bool {
    let bad = |s: &str| s.contains(['\n', '\r', '\0']);
    !name.is_empty() && !name.contains('=') && !bad(name) && !bad(value)
}

/// The `-e 'NAME=VALUE'` arguments for `new-session`, sorted by name so the
/// command is deterministic, with wire-unsafe entries dropped (their names
/// returned for the caller to report).
pub fn new_session_env_args<'a>(
    env: impl IntoIterator<Item = (&'a String, &'a String)>,
) -> (String, Vec<String>) {
    let mut pairs: Vec<_> = env.into_iter().collect();
    pairs.sort();
    let mut args = String::new();
    let mut skipped = Vec::new();
    for (name, value) in pairs {
        if is_wire_safe(name, value) {
            args.push_str(" -e ");
            args.push_str(&quote_env_value(&format!("{name}={value}")));
        } else {
            skipped.push(name.clone());
        }
    }
    (args, skipped)
}

impl MuxSessionClient {
    /// Set one variable in session `$session`'s environment. `Ok(false)`
    /// means the daemon refused it (a daemon that predates
    /// `set-environment`, or a name it rejects) — callers treat that as
    /// degraded, not fatal. The reply body is never surfaced: it can echo
    /// the value.
    pub fn set_session_var(&mut self, session: u64, name: &str, value: &str) -> io::Result<bool> {
        if !is_wire_safe(name, value) {
            return Ok(false);
        }
        let reply = self.send_checked(&format!(
            "set-environment -t ${session} {} {}",
            quote_env_value(name),
            quote_env_value(value)
        ))?;
        Ok(reply.ok)
    }

    /// Push every variable of `env` into session `$session`. Returns the
    /// names skipped because the wire cannot carry them, then the names
    /// the daemon refused, each sorted.
    pub fn push_session_env<'a>(
        &mut self,
        session: u64,
        env: impl IntoIterator<Item = (&'a String, &'a String)>,
    ) -> io::Result<(Vec<String>, Vec<String>)> {
        let mut pairs: Vec<_> = env.into_iter().collect();
        pairs.sort();
        let mut unsafe_names = Vec::new();
        let mut refused = Vec::new();
        for (name, value) in pairs {
            if !is_wire_safe(name, value) {
                unsafe_names.push(name.clone());
            } else if !self.set_session_var(session, name, value)? {
                refused.push(name.clone());
            }
        }
        Ok((unsafe_names, refused))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn quotes_values_with_embedded_single_quotes() {
        assert_eq!(quote_env_value("plain"), "'plain'");
        assert_eq!(quote_env_value("two words"), "'two words'");
        assert_eq!(quote_env_value("it's"), r"'it'\''s'");
        assert_eq!(quote_env_value(""), "''");
    }

    #[test]
    fn rejects_values_the_line_framed_wire_cannot_carry() {
        assert!(is_wire_safe("PATH", "/a:/b"));
        assert!(is_wire_safe("EMPTY", ""));
        assert!(!is_wire_safe("X", "line\nbreak"));
        assert!(!is_wire_safe("X", "nul\0"));
        assert!(!is_wire_safe("", "v"));
        assert!(!is_wire_safe("A=B", "v"));
    }

    #[test]
    fn new_session_args_are_sorted_and_skip_unsafe_entries() {
        let env: HashMap<String, String> = [
            ("B".to_string(), "two words".to_string()),
            ("A".to_string(), "1".to_string()),
            ("BAD".to_string(), "x\ny".to_string()),
        ]
        .into();
        let (args, skipped) = new_session_env_args(&env);
        assert_eq!(args, " -e 'A=1' -e 'B=two words'");
        assert_eq!(skipped, vec!["BAD".to_string()]);
    }
}
