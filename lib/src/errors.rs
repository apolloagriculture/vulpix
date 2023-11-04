use magick_rust::MagickError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ImageError {
  #[error("img read error: {0}")]
  ImgReadError(String),
  #[error("img write error: {0}")]
  ImgWriteError(String),
  #[error("magickwand error: {0}")]
  MagickWandError(#[from] MagickError),
}