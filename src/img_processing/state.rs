#[derive(Clone)]
pub struct ImgState {
    pub secret_salt: &'static str,
    pub bucket_name: &'static str,
    pub cache_bucket_name: &'static str,
}
