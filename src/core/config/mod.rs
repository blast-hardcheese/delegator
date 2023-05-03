pub mod http_method;
pub mod path_and_query;
pub mod scheme;
mod stringy_duration;

use std::time::Duration;

use actix_web::http::{
    uri::{Authority, PathAndQuery, Scheme},
    Method,
};
use derive_more::Display;
use hashbrown::HashMap;

use hocon::{Error, HoconLoader};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Virtualhosts {
    pub catalog: String,
    pub pricing: String,
}

#[derive(Clone, Debug, Deserialize, Display, Eq, Hash, PartialEq)]
pub enum MethodName {
    #[serde(alias = "explore")]
    Explore,
    #[serde(alias = "search")]
    Search,
    #[serde(alias = "lookup")]
    Lookup,
}

#[derive(Clone, Debug, Deserialize, Display, Eq, Hash, PartialEq)]
pub enum ServiceName {
    #[serde(rename(deserialize = "catalog"))]
    Catalog,
    #[serde(rename(deserialize = "pricing"))]
    Pricing,
    #[serde(rename(deserialize = "recommendations"))]
    Recommendations,
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
pub struct SentryConfig {
    pub dsn: Option<String>,
    pub environment: Option<String>,
    pub release: Option<String>,
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
pub struct ServiceLocation {
    #[serde(with = "scheme")]
    pub scheme: Scheme,
    #[serde(with = "http_serde::authority")]
    pub authority: Authority,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Configuration {
    pub authorities: HashMap<ServiceName, ServiceLocation>,
    pub environment: String,
    pub http: HttpConfig,
    pub sentry: SentryConfig,
    pub services: Services,
    pub virtualhosts: Virtualhosts,
}

pub fn load_file(path: &str) -> Result<Configuration, Error> {
    HoconLoader::new().load_file(path)?.hocon()?.resolve()
}
