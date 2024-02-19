mod stringy_duration;

use std::time::Duration;

use serde::Deserialize;

use toml;

#[derive(Clone, Debug, Deserialize)]
pub struct HttpClientConfig {
    #[serde(alias = "user-agent")]
    pub user_agent: String,
    #[serde(alias = "default-timeout", with = "stringy_duration")]
    pub default_timeout: Duration,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HttpConfig {
    pub client: HttpClientConfig,
    pub host: String,
    pub port: u16,
    pub cors: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Configuration {
    pub http: HttpConfig,
}

pub fn load_file(path: &str) -> Result<Configuration, std::io::Error> {
    let config_str = std::fs::read_to_string(path)?;
    let config: Configuration = toml::from_str(&config_str).unwrap();
    Ok(config)
}
