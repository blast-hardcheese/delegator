pub mod path_and_query;
pub mod scheme;

use actix_web::http::uri::{Authority, PathAndQuery, Scheme};
use derive_more::Display;
use hashbrown::HashMap;

use hocon::{Error, HoconLoader};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, Display, Eq, Hash, PartialEq)]
pub enum MethodName {
    #[serde(alias = "search")]
    Search,
    #[serde(alias = "lookup")]
    Lookup,
}

#[derive(Clone, Debug, Deserialize, Display, Eq, Hash, PartialEq)]
pub enum ServiceName {
    #[serde(rename(deserialize = "catalog"))]
    Catalog,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HttpClientConfig {
    #[serde(alias = "user-agent")]
    pub user_agent: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HttpConfig {
    pub client: HttpClientConfig,
    pub host: String,
    pub port: u16,
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
    pub environment: String,
    pub http: HttpConfig,
    pub sentry: SentryConfig,
    pub services: Services,
    pub environments: HashMap<String, HashMap<ServiceName, ServiceLocation>>,
}

pub fn load_file(path: &str) -> Result<Configuration, Error> {
    HoconLoader::new().load_file(path)?.hocon()?.resolve()
}
