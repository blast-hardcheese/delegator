use actix_web::{
    error, guard,
    web::{self, Data, Json},
    HttpResponse,
};
use derive_more::Display;
use serde::Deserialize;
use serde_json::json;

use crate::{
    config::{HttpClientConfig, MethodName, ServiceName, Services},
    translate::make_state,
};

use super::evaluate::{do_evaluate, JsonCryptogram, JsonCryptogramStep, LiveJsonClient};

#[derive(Debug, Display)]
enum PricingError {
    Evaluate(super::evaluate::EvaluateError),
}

impl error::ResponseError for PricingError {}

#[derive(Deserialize)]
struct PostResalePrice {
    brand: String,
    image_url: String,
    q: String,
    product_variant_id: Option<String>,
}

async fn post_resale_price(
    client_config: Data<HttpClientConfig>,
    services: Data<Services>,
    req: Json<PostResalePrice>,
) -> Result<HttpResponse, PricingError> {
    let cryptogram = JsonCryptogram {
        steps: vec![JsonCryptogramStep {
            service: ServiceName::Pricing,
            method: MethodName::Lookup,
            payload: json!({ "brand": req.brand, "image_url": req.image_url, "q": req.q, "product_variant_id": req.product_variant_id, }),
            postflight: None,
        }],
    };

    let client = awc::Client::default();
    let live_client = LiveJsonClient {
        client,
        client_config: client_config.get_ref().clone(),
    };

    let result = do_evaluate(cryptogram, live_client, services.get_ref(), make_state())
        .await
        .map_err(PricingError::Evaluate)?;
    Ok(HttpResponse::Ok().json(&result))
}

pub fn configure(server: &mut web::ServiceConfig, hostname: String) {
    let host_route = || web::route().guard(guard::Host(hostname.clone()));
    server.route(
        "/resale-price",
        host_route().guard(guard::Post()).to(post_resale_price),
    );
}
