use actix_web::web;

use crate::config::Virtualhosts;

pub mod catalog;
pub mod evaluate;
pub mod healthcheck;

pub fn configure(server: &mut web::ServiceConfig, virtualhosts: &Virtualhosts) {
    server
        .configure(|server| catalog::configure(server, virtualhosts.catalog.clone()))
        .configure(evaluate::configure)
        .configure(healthcheck::configure);
}
