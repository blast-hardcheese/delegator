use actix_web::{
    http::Uri,
    web::{self, Data, Json},
    HttpResponse,
};
use serde::Deserialize;
use serde_json::Value;

use crate::config::{ServiceDefinition, Services};

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

pub async fn evaluate(cryptogram: Json<JsonCryptogram>, services: Data<Services>) -> HttpResponse {
    let client = awc::Client::default();

    let mut final_result: Option<Value> = None;

    for step in &cryptogram.steps {
        let service = services.get(&step.service).unwrap().to_owned();
        final_result = match service {
            ServiceDefinition::Rest {
                scheme,
                endpoint,
                methods,
            } => {
                let method = methods.get(&step.method).unwrap();

                let uri = Uri::builder()
                    .scheme(scheme)
                    .authority(endpoint)
                    .path_and_query(method.path.to_owned())
                    .build()
                    .unwrap();
                let req = client
                    .post(uri)
                    .insert_header(("User-Agent", "awc/3.0"))
                    .insert_header(("Content-Type", "application/json"))
                    .send_json(&step.payload.clone());
                let mut res = req.await.unwrap();
                let x = res.json::<Value>().await.unwrap();
                Some(x)
            }
        };
    }

    final_result
        .map(|res| HttpResponse::Ok().json(&res))
        .unwrap_or(HttpResponse::BadRequest().finish())
}

pub fn configure(server: &mut web::ServiceConfig) {
    server.route("/evaluate", web::post().to(evaluate));
}
