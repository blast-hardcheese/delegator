use derive_more::Display;
use std::num::ParseIntError;

use actix_web::{
    error, guard,
    web::{self, Data},
    HttpResponse,
};
use serde::Deserialize;
use serde_json::json;

use crate::{
    config::{HttpClientConfig, MethodName, ServiceName, Services},
    headers::authorization::Authorization,
    headers::{authorization::BearerFields, features::Features},
    translate::{make_state, Language},
};

use super::evaluate::{do_evaluate, JsonCryptogram, JsonCryptogramStep, LiveJsonClient};

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

impl error::ResponseError for ExploreError {}

async fn get_product_variants(
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
        steps: vec![JsonCryptogramStep {
            service: ServiceName::Catalog,
            method: MethodName::Lookup,
            payload: json!({ "product_variant_ids": ids }),
            postflight: Some(Language::Object(vec![(
                String::from("results"),
                Language::At(String::from("product_variants")),
            )])),
        }],
    };

    let live_client = LiveJsonClient::build(client_config.get_ref());

    let result = do_evaluate(cryptogram, live_client, services.get_ref(), make_state())
        .await
        .map_err(ExploreError::Evaluate)?;
    Ok(HttpResponse::Ok().json(&result))
}

async fn get_explore(
    client_config: Data<HttpClientConfig>,
    services: Data<Services>,
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
        let source = JsonCryptogramStep {
            service: ServiceName::Recommendations,
            method: MethodName::Lookup,
            payload: json!({ "size": size, "owner_id": owner_id.unwrap() }),
            postflight: Some(Language::Object(vec![(
                String::from("ids"),
                Language::At(String::from("results")),
            )])),
        };
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
        let source = JsonCryptogramStep {
            service: ServiceName::Catalog,
            method: MethodName::Explore,
            payload: json!({ "q": req.q, "start": new_start, "bucket_info": bucket_info, "size": size }),
            postflight: Some(Language::Splat(vec![
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
            ])),
        };
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
            JsonCryptogramStep {
                service: ServiceName::Catalog,
                method: MethodName::Lookup,
                payload: json!({ "product_variant_ids": [] }),
                postflight: Some(Language::Object(
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
                )),
            },
        ],
    };

    let live_client = LiveJsonClient::build(client_config.get_ref());

    let result = do_evaluate(cryptogram, live_client, services.get_ref(), make_state())
        .await
        .map_err(ExploreError::Evaluate)?;
    Ok(HttpResponse::Ok().json(&result))
}

pub fn configure(server: &mut web::ServiceConfig, hostname: String) {
    let host_route = || web::route().guard(guard::Host(hostname.clone()));
    server
        .route("/explore", host_route().guard(guard::Get()).to(get_explore))
        .route(
            "/product_variants",
            host_route().guard(guard::Get()).to(get_product_variants),
        );
}
