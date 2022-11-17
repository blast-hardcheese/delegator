use actix_web::web;

pub mod evaluate;
pub mod healthcheck;

pub fn configure(server: &mut web::ServiceConfig) {
    server
        .configure(evaluate::configure)
        .configure(healthcheck::configure);
}
