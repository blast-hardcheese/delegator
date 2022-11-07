use hocon::{Error, HoconLoader};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct HttpConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Deserialize)]
pub struct Configuration {
    pub http: HttpConfig,
}

pub fn load_file(path: &str) -> Result<Configuration, Error> {
    HoconLoader::new().load_file(path)?.hocon()?.resolve()
}
