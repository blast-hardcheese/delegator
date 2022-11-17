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
    println!("Cryptogram: {:?}", cryptogram);

    let client = awc::Client::default();

    let mut final_result: Option<Value> = None;

    for step in &cryptogram.steps {
        let req = client
            .post("http://localhost:4000/request-signup-token")
            .insert_header(("User-Agent", "awc/3.0"))
            .insert_header(("Content-Type", "application/json"))
            .send_json(&step.payload.clone());
        let mut res = req.await.unwrap();
        let x = res.json::<Value>().await.unwrap();
        final_result = Some(x);
    }

    final_result
        .map(|res| HttpResponse::Ok().json(&res))
        .unwrap_or(HttpResponse::BadRequest().finish())
}

pub fn configure(server: &mut web::ServiceConfig) {
    server.route("/evaluate", web::post().to(evaluate));
}
