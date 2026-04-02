use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    #[serde(default = "default_provider")]
    pub provider: String,

    pub gemini_api_key: String,

    #[serde(default = "default_true")]
    pub auto_ai_on_error: bool,
}

fn default_provider() -> String {
    "gemini".to_string()
}

fn config_path() -> PathBuf {
    let mut path = dirs::config_dir().expect("Cannot find config directory");
    path.push("terrek");
    fs::create_dir_all(&path).ok();
    path.push("config.json");
    path
}

pub fn save_config(cfg: &Config) -> Result<()> {
    let path = config_path();
    fs::write(path, serde_json::to_string_pretty(cfg)?)?;
    Ok(())
}

pub fn load_config() -> Option<Config> {
    let path = config_path();
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}
pub fn config_exists() -> bool {
    config_path().exists()
}

pub fn delete_config() -> anyhow::Result<()> {
    let path = config_path();

    if path.exists() {
        std::fs::remove_file(path)?;
    }

    Ok(())
}
