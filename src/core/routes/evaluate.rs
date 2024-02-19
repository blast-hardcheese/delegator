use async_trait::async_trait;

use actix_web::{
    body::BoxBody,
    error::{self, PayloadError},
    http::{uri::Authority, Method, Uri},
    web::{self, Data, Json},
    HttpResponse, ResponseError,
};
use awc::error::{JsonPayloadError, SendRequestError};
use serde_json::{json, Value};
use std::{fmt, str::Utf8Error};

use crate::config::HttpClientConfig;

use crate::model::cryptogram::Cryptogram;
use crate::routes::errors::{json_error_response, JsonResponseError};

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
    ClientError(SendRequestError),
    InvalidJsonError(JsonPayloadError),
    InvalidPayloadError(PayloadError),
    NetworkError(Value),
    UnknownService(String),
    UriBuilderError(error::HttpError),
    Utf8Error(Utf8Error),
}

impl ResponseError for EvaluateError {
    fn error_response(&self) -> HttpResponse<BoxBody> {
        json_error_response(self)
    }
}

async fn evaluate(
    cryptogram: Json<Cryptogram>,
    client_config: Data<HttpClientConfig>,
) -> Result<HttpResponse, EvaluateError> {
    let live_client = LiveJsonClient::build(client_config.get_ref());

    let result = do_evaluate(cryptogram.into_inner(), live_client).await?;
    Ok(HttpResponse::Ok().json(&result))
}

#[async_trait(?Send)]
pub trait JsonClient {
    async fn issue_request(
        &self,
        authority: Authority,
        cryptogram: &Cryptogram,
    ) -> Result<Cryptogram, EvaluateError>;
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
    ) -> Result<Cryptogram, EvaluateError> {
        let req = self
            .client
            .request(
                Method::POST,
                Uri::builder()
                    .authority(authority)
                    .path_and_query("/evaluate")
                    .build()
                    .map_err(EvaluateError::UriBuilderError)?,
            )
            .insert_header(("User-Agent", self.client_config.user_agent.clone()))
            .insert_header(("Content-Type", "application/json"));
        let mut result = req
            .send_json(payload)
            .await
            .map_err(EvaluateError::ClientError)?;
        if !result.status().is_success() {
            let context = if let Ok(json) = result.json::<Value>().await {
                json
            } else {
                let bytes = result
                    .body()
                    .await
                    .map_err(EvaluateError::InvalidPayloadError)?;
                let text = std::str::from_utf8(&bytes).map_err(EvaluateError::Utf8Error)?;
                Value::String(String::from(text))
            };

            return Err(EvaluateError::NetworkError(context));
        }
        result
            .json::<Cryptogram>()
            .await
            .map_err(EvaluateError::InvalidJsonError)
    }
}

struct TestJsonClient;

#[async_trait(?Send)]
impl JsonClient for TestJsonClient {
    async fn issue_request(
        &self,
        _authority: Authority,
        payload: &Cryptogram,
    ) -> Result<Cryptogram, EvaluateError> {
        Ok(payload.clone())
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

pub fn configure(server: &mut web::ServiceConfig) {
    server.route("/evaluate", web::post().to(evaluate));
}
