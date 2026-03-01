use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::{Duration, Instant};

use clap::Parser;
use par_term::ai_inspector::chat::{
    AGENT_SYSTEM_GUIDANCE, ChatMessage, ChatState, extract_inline_config_update,
};
use par_term::ai_inspector::shader_context::{
    build_shader_context, is_shader_activation_request, should_inject_shader_context,
};
use par_term_acp::agents::{ActionConfig, resolve_binary_in_path};
use par_term_acp::harness::{HarnessEventFlags, choose_permission_option, init_transcript};
use par_term_acp::{
    Agent, AgentConfig, AgentMessage, AgentStatus, ClientCapabilities, ContentBlock,
    FsCapabilities, SafePaths, discover_agents,
};
use par_term_config::{Config, CustomAcpAgentConfig};

const DEFAULT_SHADER_PROMPT: &str = "create a new background only shader that uses a procedural checker patern and has an effect that looks like its being pulled into some kind of vortex, then set that shader as the active shader";
const MAX_AUTO_RECOVERIES: u8 = 3;

macro_rules! println {
    () => {
        par_term_acp::harness::println_tee(format_args!(""))
    };
    ($($arg:tt)*) => {
        par_term_acp::harness::println_tee(format_args!($($arg)*))
    };
}

#[derive(Debug, Parser)]
#[command(
    name = "par-term-acp-harness",
    about = "Test ACP agents (including Claude+Ollama via claude-agent-acp) using par-term's real prompt pipeline"
)]
struct Args {
    /// ACP agent identity (from bundled agents, ~/.config/par-term/agents/*.toml, or config.yaml ai_inspector_custom_agents)
    #[arg(long, default_value = "claude-ollama.local")]
    agent: String,

    /// Prompt to send to the agent. Defaults to the shader-vortex test prompt.
    #[arg(long)]
    prompt: Option<String>,

    /// Working directory for the agent session (defaults to current directory)
    #[arg(long)]
    cwd: Option<PathBuf>,

    /// Path to the par-term binary used to host the MCP server (`par-term mcp-server`)
    #[arg(long)]
    par_term_bin: Option<PathBuf>,

    /// PNG file to return from the `terminal_screenshot` MCP tool (harness fallback)
    #[arg(long)]
    screenshot_file: Option<PathBuf>,

    /// Print available agents and exit
    #[arg(long)]
    list_agents: bool,

    /// Print the composed prompt blocks before sending
    #[arg(long)]
    print_prompt_blocks: bool,

    /// Disable shader-context injection (system guidance is still included)
    #[arg(long)]
    no_shader_context: bool,

    /// Automatically approve permission requests in the harness (default: true)
    #[arg(long, action = clap::ArgAction::Set, default_value_t = true)]
    auto_approve: bool,

    /// Apply config_update tool changes to the local par-term config.yaml (default: false)
    #[arg(long)]
    apply_config_updates: bool,

    /// Overall timeout after sending the prompt
    #[arg(long, default_value_t = 120)]
    timeout_seconds: u64,

    /// Stop after this many seconds with no incoming agent events
    #[arg(long, default_value_t = 8)]
    idle_timeout_seconds: u64,

    /// Emit one automatic recovery follow-up if the agent fails on Skill/Write
    #[arg(long, action = clap::ArgAction::Set, default_value_t = true)]
    auto_recover: bool,

    /// Write a copy of harness output to a transcript file
    #[arg(long)]
    transcript_file: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = Args::parse();
    if let Some(path) = &args.transcript_file {
        init_transcript(path)?;
        println!("[transcript] {}", path.display());
    }
    let mut config = Config::load()?;
    let available_agents = discover_and_merge_agents(&config);

    if args.list_agents {
        print_agents(&available_agents, &args.agent);
        return Ok(());
    }

    let Some(mut agent_config) = available_agents
        .iter()
        .find(|a| a.identity == args.agent)
        .cloned()
    else {
        eprintln!("Agent '{}' not found.", args.agent);
        eprintln!(
            "Use `cargo run --bin par-term-acp-harness -- --list-agents` to inspect available identities."
        );
        return Err("Agent not found (exit code 2)".into());
    };

    let cwd = args
        .cwd
        .clone()
        .unwrap_or(std::env::current_dir()?)
        .to_string_lossy()
        .to_string();

    if let Some(path) = &args.screenshot_file {
        agent_config.env.insert(
            "PAR_TERM_SCREENSHOT_FALLBACK_PATH".to_string(),
            path.to_string_lossy().to_string(),
        );
    }
    let par_term_bin = resolve_par_term_binary(args.par_term_bin.as_deref())?;

    println!("ACP harness");
    println!("agent: {} ({})", agent_config.name, agent_config.identity);
    println!("cwd: {}", cwd);
    println!("par-term mcp-server bin: {}", par_term_bin.display());
    println!("auto_approve: {}", args.auto_approve);
    println!("apply_config_updates: {}", args.apply_config_updates);
    println!("auto_recover: {}", args.auto_recover);
    if let Some(path) = &args.screenshot_file {
        println!("screenshot_fallback: {}", path.display());
    }
    println!();

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let safe_paths = SafePaths {
        config_dir: Config::config_dir(),
        shaders_dir: Config::shaders_dir(),
    };
    let mut agent = Agent::new(agent_config, tx, safe_paths, par_term_bin);
    agent
        .auto_approve
        .store(args.auto_approve, std::sync::atomic::Ordering::Relaxed);

    let capabilities = ClientCapabilities {
        fs: FsCapabilities {
            read_text_file: true,
            write_text_file: true,
            list_directory: true,
            find: true,
        },
        terminal: false,
        config: true,
    };

    if let Err(e) = agent.connect(&cwd, capabilities).await {
        return Err(format!("Connect failed: {e}").into());
    }

    let prompt_text = args
        .prompt
        .clone()
        .unwrap_or_else(|| DEFAULT_SHADER_PROMPT.to_string());

    let mut chat = ChatState::new();
    let mut prompt_count: usize = 0;
    let mut recovery_attempts: u8 = 0;
    let mut event_flags = HarnessEventFlags::default();
    let wants_shader_activation = is_shader_activation_request(&prompt_text);
    let mut last_event_at = Instant::now();
    let started_at = Instant::now();
    let total_timeout = Duration::from_secs(args.timeout_seconds);
    let idle_timeout = Duration::from_secs(args.idle_timeout_seconds);

    println!("== sending prompt ==\n{}\n", prompt_text);
    let mut prompt_rpc_inflight = Some(Box::pin(agent.send_prompt(build_harness_prompt_content(
        &config,
        &args,
        &mut prompt_count,
        &prompt_text,
    )))
        as Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>>>>);

    loop {
        let remaining_total = total_timeout.saturating_sub(started_at.elapsed());
        if remaining_total.is_zero() {
            println!("\n== timeout ==\nOverall timeout reached.");
            break;
        }

        let wait_for = idle_timeout.min(remaining_total);
        let sleep = tokio::time::sleep(wait_for);
        tokio::pin!(sleep);

        let has_prompt_inflight = prompt_rpc_inflight.is_some();

        tokio::select! {
            maybe_msg = rx.recv() => {
                match maybe_msg {
                    Some(msg) => {
                        last_event_at = Instant::now();
                        handle_agent_message(
                            &agent,
                            &mut config,
                            &mut chat,
                            &mut event_flags,
                            args.auto_approve,
                            args.apply_config_updates,
                            msg,
                        )
                        .await?;
                    }
                    None => {
                        println!("\n== disconnected ==\nAgent message channel closed.");
                        break;
                    }
                }
            }
            res = async {
                let fut = prompt_rpc_inflight
                    .as_mut()
                    .expect("guard ensures prompt future exists");
                fut.as_mut().await
            }, if has_prompt_inflight => {
                prompt_rpc_inflight = None;
                match res {
                    Ok(()) => {
                        println!("[prompt-rpc] completed");
                    }
                    Err(e) => {
                        println!("[prompt-rpc] failed: {e}");
                    }
                }
            }
            _ = &mut sleep => {
                let before = chat.messages.len();
                chat.flush_agent_message();
                print_new_chat_messages(&chat, Some(before));

                let idle_elapsed = last_event_at.elapsed();
                println!(
                    "\n== idle ==\nNo events for {:.1}s (idle timeout {:.1}s).",
                    idle_elapsed.as_secs_f32(),
                    idle_timeout.as_secs_f32()
                );

                let incomplete_shader_activation = wants_shader_activation
                    && prompt_rpc_inflight.is_none()
                    && !event_flags.saw_config_update;

                if args.auto_recover
                    && recovery_attempts < MAX_AUTO_RECOVERIES
                    && (event_flags.saw_failed_tool_since_prompt || incomplete_shader_activation)
                {
                    recovery_attempts = recovery_attempts.saturating_add(1);
                    let had_failed_tool = event_flags.saw_failed_tool_since_prompt;
                    event_flags.saw_failed_tool_since_prompt = false;
                    let retry = if incomplete_shader_activation && !had_failed_tool {
                        if recovery_attempts >= 2 {
                            "Continue the same shader task. Activation is still incomplete. Do not explore the repo or read unrelated files. Use only the existing shader file you already created (or create one direct shader file under ~/.config/par-term/shaders/ if none exists), then immediately call the par-term config_update MCP tool to set `custom_shader` to that filename and `custom_shader_enabled` to true. Confirm only the exact filename activated."
                        } else {
                            "Continue the same shader task. You have not completed the activation step yet. If you already created the shader file, do not stop there and do not switch to a different example. Call the par-term config_update MCP tool now to set `custom_shader` to the shader filename you created and set `custom_shader_enabled` to true, then confirm the exact filename activated."
                        }
                    } else if recovery_attempts >= 2 {
                        "Continue the same shader task and do not explore unrelated files or dependencies. A previous tool call failed. Do not use Skill/Task/TodoWrite and do not call EnterPlanMode or switch to plan mode. If a Read fails because the target is a directory, do not retry Read on that directory; use a listing/search tool or write the new shader file directly in ~/.config/par-term/shaders/. If using Write, use the correct parameter names `file_path` and `content` (not `filepath`). Complete the full workflow before declaring success: write a background shader matching the user's checker+vortex request, then immediately call the par-term config_update MCP tool to set `custom_shader` to that filename and `custom_shader_enabled` to true."
                    } else {
                        "Continue the same task and stay on the shader request (do not switch to unrelated examples/files). A previous tool call failed. Do not use Skill/Task/TodoWrite and do not call EnterPlanMode or switch to plan mode. If a Read fails because the target is a directory, do not retry Read on that directory; use a listing/search tool or write the new shader file directly in ~/.config/par-term/shaders/. If using Write, use the correct parameter names `file_path` and `content` (not `filepath`). Complete the full workflow before declaring success: write a background shader matching the user's checker+vortex request, then call the par-term config_update MCP tool to set `custom_shader` to that filename and `custom_shader_enabled` to true."
                    };
                    println!(
                        "\n== auto-recover ({}/{}) ==\n{}\n",
                        recovery_attempts,
                        MAX_AUTO_RECOVERIES,
                        retry
                    );
                    prompt_rpc_inflight = Some(Box::pin(agent.send_prompt(
                        build_harness_prompt_content(&config, &args, &mut prompt_count, retry),
                    ))
                        as Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>>>>
                    );
                    continue;
                }

                break;
            }
        };
    }

    chat.flush_agent_message();
    print_new_chat_messages(&chat, None);

    // Inline XML-style config_update fallback debug visibility (same parser as UI)
    for msg in &chat.messages {
        if let ChatMessage::Agent(text) = msg
            && let Some(updates) = extract_inline_config_update(text)
        {
            println!("\n[INLINE CONFIG_UPDATE FALLBACK DETECTED]");
            print_updates(&updates);
        }
    }

    println!("\n== summary ==");
    println!("prompts sent: {prompt_count}");
    println!("auto_recovery_attempts: {recovery_attempts}");
    println!("saw_failed_tool: {}", event_flags.saw_any_failed_tool);
    println!("saw_config_update: {}", event_flags.saw_config_update);

    drop(prompt_rpc_inflight);
    agent.disconnect().await;
    Ok(())
}

fn resolve_par_term_binary(
    explicit: Option<&Path>,
) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    if let Some(path) = explicit {
        return Ok(path.to_path_buf());
    }

    if let Ok(current) = std::env::current_exe()
        && let Some(dir) = current.parent()
    {
        let candidate = dir.join(if cfg!(windows) {
            "par-term.exe"
        } else {
            "par-term"
        });
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    if let Some(path) = resolve_binary_in_path("par-term") {
        return Ok(path);
    }

    Err("Could not find `par-term` binary. Pass --par-term-bin /path/to/par-term".into())
}

fn discover_and_merge_agents(config: &Config) -> Vec<AgentConfig> {
    let config_dir = Config::config_dir();
    let discovered = discover_agents(&config_dir);
    merge_custom_agents(discovered, &config.ai_inspector_custom_agents)
}

fn merge_custom_agents(
    mut agents: Vec<AgentConfig>,
    custom_agents: &[CustomAcpAgentConfig],
) -> Vec<AgentConfig> {
    for custom in custom_agents {
        if custom.identity.trim().is_empty()
            || custom.short_name.trim().is_empty()
            || custom.name.trim().is_empty()
            || custom.run_command.is_empty()
        {
            continue;
        }

        let actions: HashMap<String, HashMap<String, ActionConfig>> = custom
            .actions
            .iter()
            .map(|(action_name, variants)| {
                let mapped = variants
                    .iter()
                    .map(|(variant_name, action)| {
                        (
                            variant_name.clone(),
                            ActionConfig {
                                command: action.command.clone(),
                                description: action.description.clone(),
                            },
                        )
                    })
                    .collect::<HashMap<_, _>>();
                (action_name.clone(), mapped)
            })
            .collect();

        let mut env = custom.env.clone();
        if !env.contains_key("OLLAMA_CONTEXT_LENGTH")
            && let Some(ctx) = custom.ollama_context_length
            && ctx > 0
        {
            env.insert("OLLAMA_CONTEXT_LENGTH".to_string(), ctx.to_string());
        }

        let mut custom_agent = AgentConfig {
            identity: custom.identity.clone(),
            name: custom.name.clone(),
            short_name: custom.short_name.clone(),
            protocol: if custom.protocol.trim().is_empty() {
                "acp".to_string()
            } else {
                custom.protocol.clone()
            },
            r#type: if custom.r#type.trim().is_empty() {
                "coding".to_string()
            } else {
                custom.r#type.clone()
            },
            active: custom.active,
            run_command: custom.run_command.clone(),
            env,
            install_command: custom.install_command.clone(),
            actions,
            connector_installed: false,
        };
        custom_agent.detect_connector();
        agents.retain(|existing| existing.identity != custom_agent.identity);
        agents.push(custom_agent);
    }

    agents.retain(|agent| agent.is_active());
    agents.sort_by(|a, b| a.name.cmp(&b.name));
    agents
}

fn print_agents(agents: &[AgentConfig], selected: &str) {
    println!("Available ACP agents:");
    for agent in agents {
        let marker = if agent.identity == selected { "*" } else { " " };
        let cmd = agent.run_command_for_platform().unwrap_or("<none>");
        println!(
            "{} {} ({}) [{}] cmd={} installed={}",
            marker, agent.name, agent.identity, agent.short_name, cmd, agent.connector_installed
        );
    }
}

fn build_prompt_blocks(
    config: &Config,
    user_text: &str,
    include_shader_context: bool,
) -> Vec<ContentBlock> {
    let mut content: Vec<ContentBlock> = vec![ContentBlock::Text {
        text: format!("{}[End system instructions]", AGENT_SYSTEM_GUIDANCE),
    }];

    if include_shader_context && should_inject_shader_context(user_text, config) {
        content.push(ContentBlock::Text {
            text: build_shader_context(config),
        });
    }

    content.push(ContentBlock::Text {
        text: format!("[User message]\n{user_text}"),
    });
    content
}

fn build_harness_prompt_content(
    config: &Config,
    args: &Args,
    prompt_count: &mut usize,
    text: &str,
) -> Vec<ContentBlock> {
    let mut content = build_prompt_blocks(config, text, !args.no_shader_context);
    if *prompt_count > 0 {
        content.push(ContentBlock::Text {
            text: "[Host note]\nThis is a follow-up retry in the same conversation. Continue the original task from the existing context; do not restart or ask the user to restate the request."
                .to_string(),
        });
    }
    if args.print_prompt_blocks {
        print_prompt_preview(&content);
    }
    *prompt_count += 1;
    content
}

fn print_prompt_preview(content: &[ContentBlock]) {
    println!("== prompt blocks ==");
    for (i, block) in content.iter().enumerate() {
        match block {
            ContentBlock::Text { text } => {
                println!("-- block[{i}] text ({} chars) --", text.len());
                println!("{text}");
            }
            other => {
                println!("-- block[{i}] {other:?}");
            }
        }
    }
    println!("== end prompt blocks ==\n");
}

async fn handle_agent_message(
    agent: &Agent,
    config: &mut Config,
    chat: &mut ChatState,
    event_flags: &mut HarnessEventFlags,
    auto_approve: bool,
    apply_config_updates: bool,
    msg: AgentMessage,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match msg {
        AgentMessage::StatusChanged(status) => {
            println!("[status] {}", format_status(&status));
        }
        AgentMessage::SessionUpdate(update) => {
            // Track failures before `ChatState` potentially coalesces messages.
            match &update {
                par_term_acp::SessionUpdate::ToolCall(info) => {
                    println!(
                        "[tool] {} kind={} status={}",
                        info.title, info.kind, info.status
                    );
                    if is_config_update_tool(info.title.as_str()) {
                        event_flags.saw_config_update = true;
                        if let Some(content) = &info.content {
                            println!("[tool:config_update] content={}", truncate_json(content));
                        }
                    }
                    if is_failed_tool(info.title.as_str(), info.status.as_str()) {
                        event_flags.saw_failed_tool_since_prompt = true;
                        event_flags.saw_any_failed_tool = true;
                    }
                }
                par_term_acp::SessionUpdate::ToolCallUpdate(info) => {
                    if let Some(status) = &info.status {
                        println!(
                            "[tool-update] id={} status={} title={}",
                            info.tool_call_id,
                            status,
                            info.title.clone().unwrap_or_default()
                        );
                        let title = info.title.as_deref().unwrap_or("");
                        let status_l = status.to_ascii_lowercase();
                        if is_config_update_tool(title)
                            && !(status_l.contains("fail") || status_l.contains("error"))
                        {
                            event_flags.saw_config_update = true;
                        }
                        let failed_without_title = title.is_empty()
                            && (status_l.contains("fail") || status_l.contains("error"));
                        if failed_without_title || is_failed_tool(title, status) {
                            event_flags.saw_failed_tool_since_prompt = true;
                            event_flags.saw_any_failed_tool = true;
                        }
                    }
                }
                par_term_acp::SessionUpdate::Plan(info) => {
                    println!("[plan] {} step(s)", info.entries.len());
                    for entry in &info.entries {
                        println!("  - [{}] {}", entry.status, entry.content);
                    }
                }
                par_term_acp::SessionUpdate::CurrentModeUpdate { mode_id } => {
                    println!("[mode] {}", mode_id);
                }
                par_term_acp::SessionUpdate::Unknown(v) => {
                    println!("[update:unknown] {}", truncate_json(v));
                }
                _ => {}
            }

            let before = chat.messages.len();
            chat.handle_update(update);
            for msg in &chat.messages[before..] {
                if let ChatMessage::ToolCall { title, status, .. } = msg {
                    if is_config_update_tool(title)
                        && !status.to_ascii_lowercase().contains("fail")
                        && !status.to_ascii_lowercase().contains("error")
                    {
                        event_flags.saw_config_update = true;
                    }
                    if is_failed_tool(title, status) {
                        event_flags.saw_failed_tool_since_prompt = true;
                        event_flags.saw_any_failed_tool = true;
                    }
                }
            }
            print_new_chat_messages(chat, Some(before));
        }
        AgentMessage::PermissionRequest {
            request_id,
            tool_call,
            options,
        } => {
            let title = tool_call
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Permission requested");
            println!("[perm] id={request_id} title={title}");
            for (i, opt) in options.iter().enumerate() {
                println!(
                    "  [{}] {} (id={} kind={})",
                    i,
                    opt.name,
                    opt.option_id,
                    opt.kind.as_deref().unwrap_or("")
                );
            }
            let choice = choose_permission_option(&options, auto_approve);
            match choice {
                Some((option_id, label)) => {
                    println!("[perm] auto-select {}", label);
                    if let Err(e) = agent.respond_permission(request_id, option_id, false).await {
                        println!("[perm] respond failed: {e}");
                    }
                }
                None => {
                    println!("[perm] cancelling (auto_approve=false)");
                    if let Err(e) = agent.respond_permission(request_id, "", true).await {
                        println!("[perm] cancel failed: {e}");
                    }
                }
            }
        }
        AgentMessage::ConfigUpdate { updates, reply } => {
            event_flags.saw_config_update = true;
            println!("[config_update] received {} key(s)", updates.len());
            print_updates(&updates);

            let result = if apply_config_updates {
                apply_updates_to_config(config, &updates)
            } else {
                Ok(())
            };
            let _ = reply.send(result.map_err(|e| e.to_string()));
        }
        AgentMessage::ClientReady(_) => {
            println!("[client] ready");
        }
        AgentMessage::AutoApproved(description) => {
            println!("[auto-approved] {}", description);
        }
        AgentMessage::PromptStarted => {
            println!("[prompt] started");
        }
        AgentMessage::PromptComplete => {
            chat.flush_agent_message();
            print_new_chat_messages(chat, None);
            println!("[prompt] complete");
        }
    }
    Ok(())
}

fn apply_updates_to_config(
    config: &mut Config,
    updates: &HashMap<String, serde_json::Value>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut root = serde_json::to_value(&*config)?;
    let obj = root
        .as_object_mut()
        .ok_or("Serialized config is not a JSON object")?;
    for (k, v) in updates {
        obj.insert(k.clone(), v.clone());
    }
    let mut new_config: Config = serde_json::from_value(root)?;
    new_config.generate_snippet_action_keybindings();
    new_config.save()?;
    *config = new_config;
    println!(
        "[config_update] applied to {}",
        Config::config_path().display()
    );
    Ok(())
}

fn print_updates(updates: &HashMap<String, serde_json::Value>) {
    let mut keys: Vec<_> = updates.keys().collect();
    keys.sort();
    for key in keys {
        if let Some(value) = updates.get(key) {
            println!("  - {} = {}", key, value);
        }
    }
}

fn truncate_json(v: &serde_json::Value) -> String {
    let s = v.to_string();
    if s.len() > 500 {
        format!("{}...", &s[..500])
    } else {
        s
    }
}

fn is_failed_tool(title: &str, status: &str) -> bool {
    let title_l = title.to_ascii_lowercase();
    let status_l = status.to_ascii_lowercase();
    status_l.contains("fail") && (title_l.contains("skill") || title_l.contains("write"))
}

fn is_config_update_tool(title: &str) -> bool {
    title.to_ascii_lowercase().contains("config_update")
}

fn print_new_chat_messages(chat: &ChatState, from: Option<usize>) {
    let start = from.unwrap_or(0).min(chat.messages.len());
    for msg in &chat.messages[start..] {
        match msg {
            ChatMessage::User { text, pending } => {
                println!(
                    "[chat:user{}] {}",
                    if *pending { ":queued" } else { "" },
                    text.replace('\n', " ")
                );
            }
            ChatMessage::Agent(text) => {
                println!("[chat:agent]\n{}\n", text);
            }
            ChatMessage::Thinking(text) => {
                println!("[chat:thinking] {}", text.replace('\n', " "));
            }
            ChatMessage::ToolCall {
                title,
                kind,
                status,
                ..
            } => {
                println!("[chat:tool] {} kind={} status={}", title, kind, status);
            }
            ChatMessage::CommandSuggestion(cmd) => {
                println!("[chat:cmd] {}", cmd);
            }
            ChatMessage::Permission {
                request_id,
                description,
                resolved,
                ..
            } => {
                println!(
                    "[chat:permission] id={} resolved={} {}",
                    request_id, resolved, description
                );
            }
            ChatMessage::AutoApproved(desc) => {
                println!("[chat:auto-approved] {}", desc);
            }
            ChatMessage::System(text) => {
                println!("[chat:system] {}", text);
            }
        }
    }
}

fn format_status(status: &AgentStatus) -> String {
    match status {
        AgentStatus::Disconnected => "Disconnected".to_string(),
        AgentStatus::Connecting => "Connecting".to_string(),
        AgentStatus::Connected => "Connected".to_string(),
        AgentStatus::Error(e) => format!("Error: {e}"),
    }
}
