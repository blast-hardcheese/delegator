use serde::de::Visitor;
use serde::{Deserialize, Deserializer};
use std::fmt;

use crate::translate::parse::parse_language;
use crate::translate::Language;

struct LanguageVisitor;

impl<'de> Visitor<'de> for LanguageVisitor {
    type Value = Language;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "A valid core::translate::Language specifier")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let (rest, lang) = parse_language(v).map_err(|err| {
            serde::de::Error::custom(format!("Error parsing language: {:?}", err))
        })?;
        if !rest.is_empty() {
            return Err(serde::de::Error::custom(format!(
                "Unconsumed language input: {}",
                rest
            )));
        }
        Ok(lang)
    }
}

impl<'de> Deserialize<'de> for Language {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(LanguageVisitor)
    }
}
