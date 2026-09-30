//! The program a profile tab runs, resolved once and kept on the tab and its
//! first pane: Restart Pane reruns it (UX.md A9) and splits inherit it when
//! `split_inherits_profile` is on (D5).

use crate::config::Config;
use crate::pane::LaunchCommand;
use crate::profile::Profile;

/// The program a profile tab runs, or `None` when it runs the configured
/// shell. Priority: `ssh_host` (ssh with user/port/identity args), then
/// `command` as-is, then `shell` with the login-shell flag and `SHELL` set.
pub(crate) fn profile_launch_command(profile: &Profile, config: &Config) -> Option<LaunchCommand> {
    if let Some(ssh_args) = profile.ssh_command_args() {
        return Some(LaunchCommand {
            program: "ssh".to_string(),
            args: Some(ssh_args),
            extra_env: Vec::new(),
        });
    }
    if let Some(ref cmd) = profile.command {
        return Some(LaunchCommand {
            program: cmd.clone(),
            args: profile.command_args.clone(),
            extra_env: Vec::new(),
        });
    }
    let shell = profile.shell.as_ref()?;
    // Per-profile login_shell overrides global config.login_shell.
    #[cfg(not(target_os = "windows"))]
    let args = profile
        .login_shell
        .unwrap_or(config.shell.login_shell)
        .then(|| vec!["-l".to_string()]);
    #[cfg(target_os = "windows")]
    let args = {
        let _ = config;
        None
    };
    // SHELL reflects the selected shell for child processes, not the login shell.
    Some(LaunchCommand {
        program: shell.clone(),
        args,
        extra_env: vec![("SHELL".to_string(), shell.clone())],
    })
}

/// Whether a split of a tab opened from `profile` inherits its program.
/// A profile that attaches a tmux or par-mux session is excluded: its
/// program would attach a second time.
pub(crate) fn inheritable_launch(profile: &Profile, config: &Config) -> Option<LaunchCommand> {
    if profile.tmux_session_name.is_some() || profile.mux_session_name.is_some() {
        return None;
    }
    profile_launch_command(profile, config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ssh_profile_launches_ssh_with_its_connection_args() {
        let mut profile = Profile::new("box");
        profile.ssh_host = Some("build.example.com".to_string());
        profile.ssh_user = Some("deploy".to_string());
        profile.ssh_port = Some(2222);
        let launch = profile_launch_command(&profile, &Config::default()).expect("ssh launch");
        assert_eq!(launch.program, "ssh");
        let args = launch.args.expect("ssh args");
        assert!(args.iter().any(|a| a.contains("build.example.com")));
        assert!(args.iter().any(|a| a == "2222"));
    }

    #[test]
    fn a_command_profile_launches_its_command_and_a_plain_profile_none() {
        let mut profile = Profile::new("htop");
        assert_eq!(profile_launch_command(&profile, &Config::default()), None);
        profile.command = Some("htop".to_string());
        profile.command_args = Some(vec!["-d".to_string(), "10".to_string()]);
        assert_eq!(
            profile_launch_command(&profile, &Config::default()),
            Some(LaunchCommand::new(
                "htop".to_string(),
                vec!["-d".to_string(), "10".to_string()]
            ))
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn a_shell_profile_sets_shell_and_honors_its_login_flag() {
        let mut profile = Profile::new("fish");
        profile.shell = Some("/usr/bin/fish".to_string());
        profile.login_shell = Some(true);
        let launch = profile_launch_command(&profile, &Config::default()).expect("shell");
        assert_eq!(launch.program, "/usr/bin/fish");
        assert_eq!(launch.args, Some(vec!["-l".to_string()]));
        assert_eq!(
            launch.extra_env,
            vec![("SHELL".to_string(), "/usr/bin/fish".to_string())]
        );
        profile.login_shell = Some(false);
        assert_eq!(
            profile_launch_command(&profile, &Config::default()).and_then(|l| l.args),
            None
        );
    }

    #[test]
    fn a_session_attaching_profile_is_not_inherited_by_splits() {
        let mut profile = Profile::new("work");
        profile.command = Some("htop".to_string());
        assert!(inheritable_launch(&profile, &Config::default()).is_some());
        profile.mux_session_name = Some("work".to_string());
        assert_eq!(inheritable_launch(&profile, &Config::default()), None);
        profile.mux_session_name = None;
        profile.tmux_session_name = Some("work".to_string());
        assert_eq!(inheritable_launch(&profile, &Config::default()), None);
    }
}
