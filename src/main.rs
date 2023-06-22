use aws_sdk_s3 as s3;
use axum::{http::StatusCode, response::IntoResponse, routing::get, Extension, Router};
use magick_rust::magick_wand_genesis;
use std::net::SocketAddr;
use tower_http::trace::{self, TraceLayer};
use tracing::Level;

mod img_processing;
use crate::img_processing::{handle_img, state::ImgState};

const KENYA_BUCKET_NAME: &str = "";
const KENYA_CACHE_BUCKET_NAME: &str = "";

const ZAMBIA_BUCKET_NAME: &str = "";
const ZAMBIA_CACHE_BUCKET_NAME: &str = "";

const SECRET_SALT: &str = "";

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_target(false).init();
    magick_wand_genesis();

    let aws_configuration: aws_config::SdkConfig = aws_config::load_from_env().await;
    let s3_client: aws_sdk_s3::Client = s3::Client::new(&aws_configuration);

    let routes: Router = Router::new()
        .route("/", get(index))
        .route("/health", get(health))
        .nest(
            "",
            Router::new()
                .route("/kenya/*img_key", get(handle_img))
                .with_state(ImgState {
                    bucket_name: KENYA_BUCKET_NAME,
                    cache_bucket_name: KENYA_CACHE_BUCKET_NAME,
                    secret_salt: SECRET_SALT,
                })
                .route("/zambia/*img_key", get(handle_img))
                .with_state(ImgState {
                    bucket_name: ZAMBIA_BUCKET_NAME,
                    cache_bucket_name: ZAMBIA_CACHE_BUCKET_NAME,
                    secret_salt: SECRET_SALT,
                })
                .layer(Extension(s3_client)),
        )
        .fallback(handler_404)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new().level(Level::INFO))
                .on_response(trace::DefaultOnResponse::new().level(Level::INFO)),
        );

    let port = 6060;
    let addr: SocketAddr = format!("[::]:{}", port).parse().unwrap();
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
