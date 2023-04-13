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
    Mutex,
}

impl error::ResponseError for ExploreError {}

async fn get_explore(
    client_config: Data<HttpClientConfig>,
    services: Data<Services>,
    req: web::Query<ExploreRequest>,
) -> Result<HttpResponse, ExploreError> {
    let start = req.start.clone().unwrap_or(String::from("1"));
    let size = req.size.unwrap_or(10);
    let (page, bucket_info) = match Vec::from_iter(start.splitn(3, ':')).as_slice() {
        [legacy_page] => {
            let _page = legacy_page
                .to_owned()
                .parse::<i32>()
                .map_err(ExploreError::InvalidPage)?;
            (_page - 1, None)
        }
        ["catalog", page] => {
            let _page = page
                .parse::<i32>()
                .map_err(ExploreError::InvalidPage)?;
            (_page, None)
        }
        ["catalog", page, bucket_info] => {
            let _page = page
                .parse::<i32>()
                .map_err(ExploreError::InvalidPage)?;
            (_page, Some(bucket_info.to_owned()))
        }
        [..] => (0, None),
    };

    let source = if page == 0 {
        JsonCryptogramStep {
            service: ServiceName::Recommendations,
            method: MethodName::Lookup,
            payload: json!({ "size": size }),
            postflight: Language::Object(vec![
                (String::from("ids"), Language::At(String::from("results"))),
            ])
        }
    } else {
        let new_page = page - 1;  // Offset how many pages of recs we want
        JsonCryptogramStep {
            service: ServiceName::Catalog,
            method: MethodName::Explore,
            payload: json!({ "q": req.q, "page": new_page, "bucket_info": bucket_info, "size": size }),
            postflight: Language::Splat(vec![
                Language::Focus(String::from("next_start"), Box::new(Language::Set(String::from("next_start")))),
                Language::Object(vec![
                    (String::from("ids"), Language::At(String::from("product_variant_ids"))),
                ])
            ]),
        }
    };

    let cryptogram = JsonCryptogram {
        steps: vec![
            source,
            JsonCryptogramStep {
                service: ServiceName::Catalog,
                method: MethodName::Lookup,
                payload: json!({ "ids": [] }),
                postflight: Language::Object(vec![
                    (String::from("results"), Language::At(String::from("results"))),
                    (String::from("next_start"), Language::Get(String::from("next_start"))),
                ]),
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
