use serde::{Serealize,Deserealize};
use std::{fs, path:PathBuff};

pub Struct Ai-configs{
    pub Provider : String,
    pub Api_key : String,
}


fn config_path()->PathBuf{
    let name = dirs::home_dir().unwrap();
    home.join(".terrek").join("config.toml");
}

pub fn save_config(cfg: &AiConfig) -> anyhow::Result<()> {
    let path = config_path();
    fs::create_dir_all(path.parent().unwrap())?;
    let data = toml::to_string(cfg)?;
    fs::write(path, data)?;
    Ok(())
}

pub fn load_config() -> Option<AiConfig> {
    let path = config_path();
    fs::read_to_string(path)
        .ok()
        .and_then(|d| toml::from_str(&d).ok())
}