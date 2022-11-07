use actix_web::web;

pub mod healthcheck;

pub fn configure(server: &mut web::ServiceConfig) {
    server.configure(healthcheck::configure);
}
