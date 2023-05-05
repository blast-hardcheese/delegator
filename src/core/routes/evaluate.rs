use async_trait::async_trait;

use actix_web::{
    body::BoxBody,
    error,
    http::{Method, Uri},
    web::{self, Data, Json},
    HttpResponse, ResponseError,
};
use awc::error::{JsonPayloadError, SendRequestError};
use hashbrown::HashMap;
use sentry::types::protocol::v7::Map as SentryMap;
use sentry::Breadcrumb;
use serde::Deserialize;
use serde_json::{json, Value};
use std::fmt;

use crate::{
    config::{HttpClientConfig, MethodName, ServiceDefinition, ServiceName, Services},
    translate::{self, make_state, Language, StepError},
};

use super::errors::{json_error_response, JsonResponseError};

#[derive(Debug, Deserialize)]
pub struct JsonCryptogramStep {
    pub service: ServiceName,
    pub method: MethodName,
    pub payload: Value,
    pub postflight: Option<Language>,
}

impl fmt::Display for EvaluateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Deserialize)]
pub struct JsonCryptogram {
    pub steps: Vec<JsonCryptogramStep>,
}

#[derive(Debug)]
pub enum EvaluateError {
    ClientError(SendRequestError),
    InvalidJsonError(JsonPayloadError),
    InvalidStep(usize),
    InvalidStructure(StepError),
    InvalidTransition,
    NetworkError,
    NoStepsSpecified,
    UnknownMethod(MethodName),
    UnknownService(ServiceName),
    UriBuilderError(error::HttpError),
}
impl JsonResponseError for EvaluateError {
    fn error_as_json(&self) -> Value {
        fn err(msg: &str) -> Value {
            json!({
               "error": {
                   "kind": String::from(msg),
               }
            })
        }
        match self {
            Self::ClientError(_inner) => err("client"),
            Self::InvalidJsonError(_inner) => err("protocol"),
            Self::InvalidStep(_num) => err("unknown_step"),
            Self::InvalidStructure(_inner) => err("payload"),
            Self::InvalidTransition => err("unknown_transition"),
            Self::NetworkError => err("network"),
            Self::NoStepsSpecified => err("steps"),
            Self::UnknownMethod(_method_name) => err("unknown_method"),
            Self::UnknownService(_service_name) => err("unknown_service"),
            Self::UriBuilderError(_inner) => err("unknown_service"),
        }
    }
}

impl ResponseError for EvaluateError {
    fn error_response(&self) -> HttpResponse<BoxBody> {
        json_error_response(self)
    }
}

async fn evaluate(
    cryptogram: Json<JsonCryptogram>,
    client_config: Data<HttpClientConfig>,
    services: Data<Services>,
) -> Result<HttpResponse, EvaluateError> {
    let live_client = LiveJsonClient::build(client_config.get_ref());

    let result = do_evaluate(
        cryptogram.into_inner(),
        live_client,
        services.get_ref(),
        make_state(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(&result))
}

#[async_trait(?Send)]
pub trait JsonClient {
    async fn issue_request(
        &self,
        method: Method,
        uri: Uri,
        value: &Value,
    ) -> Result<Value, EvaluateError>;
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
        method: Method,
        uri: Uri,
        payload: &Value,
    ) -> Result<Value, EvaluateError> {
        let mut result = self
            .client
            .request(method, uri)
            .insert_header(("User-Agent", self.client_config.user_agent.clone()))
            .insert_header(("Content-Type", "application/json"))
            .send_json(payload)
            .await
            .map_err(EvaluateError::ClientError)?;
        if !result.status().is_success() {
            return Err(EvaluateError::NetworkError);
        }
        result
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

pub async fn do_evaluate<JC: JsonClient>(
    cryptogram: JsonCryptogram,
    json_client: JC,
    services: &Services,
    translator_state: translate::State,
) -> Result<Value, EvaluateError> {
    let parent_span = sentry::configure_scope(|scope| scope.get_span());

    let span: sentry::TransactionOrSpan = match &parent_span {
        Some(parent) => parent.start_child("evaluate", "do_evaluate").into(),
        None => {
            let ctx = sentry::TransactionContext::new("evaluate", "do_evaluate");
            sentry::start_transaction(ctx).into()
        }
    };

    // Set the currently running span
    sentry::configure_scope(|scope| scope.set_span(Some(span)));

    let mut final_result: Option<Value> = None;

    let mut state: HashMap<usize, JsonCryptogramStep> =
        cryptogram.steps.into_iter().enumerate().collect();
    let mut step: usize = 0;
    while step < state.len() {
        let current_step = state.get(&step).ok_or(EvaluateError::InvalidStep(step))?;
        let service_name = &current_step.service;
        let method_name = &current_step.method;
        let payload = &current_step.payload;
        let postflight = &current_step.postflight;

        let service = services
            .get(service_name)
            .ok_or_else(|| EvaluateError::UnknownService(service_name.to_owned()))?
            .to_owned();
        final_result = match service {
            ServiceDefinition::Rest {
                scheme,
                authority,
                methods,
            } => {
                let method = methods
                    .get(method_name)
                    .ok_or_else(|| EvaluateError::UnknownMethod(method_name.to_owned()))?;

                let uri = Uri::builder()
                    .scheme(scheme)
                    .authority(authority)
                    .path_and_query(method.path.to_owned())
                    .build()
                    .map_err(EvaluateError::UriBuilderError)?;

                sentry::add_breadcrumb(Breadcrumb {
                    ty: String::from("evaluate_step"),
                    data: SentryMap::from([
                        (String::from("service"), service_name.to_string().into()),
                        (String::from("method"), method_name.to_string().into()),
                    ]),
                    ..Breadcrumb::default()
                });

                let result = json_client
                    .issue_request(method.method.clone(), uri, payload)
                    .await?;

                let new_payload = if let Some(pf) = postflight {
                    translate::step(pf, &result, translator_state.clone())
                        .map_err(EvaluateError::InvalidStructure)?
                } else {
                    result
                };

                let next_idx = step + 1;
                if !state.contains_key(&next_idx) {
                    return Ok(new_payload);
                }

                let mut next = state
                    .remove(&next_idx)
                    .ok_or_else(|| EvaluateError::InvalidTransition)?;
                next.payload = new_payload.clone();
                state.insert(next_idx, next);

                Some(new_payload)
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
                postflight: Some(Language::Focus(
                    String::from("results"),
                    Box::new(Language::Object(vec![
                        (
                            String::from("ids"),
                            Language::Array(Box::new(Language::At(String::from(
                                "product_variant_id",
                            )))),
                        ),
                        (
                            String::from("results"),
                            Language::Const(
                                json!({ "product_variants": [{ "id": "12313bb7-6068-4ec9-ac49-3e834181f127" }]}),
                            ),
                        ),
                    ])),
                )),
            },
            JsonCryptogramStep {
                service: ServiceName::Catalog,
                method: MethodName::Lookup,
                payload: json!(null),
                postflight: Some(Language::Object(vec![(
                    String::from("results"),
                    Language::At(String::from("results")),
                )])),
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
            authority: Authority::from_static("0:0"),
            methods: {
                let mut methods = {
                    let s = DefaultHashBuilder::default();
                    HashMap::with_hasher(s)
                };
                methods.insert(
                    MethodName::Search,
                    MethodDefinition {
                        method: Method::POST,
                        path: PathAndQuery::from_static("/search/"),
                    },
                );
                methods.insert(
                    MethodName::Lookup,
                    MethodDefinition {
                        method: Method::POST,
                        path: PathAndQuery::from_static("/product_variants/"),
                    },
                );
                methods
            },
        },
    );

    match do_evaluate(cryptogram, TestJsonClient, &services, make_state()).await {
        Ok(value) => assert_eq!(
            value,
            json!({ "results": { "product_variants": [{ "id": "12313bb7-6068-4ec9-ac49-3e834181f127" }]} })
        ),
        other => {
            let _ = other.unwrap();
        }
    }
}

pub fn configure(server: &mut web::ServiceConfig) {
    server.route("/evaluate", web::post().to(evaluate));
}
