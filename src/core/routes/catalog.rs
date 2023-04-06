use actix_web::{
    guard,
    web::{self, Json},
    Error, HttpResponse,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ExploreRequest {}

async fn explore(_request: Json<ExploreRequest>) -> Result<HttpResponse, Error> {
    Ok(HttpResponse::Ok().finish())
}

pub fn configure(server: &mut web::ServiceConfig, hostname: String) {
    server.route(
        "/explore",
        web::route().guard(guard::Host(hostname)).to(explore),
    );
}
