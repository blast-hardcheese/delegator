use actix_web::web;

use crate::config::Virtualhosts;

pub mod catalog;
pub mod evaluate;
pub mod healthcheck;
pub mod pricing;

pub fn configure(server: &mut web::ServiceConfig, virtualhosts: &Virtualhosts) {
    server
        .configure(evaluate::configure)
        .configure(healthcheck::configure)
        .configure(|server| catalog::configure(server, virtualhosts.catalog.clone()))
        .configure(|server| pricing::configure(server, virtualhosts.pricing.clone()));
}
