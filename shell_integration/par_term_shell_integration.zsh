#!/bin/zsh
# par-term Shell Integration for Zsh
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
#   Add to your ~/.zshrc:
#     source "${XDG_CONFIG_HOME:-$HOME/.config}/par-term/shell_integration.zsh"

if [[ -o interactive ]]; then
  if [ "${ITERM_ENABLE_SHELL_INTEGRATION_WITH_TMUX-}""$TERM" != "tmux-256color" -a \
       "${ITERM_ENABLE_SHELL_INTEGRATION_WITH_TMUX-}""$TERM" != "screen" -a \
       "${PAR_TERM_SHELL_INTEGRATION_INSTALLED-}" = "" -a \
       "$TERM" != linux -a \
       "$TERM" != dumb ]; then

    PAR_TERM_SHELL_INTEGRATION_INSTALLED=Yes
    PAR_TERM_SHOULD_DECORATE_PROMPT="1"

    # OSC 133 ; C - Mark command execution start
    par_term_before_cmd_executes() {
      printf "\033]133;C;\007"
    }

    # Set a user-defined variable
    # Usage: par_term_set_user_var key value
    # Example: par_term_set_user_var git_branch "$(git branch --show-current 2>/dev/null)"
    par_term_set_user_var() {
      printf "\033]1337;SetUserVar=%s=%s\007" "$1" $(printf "%s" "$2" | base64 | tr -d '\n')
    }

    # Users can write their own version of this function
    # It should call par_term_set_user_var but not produce any other output
    whence -v par_term_print_user_vars > /dev/null 2>&1
    if [ $? -ne 0 ]; then
      par_term_print_user_vars() {
          true
      }
    fi

    # Send current directory and custom user vars
    par_term_print_state_data() {
      local _hostname=$(hostname -f 2>/dev/null || hostname)

      # OSC 7 - Set current directory
      printf "\033]7;file://%s%s\007" "$_hostname" "$PWD"

      # Custom user variables
      par_term_print_user_vars
    }

    # OSC 133 ; D ; exit_code - Report command exit status
    par_term_after_cmd_executes() {
      printf "\033]133;D;%s\007" "$STATUS"
      par_term_print_state_data
    }

    # OSC 133 ; A - Mark prompt start
    par_term_prompt_mark() {
      printf "\033]133;A\007"
    }

    # OSC 133 ; B - Mark prompt end (command input starts)
    par_term_prompt_end() {
      printf "\033]133;B\007"
    }

    # Zsh prompt lifecycle:
    #
    # 1) A command is entered at the prompt and you press return:
    #    * par_term_preexec is invoked
    #      * PS1 is set to PAR_TERM_PRECMD_PS1
    #      * PAR_TERM_SHOULD_DECORATE_PROMPT is set to 1
    #    * The command executes (possibly reading or modifying PS1)
    #    * par_term_precmd is invoked
    #      * PAR_TERM_PRECMD_PS1 is set to PS1 (as modified by command)
    #      * PS1 gets our escape sequences added to it
    #    * zsh displays your prompt
    #
    # 2) You press ^C while entering a command:
    #    * (par_term_preexec is NOT invoked)
    #    * par_term_precmd is invoked
    #      * par_term_before_cmd_executes is called
    #      * PS1 already has escape sequences
    #
    # 3) A new shell is born:
    #    * PS1 has initial value
    #    * par_term_precmd is invoked
    #      * PAR_TERM_SHOULD_DECORATE_PROMPT is set to 1
    #      * PAR_TERM_PRECMD_PS1 is set to initial PS1
    #      * PS1 gets our escape sequences added to it

    par_term_decorate_prompt() {
      # Save the raw PS1 without our escape sequences
      PAR_TERM_PRECMD_PS1="$PS1"
      PAR_TERM_SHOULD_DECORATE_PROMPT=""

      # Add escape sequences to PS1
      # Check if prompt already has our mark (for custom prompts)
      local PREFIX=""
      if [[ $PS1 == *"$(par_term_prompt_mark)"* ]]; then
        PREFIX=""
      else
        PREFIX="%{$(par_term_prompt_mark)%}"
      fi

      PS1="$PREFIX$PS1%{$(par_term_prompt_end)%}"
      PAR_TERM_DECORATED_PS1="$PS1"
    }

    # Called before each prompt display
    par_term_precmd() {
      local STATUS="$?"

      if [ -z "${PAR_TERM_SHOULD_DECORATE_PROMPT-}" ]; then
        # You pressed ^C while entering a command
        par_term_before_cmd_executes
        if [ "$PS1" != "${PAR_TERM_DECORATED_PS1-}" ]; then
          # PS1 changed, perhaps in another precmd
          PAR_TERM_SHOULD_DECORATE_PROMPT="1"
        fi
      fi

      par_term_after_cmd_executes "$STATUS"

      if [ -n "$PAR_TERM_SHOULD_DECORATE_PROMPT" ]; then
        par_term_decorate_prompt
      fi
    }

    # Called before each command execution (not run if you press ^C)
    par_term_preexec() {
      # Restore PS1 to its raw value before command execution
      PS1="$PAR_TERM_PRECMD_PS1"
      PAR_TERM_SHOULD_DECORATE_PROMPT="1"
      par_term_before_cmd_executes
    }

    # Cache hostname if not on macOS (where it can change with VPN)
    if [ -z "${par_term_hostname-}" ]; then
      if [ "$(uname)" != "Darwin" ]; then
        par_term_hostname=$(hostname -f 2>/dev/null)
        # Some BSDs don't have -f option
        if [ $? -ne 0 ]; then
          par_term_hostname=$(hostname)
        fi
      fi
    fi

    # Register our functions with zsh's hook arrays
    [[ -z ${precmd_functions-} ]] && precmd_functions=()
    precmd_functions=($precmd_functions par_term_precmd)

    [[ -z ${preexec_functions-} ]] && preexec_functions=()
    preexec_functions=($preexec_functions par_term_preexec)

    # Send initial state
    par_term_print_state_data

    # Identify shell integration version
    printf "\033]1337;ShellIntegrationVersion=1;shell=zsh\007"
  fi
fi
