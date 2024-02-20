use async_trait::async_trait;

use actix_web::{
    error::{HttpError, PayloadError},
    http::{uri::Authority, Method, Uri},
};
use awc::error::{JsonPayloadError, SendRequestError};
use serde_json::{json, Value};
use std::{fmt, str::Utf8Error};

use crate::config::HttpClientConfig;

use crate::model::cryptogram::Cryptogram;

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::convert::From<&ClientError> for serde_json::Value {
    fn from(error: &ClientError) -> Self {
        match error {
            ClientError::SendError(inner) => {
                json!({"err": "client", "value": inner.to_string()})
            }
            ClientError::InvalidJsonError(inner) => {
                json!({"err": "protocol", "value": inner.to_string()})
            }
            ClientError::InvalidPayloadError(inner) => {
                json!({"err": "payload", "value": inner.to_string()})
            }
            ClientError::NetworkError(context) => context.clone(),
            ClientError::UriBuilderError(_inner) => json!({"err": "uri_builder_error"}),
            ClientError::Utf8Error(_inner) => json!({"err": "utf8_error"}),
        }
    }
}

#[derive(Debug)]
pub enum ClientError {
    SendError(SendRequestError),
    InvalidJsonError(JsonPayloadError),
    InvalidPayloadError(PayloadError),
    NetworkError(Value),
    UriBuilderError(HttpError),
    Utf8Error(Utf8Error),
}

#[async_trait(?Send)]
pub trait JsonClient {
    async fn issue_request(
        &self,
        authority: Authority,
        cryptogram: &Cryptogram,
    ) -> Result<Cryptogram, ClientError>;
}

pub struct LiveJsonClient {
    pub client: awc::Client,
    pub client_config: HttpClientConfig,
}

impl LiveJsonClient {
    pub fn build(client_config: &HttpClientConfig) -> LiveJsonClient {
        let client = {
            awc::ClientBuilder::new()
                .timeout(client_config.default_timeout)
                .finish()
        };
        LiveJsonClient {
            client,
            client_config: client_config.clone(),
        }
    }
}

#[async_trait(?Send)]
impl JsonClient for LiveJsonClient {
    async fn issue_request(
        &self,
        authority: Authority,
        payload: &Cryptogram,
    ) -> Result<Cryptogram, ClientError> {
        let req = self
            .client
            .request(
                Method::POST,
                Uri::builder()
                    .authority(authority)
                    .path_and_query("/evaluate")
                    .build()
                    .map_err(ClientError::UriBuilderError)?,
            )
            .insert_header(("User-Agent", self.client_config.user_agent.clone()))
            .insert_header(("Content-Type", "application/json"));
        let mut result = req
            .send_json(payload)
            .await
            .map_err(ClientError::SendError)?;
        if !result.status().is_success() {
            let context = if let Ok(json) = result.json::<Value>().await {
                json
            } else {
                let bytes = result
                    .body()
                    .await
                    .map_err(ClientError::InvalidPayloadError)?;
                let text = std::str::from_utf8(&bytes).map_err(ClientError::Utf8Error)?;
                Value::String(String::from(text))
            };

            return Err(ClientError::NetworkError(context));
        }
        result
            .json::<Cryptogram>()
            .await
            .map_err(ClientError::InvalidJsonError)
    }
}

struct TestJsonClient;

#[async_trait(?Send)]
impl JsonClient for TestJsonClient {
    async fn issue_request(
        &self,
        _authority: Authority,
        payload: &Cryptogram,
    ) -> Result<Cryptogram, ClientError> {
        Ok(payload.clone())
    }
}
