use actix_web::error;
use actix_web::http::header::ToStrError;
use derive_more::Display;

pub mod features;
pub mod authorization;

#[derive(Debug, Display)]
pub enum HeaderError {
    InvalidFeatureHeader(ToStrError),
    InvalidAuthorizationHeader(ToStrError),
}

#[derive(Debug, Display, PartialEq)]
pub enum AuthScheme {
  Basic,
  Bearer,
}

impl error::ResponseError for HeaderError {}
