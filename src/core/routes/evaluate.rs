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
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde_json::Value;

use crate::{
    config::{HttpClientConfig, MethodName, ServiceDefinition, ServiceName, Services},
    translate::{self, parse::parse_language},
    translate::{make_state, Language, StepError},
};

#[derive(Debug, Deserialize)]
pub struct JsonCryptogramStep {
    pub service: ServiceName,
    pub method: MethodName,
    pub payload: Value,
}

#[derive(Debug, Deserialize)]
pub struct JsonCryptogram {
    pub steps: Vec<JsonCryptogramStep>,
}

#[derive(Debug, Display)]
pub enum EvaluateError {
    ClientError(SendRequestError),
    InvalidJsonError(JsonPayloadError),
    InvalidStep(usize),
    InvalidStructure(StepError),
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

    let result = do_evaluate(
        cryptogram.into_inner(),
        live_client,
        services.get_ref(),
        make_state(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(&result))
}

type MethodSpec = (ServiceName, MethodName);

static TRANSITIONS: Lazy<HashMap<(MethodSpec, MethodSpec), Language>> = Lazy::new(|| {
    HashMap::from_iter(vec![
        (
            (
                (ServiceName::Catalog, MethodName::Search),
                (ServiceName::Catalog, MethodName::Lookup),
            ),
            parse_language(r#".results | { "ids": map(.product_variant_id) }"#)
                .unwrap()
                .1,
        ),
        (
            (
                (ServiceName::Catalog, MethodName::Explore),
                (ServiceName::Catalog, MethodName::Lookup),
            ),
            parse_language(r#".next_start | set("next_start"), { "ids": .product_variant_ids }"#)
                .unwrap()
                .1,
        ),
    ])
});

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

pub async fn do_evaluate<JC: JsonClient>(
    cryptogram: JsonCryptogram,
    json_client: JC,
    services: &Services,
    translator_state: translate::State,
) -> Result<Value, EvaluateError> {
    let mut final_result: Option<Value> = None;

    let mut state: State = cryptogram.steps.into_iter().enumerate().collect();
    let mut step: usize = 0;
    while step < state.len() {
        let current_step = state.get(&step).ok_or(EvaluateError::InvalidStep(step))?;
        let service_name = &current_step.service;
        let method_name = &current_step.method;
        let payload = &current_step.payload;

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

                let result = json_client
                    .issue_request(Method::POST, uri, payload)
                    .await?;

                let last = &state[&step];
                let next_idx = step + 1;
                if !state.contains_key(&next_idx) {
                    return Ok(result);
                }
                let next = &state[&next_idx];

                let prog = TRANSITIONS
                    .get(&(
                        (last.service.clone(), last.method.clone()),
                        (next.service.clone(), next.method.clone()),
                    ))
                    .ok_or(EvaluateError::InvalidTransition)?;

                let new_payload = translate::step(prog, &result, translator_state.clone())
                    .map_err(EvaluateError::InvalidStructure)?;

                let mut next = state.remove(&next_idx).ok_or_else(|| EvaluateError::InvalidTransition)?;
                next.payload = new_payload.clone();
                state.insert(
                    next_idx,
                    next,
                );

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
            authority: Authority::from_static("0:0"),
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

    match do_evaluate(cryptogram, TestJsonClient, &services, make_state()).await {
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
