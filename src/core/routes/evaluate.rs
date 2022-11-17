use actix_web::{
    web::{self, Json},
    HttpResponse,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct JsonCryptogramStep {
    service: String,
    method: String,
    payload: Value,
}

#[derive(Debug, Deserialize)]
pub struct JsonCryptogram {
    steps: Vec<JsonCryptogramStep>,
}

pub async fn evaluate(cryptogram: Json<JsonCryptogram>) -> HttpResponse {
    HttpResponse::Ok().finish()
}

pub fn configure(server: &mut web::ServiceConfig) {
    server.route("/evaluate", web::post().to(evaluate));
}
