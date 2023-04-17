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
    translate::{make_state, Language}, headers::features::Features,
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
    Mutex,
}

impl error::ResponseError for ExploreError {}

async fn get_explore(
    client_config: Data<HttpClientConfig>,
    services: Data<Services>,
    req: web::Query<ExploreRequest>,
    features: Option<Features>,
) -> Result<HttpResponse, ExploreError> {
    let features = features.unwrap_or(Features::empty());

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
            let _start = start
                .parse::<i32>()
                .map_err(ExploreError::InvalidPage)?;
            (_start, None)
        }
        ["catalog", start, bucket_info] => {
            let _start = start
                .parse::<i32>()
                .map_err(ExploreError::InvalidPage)?;
            (_start, Some(bucket_info.to_owned()))
        }
        [..] => (0, None),
    };

    let (source, next_start) = if start == 0 && features.recommendations {
        let source = JsonCryptogramStep {
            service: ServiceName::Recommendations,
            method: MethodName::Lookup,
            payload: json!({ "size": size }),
            postflight: Language::Object(vec![
                (String::from("ids"), Language::At(String::from("results"))),
            ])
        };
        let next_start = format!("catalog:{}", size);
        (source, vec![
            (String::from("next_start"), Language::Const(json!(next_start))),
            (String::from("has_more"), Language::Const(json!(true))),
        ])
    } else {
        let new_start = if features.recommendations {
            start - 1  // Offset how many recs we want if we are running recommendations
        } else {
            start
        };
        let source = JsonCryptogramStep {
            service: ServiceName::Catalog,
            method: MethodName::Explore,
            payload: json!({ "q": req.q, "start": new_start, "bucket_info": bucket_info, "size": size }),
            postflight: Language::Splat(vec![
                Language::Focus(String::from("next_start"), Box::new(Language::Set(String::from("next_start")))),
                Language::Focus(String::from("has_more"), Box::new(Language::Set(String::from("has_more")))),
                Language::Object(vec![
                    (String::from("ids"), Language::At(String::from("product_variant_ids"))),
                ])
            ]),
        };
        (source, vec![
            (String::from("next_start"), Language::Get(String::from("next_start"))),
            (String::from("has_more"), Language::Get(String::from("has_more"))),
        ])
    };

    let cryptogram = JsonCryptogram {
        steps: vec![
            source,
            JsonCryptogramStep {
                service: ServiceName::Catalog,
                method: MethodName::Lookup,
                payload: json!({ "ids": [] }),
                postflight: Language::Object(vec![
                    vec![
                        (String::from("results"), Language::At(String::from("results"))),
                        (String::from("data"), Language::At(String::from("results"))),  // TODO: Delete this ASAP
                        (String::from("status"), Language::Const(json!("ok"))),         // TODO: Delete this ASAP
                    ],
                    next_start,
                ].concat()),
            },
        ],
    };

    let client = awc::Client::default();
    let live_client = LiveJsonClient {
        client,
        client_config: client_config.get_ref().clone(),
    };

    let translate_state = make_state();
    let mut result = do_evaluate(
        cryptogram,
        live_client,
        services.get_ref(),
        translate_state.clone(),
    )
    .await
    .map_err(ExploreError::Evaluate)?;
    let hm = translate_state
        .lock()
        .map_err(|_x| ExploreError::Mutex)?;
    if let Some(next) = hm.get("next_start") {
        result
            .as_object_mut()
            .unwrap()
            .insert(String::from("next_start"), (**next).clone());
    }
    Ok(HttpResponse::Ok().json(&result))
}

pub fn configure(server: &mut web::ServiceConfig, hostname: String) {
    let host_route = || web::route().guard(guard::Host(hostname.clone()));
    server.route("/explore", host_route().guard(guard::Get()).to(get_explore));
}
