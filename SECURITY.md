# Security Policy

Security policy and vulnerability reporting guidelines for par-term, a cross-platform GPU-accelerated terminal emulator.

## Table of Contents
- [Supported Versions](#supported-versions)
- [Reporting a Vulnerability](#reporting-a-vulnerability)
- [Security Model](#security-model)
  - [Language-Level Safety](#language-level-safety)
  - [ACP Agent Sandboxing](#acp-agent-sandboxing)
  - [Credential Leak Prevention](#credential-leak-prevention)
  - [TLS Certificate Verification](#tls-certificate-verification)
  - [Dynamic Profile Security](#dynamic-profile-security)
  - [Zip Extraction Protection](#zip-extraction-protection)
- [Known Security Considerations](#known-security-considerations)
  - [Session Logging](#session-logging)
  - [Trigger RunCommand](#trigger-runcommand)
  - [Config Variable Substitution](#config-variable-substitution)
  - [MCP IPC File Permissions](#mcp-ipc-file-permissions)
  - [Clipboard Paste Behavior](#clipboard-paste-behavior)
  - [Custom Shader Loading](#custom-shader-loading)
  - [Scripting Protocol](#scripting-protocol)
- [Best Practices](#best-practices)
- [Related Documentation](#related-documentation)

## Supported Versions

| Version | Supported |
|---------|-----------|
| Latest release | Yes |
| Previous releases | No |

Security fixes are applied to the current release only. Users should always run the latest version. par-term supports self-updating via the built-in update mechanism.

## Reporting a Vulnerability

> **Do not report security vulnerabilities through public GitHub issues.**

To report a vulnerability, email **probello@gmail.com** with the following details:

- Description of the vulnerability
- Steps to reproduce
- Affected component (PTY, ACP agents, shaders, SSH, MCP, scripting, etc.)
- Potential impact assessment
- Any suggested mitigations

**Expected response timeline:**
- **Acknowledgment**: 48-72 hours
- **Initial assessment**: Within 1 week
- **Fix timeline**: Depends on severity, communicated during assessment

Credit is given to reporters in release notes unless anonymity is requested.

## Security Model

### Language-Level Safety

par-term is written in Rust, which provides memory safety guarantees that eliminate entire vulnerability classes including buffer overflows, use-after-free, and data races. All `unsafe` blocks in the codebase are documented with `// SAFETY:` comments explaining the invariant being upheld.

### ACP Agent Sandboxing

ACP (Agent Communication Protocol) agents can read and write files on behalf of the user. par-term applies the following restrictions:

- **Path canonicalization**: All write paths are canonicalized to prevent symlink traversal
- **Restricted write directories**: Agent file writes are limited to safe root directories (`/tmp`, the shaders directory, and the config directory)
- **Sensitive command redaction**: Commands containing sensitive keywords (`password`, `token`, `secret`, `key`, `apikey`, `auth`, `credential`) are automatically redacted from auto-context sent to AI agents

### Credential Leak Prevention

par-term redacts sensitive commands from terminal context before sending it to ACP agents. This prevents accidental credential exposure when the AI agent receives auto-context from recent terminal output.

### TLS Certificate Verification

All HTTPS connections (dynamic profile fetching, self-update checks) use the platform's native certificate verifier. par-term does not bundle its own CA certificates or implement custom TLS verification logic.

### Dynamic Profile Security

Dynamic profiles can be fetched from remote URLs. par-term enforces a security boundary on the transport layer:

- **Auth headers blocked over plain HTTP**: If a dynamic profile URL uses `http://` instead of `https://`, par-term refuses to send authentication headers, preventing credential transmission over unencrypted connections

### Zip Extraction Protection

The self-update mechanism extracts zip archives. par-term uses `enclosed_name()` when extracting zip entries to prevent path traversal attacks (e.g., entries named `../../etc/passwd`).

## Known Security Considerations

These are behaviors users should be aware of when using par-term. They represent design decisions consistent with standard terminal emulator behavior or configurable features that carry inherent security implications.

### Session Logging

Session logging captures raw PTY I/O, which includes all text displayed in the terminal. This means:

- Passwords typed at prompts (even when hidden by the shell) pass through the PTY and are captured
- API keys, tokens, and other secrets displayed in terminal output are recorded
- Session logs in Asciicast format include timing data

> **Recommendation**: Store session logs in a location with appropriate file permissions. Delete logs containing sensitive data when no longer needed.

### Trigger RunCommand

The trigger system matches regex patterns against terminal output and can execute shell commands via the `RunCommand` action. A malicious program running inside the terminal could craft output that matches a broadly defined trigger pattern, causing unintended command execution.

> **Recommendation**: Define trigger patterns as narrowly as possible. Avoid overly broad regex patterns on triggers with `RunCommand` actions.

### Config Variable Substitution

par-term resolves `${VAR}` references in configuration values from environment variables. A configuration file from an untrusted source could use variable substitution to probe the user's environment.

> **Recommendation**: Review configuration files before importing them from untrusted sources. Be aware that variable references in config values resolve to the current environment.

### MCP IPC File Permissions

MCP (Model Context Protocol) communication uses IPC files under the par-term config directory. On Unix par-term creates them with mode `0600` at open time rather than adjusting permissions after the fact, so there is no window in which they are world-readable, and JSON payloads are written to a temporary file and renamed into place. Windows has no equivalent mode bit, so IPC files there inherit whatever the containing directory grants.

> **Recommendation**: On Windows, and whenever you redirect an IPC file with one of the `PAR_TERM_*_PATH` environment variables, choose a directory only your account can read.

### OSC 52 Clipboard Writes

Programs set the system clipboard with the OSC 52 escape sequence, and par-term applies those writes by default (`osc52_clipboard: true`). This is what lets tmux, remote editors, and workspace managers copy to your local clipboard over a plain SSH session. It also means any program holding the terminal -- including one on a remote host you do not control -- can silently replace what your next paste will insert.

The reverse direction is refused: par-term never enables the core terminal's OSC 52 clipboard *read*, so a program that queries the clipboard (`OSC 52 ; c ; ?`) receives no response and cannot exfiltrate what you copied.

This composes with paste behavior. Paste sanitization (below) strips escape sequences, so a staged clipboard cannot inject one -- but Tab, newline, and carriage return survive, because paste would be useless without them. A clipboard staged with a trailing newline therefore still submits its line the moment it is pasted into a shell that is not using bracketed paste.

> **Recommendation**: Set `osc52_clipboard: false` (Settings -> Input, "OSC 52 clipboard sync") if you do not need programs to write to your clipboard, particularly on hosts you do not control. Use **Paste Special** to review clipboard content before it reaches the PTY.

### Clipboard Paste Behavior

par-term strips control characters from clipboard content before it reaches the PTY: ESC, DEL, the C0 controls other than Tab, newline and carriage return, and the C1 range including CSI. When `warn_paste_control_chars` is enabled (the default) a paste that needed stripping also logs a warning, since such content may be crafted or binary.

What survives is ordinary text plus Tab, newline, and carriage return -- enough for a multi-line paste to execute as soon as it lands in a shell without bracketed paste. Sanitization removes the escape-sequence injection risk, not the risk of pasting a command you have not read.

> **Recommendation**: Use the **Paste Special** feature for inspecting and transforming clipboard content before pasting into the terminal. Review clipboard contents when pasting from untrusted sources.

### Custom Shader Loading

par-term loads and transpiles custom GLSL shaders from the user's shader directory. Shaders execute on the GPU and cannot access the file system or network, but a malformed shader could cause GPU driver issues.

> **Recommendation**: Only load shaders from trusted sources. Debug builds dump the transpiled WGSL to the system temp directory (`$TMPDIR` on macOS, `/tmp` on Linux, `%TEMP%` on Windows) so it can be inspected before use; the dump is created owner-only (`0600`) and refuses to write through a pre-existing symlink. Release builds emit no dump, and the `dev-release` profile behind `make build` and `make run` inherits `release`, so an ordinary local build produces none either.

### Scripting Protocol

Observer scripts drive the terminal through the scripting protocol. Its three privileged commands -- `WriteText`, `RunCommand`, and `ChangeConfig` -- are implemented, and each is opt-in per script: `allow_write_text`, `allow_run_command`, and `allow_change_config` all default to `false`, so a script that has not been granted a capability cannot use it.

Once granted:

- **`RunCommand`** tokenises the command and spawns the process directly rather than through a shell, checks it against par-term's command denylist, rate-limits it (1/s by default, `run_command_rate_limit`), and writes an audit line to the debug log.
- **`WriteText`** injects text into the active tab's PTY with VT/ANSI escape sequences stripped, rate-limited (10/s by default, `write_text_rate_limit`) and audit-logged. Printable characters and newlines pass through by design, so a script holding this capability can type a command and submit it. Unlike `RunCommand`, this path is not denylist-checked, and deliberately so: the denylist substring-matches a command plus its arguments, so run over free text it would reject ordinary script output containing `passwd` or `eval ` while missing the payloads that actually matter. The control here is confirmation instead. With `prompt_before_write_text` (default `true`), each payload is queued to the automation confirmation dialog, which shows the exact sanitized text that will be written, with the submitting newline rendered visibly as `\n`. **Always Allow** is scoped to that one script and that exact text for the rest of the session -- a payload that differs by so much as the trailing newline prompts again. Setting `prompt_before_write_text: false` restores the previous immediate write; sanitization and the rate limit still apply.
- **`ChangeConfig`** applies only keys on the runtime allowlist; anything else is rejected and logged.

> **Recommendation**: Leave the `allow_*` flags off unless a script genuinely needs them. Granting `allow_run_command` is equivalent to giving that script the ability to run commands as you, and so is granting `allow_write_text` once `prompt_before_write_text` is turned off -- with the prompt left on, each such command is one you have read and approved.

## Best Practices

1. **Keep par-term updated** -- Use the built-in self-update mechanism or check for new releases regularly
2. **Review imported configurations** -- Inspect `config.yaml` files before importing, especially from untrusted sources
3. **Narrow trigger patterns** -- Use specific regex patterns for triggers with `RunCommand` actions rather than broad matches
4. **Protect session logs** -- Store logs in restricted directories and clean up logs containing sensitive data
5. **Use HTTPS for dynamic profiles** -- Always use `https://` URLs for dynamic profile sources; par-term blocks auth headers over plain HTTP but the profile content itself would still be transmitted in the clear
6. **Review ACP agent permissions** -- Understand what file system access an ACP agent has before granting it
7. **Restrict shaders to trusted sources** -- Only install custom shaders from sources you trust
8. **Audit MCP configurations** -- Review MCP server configurations, especially on shared systems

## Related Documentation

- [Architecture](docs/architecture/ARCHITECTURE.md) - System design and component overview
- [Assistant Panel](docs/ASSISTANT_PANEL.md) - ACP agent integration and permissions
- [Automation](docs/features/AUTOMATION.md) - Trigger system, coprocesses, and scripting
- [Custom Shaders](docs/features/CUSTOM_SHADERS.md) - Shader loading and configuration
- [Profiles](docs/features/PROFILES.md) - Dynamic profile fetching and security
- [Session Logging](docs/features/SESSION_LOGGING.md) - Recording formats and configuration
- [SSH](docs/features/SSH.md) - SSH host management
- [Self Update](docs/features/SELF_UPDATE.md) - Update mechanism and zip extraction
- [Paste Special](docs/features/PASTE_SPECIAL.md) - Clipboard inspection before pasting
- [Snippets](docs/features/SNIPPETS.md) - Text snippets and custom actions
