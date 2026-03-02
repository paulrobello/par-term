//! Prompt construction utilities for the ACP harness binary.
//!
//! Assembles the multi-block prompt content sent to the ACP agent,
//! including system guidance, shader context, and the user message.

use par_term_acp::ContentBlock;
use par_term_config::Config;

use crate::ai_inspector::chat::AGENT_SYSTEM_GUIDANCE;
use crate::ai_inspector::shader_context::{build_shader_context, should_inject_shader_context};

/// Build the base prompt content blocks from system guidance, optional shader
/// context, and the user's message text.
pub fn build_prompt_blocks(
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

/// Print a preview of the outgoing prompt blocks to the console.
pub fn print_prompt_preview(content: &[ContentBlock]) {
    par_term_acp::harness::println_tee(format_args!("== prompt blocks =="));
    for (i, block) in content.iter().enumerate() {
        match block {
            ContentBlock::Text { text } => {
                par_term_acp::harness::println_tee(format_args!(
                    "-- block[{i}] text ({} chars) --",
                    text.len()
                ));
                par_term_acp::harness::println_tee(format_args!("{text}"));
            }
            other => {
                par_term_acp::harness::println_tee(format_args!("-- block[{i}] {other:?}"));
            }
        }
    }
    par_term_acp::harness::println_tee(format_args!("== end prompt blocks ==\n"));
}
