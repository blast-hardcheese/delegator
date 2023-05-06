use actix_web::{
    body::BoxBody,
    http::{
        header::{self, TryIntoHeaderValue},
        StatusCode,
    },
    HttpResponse,
};
use serde_json::Value;
use std::fmt;

pub trait JsonResponseError {
    fn error_as_json(&self) -> Value;
}

pub fn json_error_response<A>(err: &A) -> HttpResponse<BoxBody>
where
    A: fmt::Display + fmt::Debug + JsonResponseError,
{
    let mut res = HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR);

    let json = mime::APPLICATION_JSON.try_into_value().unwrap();
    res.headers_mut().insert(header::CONTENT_TYPE, json);

    let x = serde_json::to_string(&err.error_as_json()).unwrap();
    res.set_body(BoxBody::new(x))
}
