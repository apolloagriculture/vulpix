use aws_sdk_s3 as s3;

#[derive(Clone)]
pub struct ImgState {
    pub s3_client: s3::Client,
    pub secret_salt: &'static str,
    pub bucket_name: &'static str,
}
