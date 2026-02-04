#!/bin/bash
# par-term Shell Integration for Bash
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
#   Add to your ~/.bashrc or ~/.bash_profile:
#     source "${XDG_CONFIG_HOME:-$HOME/.config}/par-term/shell_integration.bash"

# Only install if:
# - Running in interactive mode
# - Not already installed
# - Not in tmux/screen (unless explicitly enabled)
# - Not in dumb/linux terminal
if [[ "$ITERM_ENABLE_SHELL_INTEGRATION_WITH_TMUX""$TERM" != screen && \
      "$ITERM_ENABLE_SHELL_INTEGRATION_WITH_TMUX""$TERM" != tmux-256color && \
      "$PAR_TERM_SHELL_INTEGRATION_INSTALLED" = "" && \
      "$-" == *i* && \
      "$TERM" != linux && \
      "$TERM" != dumb ]]; then

# Check for incompatible bash features
if shopt extdebug | grep on > /dev/null; then
  echo "par-term Shell Integration not installed."
  echo ""
  echo "Your shell has 'extdebug' turned on."
  echo "This is incompatible with shell integration."
  echo "Find 'shopt -s extdebug' in bash's rc scripts and remove it."
  return 0
fi

PAR_TERM_SHELL_INTEGRATION_INSTALLED=Yes
PAR_TERM_PREV_PS1="$PS1"

# Install bash-preexec for precmd/preexec support
# This is a minimal inline version - for full version see https://github.com/rcaloras/bash-preexec
_install_bash_preexec () {

# Avoid duplicate inclusion
if [[ -n "${bash_preexec_imported:-}" ]]; then
    return 0
fi
bash_preexec_imported="defined"

__bp_last_ret_value="$?"
BP_PIPESTATUS=("${PIPESTATUS[@]}")
__bp_last_argument_prev_command="$_"

__bp_inside_precmd=0
__bp_inside_preexec=0
__bp_preexec_interactive_mode=""

declare -a precmd_functions
declare -a preexec_functions

__bp_trim_whitespace() {
    local var=${1:?} text=${2:-}
    text="${text#"${text%%[![:space:]]*}"}"
    text="${text%"${text##*[![:space:]]}"}"
    printf -v "$var" '%s' "$text"
}

__bp_interactive_mode() {
    __bp_preexec_interactive_mode="on";
}

__bp_precmd_invoke_cmd() {
    __bp_last_ret_value="$?" BP_PIPESTATUS=("${PIPESTATUS[@]}")

    if (( __bp_inside_precmd > 0 )); then
      (exit $__bp_last_ret_value)
      return
    fi
    local __bp_inside_precmd=1

    local precmd_function
    for precmd_function in "${precmd_functions[@]}"; do
        if type -t "$precmd_function" 1>/dev/null; then
            "$precmd_function"
        fi
    done
    (exit $__bp_last_ret_value)
}

__bp_set_ret_value() {
    return ${1:-}
}

__bp_in_prompt_command() {
    local prompt_command_array
    IFS=$'\n;' read -rd '' -a prompt_command_array <<< "${PROMPT_COMMAND:-}"

    local trimmed_arg
    __bp_trim_whitespace trimmed_arg "${1:-}"

    local command trimmed_command
    for command in "${prompt_command_array[@]:-}"; do
        __bp_trim_whitespace trimmed_command "$command"
        if [[ "$trimmed_command" == "$trimmed_arg" ]]; then
            return 0
        fi
    done
    return 1
}

__bp_preexec_invoke_exec() {
    __bp_last_argument_prev_command="${1:-}"

    if (( __bp_inside_preexec > 0 )); then
      return
    fi
    local __bp_inside_preexec=1

    if [[ ! -t 1 && -z "${__bp_delay_install:-}" ]]; then
        return
    fi

    if [[ -n "${COMP_LINE:-}" ]]; then
        return
    fi

    if [[ -z "${__bp_preexec_interactive_mode:-}" ]]; then
        return
    else
        if [[ 0 -eq "${BASH_SUBSHELL:-}" ]]; then
            __bp_preexec_interactive_mode=""
        fi
    fi

    if  __bp_in_prompt_command "${BASH_COMMAND:-}"; then
        __bp_preexec_interactive_mode=""
        return
    fi

    local this_command
    this_command=$(
        export LC_ALL=C
        HISTTIMEFORMAT= builtin history 1 | sed '1 s/^ *[0-9][0-9]*[* ] //'
    )

    if [[ -z "$this_command" ]]; then
        return
    fi

    local preexec_function
    for preexec_function in "${preexec_functions[@]:-}"; do
        if type -t "$preexec_function" 1>/dev/null; then
            "$preexec_function" "$this_command"
        fi
    done
}

__bp_install() {
    if [[ "${PROMPT_COMMAND:-}" == *"__bp_precmd_invoke_cmd"* ]]; then
        return 1;
    fi

    trap '__bp_preexec_invoke_exec "$_"' DEBUG

    PROMPT_COMMAND=$'__bp_precmd_invoke_cmd\n'
    if [[ -n "${existing_prompt_command:-}" ]]; then
        PROMPT_COMMAND+=${existing_prompt_command}$'\n'
    fi;
    PROMPT_COMMAND+='__bp_interactive_mode'

    precmd_functions+=(precmd)
    preexec_functions+=(preexec)

    __bp_precmd_invoke_cmd
    __bp_interactive_mode
}

local existing_prompt_command="${PROMPT_COMMAND:-}"
__bp_install

}
_install_bash_preexec
unset -f _install_bash_preexec

# par-term specific functions

function par_term_begin_osc {
  printf "\033]"
}

function par_term_end_osc {
  printf "\007"
}

function par_term_print_state_data() {
  local _hostname=$(hostname -f 2>/dev/null || hostname)

  # Send current directory (OSC 7)
  par_term_begin_osc
  printf "7;file://%s%s" "$_hostname" "$PWD"
  par_term_end_osc

  # Send custom user vars if defined
  par_term_print_user_vars
}

# Users can define this function to set custom variables
# Example:
#   function par_term_print_user_vars() {
#     par_term_set_user_var "git_branch" "$(git branch --show-current 2>/dev/null)"
#   }
if [ -z "$(type -t par_term_print_user_vars)" ] || [ "$(type -t par_term_print_user_vars)" != function ]; then
  function par_term_print_user_vars() {
    true
  }
fi

# Set a user-defined variable
# Usage: par_term_set_user_var key value
function par_term_set_user_var() {
  par_term_begin_osc
  printf "1337;SetUserVar=%s=%s" "$1" $(printf "%s" "$2" | base64 | tr -d '\n')
  par_term_end_osc
}

# OSC 133 ; A - Mark prompt start
function par_term_prompt_mark() {
  par_term_begin_osc
  printf "133;A"
  par_term_end_osc
}

# OSC 133 ; B - Mark prompt end (command input starts)
function par_term_prompt_suffix() {
  par_term_begin_osc
  printf "133;B"
  par_term_end_osc
}

# OSC 133 ; D ; exit_code - Report last command exit code
function par_term_prompt_prefix() {
  local _status="${1:-$__par_term_last_ret_value}"
  par_term_begin_osc
  printf "133;D;%s" "$_status"
  par_term_end_osc
}

# Runs before command execution
__par_term_preexec() {
    __par_term_last_ret_value="$?"

    # OSC 133 ; C - Mark command execution start
    par_term_begin_osc
    printf "133;C;"
    par_term_end_osc

    # Restore PS1 if we modified it
    if [ -n "${PAR_TERM_ORIG_PS1+xxx}" -a "$PS1" = "$PAR_TERM_PREV_PS1" ]; then
      export PS1="$PAR_TERM_ORIG_PS1"
    fi

    par_term_ran_preexec="yes"
    return 0
}

# Runs before prompt display
function __par_term_prompt_command () {
    local par_term_status="$?"
    __par_term_last_ret_value="$par_term_status"

    # Handle ^C (preexec didn't run)
    if [[ -z "${par_term_ran_preexec:-}" ]]; then
        ( exit $__par_term_last_ret_value )
        __par_term_preexec ""
    fi
    par_term_ran_preexec=""

    # Save original PS1 on first run
    if [ -z "${PAR_TERM_ORIG_PS1+xxx}" ]; then
      export PAR_TERM_ORIG_PS1="$PS1"
    fi

    # Allow dynamic PS1 generation
    if [ -n "$(type -t par_term_generate_ps1)" ] && [ "$(type -t par_term_generate_ps1)" = function ]; then
      par_term_generate_ps1
    fi

    # Update ORIG_PS1 if user changed PS1
    if [[ "$PS1" != "$PAR_TERM_PREV_PS1" ]]; then
      export PAR_TERM_ORIG_PS1="$PS1"
    fi

    # Get prompt prefix (with exit code)
    \local par_term_prompt_prefix_value="$(par_term_prompt_prefix "$par_term_status")"

    # Always emit a prompt marker so scrollback marks render even if PS1 already embeds one.
    par_term_prompt_mark

    # Add prompt mark unless PS1 already contains it
    if [[ $PAR_TERM_ORIG_PS1 != *'$(par_term_prompt_mark)'* ]]; then
      par_term_prompt_prefix_value="$par_term_prompt_prefix_value$(par_term_prompt_mark)"
    fi

    # Send state data (directory, etc.)
    par_term_print_state_data

    # Build PS1 with markers
    export PS1="\[$par_term_prompt_prefix_value\]$PAR_TERM_ORIG_PS1\[$(par_term_prompt_suffix)\]"
    export PAR_TERM_PREV_PS1="$PS1"
}

# Install our functions
preexec_functions+=(__par_term_preexec)
if [[ -n "$PROMPT_COMMAND" ]]; then
    PROMPT_COMMAND+=$'\n'
fi
PROMPT_COMMAND+='__par_term_prompt_command'

# Send initial state
par_term_print_state_data

# Identify shell integration version
par_term_begin_osc
printf "1337;ShellIntegrationVersion=1;shell=bash"
par_term_end_osc

fi  # End of installation guard
