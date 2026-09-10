use thiserror::Error;

pub type Result<T> = std::result::Result<T, SaycmdError>;

#[derive(Debug, Error)]
pub enum SaycmdError {
    #[error("prompt is empty")]
    EmptyPrompt,
    #[error("prompt exceeds the {0}-byte limit")]
    PromptTooLarge(usize),
    #[error("generated command exceeds the {0}-byte limit")]
    CommandTooLarge(usize),
    #[error("unsupported shell: {0}")]
    UnsupportedShell(String),
    #[error("model is not configured; set SAYCMD_MODEL or add model to the config file")]
    MissingModel,
    #[error("API key environment variable {0} is not set")]
    MissingApiKey(String),
    #[error("cannot read config {path}: {source}")]
    ConfigRead {
        path: String,
        source: std::io::Error,
    },
    #[error("cannot parse config {path}: {source}")]
    ConfigParse {
        path: String,
        source: toml::de::Error,
    },
    #[error("invalid API base URL: {0}")]
    InvalidBaseUrl(String),
    #[error("invalid SAYCMD_EXPLAIN: {0}; expected true or false")]
    InvalidExplain(String),
    #[error("invalid timeout in SAYCMD_TIMEOUT_SECONDS: {0}")]
    InvalidTimeout(String),
    #[error("API request failed: {}", http_error_message(.0))]
    Http(#[from] reqwest::Error),
    #[error("API returned HTTP {status}: {message}")]
    ApiStatus { status: u16, message: String },
    #[error("API response did not contain assistant text")]
    MissingAssistantContent,
    #[error("model response is not a valid command object: {0}")]
    InvalidModelOutput(String),
    #[error("generated command contains a NUL byte")]
    NulByte,
    #[error("generated {shell} script has invalid syntax: {message}")]
    InvalidSyntax { shell: String, message: String },
    #[error("cannot run {program}: {source}")]
    Process {
        program: String,
        source: std::io::Error,
    },
    #[error("current working directory is invalid: {0}")]
    InvalidCwd(String),
}

fn http_error_message(error: &reqwest::Error) -> String {
    use std::error::Error;

    if error.is_timeout() {
        return "request timed out; increase timeout_seconds in config.toml or SAYCMD_TIMEOUT_SECONDS".into();
    }
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    message.split_whitespace().collect::<Vec<_>>().join(" ")
}
