pub mod api;
pub mod config;
pub mod context;
pub mod error;
pub mod output;
pub mod shell;

use std::path::Path;

use api::{ChatClient, PreviousCommand};
use config::Config;
use context::CommandContext;
use error::{Result, SaycmdError};

pub const MAX_PROMPT_BYTES: usize = 8 * 1024;
pub const MAX_COMMAND_BYTES: usize = 32 * 1024;

pub fn translate(config: &Config, shell: &str, cwd: &Path, prompt: &str) -> Result<String> {
    Ok(translate_output(config, shell, cwd, prompt)?.command)
}

pub fn translate_output(
    config: &Config,
    shell: &str,
    cwd: &Path,
    prompt: &str,
) -> Result<output::CommandOutput> {
    translate_with_previous(config, shell, cwd, prompt, None)
}

pub fn translate_with_previous(
    config: &Config,
    shell: &str,
    cwd: &Path,
    prompt: &str,
    previous: Option<&PreviousCommand>,
) -> Result<output::CommandOutput> {
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return Err(SaycmdError::EmptyPrompt);
    }
    if prompt.len() > MAX_PROMPT_BYTES {
        return Err(SaycmdError::PromptTooLarge(MAX_PROMPT_BYTES));
    }
    if shell != "zsh" {
        return Err(SaycmdError::UnsupportedShell(shell.to_owned()));
    }

    if let Some(previous) = previous {
        if previous.command.trim().is_empty() {
            return Err(SaycmdError::InvalidModelOutput(
                "previous command is empty".into(),
            ));
        }
        if previous.command.len() > MAX_COMMAND_BYTES {
            return Err(SaycmdError::CommandTooLarge(MAX_COMMAND_BYTES));
        }
        if previous.command.contains('\0') {
            return Err(SaycmdError::NulByte);
        }
        if previous.requests.len() > 8
            || previous.requests.iter().any(|r| r.len() > MAX_PROMPT_BYTES)
        {
            return Err(SaycmdError::PromptTooLarge(8 * MAX_PROMPT_BYTES));
        }
    }
    let context = CommandContext::collect(shell, cwd);
    let content = ChatClient::new(config)?.translate_with_previous(prompt, &context, previous)?;
    let result = output::parse_output(&content, config.explain)?;
    output::validate_command(&result.command, shell)?;
    Ok(result)
}
