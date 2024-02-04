pub mod events;
pub mod http_method;
pub mod path_and_query;
pub mod scheme;
mod stringy_duration;

use std::time::Duration;

use actix_web::http::{
    uri::{Authority, PathAndQuery, Scheme},
    Method,
};
use hashbrown::HashMap;

use serde::Deserialize;

use toml;

use self::events::EventConfig;

#[derive(Clone, Debug, Deserialize)]
pub struct Virtualhosts {
    pub catalog: String,
    pub closet: String,
    pub pricing: String,
}

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
pub struct MethodDefinition {
    #[serde(with = "path_and_query")]
    pub path: PathAndQuery,
    #[serde(with = "http_method")]
    pub method: Method,
}

/* ServiceDefinition
 *
 * This enumeration is intended to support multiple transport protocols in the future,
 * so the `protocol` field must be set to `rest` for the time being.
 *
 * NB: Attempting to use "untagged" deserializing obscured underlying errors.
 */
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "protocol")]
pub enum ServiceDefinition {
    #[serde(rename(serialize = "rest", deserialize = "rest"))]
    Rest {
        #[serde(with = "scheme")]
        scheme: Scheme,
        #[serde(with = "http_serde::authority")]
        authority: Authority,
        methods: HashMap<String, MethodDefinition>,
    },
}

pub type Services = HashMap<String, ServiceDefinition>;

#[derive(Clone, Debug, Deserialize)]
pub struct Configuration {
    pub http: HttpConfig,
    pub services: Services,
    pub virtualhosts: Virtualhosts,
    pub events: EventConfig,
}

pub fn load_file(path: &str) -> Result<Configuration, std::io::Error> {
    let config_str = std::fs::read_to_string(path)?;
    let config: Configuration = toml::from_str(&config_str).unwrap();
    Ok(config)
}
