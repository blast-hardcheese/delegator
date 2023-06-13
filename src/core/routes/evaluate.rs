use async_trait::async_trait;

use actix_web::{
    body::BoxBody,
    error::{self, PayloadError},
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
use std::{fmt, str::Utf8Error};

use crate::{
    config::{HttpClientConfig, MethodName, ServiceDefinition, ServiceName, Services},
    translate::{self, make_state, Language, StepError, TranslateContext},
};

use super::errors::{json_error_response, JsonResponseError};

#[derive(Debug, Deserialize)]
pub struct JsonCryptogramStep {
    pub service: ServiceName,
    pub method: MethodName,
    pub payload: Value,
    pub preflight: Option<Language>,
    pub postflight: Option<Language>,
}

impl JsonCryptogramStep {
    pub fn build(service: ServiceName, method: MethodName) -> JsonCryptogramStepNeedsPayload {
        JsonCryptogramStepNeedsPayload { service, method }
    }
}

pub struct JsonCryptogramStepNeedsPayload {
    service: ServiceName,
    method: MethodName,
}

impl JsonCryptogramStepNeedsPayload {
    pub fn payload(self, payload: Value) -> JsonCryptogramStepBuilder {
        JsonCryptogramStepBuilder {
            inner: JsonCryptogramStep {
                service: self.service,
                method: self.method,
                payload,
                preflight: None,
                postflight: None,
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

    pub fn finish(self) -> JsonCryptogramStep {
        self.inner
    }
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
    InvalidPayloadError(PayloadError),
    UnknownStep(usize),
    InvalidStructure(StepError),
    InvalidTransition(Vec<usize>, usize),
    NetworkError(Value),
    NoStepsSpecified,
    UnknownMethod(ServiceName, MethodName),
    UnknownService(ServiceName),
    UriBuilderError(error::HttpError),
    Utf8Error(Utf8Error),
}

impl JsonResponseError for EvaluateError {
    fn error_as_json(&self) -> Value {
        fn breadcrumb(msg: &str) -> Breadcrumb {
            Breadcrumb {
                message: Some(String::from(msg)),
                ty: String::from("evaluate_step"),
                category: Some(String::from("error")),
                ..Breadcrumb::default()
            }
        }
        fn err(msg: &str) -> Value {
            json!({
               "error": {
                   "kind": String::from(msg),
               }
            })
        }
        match self {
            Self::ClientError(inner) => {
                sentry::add_breadcrumb(breadcrumb("ClientError"));
                sentry::capture_error(inner);
                err("client")
            }
            Self::InvalidJsonError(inner) => {
                sentry::add_breadcrumb(breadcrumb("InvalidJsonError"));
                sentry::capture_error(inner);
                err("protocol")
            }
            Self::InvalidPayloadError(inner) => {
                sentry::add_breadcrumb(breadcrumb("PayloadError"));
                sentry::capture_error(inner);
                err("payload")
            }
            Self::UnknownStep(num) => {
                sentry::add_breadcrumb({
                    let mut b = breadcrumb("UnknownStep");
                    b.data
                        .insert(String::from("step"), Value::Number((*num).into()));
                    b
                });
                err("unknown_step")
            }
            Self::InvalidStructure(inner) => {
                sentry::add_breadcrumb(breadcrumb("PayloadError"));
                sentry::capture_error(inner);
                err("payload")
            }
            Self::InvalidTransition(steps, step) => {
                sentry::add_breadcrumb({
                    let mut b = breadcrumb("InvalidTransition");
                    b.data.insert(
                        String::from("steps"),
                        Value::Array(steps.iter().map(|i| Value::Number((*i).into())).collect()),
                    );
                    b.data
                        .insert(String::from("step"), Value::Number((*step).into()));
                    b
                });
                err("unknown_transition")
            }
            Self::NetworkError(context) => {
                sentry::add_breadcrumb({
                    let mut b = breadcrumb("NetworkError");
                    if let Value::Object(hm) = context {
                        for (k, v) in hm.iter() {
                            b.data.insert(k.clone(), v.clone());
                        }
                    } else {
                        b.data.insert(String::from("_json"), context.clone());
                    }
                    b
                });
                err("network")
            }
            Self::NoStepsSpecified => {
                sentry::add_breadcrumb(breadcrumb("NoStepsSpecified"));
                err("steps")
            }
            Self::UnknownMethod(service_name, method_name) => {
                sentry::add_breadcrumb({
                    let mut b = breadcrumb("UnknownMethod");
                    b.data.insert(
                        String::from("service"),
                        Value::String(service_name.to_string()),
                    );
                    b.data.insert(
                        String::from("method"),
                        Value::String(method_name.to_string()),
                    );
                    b
                });
                err("unknown_method")
            }
            Self::UnknownService(service_name) => {
                sentry::add_breadcrumb({
                    let mut b = breadcrumb("UnknownService");
                    b.data.insert(
                        String::from("service"),
                        Value::String(service_name.to_string()),
                    );
                    b
                });
                err("unknown_service")
            }
            Self::UriBuilderError(inner) => {
                sentry::add_breadcrumb(breadcrumb("UriBuilderError"));
                sentry::capture_error(inner);
                err("unknown_service")
            }
            Self::Utf8Error(inner) => {
                sentry::add_breadcrumb(breadcrumb("Utf8Error"));
                sentry::capture_error(inner);
                err("encoding")
            }
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
    let ctx = TranslateContext::noop();

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
        let current_step = state.get(&step).ok_or(EvaluateError::UnknownStep(step))?;
        let service_name = &current_step.service;
        let method_name = &current_step.method;
        let payload = &current_step.payload;
        let preflight = &current_step.preflight;
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
                let method = methods.get(method_name).ok_or_else(|| {
                    EvaluateError::UnknownMethod(service_name.to_owned(), method_name.to_owned())
                })?;

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

                let outgoing_payload = if let Some(pf) = preflight {
                    translate::step(&ctx, pf, payload, translator_state.clone())
                        .map_err(EvaluateError::InvalidStructure)?
                } else {
                    payload.clone()
                };

                let result = json_client
                    .issue_request(method.method.clone(), uri, &outgoing_payload)
                    .await?;

                let new_payload = if let Some(pf) = postflight {
                    translate::step(&ctx, pf, &result, translator_state.clone())
                        .map_err(EvaluateError::InvalidStructure)?
                } else {
                    result
                };

                let next_idx = step + 1;
                if !state.contains_key(&next_idx) {
                    return Ok(new_payload);
                }

                let mut next = state.remove(&next_idx).ok_or_else(|| {
                    EvaluateError::InvalidTransition(state.keys().copied().collect(), next_idx)
                })?;
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
            JsonCryptogramStep::build(ServiceName::Catalog, MethodName::Search)
                .payload(json!({ "q": "Foo", "results": [{"product_variant_id": "12313bb7-6068-4ec9-ac49-3e834181f127"}] }))
                .postflight(Language::Focus(
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
                ))
                .finish()
            ,
            JsonCryptogramStep::build(ServiceName::Catalog, MethodName::Lookup)
                .payload(json!(null))
                .postflight(Language::Object(vec![(
                    String::from("results"),
                    Language::At(String::from("results")),
                )]))
                .finish(),
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
