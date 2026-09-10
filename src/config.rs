use std::{env, fs, path::PathBuf, time::Duration};

use serde::Deserialize;

use crate::error::{Result, SaycmdError};

const DEFAULT_PREFIX: &str = "？";
const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_KEY_ENV: &str = "OPENAI_API_KEY";
const DEFAULT_TIMEOUT_SECONDS: u64 = 30;

#[derive(Debug, Default, Deserialize)]
struct FileConfig {
    prefix: Option<String>,
    explain: Option<bool>,
    base_url: Option<String>,
    model: Option<String>,
    api_key_env: Option<String>,
    timeout_seconds: Option<u64>,
}

impl FileConfig {
    fn load() -> Result<Self> {
        let path = config_path();
        if path.exists() {
            let text = fs::read_to_string(&path).map_err(|source| SaycmdError::ConfigRead {
                path: path.display().to_string(),
                source,
            })?;
            toml::from_str(&text).map_err(|source| SaycmdError::ConfigParse {
                path: path.display().to_string(),
                source,
            })
        } else {
            Ok(Self::default())
        }
    }
}

/// Load shell settings without requiring model or API configuration.
pub fn load_prefix() -> Result<String> {
    let file = FileConfig::load()?;
    Ok(env_value("SAYCMD_PREFIX")
        .or(file.prefix.filter(|value| !value.trim().is_empty()))
        .unwrap_or_else(|| DEFAULT_PREFIX.to_owned()))
}

#[derive(Debug, Clone)]
pub struct Config {
    pub explain: bool,
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub api_key_env: String,
    pub timeout: Duration,
}

impl Config {
    pub fn load() -> Result<Self> {
        let file = FileConfig::load()?;
        Self::from_file_and_env(file)
    }

    fn from_file_and_env(file: FileConfig) -> Result<Self> {
        let base_url = env_value("SAYCMD_BASE_URL")
            .or(file.base_url)
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_owned());
        let model = env_value("SAYCMD_MODEL")
            .or(file.model)
            .ok_or(SaycmdError::MissingModel)?;
        let api_key_env = env_value("SAYCMD_API_KEY_ENV")
            .or(file.api_key_env)
            .unwrap_or_else(|| DEFAULT_KEY_ENV.to_owned());
        let api_key = env_value("SAYCMD_API_KEY").or_else(|| {
            if api_key_env.is_empty() {
                None
            } else {
                env_value(&api_key_env)
            }
        });
        let timeout_seconds = match env_value("SAYCMD_TIMEOUT_SECONDS") {
            Some(raw) => raw
                .parse::<u64>()
                .map_err(|_| SaycmdError::InvalidTimeout(raw))?,
            None => file.timeout_seconds.unwrap_or(DEFAULT_TIMEOUT_SECONDS),
        };

        let explain = match env_value("SAYCMD_EXPLAIN") {
            Some(raw) => raw
                .parse::<bool>()
                .map_err(|_| SaycmdError::InvalidExplain(raw))?,
            None => file.explain.unwrap_or(false),
        };

        Ok(Self {
            explain,
            base_url: base_url.trim_end_matches('/').to_owned(),
            model,
            api_key,
            api_key_env,
            timeout: Duration::from_secs(timeout_seconds),
        })
    }

    pub fn require_api_key_if_configured(&self) -> Result<()> {
        if !self.api_key_env.is_empty() && self.api_key.is_none() {
            return Err(SaycmdError::MissingApiKey(self.api_key_env.clone()));
        }
        Ok(())
    }
}

fn env_value(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

pub fn config_path() -> PathBuf {
    if let Some(path) = env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(path).join("saycmd/config.toml");
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("saycmd/config.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_stable() {
        let file = FileConfig {
            model: Some("test-model".into()),
            api_key_env: Some(String::new()),
            ..FileConfig::default()
        };
        let config = Config::from_file_and_env(file).unwrap();
        assert_eq!(config.base_url, DEFAULT_BASE_URL);
        assert_eq!(config.model, "test-model");
        assert_eq!(config.timeout, Duration::from_secs(30));
    }
}
