//! Configuration files and CLI-facing option enums.

use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum, Default)]
#[serde(rename_all = "lowercase")]
pub enum Keys {
    #[default]
    Vim,
    Standard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum, Default)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    #[default]
    Auto,
    Kitty,
    Iterm2,
    Sixel,
    Halfblocks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Auto,
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum, Default)]
#[serde(rename_all = "lowercase")]
pub enum MermaidEngine {
    #[default]
    Auto,
    Rust,
    Mmdc,
    Source,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub keys: Keys,
    pub mouse: bool,
    pub protocol: Protocol,
    pub theme: Theme,
    pub mermaid_engine: MermaidEngine,
    pub cache_dir: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            keys: Keys::Vim,
            mouse: true,
            protocol: Protocol::Auto,
            theme: Theme::Auto,
            mermaid_engine: MermaidEngine::Auto,
            cache_dir: None,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        let Some(path) = config_path() else {
            return Ok(Self::default());
        };
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&path).map_err(|source| ConfigError::Io {
            path: path.clone(),
            source,
        })?;
        Self::parse_at(&raw, &path)
    }

    fn parse_at(raw: &str, path: &std::path::Path) -> Result<Self, ConfigError> {
        let value: toml::Value = toml::from_str(raw).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
        if let Some(table) = value.as_table() {
            const KNOWN: [&str; 6] = [
                "keys",
                "mouse",
                "protocol",
                "theme",
                "mermaid_engine",
                "cache_dir",
            ];
            for key in table.keys() {
                if !KNOWN.contains(&key.as_str()) {
                    eprintln!("mdreview: warning: unknown config key '{key}'");
                }
            }
        }
        toml::from_str(raw).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })
    }

    #[cfg(test)]
    pub fn parse(raw: &str) -> Result<Self, ConfigError> {
        Self::parse_at(raw, std::path::Path::new("<config>"))
    }

    pub fn cache_dir(&self) -> PathBuf {
        let dir = self.cache_dir.clone().unwrap_or_else(|| {
            dirs::cache_dir()
                .unwrap_or_else(|| PathBuf::from(".cache"))
                .join("mdreview")
        });
        if let Some(text) = dir.to_str()
            && let Some(rest) = text.strip_prefix("~/")
            && let Some(home) = dirs::home_dir()
        {
            return home.join(rest);
        }
        dir
    }
}

pub fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("mdreview").join("config.toml"))
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read config {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot parse config {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_stable() {
        let config = Config::default();
        assert_eq!(config.keys, Keys::Vim);
        assert!(config.mouse);
        assert_eq!(config.protocol, Protocol::Auto);
        assert!(config.cache_dir().ends_with("mdreview"));
    }

    #[test]
    fn parses_partial_toml_and_ignores_unknown_keys() {
        let config = Config::parse(
            r#"
            keys = "standard"
            mouse = false
            protocol = "halfblocks"
            mermaid_engine = "source"
            unknown_key = true
            "#,
        )
        .unwrap();
        assert_eq!(config.keys, Keys::Standard);
        assert!(!config.mouse);
        assert_eq!(config.protocol, Protocol::Halfblocks);
        assert_eq!(config.mermaid_engine, MermaidEngine::Source);
    }
}
