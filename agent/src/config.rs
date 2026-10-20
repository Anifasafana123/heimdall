use serde::Deserialize;
use std::{fs, path::Path};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server_url: String,
    pub computer_id: String,
    #[serde(default = "default_interval")]
    pub interval_secs: u64,
    pub auth_token: String,
}

fn default_interval() -> u64 {
    10
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Config, String> {
        let text = fs::read_to_string(path.as_ref())
            .map_err(|e| format!("cannot read config file: {e}"))?;
        let cfg: Config = toml::from_str(&text).map_err(|e| format!("invalid config file: {e}"))?;
        if cfg.interval_secs == 0 {
            return Err("interval_secs must be greater than 0".to_string());
        }
        Ok(cfg)
    }
}
