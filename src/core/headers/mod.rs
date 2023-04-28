use actix_web::error;
use actix_web::http::header::ToStrError;
use derive_more::Display;

pub mod authorization;
pub mod features;

#[derive(Debug, Display)]
pub enum HeaderError {
    InvalidFeatureHeader(ToStrError),
    InvalidAuthorizationHeader(ToStrError),
}

impl error::ResponseError for HeaderError {}
