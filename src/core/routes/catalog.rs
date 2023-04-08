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
    translate::make_state,
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

    let cryptogram = JsonCryptogram {
        steps: vec![
            JsonCryptogramStep {
                service: ServiceName::Catalog,
                method: MethodName::Explore,
                payload: json!({ "q": req.q, "page": page, "bucket_info": bucket_info, "size": size }),
            },
            JsonCryptogramStep {
                service: ServiceName::Catalog,
                method: MethodName::Lookup,
                payload: json!({ "ids": [] }),
            },
        ],
    };

    let client = awc::Client::default();
    let live_client = LiveJsonClient {
        client,
        client_config: client_config.get_ref().clone(),
    };

    let translate_state = make_state();
    let result = do_evaluate(
        cryptogram,
        live_client,
        services.get_ref(),
        translate_state.clone(),
    )
    .await
    .map_err(ExploreError::Evaluate)?;
    Ok(HttpResponse::Ok().json(&result))
}

pub fn configure(server: &mut web::ServiceConfig, hostname: String) {
    let host_route = || web::route().guard(guard::Host(hostname.clone()));
    server.route("/explore", host_route().guard(guard::Get()).to(get_explore));
}
