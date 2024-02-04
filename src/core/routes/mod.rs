use actix_web::web;

use crate::config::Virtualhosts;

pub mod errors;
pub mod evaluate;

pub fn configure(server: &mut web::ServiceConfig, virtualhosts: &Virtualhosts) {
    server.configure(evaluate::configure);
}
