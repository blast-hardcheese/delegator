use derive_more::Display;
use sentry::Breadcrumb;
use std::num::ParseIntError;

use actix_web::{
    body::BoxBody,
    error, guard,
    web::{self, Data, Json},
    HttpResponse,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    config::{events::EventConfig, HttpClientConfig, MethodName, ServiceName, Services},
    headers::authorization::Authorization,
    headers::{authorization::BearerFields, features::Features},
    translate::{make_state, Language, TranslateContext},
};

use super::{
    errors::{json_error_response, JsonResponseError},
    evaluate::{do_evaluate, JsonCryptogram, JsonCryptogramStep, LiveJsonClient},
};

#[derive(Debug, Deserialize)]
pub struct ExploreRequest {
    q: Option<String>,
    size: Option<i32>,
    start: Option<String>,
}

#[derive(Debug, Display)]
enum ExploreError {
    Evaluate(super::evaluate::EvaluateError),
    InvalidPage(ParseIntError),
}

impl JsonResponseError for ExploreError {
    fn error_as_json(&self) -> Value {
        fn breadcrumb(msg: &str) -> Breadcrumb {
            Breadcrumb {
                message: Some(String::from(msg)),
                ty: String::from("input"),
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
            Self::InvalidPage(inner) => {
                sentry::add_breadcrumb(breadcrumb("InvalidPage"));
                sentry::capture_error(inner);
                err("invalid_page")
            }
            Self::Evaluate(_inner) => {
                json!(null) // NB: JsonResponseError is as good as I'm able to write it at this
                            // point, but this is an unfortunate edge case. error_response calls
                            // the underlying json_error_response(inner) in this case, but we
                            // still need to define it here.
            }
        }
    }
}

impl error::ResponseError for ExploreError {
    fn error_response(&self) -> HttpResponse<BoxBody> {
        match self {
            Self::InvalidPage(_inner) => json_error_response(self),
            Self::Evaluate(inner) => {
                sentry::add_breadcrumb(Breadcrumb {
                    message: Some(String::from("Error during catalog")),
                    ty: String::from("evaluate_step"),
                    category: Some(String::from("error")),
                    ..Breadcrumb::default()
                });
                json_error_response(inner)
            }
        }
    }
}

async fn get_product_variant_image(
    ctx: Data<TranslateContext>,
    client_config: Data<HttpClientConfig>,
    services: Data<Services>,
    pvid: web::Path<(String,)>,
) -> Result<HttpResponse, ExploreError> {
    let cryptogram = JsonCryptogram {
        steps: vec![
            JsonCryptogramStep::build(ServiceName::Catalog, MethodName::Lookup)
                .payload(json!({ "product_variant_ids": [pvid.0] }))
                .postflight(Language::Object(vec![(
                    String::from("results"),
                    Language::At(String::from("product_variants")),
                )]))
                .finish(),
        ],
    };

    let live_client = LiveJsonClient::build(client_config.get_ref());

    let result = do_evaluate(
        ctx.get_ref(),
        cryptogram,
        live_client,
        services.get_ref(),
        make_state(),
    )
    .await
    .map_err(ExploreError::Evaluate)?;

    let results = result
        .get("results")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|i| i.get("primary_image"))
        .and_then(|s| s.as_str());

    match results {
        Some(primary_image) => Ok(HttpResponse::TemporaryRedirect()
            .append_header(("location", primary_image))
            .finish()),
        _ => Ok(HttpResponse::NotFound().finish()),
    }
}

async fn get_product_variants(
    ctx: Data<TranslateContext>,
    client_config: Data<HttpClientConfig>,
    services: Data<Services>,
    raw_req: web::Query<Vec<(String, String)>>,
) -> Result<HttpResponse, ExploreError> {
    // There seems to be no equivalent to Flask's MultiDict in actix-web:
    //   https://stackoverflow.com/questions/63844460/how-can-i-receive-multiple-query-params-with-the-same-name-in-actix-web
    // Maybe something that can be contributed back to https://github.com/actix/actix-extras
    // For the time being, Query<Vec<(String, String)>> seems to be a workaround.
    let ids = {
        let mut ids: Vec<String> = vec![];

        for (k, v) in raw_req.0 {
            if k == "id" {
                ids.push(v);
            }
        }

        ids
    };

    let cryptogram = JsonCryptogram {
        steps: vec![
            JsonCryptogramStep::build(ServiceName::Catalog, MethodName::Lookup)
                .payload(json!({ "product_variant_ids": ids }))
                .postflight(Language::Object(vec![(
                    String::from("results"),
                    Language::At(String::from("product_variants")),
                )]))
                .finish(),
        ],
    };

    let live_client = LiveJsonClient::build(client_config.get_ref());

    let result = do_evaluate(
        ctx.get_ref(),
        cryptogram,
        live_client,
        services.get_ref(),
        make_state(),
    )
    .await
    .map_err(ExploreError::Evaluate)?;
    Ok(HttpResponse::Ok().json(&result))
}

async fn get_explore(
    ctx: Data<TranslateContext>,
    client_config: Data<HttpClientConfig>,
    services: Data<Services>,
    events: Data<EventConfig>,
    req: web::Query<ExploreRequest>,
    features: Option<Features>,
    authorization: Option<Authorization>,
) -> Result<HttpResponse, ExploreError> {
    let features: Features = features.unwrap_or(Features::empty());
    let authorization: Authorization = authorization.unwrap_or(Authorization::empty());

    let start = req.start.clone().unwrap_or(String::from("1"));
    let size = req.size.unwrap_or(10);
    let (start, bucket_info) = match Vec::from_iter(start.splitn(3, ':')).as_slice() {
        [legacy_start] => {
            let _start = legacy_start
                .to_owned()
                .parse::<i32>()
                .map_err(ExploreError::InvalidPage)?;
            (_start - 1, None)
        }
        ["catalog", start] => {
            let _start = start.parse::<i32>().map_err(ExploreError::InvalidPage)?;
            (_start, None)
        }
        ["catalog", start, bucket_info] => {
            let _start = start.parse::<i32>().map_err(ExploreError::InvalidPage)?;
            (_start, Some(bucket_info.to_owned()))
        }
        [..] => (0, None),
    };

    let owner_id = if let Authorization::Bearer(BearerFields { owner_id }) = authorization {
        Some(owner_id)
    } else {
        None
    };

    let (source, next_start) = if start == 0 && owner_id.is_some() && features.recommendations {
        let source = JsonCryptogramStep::build(ServiceName::Recommendations, MethodName::Lookup)
            .payload(json!({ "size": size, "owner_id": owner_id.unwrap() }))
            .postflight(Language::Object(vec![(
                String::from("ids"),
                Language::At(String::from("results")),
            )]))
            .finish();
        let next_start = format!("catalog:{}", size);
        (
            source,
            vec![
                (
                    String::from("next_start"),
                    Language::Const(json!(next_start)),
                ),
                (String::from("has_more"), Language::Const(json!(true))),
            ],
        )
    } else {
        let new_start = if owner_id.is_some() && features.recommendations {
            start - 1 // Offset how many recs we want if we are running recommendations
        } else {
            start
        };
        let source = JsonCryptogramStep::build(ServiceName::Catalog, MethodName::Explore)
            .payload(
                json!({ "q": req.q, "start": new_start, "bucket_info": bucket_info, "size": size }),
            )
            .preflight(Language::EmitEvent(events.user_action.clone(), json!(null)))
            .postflight(Language::Splat(vec![
                Language::Focus(
                    String::from("next_start"),
                    Box::new(Language::Set(String::from("next_start"))),
                ),
                Language::Focus(
                    String::from("has_more"),
                    Box::new(Language::Set(String::from("has_more"))),
                ),
                Language::Object(vec![(
                    String::from("product_variant_ids"),
                    Language::At(String::from("product_variant_ids")),
                )]),
            ]))
            .finish();
        (
            source,
            vec![
                (
                    String::from("next_start"),
                    Language::Get(String::from("next_start")),
                ),
                (
                    String::from("has_more"),
                    Language::Get(String::from("has_more")),
                ),
            ],
        )
    };

    let cryptogram = JsonCryptogram {
        steps: vec![
            source,
            JsonCryptogramStep::build(ServiceName::Catalog, MethodName::Lookup)
                .payload(json!({ "product_variant_ids": [] }))
                .postflight(Language::Object(
                    vec![
                        vec![
                            (
                                String::from("results"),
                                Language::At(String::from("product_variants")),
                            ),
                            (
                                String::from("data"),
                                Language::Focus(
                                    String::from("product_variants"),
                                    Box::new(Language::Array(Box::new(Language::Object(vec![
                                        (
                                            String::from("brand_name"),
                                            Language::At(String::from("brand_variant_name")),
                                        ),
                                        (
                                            String::from("catalog_id"),
                                            Language::At(String::from("id")),
                                        ),
                                        (String::from("id"), Language::At(String::from("id"))),
                                        (String::from("item_id"), Language::At(String::from("id"))),
                                        (
                                            String::from("link"),
                                            Language::At(String::from("primary_image")),
                                        ),
                                        (String::from("title"), Language::At(String::from("name"))),
                                    ])))),
                                ),
                            ), // TODO: Delete this ASAP
                            (String::from("query_id"), Language::Const(json!(null))), // TODO: Delete this ASAP
                            (String::from("status"), Language::Const(json!("ok"))), // TODO: Delete this ASAP
                        ],
                        next_start,
                    ]
                    .concat(),
                ))
                .finish(),
        ],
    };

    let live_client = LiveJsonClient::build(client_config.get_ref());

    let result = do_evaluate(
        ctx.get_ref(),
        cryptogram,
        live_client,
        services.get_ref(),
        make_state(),
    )
    .await
    .map_err(ExploreError::Evaluate)?;
    Ok(HttpResponse::Ok().json(&result))
}

#[derive(Debug, Deserialize)]
struct SuggestionsRequest {
    q: String,
}

async fn post_suggestions(
    ctx: Data<TranslateContext>,
    client_config: Data<HttpClientConfig>,
    services: Data<Services>,
    req: Json<SuggestionsRequest>,
) -> Result<HttpResponse, ExploreError> {
    let cryptogram = JsonCryptogram {
        steps: vec![
            JsonCryptogramStep::build(ServiceName::Catalog, MethodName::Autocomplete)
                .payload(json!({ "q": req.q }))
                .finish(),
        ],
    };

    let live_client = LiveJsonClient::build(client_config.get_ref());

    let result = do_evaluate(
        ctx.get_ref(),
        cryptogram,
        live_client,
        services.get_ref(),
        make_state(),
    )
    .await
    .map_err(ExploreError::Evaluate)?;
    Ok(HttpResponse::Ok().json(&result))
}

pub fn configure(server: &mut web::ServiceConfig, hostname: String) {
    let host_route = || web::route().guard(guard::Host(hostname.clone()));
    server
        .route("/explore", host_route().guard(guard::Get()).to(get_explore))
        .route(
            "/explore/suggestions",
            host_route().guard(guard::Post()).to(post_suggestions),
        )
        .route(
            "/product_variants",
            host_route().guard(guard::Get()).to(get_product_variants),
        )
        .route(
            "/product_variants/{pvid}.jpg",
            host_route()
                .guard(guard::Get())
                .to(get_product_variant_image),
        );
}
