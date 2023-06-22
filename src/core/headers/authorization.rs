use base64::{engine::general_purpose, Engine as _};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::{future::Future, pin::Pin};

use actix_web::FromRequest;

use super::HeaderError;

pub struct BearerFields {
    pub owner_id: String,
    pub raw_value: String,
}

pub enum Authorization {
    Bearer(BearerFields),
    Empty,
}

fn hmac_verify(token: String) -> Option<String> {
    let secret = std::env::var("HTTP_COOKIE_SECRET").ok()?;

    match Vec::from_iter(token.rsplitn(2, '.')).as_slice() {
        [signature, owner_id] => {
            type HmacSha256 = Hmac<Sha256>;
            let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
            mac.update(owner_id.as_bytes());
            let fin = general_purpose::STANDARD_NO_PAD.encode(mac.clone().finalize().into_bytes());
            if fin == **signature {
                Some(String::from(*owner_id))
            } else {
                None
            }
        }
        [..] => None,
    }
}

impl Authorization {
    pub fn empty() -> Authorization {
        Authorization::Empty
    }
}

impl FromRequest for Authorization {
    type Error = HeaderError;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;
    fn from_request(
        req: &actix_web::HttpRequest,
        _payload: &mut actix_web::dev::Payload,
    ) -> Self::Future {
        let req = req.clone();
        Box::pin(async move {
            let auth = if let Some(v) = req.headers().get(String::from("Authorization")) {
                let value = v
                    .to_str()
                    .map_err(HeaderError::InvalidAuthorizationHeader)?;
                match Vec::from_iter(value.splitn(2, ' ')).as_slice() {
                    // s: is a leading signature of a "signed cookie" from express.js
                    // We use it here as a sentinel to indicate legacy Bearer format
                    ["Bearer", token] if token.starts_with("s:") => {
                        if let Some(owner_id) = hmac_verify(String::from(*token)[2..].to_string()) {
                            Authorization::Bearer(BearerFields {
                                owner_id,
                                raw_value: String::from(*token),
                            })
                        } else {
                            // TODO: This should likely be an error. Invalid auth specified is
                            // different than no auth specified.
                            Authorization::Empty
                        }
                    }
                    [..] => Authorization::Empty,
                }
            } else {
                Authorization::Empty
            };
            Ok(auth)
        })
    }
}
