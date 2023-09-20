use config::{Config, ConfigError, File};
use serde::Deserialize;
use std::fmt;

#[derive(Debug, Deserialize, Clone)]
pub struct Server {
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ImgSource {
    pub path: String,
    pub bucket: String,
    pub cache_bucket: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub server: Server,
    pub img_sources: Vec<ImgSource>,
    pub env: ENV,
}

const VULPIX_APP_ENVIRONMENT: &str = "VULPIX_APP_ENVIRONMENT";
const CONFIG_FILE_PREFIX: &str = "config";

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let env = std::env::var(VULPIX_APP_ENVIRONMENT).unwrap_or_else(|_| "Local".into());
        Config::builder()
            .add_source(File::with_name(&format!(
                "./{}/Default.toml",
                CONFIG_FILE_PREFIX
            )))
            .add_source(
                File::with_name(&format!("./{}/{}", CONFIG_FILE_PREFIX, env)).required(false),
            )
            .set_override("env", env)?
            .build()?
            .try_deserialize()
    }
}

#[derive(Clone, Debug, Deserialize)]
pub enum ENV {
    Local,
    Production,
}

impl fmt::Display for ENV {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ENV::Local => write!(f, "Local"),
            ENV::Production => write!(f, "Production"),
        }
    }
}

impl From<&str> for ENV {
    fn from(env: &str) -> Self {
        match env {
            "Local" => ENV::Local,
            "Production" => ENV::Production,
            _ => ENV::Local,
        }
    }
}
