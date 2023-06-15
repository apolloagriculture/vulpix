use aws_sdk_s3 as s3;
use axum::{http::StatusCode, response::IntoResponse, routing::get, Router};
use magick_rust::magick_wand_genesis;
use std::net::SocketAddr;
use tower_http::trace::{self, TraceLayer};
use tracing::Level;

mod img_processing;
use crate::img_processing::{image_router, state::ImgState};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_target(false).pretty().init();

    let aws_configuration = aws_config::load_from_env().await;
    let s3_client = s3::Client::new(&aws_configuration);


    magick_wand_genesis();

    static BUCKET_NAME: &str = "";
    static SECRET_SALT: &str = "";
    
    let routes: Router = Router::new()
        .route("/", get(index))
        .nest(
            "/img",
            image_router().with_state(ImgState {
                s3_client,
                bucket_name: BUCKET_NAME,
                secret_salt: SECRET_SALT,
            }),
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

async fn handler_404() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "nothing to see here")
}
