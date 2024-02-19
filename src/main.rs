use std::io::{Error, ErrorKind, Result};

use actix_cors::Cors;
use actix_web::{middleware::Logger, web::Data, App, HttpServer};
use delegator_core::config::Configuration;

enum InitErrors {
    MissingConfigFile,
    ErrorLoadingConfig(std::io::Error),
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
    let path = std::env::args()
        .nth(1)
        .ok_or(InitErrors::MissingConfigFile)?;
    let Configuration {
        http,
    } = delegator_core::config::load_file(path.as_str()).map_err(InitErrors::ErrorLoadingConfig)?;

    // This is from the Sentry docs, https://docs.sentry.io/platforms/rust/guides/actix-web/
    // I suspect it's so we get error traces in Sentry. We may need to revisit this.
    std::env::set_var("RUST_BACKTRACE", "1");
    println!("Preparing to bind to {}:{}", http.host, http.port);

    HttpServer::new(move || {
        // let allowed_origins = http.cors.clone();
        let cors = Cors::default()
            // .allowed_origin_fn(move |origin, _req_head| {
            //     if let Ok(origin) = origin.to_str() {
            //         let origin = String::from(origin);
            //         allowed_origins.contains(&origin)
            //     } else {
            //         false
            //     }
            // })
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .send_wildcard()
            .max_age(3600);

        App::new()
            .wrap(Logger::default().log_target("accesslog"))
            .wrap(cors)
            .app_data(Data::new(http.client.clone()))
            .configure(delegator_core::routes::configure)
    })
    .bind((http.host, http.port))?
    .run()
    .await
}
