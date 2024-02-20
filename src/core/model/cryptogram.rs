use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Cryptogram {
    pub current: usize,
    pub steps: Vec<CryptogramStep>,
}

impl FromStr for Cryptogram {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CryptogramStep {
    pub service: String,
    pub method: String,
    pub payload: String,
}

impl CryptogramStep {
    pub fn build(service: &str, method: &str) -> CryptogramStepNeedsPayload {
        CryptogramStepNeedsPayload {
            service: service.to_string(),
            method: method.to_string(),
        }
    }
}

pub struct CryptogramStepNeedsPayload {
    service: String,
    method: String,
}

impl CryptogramStepNeedsPayload {
    pub fn payload(self, payload: String) -> CryptogramStepBuilder {
        CryptogramStepBuilder {
            inner: CryptogramStep {
                service: self.service,
                method: self.method,
                payload,
            },
        }
    }
}

pub struct CryptogramStepBuilder {
    inner: CryptogramStep,
}

impl CryptogramStepBuilder {
    pub fn finish(self) -> CryptogramStep {
        self.inner
    }
}
