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

use serde::{Deserialize, Deserializer};

use toml;

use self::events::EventConfig;
use crate::routes::evaluate::JsonCryptogram;

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
        virtualhosts: Option<Vec<String>>,
    },
}

fn decode_cryptogram<'de, D>(deserializer: D) -> Result<JsonCryptogram, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    use std::str::FromStr;

    let s = String::deserialize(deserializer)?;
    JsonCryptogram::from_str(&s).map_err(Error::custom)
}

#[derive(Clone, Debug, Deserialize)]
pub struct EdgeRoute {
    #[serde(deserialize_with = "decode_cryptogram")]
    pub cryptogram: JsonCryptogram,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Virtualhost {
    pub host: String,
    pub routes: HashMap<String, EdgeRoute>,
}

pub type Services = HashMap<String, ServiceDefinition>;
pub type Virtualhosts = HashMap<String, Virtualhost>;

#[derive(Clone, Debug, Deserialize)]
pub struct Configuration {
    pub http: HttpConfig,
    pub services: Services,
    pub events: EventConfig,
    pub virtualhosts: Virtualhosts,
}

pub fn load_file(path: &str) -> Result<Configuration, std::io::Error> {
    let config_str = std::fs::read_to_string(path)?;
    let config: Configuration = toml::from_str(&config_str).unwrap();
    Ok(config)
}
