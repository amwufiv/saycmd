use std::process::Command;

use serde::Deserialize;

use crate::{
    error::{Result, SaycmdError},
    MAX_COMMAND_BYTES,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandOutput {
    pub command: String,
    #[serde(default)]
    pub explanation: Option<String>,
}

pub fn parse_command(content: &str) -> Result<String> {
    Ok(parse_output(content, false)?.command)
}

pub fn parse_output(content: &str, explain: bool) -> Result<CommandOutput> {
    let content = strip_fence(content.trim());
    let parsed: CommandOutput = serde_json::from_str(content)
        .map_err(|error| SaycmdError::InvalidModelOutput(error.to_string()))?;
    let command = parsed.command.trim().to_owned();
    if command.is_empty() {
        return Err(SaycmdError::InvalidModelOutput(
            "command field is empty".to_owned(),
        ));
    }
    if command.len() > MAX_COMMAND_BYTES {
        return Err(SaycmdError::CommandTooLarge(MAX_COMMAND_BYTES));
    }
    if command.contains('\0') {
        return Err(SaycmdError::NulByte);
    }
    let explanation = if explain {
        let text = parsed.explanation.ok_or_else(|| {
            SaycmdError::InvalidModelOutput("explanation field is missing".into())
        })?;
        let text: String = text
            .chars()
            .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
            .take(4000)
            .collect();
        if text.trim().is_empty() {
            return Err(SaycmdError::InvalidModelOutput(
                "explanation field is empty".into(),
            ));
        }
        Some(text.trim().to_owned())
    } else {
        None
    };
    Ok(CommandOutput {
        command,
        explanation,
    })
}

fn strip_fence(content: &str) -> &str {
    let Some(rest) = content.strip_prefix("```") else {
        return content;
    };
    let Some(first_newline) = rest.find('\n') else {
        return content;
    };
    let language = rest[..first_newline].trim();
    if !language.is_empty() && language != "json" {
        return content;
    }
    let body = &rest[first_newline + 1..];
    body.strip_suffix("```").map(str::trim).unwrap_or(content)
}

pub fn validate_command(command: &str, shell: &str) -> Result<()> {
    let output = Command::new(shell)
        .args(["-n", "-c", command])
        .output()
        .map_err(|source| SaycmdError::Process {
            program: shell.to_owned(),
            source,
        })?;
    if output.status.success() {
        return Ok(());
    }
    let message = String::from_utf8_lossy(&output.stderr)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    Err(SaycmdError::InvalidSyntax {
        shell: shell.to_owned(),
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multiline_command() {
        let command = parse_command(r#"{"command":"printf 'one\\n'\nprintf 'two\\n'"}"#).unwrap();
        assert_eq!(command.lines().count(), 2);
    }

    #[test]
    fn tolerates_json_code_fence_only() {
        let command = parse_command("```json\n{\"command\":\"pwd\"}\n```").unwrap();
        assert_eq!(command, "pwd");
    }

    #[test]
    fn rejects_prose_and_extra_fields() {
        assert!(parse_command("Use this: pwd").is_err());
        assert!(parse_command(r#"{"command":"pwd","why":"requested"}"#).is_err());
    }

    #[test]
    fn explanation_is_separate_optional_and_strips_control_characters() {
        let content = r#"{"command":"ls -la","explanation":"-l: long\n-a: all\u0000\u001b"}"#;
        let result = parse_output(content, true).unwrap();
        assert_eq!(result.command, "ls -la");
        assert_eq!(result.explanation.as_deref(), Some("-l: long\n-a: all"));
        assert!(parse_output(content, false).unwrap().explanation.is_none());
        assert!(parse_output(r#"{"command":"ls"}"#, true).is_err());
        assert!(parse_output(r#"{"command":"ls","explanation":" "}"#, true).is_err());
    }

    #[test]
    fn validates_zsh_without_execution() {
        assert!(validate_command("echo $(printf safe)", "zsh").is_ok());
        assert!(validate_command("if then", "zsh").is_err());
    }
}
