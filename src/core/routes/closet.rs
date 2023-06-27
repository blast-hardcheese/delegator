use actix_web::{
    body::BoxBody,
    error, guard,
    web::{self, Json},
    HttpResponse,
};
use derive_more::Display;
use iso8601::DateTime;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::headers::authorization::{Authorization, BearerFields};

#[derive(Debug, Display)]
enum ClosetError {}

impl error::ResponseError for ClosetError {
    fn error_response(&self) -> HttpResponse<BoxBody> {
        panic!("Error with unknown cause: {:?}", self);
    }
}

#[derive(Deserialize)]
struct PostPaginateListsRequest {
    #[serde(rename = "type")]
    list_type: String,
}

async fn post_paginate_lists(
    authorization: Option<Authorization>,
    req: Json<PostPaginateListsRequest>,
) -> Result<HttpResponse, ClosetError> {
    let authorization: Authorization = authorization.unwrap_or(Authorization::empty());
    let (owner_id, _) = if let Authorization::Bearer(BearerFields {
        owner_id,
        raw_value,
    }) = authorization
    {
        (owner_id, raw_value)
    } else {
        return Ok(HttpResponse::Unauthorized().json(json!({})));
    };

    log::warn!(
        "Stubbing out list pagination response for {}, lists of type {}",
        owner_id,
        req.list_type
    );
    Ok(HttpResponse::Ok().json(json!(
        {
            "results": [
                {
                    "id": uuid::Uuid::new_v4(),
                    "name": "Default Closet",
                    "createdAt": DateTime { ..Default::default() },
                }
            ]
        }
    )))
}

#[derive(Deserialize)]
struct PostPaginateListRequest {}

async fn post_paginate_list(
    _req: Json<PostPaginateListRequest>,
    args: web::Path<(Uuid,)>,
    authorization: Option<Authorization>,
) -> Result<HttpResponse, ClosetError> {
    let list_id = args.0;
    let authorization: Authorization = authorization.unwrap_or(Authorization::empty());
    let (owner_id, _) = if let Authorization::Bearer(BearerFields {
        owner_id,
        raw_value,
    }) = authorization
    {
        (owner_id, raw_value)
    } else {
        return Ok(HttpResponse::Unauthorized().json(json!({})));
    };

    log::warn!("Stubbing out list pagination response for {}", owner_id);

    Ok(HttpResponse::Ok().json(json!(
        {
            "id": list_id,
            "product_variant_ids": [],
            "has_more": false,
        }
    )))
}

pub fn configure(server: &mut web::ServiceConfig, hostname: String) {
    let host_route = || web::route().guard(guard::Host(hostname.clone()));
    server.route(
        "/lists",
        host_route().guard(guard::Post()).to(post_paginate_lists),
    );
    server.route(
        "/list/{id}",
        host_route().guard(guard::Post()).to(post_paginate_list),
    );
}
