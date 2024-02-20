use actix_web::error::{self, PayloadError};
use awc::error::JsonPayloadError;
use serde_json::{json, Value};
use std::{fmt, str::Utf8Error};

use crate::client::JsonClient;
use crate::model::cryptogram::Cryptogram;
use crate::routes::errors::JsonResponseError;

impl fmt::Display for EvaluateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl JsonResponseError for EvaluateError {
    fn error_as_json(&self) -> Value {
        serde_json::Value::from(self)
    }
}

impl std::convert::From<&EvaluateError> for serde_json::Value {
    fn from(error: &EvaluateError) -> Self {
        match error {
            EvaluateError::ClientError(inner) => {
                json!({"err": "client", "value": inner.to_string()})
            }
            EvaluateError::InvalidJsonError(inner) => {
                json!({"err": "protocol", "value": inner.to_string()})
            }
            EvaluateError::InvalidPayloadError(inner) => {
                json!({"err": "payload", "value": inner.to_string()})
            }
            EvaluateError::NetworkError(context) => context.clone(),
            EvaluateError::UnknownService(service_name) => {
                json!({"err": "unknown_service", "service_name": service_name})
            }
            EvaluateError::UriBuilderError(_inner) => json!({"err": "uri_builder_error"}),
            EvaluateError::Utf8Error(_inner) => json!({"err": "utf8_error"}),
        }
    }
}

#[derive(Debug)]
pub enum EvaluateError {
    ClientError(crate::client::ClientError),
    InvalidJsonError(JsonPayloadError),
    InvalidPayloadError(PayloadError),
    NetworkError(Value),
    UnknownService(String),
    UriBuilderError(error::HttpError),
    Utf8Error(Utf8Error),
}

impl From<crate::client::ClientError> for EvaluateError {
    fn from(error: crate::client::ClientError) -> Self {
        EvaluateError::ClientError(error)
    }
}

pub async fn do_evaluate<JC: JsonClient>(
    mut cryptogram: Cryptogram,
    json_client: JC,
) -> Result<Cryptogram, EvaluateError> {
    while cryptogram.current < cryptogram.steps.len() {
        let current_step = &cryptogram.steps[cryptogram.current];
        let service_name = &current_step.service;

        let service_metadata = crate::registry::lookup(&cryptogram).await;
        let service = service_metadata
            .images
            .get(service_name)
            .ok_or(EvaluateError::UnknownService(service_name.to_string()))?;

        let authority = crate::provisioner::lookup(service.spec.clone()).await;

        cryptogram = json_client.issue_request(authority, &cryptogram).await?
    }
    Ok(cryptogram)
}
