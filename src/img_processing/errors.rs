use aws_sdk_s3::{
    error::SdkError, operation::get_object::GetObjectError, primitives::ByteStreamError,
};
use magick_rust::MagickError;

#[derive(Debug)]
pub enum ImageError {
    AwsError(SdkError<GetObjectError>),
    StreamError(ByteStreamError),
    MagickWandError(MagickError),
    EncryptionInvalid(String),
    Expired(String),
}

impl From<SdkError<GetObjectError>> for ImageError {
    fn from(err: SdkError<GetObjectError>) -> ImageError {
        ImageError::AwsError(err)
    }
}

impl From<ByteStreamError> for ImageError {
    fn from(err: ByteStreamError) -> ImageError {
        ImageError::StreamError(err)
    }
}

impl ImageError {
    pub fn get_err_message(&self) -> String {
        match self {
            ImageError::AwsError(e) => e.to_string(),
            ImageError::StreamError(e) => e.to_string(),
            ImageError::MagickWandError(e) => e.to_string(),
            ImageError::EncryptionInvalid(s) => s.to_string(),
            ImageError::Expired(s) => s.to_string(),
        }
    }
}
