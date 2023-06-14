use std::{
    io::{Error, ErrorKind, Result},
    sync::Arc,
};

use actix_cors::Cors;
use actix_web::{middleware::Logger, web::Data, App, HttpServer};
use delegator_core::{
    config::{
        events::EventConfig, Configuration, HttpClientConfig, ServiceDefinition, ServiceLocation,
        Services,
    },
    events::EventClient,
    translate::TranslateContext,
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
        events,
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
            session_mode: sentry::SessionMode::Request,
            auto_session_tracking: true,
            traces_sample_rate: 1.0,
            enable_profiling: true,
            profiles_sample_rate: 1.0,
            ..Default::default()
        },
    ));

    // This is from the Sentry docs, https://docs.sentry.io/platforms/rust/guides/actix-web/
    // I suspect it's so we get error traces in Sentry. We may need to revisit this.
    std::env::set_var("RUST_BACKTRACE", "1");
    println!("Preparing to bind to {}:{}", http.host, http.port);

    let event_client = {
        let client = EventClient::new().await;
        Arc::new(client)
    };

    let ctx = TranslateContext::build(event_client);

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
            .wrap(sentry_actix::Sentry::new())
            .app_data::<Data<EventConfig>>(Data::new(events.clone()))
            .app_data::<Data<HttpClientConfig>>(Data::new(http.client.clone()))
            .app_data::<Data<Services>>(Data::new(services.clone()))
            .app_data::<Data<TranslateContext>>(Data::new(ctx.clone()))
            .configure(|server| delegator_core::routes::configure(server, &virtualhosts))
    })
    .bind((http.host, http.port))?
    .run()
    .await
}
