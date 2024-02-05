use actix_web::web;

pub mod errors;
pub mod evaluate;

use crate::config::Virtualhosts;

pub fn configure(server: &mut web::ServiceConfig, virtualhosts: &Virtualhosts) {
    server.configure(|server| evaluate::configure(server, virtualhosts));
}
