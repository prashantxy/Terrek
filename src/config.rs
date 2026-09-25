//! User configuration, stored as TOML in `<config dir>/terrek/config.toml`.
//!
//! Override the directories with `TERREK_CONFIG_DIR` / `TERREK_DATA_DIR`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::ai::provider::ProviderKind;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub ai: AiConfig,
    pub shell: ShellConfig,
    pub history: HistoryConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AiConfig {
    /// When unset, the provider is inferred from whichever API key env var is present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Prefer an env var (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, ...) over storing it here.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ShellConfig {
    /// Shell to launch; defaults to `$SHELL`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
    /// Install the zsh/bash/fish hooks that record commands.
    pub integration: bool,
    /// Key that opens the palette, e.g. `ctrl-t`, `ctrl-g`, `ctrl-]`.
    pub palette_key: String,
    /// Print a one-line hint under failed commands.
    pub failure_hints: bool,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            program: None,
            integration: true,
            palette_key: "ctrl-t".into(),
            failure_hints: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct HistoryConfig {
    pub enabled: bool,
    /// Keep the tail of each command's output so `fix` works later.
    pub store_output: bool,
    pub max_output_bytes: usize,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            store_output: true,
            max_output_bytes: 8 * 1024,
        }
    }
}

pub fn config_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("TERREK_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("terrek")
}

pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("TERREK_DATA_DIR") {
        return PathBuf::from(dir);
    }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("terrek")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

impl Config {
    /// Load the config file, migrating the pre-0.2 `config.json` if that is all there is.
    pub fn load() -> Result<Self> {
        Self::load_from(&config_dir())
    }

    pub fn load_from(dir: &Path) -> Result<Self> {
        let path = dir.join("config.toml");
        if path.exists() {
            let text =
                fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
            return toml::from_str(&text).with_context(|| format!("parsing {}", path.display()));
        }

        let legacy = dir.join("config.json");
        if legacy.exists() {
            if let Some(cfg) = migrate_legacy(&legacy) {
                // Best effort: if saving fails we still run with the migrated values.
                let _ = cfg.save_to(dir);
                return Ok(cfg);
            }
        }

        Ok(Self::default())
    }

    pub fn save(&self) -> Result<PathBuf> {
        self.save_to(&config_dir())
    }

    pub fn save_to(&self, dir: &Path) -> Result<PathBuf> {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        let path = dir.join("config.toml");
        let text = toml::to_string_pretty(self)?;
        write_private(&path, text.as_bytes())?;
        Ok(path)
    }
}

/// The 0.1 format: `{"provider": "gemini", "api_key": "...", "auto_ai_on_error": true}`.
fn migrate_legacy(path: &Path) -> Option<Config> {
    #[derive(Deserialize)]
    struct Legacy {
        provider: Option<String>,
        api_key: Option<String>,
    }

    let legacy: Legacy = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    let mut cfg = Config::default();
    cfg.ai.provider = legacy.provider.as_deref().and_then(ProviderKind::parse);
    cfg.ai.api_key = legacy.api_key.filter(|k| !k.trim().is_empty());
    Some(cfg)
}

/// Write a file readable only by the current user (it may contain API keys).
fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .with_context(|| format!("writing {}", path.display()))?;
        // `mode` only applies on creation; tighten files left by older versions too.
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
        file.write_all(bytes)?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        fs::write(path, bytes).with_context(|| format!("writing {}", path.display()))
    }
}

/// `sk-ant-api03-abcd...wxyz` -> `sk-ant…wxyz`
pub fn mask_secret(secret: &str) -> String {
    let chars: Vec<char> = secret.chars().collect();
    if chars.len() <= 10 {
        return "…".repeat(3);
    }
    let head: String = chars[..6].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}…{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Config::load_from(dir.path()).unwrap(), Config::default());
    }

    #[test]
    fn round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = Config::default();
        cfg.ai.provider = Some(ProviderKind::Anthropic);
        cfg.ai.model = Some("claude-opus-5".into());
        cfg.shell.palette_key = "ctrl-g".into();
        cfg.save_to(dir.path()).unwrap();
        assert_eq!(Config::load_from(dir.path()).unwrap(), cfg);
    }

    #[cfg(unix)]
    #[test]
    fn saved_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = Config::default().save_to(dir.path()).unwrap();
        let mode = fs::metadata(path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn partial_file_fills_defaults() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("config.toml"),
            "[ai]\nprovider = \"claude\"\n",
        )
        .unwrap();
        let cfg = Config::load_from(dir.path()).unwrap();
        assert_eq!(cfg.ai.provider, Some(ProviderKind::Anthropic));
        assert!(cfg.history.enabled);
        assert_eq!(cfg.shell.palette_key, "ctrl-t");
    }

    #[test]
    fn migrates_legacy_json() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("config.json"),
            r#"{"provider":"gemini","api_key":"AIzaTEST","auto_ai_on_error":true}"#,
        )
        .unwrap();
        let cfg = Config::load_from(dir.path()).unwrap();
        assert_eq!(cfg.ai.provider, Some(ProviderKind::Gemini));
        assert_eq!(cfg.ai.api_key.as_deref(), Some("AIzaTEST"));
        assert!(dir.path().join("config.toml").exists());
    }

    #[test]
    fn masks_secrets() {
        assert_eq!(mask_secret("sk-ant-api03-abcdefghijwxyz"), "sk-ant…wxyz");
        assert_eq!(mask_secret("short"), "………");
    }
}
