use axum::response::{Response, IntoResponse};
use vulpix::params::ImgFormat;

#[derive(Clone)]
pub struct ImageResult {
  pub image_bytes: Vec<u8>,
  pub format: ImgFormat,
}

impl IntoResponse for ImageResult {
  fn into_response(self) -> Response {
      let content_type = format!("image/{}", self.format);
      (
          ([(axum::http::header::CONTENT_TYPE, content_type.clone())]),
          axum::response::AppendHeaders([(axum::http::header::CONTENT_TYPE, content_type)]),
          self.image_bytes,
      )
          .into_response()
  }
}
