use super::params::ImgFormat;

#[derive(Clone)]
pub struct ImgResult {
  pub img_bytes: Vec<u8>,
  pub format: ImgFormat,
}