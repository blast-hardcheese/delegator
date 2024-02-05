use async_trait::async_trait;

use actix_web::{
    body::BoxBody,
    error::{self, PayloadError},
    guard,
    http::{Method, Uri},
    web::{self, Data, Json},
    HttpResponse, ResponseError,
};
use awc::error::{JsonPayloadError, SendRequestError};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    fmt,
    str::{FromStr, Utf8Error},
    sync::Arc,
    time::Duration,
};
use tokio::sync::Mutex;

use crate::{
    cache::{hash_value, MemoizationCache},
    config::{EdgeRoute, HttpClientConfig, ServiceDefinition, Services, Virtualhosts},
    translate::{self, make_state, Language, StepError, TranslateContext},
};

use super::errors::{json_error_response, JsonResponseError};

#[derive(Clone, Debug, Deserialize)]
pub struct JsonCryptogramStep {
    pub service: Option<String>,
    pub method: Option<String>,
    pub payload: Option<Value>,
    pub preflight: Option<Language>,
    pub postflight: Option<Language>,
    pub memoization_prefix: Option<String>,
    pub headers: Option<Vec<(String, String)>>,
}

impl JsonCryptogramStep {
    pub fn build(service: &str, method: &str) -> JsonCryptogramStepNeedsPayload {
        JsonCryptogramStepNeedsPayload {
            service: service.to_string(),
            method: method.to_string(),
        }
    }
}

pub struct JsonCryptogramStepNeedsPayload {
    service: String,
    method: String,
}

impl JsonCryptogramStepNeedsPayload {
    pub fn payload(self, payload: Value) -> JsonCryptogramStepBuilder {
        JsonCryptogramStepBuilder {
            inner: JsonCryptogramStep {
                service: Some(self.service),
                method: Some(self.method),
                payload: Some(payload),
                preflight: None,
                postflight: None,
                memoization_prefix: None,
                headers: None,
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

    pub fn memoization_prefix(self, prefix: String) -> JsonCryptogramStepBuilder {
        JsonCryptogramStepBuilder {
            inner: JsonCryptogramStep {
                memoization_prefix: Some(prefix),
                ..self.inner
            },
        }
    }

    pub fn header(self, key: String, value: String) -> JsonCryptogramStepBuilder {
        let mut headers = self.inner.headers.unwrap_or_default();
        headers.push((key, value));
        JsonCryptogramStepBuilder {
            inner: JsonCryptogramStep {
                headers: Some(headers),
                ..self.inner
            },
        }
    }

    pub fn headers(self, pairs: Vec<(String, String)>) -> JsonCryptogramStepBuilder {
        let mut headers = self.inner.headers.unwrap_or_default();
        for pair in pairs {
            headers.push(pair);
        }
        JsonCryptogramStepBuilder {
            inner: JsonCryptogramStep {
                headers: Some(headers),
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
            EvaluateError::UnknownStep(num) => json!({"err": "unknown_step", "num": num}),
            EvaluateError::InvalidStructure(inner) => {
                json!({"err": "invalid_structure", "value": inner})
            }
            EvaluateError::InvalidTransition(steps, step) => {
                json!({"err": "unknown_transition", "steps": steps, "step": step})
            }
            EvaluateError::NetworkError(context) => context.clone(),
            EvaluateError::NoStepsSpecified => json!({"err": "no_steps_specified"}),
            EvaluateError::UnknownMethod(service_name, method_name) => {
                json!({"err": "unknown_method", "service_name": service_name, "method_name": method_name})
            }
            EvaluateError::UnknownService(service_name) => {
                json!({"err": "unknown_service", "service_name": service_name})
            }
            EvaluateError::UriBuilderError(_inner) => json!({"err": "uri_builder_error"}),
            EvaluateError::Utf8Error(_inner) => json!({"err": "utf8_error"}),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct JsonCryptogram {
    pub steps: Vec<JsonCryptogramStep>,
}

impl FromStr for JsonCryptogram {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
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
    UnknownMethod(String, String),
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
    ctx: Data<TranslateContext>,
    cryptogram: Json<JsonCryptogram>,
    client_config: Data<HttpClientConfig>,
    cache_state: Data<Mutex<MemoizationCache>>,
    services: Data<Services>,
) -> Result<HttpResponse, EvaluateError> {
    let live_client = LiveJsonClient::build(client_config.get_ref());

    let (result, _) = do_evaluate(
        ctx.get_ref(),
        cache_state.into_inner(),
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
        headers: Vec<(String, String)>,
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
        headers: Vec<(String, String)>,
    ) -> Result<Value, EvaluateError> {
        let mut req = self
            .client
            .request(method, uri)
            .insert_header(("User-Agent", self.client_config.user_agent.clone()))
            .insert_header(("Content-Type", "application/json"));
        for pair in headers.iter() {
            req = req.insert_header(pair.clone());
        }
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
        _headers: Vec<(String, String)>,
    ) -> Result<Value, EvaluateError> {
        Ok(payload.clone())
    }
}

pub async fn do_evaluate<JC: JsonClient>(
    ctx: &TranslateContext,
    memoization_cache: Arc<Mutex<MemoizationCache>>,
    mut cryptogram: JsonCryptogram,
    json_client: JC,
    services: &Services,
    translator_state: translate::State,
) -> Result<(Value, JsonCryptogram), EvaluateError> {
    let mut final_result: Option<Value> = None;

    let mut step: usize = 0;
    while step < cryptogram.steps.len() {
        let current_step = &cryptogram.steps[step];
        let service_name = &current_step.service;
        let method_name = &current_step.method;
        let payload = &current_step.payload.clone().unwrap_or(Value::Null);
        let preflight = &current_step.preflight;
        let postflight = &current_step.postflight;
        let memoization_prefix = &current_step.memoization_prefix;
        let headers = &current_step.headers;

        let outgoing_payload = if let Some(pf) = preflight {
            translate::step(ctx, pf, payload, translator_state.clone())
                .map_err(EvaluateError::InvalidStructure)?
        } else {
            payload.clone()
        };

        let memo_key = memoization_prefix
            .clone()
            .map(|prefix| format!("{}{}", prefix, hash_value(&outgoing_payload)));

        let maybe_cache = if let Some(key) = memo_key.as_ref() {
            memoization_cache.lock().await.get(key).cloned()
        } else {
            None
        };
        let new_payload = if let Some(cached_value) = maybe_cache {
            cached_value
        } else if let (Some(service_name), Some(method_name)) = (service_name, method_name) {
            let service = services
                .get(service_name)
                .ok_or_else(|| EvaluateError::UnknownService(service_name.to_owned()))?
                .to_owned();
            let new_payload = match service {
                ServiceDefinition::Rest {
                    scheme,
                    authority,
                    methods,
                    ..
                } => {
                    let method = methods.get(method_name).ok_or_else(|| {
                        EvaluateError::UnknownMethod(
                            service_name.to_owned(),
                            method_name.to_owned(),
                        )
                    })?;

                    let uri = Uri::builder()
                        .scheme(scheme)
                        .authority(authority)
                        .path_and_query(method.path.to_owned())
                        .build()
                        .map_err(EvaluateError::UriBuilderError)?;

                    let result = json_client
                        .issue_request(
                            method.method.clone(),
                            uri,
                            &outgoing_payload,
                            headers.clone().unwrap_or_default(),
                        )
                        .await?;

                    if let Some(pf) = postflight {
                        translate::step(ctx, pf, &result, translator_state.clone())
                            .map_err(EvaluateError::InvalidStructure)?
                    } else {
                        result
                    }
                }
            };
            if let Some(key) = memo_key {
                memoization_cache
                    .lock()
                    .await
                    .insert(key, new_payload, Duration::from_secs(600))
            } else {
                new_payload
            }
        } else if let Some(pf) = postflight {
            translate::step(ctx, pf, &outgoing_payload, translator_state.clone())
                .map_err(EvaluateError::InvalidStructure)?
        } else {
            outgoing_payload
        };

        let next_idx = step + 1;
        if next_idx < cryptogram.steps.len() {
            if cryptogram.steps[next_idx].payload.is_some() {
                println!(
                    "Warning: Discarding payload for step {}: {:?}",
                    next_idx, cryptogram.steps[next_idx].payload
                );
            }
            cryptogram.steps[next_idx].payload = Some(new_payload);
        } else {
            final_result = Some(new_payload);
        }

        step = next_idx;
    }

    final_result
        .map(|v| (v, cryptogram))
        .ok_or(EvaluateError::NoStepsSpecified)
}

#[actix_web::test]
async fn routes_evaluate() {
    use crate::config::MethodDefinition;
    use actix_web::http::uri::{Authority, PathAndQuery, Scheme};
    use hashbrown::hash_map::DefaultHashBuilder;
    use hashbrown::HashMap;
    use serde_json::json;

    let cryptogram = JsonCryptogram {
        steps: vec![
            JsonCryptogramStep::build("catalog", "search")
                .payload(json!({ "q": "Foo", "results": [{"product_variant_id": "12313bb7-6068-4ec9-ac49-3e834181f127"}] }))
                .postflight(Language::at("results").map(Language::Object(vec![
                        (
                            String::from("ids"),
                            Language::array(Language::at(
                                "product_variant_id",
                            )),
                        ),
                        (
                            String::from("results"),
                            Language::Const(
                                json!({ "product_variants": [{ "id": "12313bb7-6068-4ec9-ac49-3e834181f127" }]}),
                            ),
                        ),
                    ])),
                )
                .finish()
            ,
            JsonCryptogramStep::build("catalog", "lookup")
                .payload(json!(null))
                .postflight(Language::Object(vec![(
                    String::from("results"),
                    Language::at("results"),
                )]))
                .finish(),
        ],
    };

    let mut services: Services = {
        let s = DefaultHashBuilder::default();
        HashMap::with_hasher(s)
    };

    services.insert(
        "catalog".to_string(),
        ServiceDefinition::Rest {
            scheme: Scheme::HTTP,
            authority: Authority::from_static("0:0"),
            methods: {
                let mut methods = {
                    let s = DefaultHashBuilder::default();
                    HashMap::with_hasher(s)
                };
                methods.insert(
                    "search".to_string(),
                    MethodDefinition {
                        method: Method::POST,
                        path: PathAndQuery::from_static("/search/"),
                    },
                );
                methods.insert(
                    "lookup".to_string(),
                    MethodDefinition {
                        method: Method::POST,
                        path: PathAndQuery::from_static("/product_variants/"),
                    },
                );
                methods
            },
            virtualhosts: None,
        },
    );

    let ctx = TranslateContext::noop();
    let memoization_cache = Arc::new(MemoizationCache::new());
    match do_evaluate(
        &ctx,
        memoization_cache,
        cryptogram,
        TestJsonClient,
        &services,
        make_state(),
    )
    .await
    {
        Ok((value, _)) => assert_eq!(
            value,
            json!({ "results": { "product_variants": [{ "id": "12313bb7-6068-4ec9-ac49-3e834181f127" }]} })
        ),
        other => {
            let _ = other.unwrap();
        }
    }
}

async fn bound_function(
    ctx: Data<TranslateContext>,
    input: Json<Value>,
    client_config: Data<HttpClientConfig>,
    cache_state: Data<Mutex<MemoizationCache>>,
    services: Data<Services>,
    edge_route: EdgeRoute,
) -> Result<HttpResponse, EvaluateError> {
    let live_client = LiveJsonClient::build(client_config.get_ref());
    let translator_state = make_state();

    let input = input.into_inner();
    let mut cryptogram = edge_route.cryptogram;
    if !cryptogram.steps.is_empty() && cryptogram.steps[0].preflight.is_some() {
        let input = translate::step(
            ctx.get_ref(),
            &cryptogram.steps[0].preflight.clone().unwrap(),
            &input,
            translator_state.clone(),
        )
        .map_err(EvaluateError::InvalidStructure)?;
        cryptogram.steps[0].payload = Some(input);
        cryptogram.steps[0].preflight = None;
    };
    let (result, _) = do_evaluate(
        ctx.get_ref(),
        cache_state.into_inner(),
        cryptogram,
        live_client,
        services.get_ref(),
        translator_state,
    )
    .await?;
    Ok(HttpResponse::Ok().json(&result))
}

pub fn configure(server: &mut web::ServiceConfig, virtualhosts: &Virtualhosts) {
    let mut server = server;

    for (_name, vhost) in virtualhosts {
        let host_route = || web::route().guard(guard::Host(vhost.hostname.clone()));
        for (route, edge_route) in &vhost.routes {
            let edge_route = edge_route.clone();
            server = server.route(
                route,
                host_route().guard(guard::Post()).to(
                    move |ctx: Data<TranslateContext>,
                          input: Json<Value>,
                          client_config: Data<HttpClientConfig>,
                          cache_state: Data<Mutex<MemoizationCache>>,
                          services: Data<Services>| {
                        bound_function(
                            ctx,
                            input,
                            client_config,
                            cache_state,
                            services,
                            edge_route.clone(),
                        )
                    },
                ),
            );
        }
    }

    server.route("/evaluate", web::post().to(evaluate));
}
