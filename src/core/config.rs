use std::collections::HashMap;

use hocon::{Error, HoconLoader};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct HttpConfig {
    pub host: String,
    pub port: u16,
}

pub type Services = HashMap<String, String>;

#[derive(Clone, Debug, Deserialize)]
pub struct Configuration {
    pub http: HttpConfig,
    pub services: Services,
}

pub fn load_file(path: &str) -> Result<Configuration, Error> {
    HoconLoader::new().load_file(path)?.hocon()?.resolve()
}
