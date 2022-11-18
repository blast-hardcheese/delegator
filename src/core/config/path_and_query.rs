use actix_web::http::uri::PathAndQuery;
use serde::{de::Visitor, Deserializer};

struct ConfigPathAndQueryVisitor;

impl<'de> Visitor<'de> for ConfigPathAndQueryVisitor {
    type Value = PathAndQuery;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("EXPECTED: path and query component, eg: /foo?bar=baz")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        PathAndQuery::try_from(v).map_err(|_err| E::custom("Unable to parse path and query"))
    }
}

pub fn deserialize<'de, D>(de: D) -> Result<PathAndQuery, D::Error>
where
    D: Deserializer<'de>,
{
    de.deserialize_str(ConfigPathAndQueryVisitor)
}
