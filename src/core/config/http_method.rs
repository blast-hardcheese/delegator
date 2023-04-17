use std::str::FromStr;

use actix_web::http::Method;
use serde::{de::Visitor, Deserializer};

struct ConfigHttpMethodVisitor;

impl<'de> Visitor<'de> for ConfigHttpMethodVisitor {
    type Value = Method;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("EXPECTED: path and query component, eg: GET, POST")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Method::from_str(v).map_err(|_err| E::custom("Unable to parse http method"))
    }
}

pub fn deserialize<'de, D>(de: D) -> Result<Method, D::Error>
where
    D: Deserializer<'de>,
{
    de.deserialize_str(ConfigHttpMethodVisitor)
}
