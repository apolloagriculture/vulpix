use aws_sdk_s3::{
    error::SdkError, operation::get_object::GetObjectError, primitives::ByteStreamError,
};
use magick_rust::MagickError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ImageError {
    #[error("aws error: {0}")]
    AwsError(#[from] SdkError<GetObjectError>),
    #[error("stream error: {0}")]
    StreamError(#[from] ByteStreamError),
    #[error("magickwand error: {0}")]
    MagickWandError(MagickError),
    #[error("encryption key invalid")]
    EncryptionInvalid,
    #[error("image has already expired")]
    Expired,
}
