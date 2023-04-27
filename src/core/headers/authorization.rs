use base64::{engine::general_purpose, Engine as _};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::{future::Future, pin::Pin};

use actix_web::FromRequest;

use super::AuthScheme;
use super::HeaderError;

pub struct Authorization {
    pub auth_scheme: Option<AuthScheme>,
    pub token: Option<String>,
}

impl Authorization {
    pub fn empty() -> Authorization {
        Authorization {
            auth_scheme: None,
            token: None,
        }
    }
    pub fn hmac_verify(&self, secret: String) -> Option<String> {
        if (self.auth_scheme != Some(AuthScheme::Bearer)) {
            return None;
        }
        let owner_id: Option<String> = match Vec::from_iter(
            self.token
                .clone()
                .unwrap_or(String::from(""))
                .rsplitn(2, '.'),
        )
        .as_slice()
        {
            [signature, id] => {
                let signature_bytes = signature.as_bytes();
                type HmacSha256 = Hmac<Sha256>;
                let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
                mac.update((id).as_bytes());
                let fin =
                    general_purpose::STANDARD_NO_PAD.encode(mac.clone().finalize().into_bytes());
                if fin == String::from(*signature) {
                    Some(String::from(*id))
                } else {
                    None
                }
            }
            [..] => None,
        };
        owner_id
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
            let (auth_scheme, token) =
                if let Some(v) = req.headers().get(String::from("Authorization")) {
                    let value = v
                        .to_str()
                        .map_err(HeaderError::InvalidAuthorizationHeader)?;
                    match Vec::from_iter(value.splitn(2, ' ')).as_slice() {
                        ["Basic", string] => (Some(AuthScheme::Basic), Some(string.to_string())),
                        ["Bearer", string] => (Some(AuthScheme::Bearer), Some(string.to_string())),
                        [..] => (None, None),
                    }
                } else {
                    (None, None)
                };
            Ok(Authorization { auth_scheme, token })
        })
    }
}
