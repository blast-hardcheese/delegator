use actix_web::{
    error, guard,
    web::{self, Data, Json},
    HttpResponse,
};
use derive_more::Display;
use serde::Deserialize;
use serde_json::json;

use crate::config::{HttpClientConfig, Services};

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
}

async fn post_resale_price(
    client_config: Data<HttpClientConfig>,
    services: Data<Services>,
    req: Json<PostResalePrice>,
) -> Result<HttpResponse, PricingError> {
    Ok(HttpResponse::Ok().json(json!(null)))
}

pub fn configure(server: &mut web::ServiceConfig, hostname: String) {
    let host_route = || web::route().guard(guard::Host(hostname.clone()));
    server.route(
        "/resale-price",
        host_route().guard(guard::Post()).to(post_resale_price),
    );
}
