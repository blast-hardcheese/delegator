use actix_web::{
    error,
    http::Uri,
    web::{self, Data, Json},
    HttpResponse,
};
use awc::error::{JsonPayloadError, SendRequestError};
use derive_more::Display;
use serde::Deserialize;
use serde_json::Value;

use crate::config::{HttpClientConfig, ServiceDefinition, Services};

#[derive(Debug, Deserialize)]
pub struct JsonCryptogramStep {
    service: String,
    method: String,
    payload: Value,
}

#[derive(Debug, Deserialize)]
pub struct JsonCryptogram {
    steps: Vec<JsonCryptogramStep>,
}

#[derive(Debug, Display)]
enum EvaluateError {
    ClientError(SendRequestError),
    InvalidJsonError(JsonPayloadError),
    NoStepsSpecified,
    UnknownMethod(String),
    UnknownService(String),
    UriBuilderError(error::HttpError),
}

impl error::ResponseError for EvaluateError {}

async fn evaluate(
    cryptogram: Json<JsonCryptogram>,
    client_config: Data<HttpClientConfig>,
    services: Data<Services>,
) -> Result<HttpResponse, EvaluateError> {
    let client = awc::Client::default();

    let mut final_result: Option<Value> = None;

    for step in &cryptogram.steps {
        let service = services
            .get(&step.service)
            .ok_or_else(|| EvaluateError::UnknownService(step.service.to_owned()))?
            .to_owned();
        final_result = match service {
            ServiceDefinition::Rest {
                scheme,
                endpoint,
                methods,
            } => {
                let method = methods
                    .get(&step.method)
                    .ok_or_else(|| EvaluateError::UnknownMethod(step.method.clone()))?;

                let uri = Uri::builder()
                    .scheme(scheme)
                    .authority(endpoint)
                    .path_and_query(method.path.to_owned())
                    .build()
                    .map_err(EvaluateError::UriBuilderError)?;
                let req = client
                    .post(uri)
                    .insert_header(("User-Agent", client_config.user_agent.clone()))
                    .insert_header(("Content-Type", "application/json"))
                    .send_json(&step.payload.clone());
                let mut res = req.await.map_err(EvaluateError::ClientError)?;
                let x = res
                    .json::<Value>()
                    .await
                    .map_err(EvaluateError::InvalidJsonError)?;
                Some(x)
            }
        };
    }

    let result = final_result.ok_or(EvaluateError::NoStepsSpecified)?;
    Ok(HttpResponse::Ok().json(&result))
}

pub fn configure(server: &mut web::ServiceConfig) {
    server.route("/evaluate", web::post().to(evaluate));
}
