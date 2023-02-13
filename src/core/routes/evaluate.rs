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

use crate::config::{HttpClientConfig, MethodName, ServiceDefinition, ServiceName, Services};

#[derive(Debug, Deserialize)]
pub struct JsonCryptogramStep {
    service: ServiceName,
    method: MethodName,
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
    UnknownMethod(MethodName),
    UnknownService(ServiceName),
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
        steps: vec![
            JsonCryptogramStep {
                service: ServiceName::Catalog,
                method: MethodName::Search,
                payload: Value::String("first".to_owned()),
            },
            JsonCryptogramStep {
                service: ServiceName::Catalog,
                method: MethodName::Lookup,
                payload: Value::String("second".to_owned()),
            },
        ],
    };

    let mut services = {
        let s = DefaultHashBuilder::default();
        HashMap::with_hasher(s)
    };

    services.insert(
        ServiceName::Catalog,
        ServiceDefinition::Rest {
            scheme: Scheme::HTTP,
            endpoint: Authority::from_static("0:0"),
            methods: {
                let mut methods = {
                    let s = DefaultHashBuilder::default();
                    HashMap::with_hasher(s)
                };
                methods.insert(
                    MethodName::Search,
                    MethodDefinition {
                        path: PathAndQuery::from_static("/search/"),
                    },
                );
                methods.insert(
                    MethodName::Lookup,
                    MethodDefinition {
                        path: PathAndQuery::from_static("/product_variants/"),
                    },
                );
                methods
            },
        },
    );

    match do_evaluate(cryptogram, TestJsonClient, &services).await {
        Ok(value) => assert_eq!(value, Value::String("second".to_owned())),
        other => {
            let _ = other.unwrap();
        }
    }
}

pub fn configure(server: &mut web::ServiceConfig) {
    server.route("/evaluate", web::post().to(evaluate));
}
