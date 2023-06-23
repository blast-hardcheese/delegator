use actix_web::{ guard, web, HttpResponse };
use serde_json::json;

async fn post_paginate_list() -> HttpResponse {
    HttpResponse::Ok().json(json!({}))
}

pub fn configure(server: &mut web::ServiceConfig, hostname: String) {
    let host_route = || web::route().guard(guard::Host(hostname.clone()));
    server.route(
        "/closet",
        host_route().guard(guard::Post()).to(post_paginate_list),
    );
}
