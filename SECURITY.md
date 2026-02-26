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

MCP (Model Context Protocol) communication uses IPC files. The file permissions on these IPC endpoints are set by the operating system defaults rather than explicitly restricted by par-term.

> **Recommendation**: Ensure your system's default umask provides appropriate restrictions on IPC files, particularly on shared or multi-user systems.

### Clipboard Paste Behavior

par-term does not sanitize control characters when pasting clipboard content into the terminal. This is consistent with standard terminal emulator behavior -- pasted text is sent directly to the PTY. A crafted clipboard payload could contain escape sequences or control characters.

> **Recommendation**: Use the **Paste Special** feature for inspecting clipboard content before pasting into the terminal. Review clipboard contents when pasting from untrusted sources.

### Custom Shader Loading

par-term loads and transpiles custom GLSL shaders from the user's shader directory. Shaders execute on the GPU and cannot access the file system or network, but a malformed shader could cause GPU driver issues.

> **Recommendation**: Only load shaders from trusted sources. par-term writes transpiled WGSL to `/tmp/` for debugging, which can be inspected before use.

### Scripting Protocol

The scripting protocol defines `WriteText` and `RunCommand` commands (currently unimplemented). When implemented, these will allow observer scripts to write text to the terminal and execute commands. A security model for these capabilities will be defined before implementation.

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

- [Architecture](docs/ARCHITECTURE.md) - System design and component overview
- [Assistant Panel](docs/ASSISTANT_PANEL.md) - ACP agent integration and permissions
- [Automation](docs/AUTOMATION.md) - Trigger system, coprocesses, and scripting
- [Custom Shaders](docs/CUSTOM_SHADERS.md) - Shader loading and configuration
- [Profiles](docs/PROFILES.md) - Dynamic profile fetching and security
- [Session Logging](docs/SESSION_LOGGING.md) - Recording formats and configuration
- [SSH](docs/SSH.md) - SSH host management
- [Self Update](docs/SELF_UPDATE.md) - Update mechanism and zip extraction
- [Paste Special](docs/PASTE_SPECIAL.md) - Clipboard inspection before pasting
- [Snippets](docs/SNIPPETS.md) - Text snippets and custom actions
