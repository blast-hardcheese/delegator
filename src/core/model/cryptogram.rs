use serde::Deserialize;
use serde_json::Value;
use std::str::FromStr;

use crate::translate::Language;

#[derive(Clone, Debug, Deserialize)]
pub struct JsonCryptogram {
    pub steps: Vec<JsonCryptogramStep>,
}

impl FromStr for JsonCryptogram {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct JsonCryptogramStep {
    pub service: Option<String>,
    pub method: Option<String>,
    pub payload: Option<Value>,
    pub preflight: Option<Language>,
    pub postflight: Option<Language>,
    pub memoization_prefix: Option<String>,
    pub headers: Option<Vec<(String, String)>>,
}

impl JsonCryptogramStep {
    pub fn build(service: &str, method: &str) -> JsonCryptogramStepNeedsPayload {
        JsonCryptogramStepNeedsPayload {
            service: service.to_string(),
            method: method.to_string(),
        }
    }
}

pub struct JsonCryptogramStepNeedsPayload {
    service: String,
    method: String,
}

impl JsonCryptogramStepNeedsPayload {
    pub fn payload(self, payload: Value) -> JsonCryptogramStepBuilder {
        JsonCryptogramStepBuilder {
            inner: JsonCryptogramStep {
                service: Some(self.service),
                method: Some(self.method),
                payload: Some(payload),
                preflight: None,
                postflight: None,
                memoization_prefix: None,
                headers: None,
            },
        }
    }
}

pub struct JsonCryptogramStepBuilder {
    inner: JsonCryptogramStep,
}

impl JsonCryptogramStepBuilder {
    pub fn preflight(self, preflight: Language) -> JsonCryptogramStepBuilder {
        JsonCryptogramStepBuilder {
            inner: JsonCryptogramStep {
                preflight: Some(preflight),
                ..self.inner
            },
        }
    }

    pub fn postflight(self, postflight: Language) -> JsonCryptogramStepBuilder {
        JsonCryptogramStepBuilder {
            inner: JsonCryptogramStep {
                postflight: Some(postflight),
                ..self.inner
            },
        }
    }

    pub fn memoization_prefix(self, prefix: String) -> JsonCryptogramStepBuilder {
        JsonCryptogramStepBuilder {
            inner: JsonCryptogramStep {
                memoization_prefix: Some(prefix),
                ..self.inner
            },
        }
    }

    pub fn header(self, key: String, value: String) -> JsonCryptogramStepBuilder {
        let mut headers = self.inner.headers.unwrap_or_default();
        headers.push((key, value));
        JsonCryptogramStepBuilder {
            inner: JsonCryptogramStep {
                headers: Some(headers),
                ..self.inner
            },
        }
    }

    pub fn headers(self, pairs: Vec<(String, String)>) -> JsonCryptogramStepBuilder {
        let mut headers = self.inner.headers.unwrap_or_default();
        for pair in pairs {
            headers.push(pair);
        }
        JsonCryptogramStepBuilder {
            inner: JsonCryptogramStep {
                headers: Some(headers),
                ..self.inner
            },
        }
    }

    pub fn finish(self) -> JsonCryptogramStep {
        self.inner
    }
}
