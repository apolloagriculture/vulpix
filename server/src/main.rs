use aws_sdk_s3 as s3;
use axum::{http::StatusCode, response::IntoResponse, routing::get, Router};
use std::net::SocketAddr;
use tower_http::trace::{self, TraceLayer};
use tracing::Level;

mod app_error;
mod image_access;
mod image_result;
mod image_router;
mod settings;
mod state;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().json().init();

    let config = match settings::Settings::new() {
        Ok(settings) => {
            tracing::log::info!("starting with config | {:?}", settings);
            settings
        }
        Err(err) => {
            tracing::log::error!("couldn't load config: {:?}", err);
            panic!("couldn't load config: {:?}", err)
        }
    };

    if let Some(sentry_dns) = config.sentry.map(|s| s.dsn) {
        let _guard = sentry::init((
            sentry_dns,
            sentry::ClientOptions {
                release: sentry::release_name!(),
                environment: Some(format!("{}", config.env).into()),
                ..Default::default()
            },
        ));
    }

    vulpix::init();

    let aws_configuration: aws_config::SdkConfig = aws_config::load_from_env().await;
    let s3_client: aws_sdk_s3::Client = s3::Client::new(&aws_configuration);
    let rek_client: aws_sdk_rekognition::Client =
        aws_sdk_rekognition::Client::new(&aws_configuration);

    let image_router = image_router::create_image_router(config.img_sources, s3_client, rek_client);

    let routes: Router = Router::new()
        .route("/", get(index))
        .nest("", image_router)
        .route("/health", get(health))
        .fallback(handler_404)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new().level(Level::INFO))
                .on_response(trace::DefaultOnResponse::new().level(Level::INFO)),
        );

    let addr: SocketAddr = format!("[::]:{}", config.server.port).parse().unwrap();
    tracing::info!("listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(routes.into_make_service())
        .await
        .unwrap();
}

async fn index() -> impl IntoResponse {
    (StatusCode::OK, "welcome to vulpix")
}

async fn health() -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

async fn handler_404() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "nothing to see here")
}
