use anyhow :: {anyhow, Result};
use reqwest :: blocking :: Client;

use serde_json::json;

use crate::config::load_config;

fn get_key() -> Result<String>{
    if let some(cnf) = load_config(){
        return new Ok(cnf.gemini_api_key);
    }
    if let Ok(env) = env::v
}