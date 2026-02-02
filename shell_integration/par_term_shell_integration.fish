#!/usr/bin/env fish
# par-term Shell Integration for Fish
#
# This script provides shell integration markers for par-term terminal.
# Based on OSC 133 standard (also used by iTerm2, VSCode, etc.)
#
# Features:
# - Prompt navigation (jump between commands)
# - Command status tracking (exit codes)
# - Working directory tracking
# - Command duration measurement
# - Smart selection (select command vs output)
#
# Usage:
#   Add to your ~/.config/fish/config.fish:
#     set -l _pte_dir (begin; if set -q XDG_CONFIG_HOME; echo $XDG_CONFIG_HOME; else; echo $HOME/.config; end; end)/par-term
#     source "$_pte_dir/shell_integration.fish"
#
# Requires: Fish 2.3 or later

if begin; status --is-interactive; and \
          not functions -q -- par_term_status; and \
          test "$ITERM_ENABLE_SHELL_INTEGRATION_WITH_TMUX""$TERM" != screen; and \
          test "$ITERM_ENABLE_SHELL_INTEGRATION_WITH_TMUX""$TERM" != screen-256color; and \
          test "$ITERM_ENABLE_SHELL_INTEGRATION_WITH_TMUX""$TERM" != tmux-256color; and \
          test "$TERM" != dumb; and \
          test "$TERM" != linux; end

  # OSC 133 ; D ; exit_code - Report command exit status
  function par_term_status
    printf "\033]133;D;%s\007" $argv
  end

  # OSC 133 ; A - Mark prompt start
  function par_term_prompt_mark
    printf "\033]133;A\007"
  end

  # OSC 133 ; B - Mark prompt end (command input starts)
  function par_term_prompt_end
    printf "\033]133;B\007"
  end

  # OSC 133 ; C - Mark command execution start
  # This runs automatically before each command via fish_preexec event
  function par_term_preexec --on-event fish_preexec
    printf "\033]133;C;\007"
  end

  # Set a user-defined variable
  # Usage: par_term_set_user_var key value
  # Example: par_term_set_user_var git_branch (git branch --show-current 2>/dev/null)
  #
  # These variables can be accessed by the terminal emulator for:
  # - Display in status bars or badges
  # - Custom prompt decorations
  # - Integration with other tools
  function par_term_set_user_var
    printf "\033]1337;SetUserVar=%s=%s\007" $argv[1] (printf "%s" $argv[2] | base64 | tr -d "\n")
  end

  # Send current directory and custom user vars
  function par_term_write_remotehost_currentdir_uservars
    set -l hostname_value (hostname -f 2>/dev/null; or hostname)

    # OSC 7 - Set current directory (file:// URL format)
    printf "\033]7;file://%s%s\007" $hostname_value $PWD

    # Users can define a function called par_term_print_user_vars
    # It should call par_term_set_user_var and produce no other output
    # Example:
    #   function par_term_print_user_vars
    #     set -l git_branch (git branch --show-current 2>/dev/null)
    #     if test -n "$git_branch"
    #       par_term_set_user_var git_branch $git_branch
    #     end
    #   end
    if functions -q -- par_term_print_user_vars
      par_term_print_user_vars
    end
  end

  # Save the original fish_prompt function
  functions -c fish_prompt par_term_fish_prompt

  # Common prompt logic that runs before displaying the prompt
  function par_term_common_prompt
    set -l last_status $status

    # Report last command exit status
    par_term_status $last_status

    # Send current directory and user vars
    par_term_write_remotehost_currentdir_uservars

    # Add prompt mark if not already in the prompt function
    if not functions par_term_fish_prompt | string match -q "*par_term_prompt_mark*"
      par_term_prompt_mark
    end

    return $last_status
  end

  # Check if a function is defined and non-empty
  function par_term_check_function -d "Check if function is defined and non-empty"
    test (functions $argv[1] | grep -cvE '^ *(#|function |end$|$)') != 0
  end

  # Handle fish_mode_prompt (for vi mode indicator, etc.)
  if par_term_check_function fish_mode_prompt
    # Only override if non-empty (workaround for starship and similar tools)
    functions -c fish_mode_prompt par_term_fish_mode_prompt

    function fish_mode_prompt --description 'Write out the mode prompt; do not replace this. Instead, change fish_mode_prompt before sourcing par_term_shell_integration.fish, or modify par_term_fish_mode_prompt instead.'
      par_term_common_prompt
      par_term_fish_mode_prompt $argv
    end

    function fish_prompt --description 'Write out the prompt; do not replace this. Instead, change fish_prompt before sourcing par_term_shell_integration.fish, or modify par_term_fish_prompt instead.'
      # Remove trailing newline and print the prompt
      # Use %b for printf to correctly interpret escape codes
      printf "%b" (string join "\n" -- (par_term_fish_prompt $argv))

      # Mark end of prompt
      par_term_prompt_end
    end
  else
    # fish_mode_prompt is empty or unset
    function fish_prompt --description 'Write out the prompt; do not replace this. Instead, change fish_prompt before sourcing par_term_shell_integration.fish, or modify par_term_fish_prompt instead.'
      par_term_common_prompt

      # Remove trailing newline and print the prompt
      # Use %b for printf to correctly interpret escape codes
      printf "%b" (string join "\n" -- (par_term_fish_prompt $argv))

      # Mark end of prompt
      par_term_prompt_end
    end
  end

  # Send initial state
  par_term_write_remotehost_currentdir_uservars

  # Identify shell integration version
  printf "\033]1337;ShellIntegrationVersion=1;shell=fish\007"
end
