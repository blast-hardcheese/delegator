use actix_web::{web, HttpResponse};

pub async fn healthcheck() -> HttpResponse {
    HttpResponse::Ok().finish()
}

pub fn configure(server: &mut web::ServiceConfig) {
    server.route("/health", web::get().to(healthcheck));
}
