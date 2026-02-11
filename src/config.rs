use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub provider: String,
    pub gemini_api_key: String,
}

fn config_path() -> PathBuf {
    let mut path = dirs::home_dir().unwrap();
    path.push(".terrek_config.json");
    path
}

pub fn load_config() -> Option<Config> {
    let path = config_path();
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

pub fn save_config(cfg: &Config) -> anyhow::Result<()> {
    let path = config_path();
    fs::write(path, serde_json::to_string_pretty(&cfg)?)?;
    Ok(())
}
