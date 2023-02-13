use async_trait::async_trait;

use actix_web::{
    error,
    http::{Method, Uri},
    web::{self, Data, Json},
    HttpResponse,
};
use awc::error::{JsonPayloadError, SendRequestError};
use derive_more::Display;
use hashbrown::HashMap;
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
    InvalidStructure,
    InvalidTransition,
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

fn post(step: &usize, state: &mut State, response: Value) -> Result<Value, EvaluateError> {
    let last = &state[step];
    let next_idx = step + 1;
    if !state.contains_key(&next_idx) {
        return Ok(response);
    }
    let next = &state[&next_idx];

    match ((&last.service, &last.method), (&next.service, &next.method)) {
        (
            (ServiceName::Catalog, MethodName::Search),
            (ServiceName::Catalog, MethodName::Lookup),
        ) => {
            let mut query = next
                .payload
                .as_object()
                .ok_or(EvaluateError::InvalidStructure)?
                .clone();

            // Results from search
            let results = response
                .as_object()
                .and_then(|o| o.get("results"))
                .and_then(|o| o.as_array())
                .ok_or(EvaluateError::InvalidStructure)?;
            let mut ids = vec![];
            for result in results {
                let id = result
                    .as_object()
                    .and_then(|o| o.get("product_variant_id"))
                    .ok_or(EvaluateError::InvalidStructure)?;
                ids.push(id.to_owned());
            }
            query.insert(String::from("ids"), Value::Array(ids.to_owned()));
            let new_payload = Value::Object(query);
            state.insert(
                next_idx,
                JsonCryptogramStep {
                    service: next.service.clone(),
                    method: next.method.clone(),
                    payload: new_payload.clone(),
                },
            );
            Ok(new_payload)
        }
        _other => Err(EvaluateError::InvalidTransition),
    }
}

#[async_trait(?Send)]
trait JsonClient {
    async fn issue_request(
        &self,
        method: Method,
        uri: Uri,
        value: &Value,
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
        payload: &Value,
    ) -> Result<Value, EvaluateError> {
        self.client
            .request(method, uri)
            .insert_header(("User-Agent", self.client_config.user_agent.clone()))
            .insert_header(("Content-Type", "application/json"))
            .send_json(payload)
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
        payload: &Value,
    ) -> Result<Value, EvaluateError> {
        Ok(payload.clone())
    }
}

type State = HashMap<usize, JsonCryptogramStep>;

async fn do_evaluate<JC: JsonClient>(
    cryptogram: JsonCryptogram,
    json_client: JC,
    services: &Services,
) -> Result<Value, EvaluateError> {
    let mut final_result: Option<Value> = None;

    let mut state: State = cryptogram.steps.into_iter().enumerate().collect();
    let mut step: usize = 0;
    while step < state.len() {
        let service_name = &state[&step].service;
        let method_name = &state[&step].method;
        let service = services
            .get(service_name)
            .ok_or_else(|| EvaluateError::UnknownService(service_name.to_owned()))?
            .to_owned();
        final_result = match service {
            ServiceDefinition::Rest {
                scheme,
                endpoint,
                methods,
            } => {
                let method = methods
                    .get(method_name)
                    .ok_or_else(|| EvaluateError::UnknownMethod(method_name.to_owned()))?;

                let uri = Uri::builder()
                    .scheme(scheme)
                    .authority(endpoint)
                    .path_and_query(method.path.to_owned())
                    .build()
                    .map_err(EvaluateError::UriBuilderError)?;

                let payload = &state[&step].payload;
                let result = json_client
                    .issue_request(Method::POST, uri, payload)
                    .await?;
                Some(post(&step, &mut state, result)?)
            }
        };
        step += 1;
    }

    final_result.ok_or(EvaluateError::NoStepsSpecified)
}

#[actix_web::test]
async fn routes_evaluate() {
    use crate::config::MethodDefinition;
    use crate::config::{MethodName, ServiceName};
    use actix_web::http::uri::{Authority, PathAndQuery, Scheme};
    use hashbrown::hash_map::DefaultHashBuilder;
    use serde_json::json;

    let cryptogram = JsonCryptogram {
        steps: vec![
            JsonCryptogramStep {
                service: ServiceName::Catalog,
                method: MethodName::Search,
                payload: json!({ "q": "Foo", "results": [{"product_variant_id": "12313bb7-6068-4ec9-ac49-3e834181f127"}] }),
            },
            JsonCryptogramStep {
                service: ServiceName::Catalog,
                method: MethodName::Lookup,
                payload: json!({ "ids": [] }),
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
        Ok(value) => assert_eq!(
            value,
            json!({ "ids": ["12313bb7-6068-4ec9-ac49-3e834181f127"] })
        ),
        other => {
            let _ = other.unwrap();
        }
    }
}

pub fn configure(server: &mut web::ServiceConfig) {
    server.route("/evaluate", web::post().to(evaluate));
}
