use actix_web::{
    body::BoxBody,
    error, guard,
    web::{self, Data, Json},
    HttpResponse,
};
use derive_more::Display;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Mutex;

use crate::{
    cache::MemoizationCache,
    config::{HttpClientConfig, Services},
    translate::{make_state, TranslateContext},
};

use super::{
    errors::json_error_response,
    evaluate::{do_evaluate, JsonCryptogram, JsonCryptogramStep, LiveJsonClient},
};

#[derive(Debug, Display)]
enum PricingError {
    Evaluate(super::evaluate::EvaluateError),
}

impl error::ResponseError for PricingError {
    fn error_response(&self) -> HttpResponse<BoxBody> {
        match self {
            Self::Evaluate(inner) => {
                json_error_response(inner)
            }
        }
    }
}

#[derive(Deserialize)]
struct PostResalePrice {
    brand: String,
    image_url: String,
    q: String,
    product_variant_id: Option<String>,
}

async fn post_resale_price(
    cache_state: Data<Mutex<MemoizationCache>>,
    ctx: Data<TranslateContext>,
    client_config: Data<HttpClientConfig>,
    services: Data<Services>,
    req: Json<PostResalePrice>,
) -> Result<HttpResponse, PricingError> {
    let cryptogram = JsonCryptogram {
        steps: vec![
            JsonCryptogramStep::build("pricing", "lookup")
            .payload(json!({ "brand": req.brand, "image_url": req.image_url, "q": req.q, "product_variant_id": req.product_variant_id, }))
            .finish()
        ],
    };

    let live_client = LiveJsonClient::build(client_config.get_ref());

    let (result, _) = do_evaluate(
        ctx.get_ref(),
        cache_state.into_inner(),
        cryptogram,
        live_client,
        services.get_ref(),
        make_state(),
    )
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
