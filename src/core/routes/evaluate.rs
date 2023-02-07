use async_trait::async_trait;

use actix_web::{
    error,
    http::{Method, Uri},
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
    let live_client = LiveJsonClient {
        client,
        client_config: client_config.get_ref().clone(),
    };

    let result = do_evaluate(cryptogram.into_inner(), live_client, services.get_ref()).await?;
    Ok(HttpResponse::Ok().json(&result))
}

#[async_trait(?Send)]
trait JsonClient {
    async fn issue_request(
        &self,
        method: Method,
        uri: Uri,
        value: Value,
    ) -> Result<Value, EvaluateError>;
}

struct LiveJsonClient {
    client: awc::Client,
    client_config: HttpClientConfig,
}

#[async_trait(?Send)]
impl JsonClient for LiveJsonClient {
    async fn issue_request(
        &self,
        method: Method,
        uri: Uri,
        payload: Value,
    ) -> Result<Value, EvaluateError> {
        self.client
            .request(method, uri)
            .insert_header(("User-Agent", self.client_config.user_agent.clone()))
            .insert_header(("Content-Type", "application/json"))
            .send_json(&payload)
            .await
            .map_err(EvaluateError::ClientError)?
            .json::<Value>()
            .await
            .map_err(EvaluateError::InvalidJsonError)
    }
}

struct TestJsonClient;

#[async_trait(?Send)]
impl JsonClient for TestJsonClient {
    async fn issue_request(
        &self,
        _method: Method,
        _uri: Uri,
        payload: Value,
    ) -> Result<Value, EvaluateError> {
        Ok(payload)
    }
}

async fn do_evaluate<JC: JsonClient>(
    cryptogram: JsonCryptogram,
    json_client: JC,
    services: &Services,
) -> Result<Value, EvaluateError> {
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

                let result = json_client
                    .issue_request(Method::POST, uri, step.payload.clone())
                    .await?;
                Some(result)
            }
        };
    }

    final_result.ok_or(EvaluateError::NoStepsSpecified)
}

#[actix_web::test]
async fn routes_evaluate() {
    use crate::config::MethodDefinition;
    use actix_web::http::uri::{Authority, PathAndQuery, Scheme};
    use hashbrown::hash_map::DefaultHashBuilder;
    use hashbrown::HashMap;

    let cryptogram = JsonCryptogram {
        steps: vec![JsonCryptogramStep {
            service: "foo".to_owned(),
            method: "bar".to_owned(),
            payload: Value::Null,
        }],
    };

    let mut services = {
        let s = DefaultHashBuilder::default();
        HashMap::with_hasher(s)
    };

    services.insert(
        "foo".to_owned(),
        ServiceDefinition::Rest {
            scheme: Scheme::HTTP,
            endpoint: Authority::from_static("localhost:8080"),
            methods: {
                let mut methods = {
                    let s = DefaultHashBuilder::default();
                    HashMap::with_hasher(s)
                };
                methods.insert(
                    "bar".to_owned(),
                    MethodDefinition {
                        path: PathAndQuery::from_static("/foo?bar=baz"),
                    },
                );
                methods
            },
        },
    );

    match do_evaluate(cryptogram, TestJsonClient, &services).await {
        Ok(value) => assert_eq!(value, Value::Null),
        other => {
            let _ = other.unwrap();
        }
    }
}

pub fn configure(server: &mut web::ServiceConfig) {
    server.route("/evaluate", web::post().to(evaluate));
}
