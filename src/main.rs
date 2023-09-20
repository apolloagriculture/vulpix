use aws_sdk_s3 as s3;
use axum::{http::StatusCode, response::IntoResponse, routing::get, Extension, Router};
use magick_rust::magick_wand_genesis;
use settings::ImgSource;
use std::net::SocketAddr;
use tower_http::trace::{self, TraceLayer};
use tracing::Level;

mod settings;

mod img_processing;
use crate::img_processing::{handle_img, state::BucketConfig};

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

    magick_wand_genesis();

    let aws_configuration: aws_config::SdkConfig = aws_config::load_from_env().await;
    let s3_client: aws_sdk_s3::Client = s3::Client::new(&aws_configuration);
    let rek_client: aws_sdk_rekognition::Client =
        aws_sdk_rekognition::Client::new(&aws_configuration);

    let img_router = create_img_router(config.img_sources, s3_client, rek_client);

    let routes: Router = Router::new()
        .route("/", get(index))
        .nest("", img_router)
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

fn create_img_router(
    img_sources: Vec<ImgSource>,
    s3_client: aws_sdk_s3::Client,
    rek_client: aws_sdk_rekognition::Client,
) -> Router {
    let mut img_router = Router::new();

    for img_source in img_sources.iter() {
        img_router = img_router.route(
            &format!("/{}/*img_key", img_source.path.clone()),
            get(handle_img).with_state(BucketConfig {
                bucket_name: img_source.bucket.clone(),
                cache_bucket_name: img_source.cache_bucket.clone(),
            }),
        );
    }

    img_router
        .layer(Extension(s3_client))
        .layer(Extension(rek_client))
}
