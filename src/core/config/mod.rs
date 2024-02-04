pub mod events;
pub mod http_method;
pub mod path_and_query;
pub mod scheme;
mod stringy_duration;

use std::time::Duration;

use actix_web::http::{
    Method,
    uri::{Authority, PathAndQuery, Scheme},
};
use derive_more::Display;
use hashbrown::HashMap;

use serde::{Deserialize, Deserializer};
use serde_plain;

use toml;

use self::events::EventConfig;

#[derive(Clone, Debug, Deserialize)]
pub struct Virtualhosts {
    pub catalog: String,
    pub closet: String,
    pub pricing: String,
}

#[derive(Clone, Debug, Deserialize, Display, Eq, Hash, PartialEq)]
pub enum MethodName {
    #[serde(alias = "autocomplete")]
    Autocomplete,
    #[serde(alias = "explore")]
    Explore,
    #[serde(alias = "search")]
    Search,
    #[serde(alias = "search_history")]
    SearchHistory,
    #[serde(alias = "list")]
    List,
    #[serde(alias = "lookup")]
    Lookup,
}


#[derive(Clone, Debug, Deserialize, Display, Eq, Hash, PartialEq)]
pub enum ServiceName {
    #[serde(rename = "apex")]
    Apex,
    #[serde(rename = "catalog")]
    Catalog,
    #[serde(rename= "closet")]
    Closet,
    #[serde(rename = "identity")]
    Identity,
    #[serde(rename = "pricing")]
    Pricing,
    #[serde(rename = "recommendations")]
    Recommendations,
}

impl ServiceName {
    pub fn from_str(name: &str) -> Result<Self, &'static str> {
        let deserialized_name = serde_plain::from_str::<ServiceName>(name).map_err(|_| "No matching service name found")?;
        Ok(deserialized_name)
    }
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
        methods: HashMap<MethodName, MethodDefinition>,
    },
}

pub type Services = HashMap<ServiceName, ServiceDefinition>;

#[derive(Clone, Debug, Deserialize)]
pub struct Configuration {
    pub http: HttpConfig,
    #[serde(deserialize_with = "deserialize_services")]
    pub services: Services,
    pub virtualhosts: Virtualhosts,
    pub events: EventConfig,
}

fn deserialize_services<'de, D>(deserializer: D) -> Result<Services, D::Error>
where
    D: Deserializer<'de>,
{
    let mut services: Services = HashMap::new();
    let service_defs: HashMap<String, ServiceDefinition> = Deserialize::deserialize(deserializer)?;
    for (key, value) in service_defs {
        let service_name = ServiceName::from_str(&key).map_err(serde::de::Error::custom)?;
        services.insert(service_name, value);
    }
    Ok(services)
}

pub fn load_file(path: &str) -> Result<Configuration, std::io::Error> {
    let config_str = std::fs::read_to_string(path)?;
    let config: Configuration = toml::from_str(&config_str).unwrap();
    Ok(config)
}
