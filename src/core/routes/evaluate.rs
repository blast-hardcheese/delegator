use async_trait::async_trait;

use actix_web::{
    body::BoxBody,
    error::{self, PayloadError},
    http::{Method, Uri},
    web::{self, Data, Json},
    HttpResponse, ResponseError,
};
use awc::error::{JsonPayloadError, SendRequestError};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{fmt, str::Utf8Error, sync::Arc, time::Duration};
use tokio::sync::Mutex;

use crate::{
    cache::{hash_value, MemoizationCache},
    config::{HttpClientConfig, ServiceDefinition, Services},
    translate::{self, make_state, Language, StepError, TranslateContext},
};

use super::errors::{json_error_response, JsonResponseError};

#[derive(Clone, Debug, Deserialize)]
pub struct JsonCryptogramStep {
    pub service: String,
    pub method: String,
    pub payload: Value,
    pub preflight: Option<Language>,
    pub postflight: Option<Language>,
    pub memoization_prefix: Option<String>,
    pub headers: Vec<(String, String)>,
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
                service: self.service,
                method: self.method,
                payload,
                preflight: None,
                postflight: None,
                memoization_prefix: None,
                headers: vec![],
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
        let mut headers = self.inner.headers;
        headers.push((key, value));
        JsonCryptogramStepBuilder {
            inner: JsonCryptogramStep {
                headers,
                ..self.inner
            },
        }
    }

    pub fn headers(self, pairs: Vec<(String, String)>) -> JsonCryptogramStepBuilder {
        let mut headers = self.inner.headers;
        for pair in pairs {
            headers.push(pair);
        }
        JsonCryptogramStepBuilder {
            inner: JsonCryptogramStep {
                headers,
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
    UnknownMethod(String, String),
    UnknownService(String),
    UriBuilderError(error::HttpError),
    Utf8Error(Utf8Error),
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
        fn err_value(msg: &Value) -> Value {
            json!({
               "error": {
                   "kind": msg,
               }
            })
        }
        match self {
            Self::ClientError(inner) =>
              err_value(&json!({"err": "client", "value": inner.to_string() })),
            Self::InvalidJsonError(inner) =>
              err_value(&json!({ "err": "protocol", "value": inner.to_string() })),
            Self::InvalidPayloadError(inner) =>
              err_value(&json!({"err": "payload", "value": inner.to_string()})),
            Self::UnknownStep(num) => err(&format!("unknown_step: {}", num)),
            Self::InvalidStructure(inner) =>
              err_value(&json!({"err": "invalid_structure", "value": inner })),
            Self::InvalidTransition(steps, step) =>
              err_value(&json!({ "err": "unknown_transition", "steps": steps, "step": step })),
            Self::NetworkError(context) => err_value(context),
            Self::NoStepsSpecified => err("no_steps_specified"),
            Self::UnknownMethod(service_name, method_name) =>
              err(&format!("unknown_method: {}::{}", service_name, method_name)),
            Self::UnknownService(service_name) =>
              err(&format!("unknown_service: {}", service_name)),
            Self::UriBuilderError(_inner) => err("unknown_service"),
            Self::Utf8Error(_inner) => err("encoding"),
        }
    }
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
        let payload = &current_step.payload;
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
        } else {
            let service = services
                .get(service_name)
                .ok_or_else(|| EvaluateError::UnknownService(service_name.to_owned()))?
                .to_owned();
            let new_payload = match service {
                ServiceDefinition::Rest {
                    scheme,
                    authority,
                    methods,
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
                            headers.clone(),
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
        };

        let next_idx = step + 1;
        if next_idx >= cryptogram.steps.len() {
            return Ok((new_payload, cryptogram));
        }

        cryptogram.steps[next_idx].payload = new_payload.clone();

        final_result = Some(new_payload);

        step += 1;
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

    let mut services = {
        let s = DefaultHashBuilder::default();
        HashMap::with_hasher(s)
    };

    services.insert(
        "catalog",
        ServiceDefinition::Rest {
            scheme: Scheme::HTTP,
            authority: Authority::from_static("0:0"),
            methods: {
                let mut methods = {
                    let s = DefaultHashBuilder::default();
                    HashMap::with_hasher(s)
                };
                methods.insert(
                    "search",
                    MethodDefinition {
                        method: Method::POST,
                        path: PathAndQuery::from_static("/search/"),
                    },
                );
                methods.insert(
                    "lookup",
                    MethodDefinition {
                        method: Method::POST,
                        path: PathAndQuery::from_static("/product_variants/"),
                    },
                );
                methods
            },
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

pub fn configure(server: &mut web::ServiceConfig) {
    server.route("/evaluate", web::post().to(evaluate));
}
