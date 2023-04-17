use std::{future::Future, pin::Pin};

use actix_web::FromRequest;

use super::HeaderError;

pub struct Features {
    pub recommendations: bool,
}

impl Features {
    pub fn empty() -> Features {
        Features {
            recommendations: false,
        }
    }
}

impl FromRequest for Features {
    type Error = HeaderError;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;
    fn from_request(
        req: &actix_web::HttpRequest,
        _payload: &mut actix_web::dev::Payload,
    ) -> Self::Future {
        let req = req.clone();
        Box::pin(async move {
            let recommendations: bool = if let Some(v) = req.headers().get(String::from("Features"))
            {
                let value = v.to_str().map_err(HeaderError::InvalidFeatureHeader)?;
                value.contains("recommendations")
            } else {
                false
            };
            Ok(Features { recommendations })
        })
    }
}
