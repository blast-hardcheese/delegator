use crate::config::HttpClientConfig;
use actix_web::{
    body::BoxBody,
    web::{self, Data, Json},
    HttpResponse, ResponseError,
};

use crate::client::LiveJsonClient;
use crate::evaluator::{do_evaluate, EvaluateError};
use crate::model::cryptogram::Cryptogram;
use crate::routes::errors::json_error_response;

impl ResponseError for EvaluateError {
    fn error_response(&self) -> HttpResponse<BoxBody> {
        json_error_response(self)
    }
}

async fn evaluate(
    cryptogram: Json<Cryptogram>,
    client_config: Data<HttpClientConfig>,
) -> Result<HttpResponse, EvaluateError> {
    let live_client = LiveJsonClient::build(client_config.get_ref());

    let result = do_evaluate(cryptogram.into_inner(), live_client).await?;
    Ok(HttpResponse::Ok().json(&result))
}

pub fn configure(server: &mut web::ServiceConfig) {
    server.route("/evaluate", web::post().to(evaluate));
}
