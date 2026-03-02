use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::time::{Duration, Instant};

use clap::Parser;
use par_term::acp_harness::agent_discovery::discover_and_merge_agents;
use par_term::acp_harness::binary_resolver::resolve_par_term_binary;
use par_term::acp_harness::harness_output::{print_agents, print_new_chat_messages, print_updates};
use par_term::acp_harness::message_handler::handle_agent_message;
use par_term::acp_harness::prompt_builder::{build_prompt_blocks, print_prompt_preview};
use par_term::ai_inspector::chat::{ChatMessage, ChatState, extract_inline_config_update};
use par_term::ai_inspector::shader_context::is_shader_activation_request;
use par_term_acp::harness::{HarnessEventFlags, init_transcript};
use par_term_acp::{Agent, ClientCapabilities, ContentBlock, FsCapabilities, SafePaths};
use par_term_config::Config;

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
