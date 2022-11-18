use std::io::{Error, ErrorKind, Result};

use actix_web::{middleware::Logger, web::Data, App, HttpServer};
use delegator_core::config::{Configuration, Services};

enum InitErrors {
    MissingConfigFile,
    ErrorLoadingConfig(hocon::Error),
}

impl From<InitErrors> for Error {
    fn from(err: InitErrors) -> Self {
        match err {
            InitErrors::MissingConfigFile => Error::new(
                ErrorKind::Other,
                "First argument to the server must be a path to the config file",
            ),
            InitErrors::ErrorLoadingConfig(err) => Error::new(ErrorKind::Other, err.to_string()),
        }
    }
}

#[actix_web::main]
async fn main() -> Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let path = std::env::args()
        .nth(1)
        .ok_or(InitErrors::MissingConfigFile)?;
    let Configuration { http, services } =
        delegator_core::config::load_file(path.as_str()).map_err(InitErrors::ErrorLoadingConfig)?;
    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default().log_target("accesslog"))
            .app_data::<Data<Services>>(Data::new(services.clone()))
            .configure(delegator_core::routes::configure)
    })
    .bind((http.host, http.port))?
    .run()
    .await
}
