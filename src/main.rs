use std::io::{Error, ErrorKind, Result};

use actix_web::{middleware::Logger, web::Data, App, HttpServer};
use delegator_core::config::{
    Configuration, HttpClientConfig, ServiceDefinition, ServiceLocation, Services,
};

enum InitErrors {
    MissingConfigFile,
    ErrorLoadingConfig(hocon::Error),
    ErrorLoadingRegistryService(String, String),
}

impl From<InitErrors> for Error {
    fn from(err: InitErrors) -> Self {
        match err {
            InitErrors::MissingConfigFile => Error::new(
                ErrorKind::Other,
                "First argument to the server must be a path to the config file",
            ),
            InitErrors::ErrorLoadingConfig(err) => Error::new(ErrorKind::Other, err.to_string()),
            InitErrors::ErrorLoadingRegistryService(env, service_name) => Error::new(
                ErrorKind::Other,
                format!("Missing registry service: {}:{}", env, service_name),
            ),
        }
    }
}

#[actix_web::main]
async fn main() -> Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let path = std::env::args()
        .nth(1)
        .ok_or(InitErrors::MissingConfigFile)?;
    let Configuration {
        authorities,
        environment,
        http,
        sentry,
        mut services,
        virtualhosts,
    } = delegator_core::config::load_file(path.as_str()).map_err(InitErrors::ErrorLoadingConfig)?;

    for (service_name, service) in services.iter_mut() {
        let ServiceDefinition::Rest {
            scheme,
            authority,
            methods: _,
        } = service;
        let ServiceLocation {
            scheme: new_scheme,
            authority: new_authority,
        } = authorities
            .get(service_name)
            .ok_or_else(|| {
                InitErrors::ErrorLoadingRegistryService(
                    environment.clone(),
                    service_name.to_string(),
                )
            })?
            .clone();
        *scheme = new_scheme;
        *authority = new_authority;
    }

    let _guard = sentry::init((
        sentry.dsn,
        sentry::ClientOptions {
            environment: sentry.environment.map(|e| e.into()),
            release: sentry.release.map(|r| r.into()),
            traces_sample_rate: 1f32,
            ..Default::default()
        },
    ));

    // This is from the Sentry docs, https://docs.sentry.io/platforms/rust/guides/actix-web/
    // I suspect it's so we get error traces in Sentry. We may need to revisit this.
    std::env::set_var("RUST_BACKTRACE", "1");

    HttpServer::new(move || {
        App::new()
            .wrap(sentry_actix::Sentry::new())
            .wrap(Logger::default().log_target("accesslog"))
            .app_data::<Data<HttpClientConfig>>(Data::new(http.client.clone()))
            .app_data::<Data<Services>>(Data::new(services.clone()))
            .configure(|server| delegator_core::routes::configure(server, &virtualhosts))
    })
    .bind((http.host, http.port))?
    .run()
    .await
}
