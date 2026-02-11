use serde::{Serealize,Deserealize};
use std::{fs, path:PathBuff};

pub Struct Ai-configs{
    pub Provider : String,
    pub Api_key : String,
}


fn config_path()->PathBuf{
    let name = dirs::home_dir().unwrap();
    home.join("terrek").join("config.toml");
    
}