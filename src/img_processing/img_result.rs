use super::params::ImgFormat;

pub struct ImgResult {
  pub img_bytes: Vec<u8>,
  pub format: ImgFormat,
}