use aws_sdk_s3::{
    error::SdkError,
    operation::{get_object::GetObjectError, put_object::PutObjectError},
    primitives::{ByteStreamError, SdkBody},
};
use magick_rust::MagickError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ImageError {
    #[error("aws read error: {0}")]
    AwsReadError(#[from] SdkError<GetObjectError, axum::http::Response<SdkBody>>),
    #[error("aws write error: {0}")]
    AwsWriteError(#[from] SdkError<PutObjectError, axum::http::Response<SdkBody>>),
    #[error("stream error: {0}")]
    StreamError(#[from] ByteStreamError),
    #[error("magickwand error: {0}")]
    MagickWandError(MagickError),
}
