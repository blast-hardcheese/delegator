use actix_web::http::uri::Scheme;
use serde::{de::Visitor, Deserializer};

struct ConfigSchemeVisitor;

impl<'de> Visitor<'de> for ConfigSchemeVisitor {
    type Value = Scheme;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("EXPECTED: http")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        match v {
            "http" => Ok(Scheme::HTTP),
            _other => Err(E::custom("Unexpected scheme")),
        }
    }
}

pub fn deserialize<'de, D>(de: D) -> Result<Scheme, D::Error>
where
    D: Deserializer<'de>,
{
    de.deserialize_str(ConfigSchemeVisitor)
}
