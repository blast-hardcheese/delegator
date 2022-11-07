use actix_web::{App, HttpServer};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| App::new().configure(delegator_core::routes::configure))
        .bind(("0.0.0.0", 8080))?
        .run()
        .await
}
