use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub gemini_api_key: String,
}

fn config_path() -> PathBuf {
    dirs::home_dir().unwrap().join(".terrek/config.json")
}

pub fn load_config() -> Option<Config> {
    let path = config_path();
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

pub fn save_config(key: &str) -> anyhow::Result<()> {
    let path = config_path();

    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }

    let cfg = Config {
        gemini_api_key: key.to_string(),
    };

    fs::write(path, serde_json::to_string_pretty(&cfg)?)?;
    Ok(())
}
