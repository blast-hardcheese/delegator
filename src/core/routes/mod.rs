use actix_web::web;

pub mod errors;
pub mod evaluate;

pub fn configure(server: &mut web::ServiceConfig) {
    server.configure(evaluate::configure);
}
