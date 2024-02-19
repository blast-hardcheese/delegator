use actix_web::http::uri::Authority;

pub async fn lookup(_docker_spec: String) -> Authority {
  Authority::from_static("localhost")
}