use actix_web::error;
use actix_web::http::header::ToStrError;
use derive_more::Display;

pub mod features;

#[derive(Debug, Display)]
pub enum HeaderError {
    InvalidFeatureHeader(ToStrError),
}

impl error::ResponseError for HeaderError {}
