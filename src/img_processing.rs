use aws_sdk_s3 as s3;
use axum::{
    body::Bytes,
    extract::{Path, Query, RawQuery, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use magick_rust::MagickWand;
use md5;
use std::time::{SystemTime, UNIX_EPOCH};

mod errors;
mod img_result;
mod params;
pub mod state;

use self::{
    errors::ImageError,
    img_result::ImgResult,
    params::{ImgFormat, ImgParams},
    state::ImgState,
};

pub fn image_router() -> Router<ImgState> {
    Router::new().route("/*img_key", get(handle_img))
}

async fn handle_img(
    State(ImgState {
        s3_client,
        secret_salt,
        bucket_name,
    }): State<ImgState>,
    Path(img_key): Path<String>,
    Query(params): Query<ImgParams>,
    RawQuery(query): RawQuery,
) -> impl IntoResponse {
    let validation = validate_params(
        &params,
        query.unwrap_or("".to_string()),
        &img_key,
        &secret_salt,
    );

    let s3_img = match validation {
        Ok(_) => get_aws_img(s3_client, &bucket_name, &img_key).await,
        Err(err) => Err(err),
    };

    let transformed_img = s3_img.and_then(|img| transform_img(img, params));

    match transformed_img {
        Ok(img) => handle_img_response(img).into_response(),
        Err(err) => handle_img_err(err).into_response(),
    }
}

fn validate_params(
    params: &ImgParams,
    raw_query_param: String,
    img_key: &String,
    secret_salt: &String,
) -> Result<(), ImageError> {
    match (&params.s, params.expires) {
        (Some(encrypted_key), Some(expires)) => {
            validate_signature(&raw_query_param, &img_key, &secret_salt, encrypted_key)
                .and_then(|_| validate_expiration(expires))
        }
        (Some(encrypted_key), None) => {
            validate_signature(&raw_query_param, &img_key, &secret_salt, encrypted_key)
        }
        (None, Some(expires)) => validate_expiration(expires),
        (None, None) => Ok(()),
    }
}

fn validate_signature(
    raw_query_param: &String,
    img_key: &String,
    secret_salt: &String,
    encrypted_key: &String,
) -> Result<(), ImageError> {
    let raw_query_param_without_encryption = raw_query_param
        .replace(&format!("&s={}", encrypted_key), "")
        .replace(&format!("?s={}", encrypted_key), "");
    let md5_digest = md5::compute(format!(
        "{}/{}?{}",
        &secret_salt, img_key, raw_query_param_without_encryption
    ));
    if &format!("{:x}", md5_digest) == encrypted_key {
        Ok(())
    } else {
        Err(ImageError::EncryptionInvalid(String::from(
            "encryption key invalid",
        )))
    }
}

fn validate_expiration(expiration: f32) -> Result<(), ImageError> {
    let current_epech_in_seconds = SystemTime::now().duration_since(UNIX_EPOCH);
    match current_epech_in_seconds {
        Ok(t) if t.as_secs_f32() < expiration as f32 => Ok(()),
        _ => Err(ImageError::Expired(String::from(
            "image is already expired",
        ))),
    }
}

async fn get_aws_img(
    s3_client: s3::Client,
    bucket_name: &str,
    img_key: &str,
) -> Result<axum::body::Bytes, ImageError> {
    let aws_img = s3_client
        .get_object()
        .bucket(bucket_name)
        .key(&*img_key)
        .send()
        .await?;
    let img_bytes = aws_img.body.collect().await?.into_bytes();
    Ok(img_bytes)
}

fn transform_img(orig_img: Bytes, params: ImgParams) -> Result<ImgResult, ImageError> {
    let wand = MagickWand::new();
    let _ = wand.read_image_blob(orig_img);

    let orig_width = wand.get_image_width() as f32;
    let orig_height = wand.get_image_height() as f32;
    let aspect_ratio = orig_width as f32 / orig_height as f32;
    let (new_width, new_height) = match (params.w, params.h) {
        (Some(w), None) => (w as f32, w as f32 / aspect_ratio),
        (None, Some(h)) => (h as f32 * aspect_ratio, h as f32),
        (Some(w), Some(h)) => (w as f32, h as f32),
        (None, None) => (orig_width, orig_height),
    };

    let _ = wand.adaptive_resize_image(new_width.round() as usize, new_height.round() as usize);

    if params.enhance.unwrap_or(false) {
        let _ = wand.auto_level();
        let _ = wand.auto_gamma();
    }
    if params.blur.unwrap_or(false) {
        let _ = wand.blur_image(20.0, 10.0);
    }

    let format = params.format.unwrap_or(ImgFormat::JPEG);
    wand.write_image_blob(&format.to_string())
        .map_err(ImageError::MagickWandError)
        .map(|img_bytes| ImgResult { img_bytes, format })
}

fn handle_img_response(ImgResult { img_bytes, format }: ImgResult) -> impl IntoResponse {
    let content_type = format!("image/{}", format.to_string());
    (
        ([(axum::http::header::CONTENT_TYPE, content_type.clone())]),
        axum::response::AppendHeaders([(axum::http::header::CONTENT_TYPE, content_type.clone())]),
        img_bytes,
    )
}

fn handle_img_err(err: ImageError) -> impl IntoResponse {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(
            serde_json::json!({"err": "an error occured while processing image", "msg": err.get_err_message()}),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_signature() {
        let raw_query_param = "foo=1&bar=2".to_string();
        let img_key = "xyz".to_string();
        let secret_salt = "abcd".to_string();
        // md5 of "abcd/xyz?foo=1&bar=2"
        let correct_encrypted_key: String = "d4459a3f3836da68fdf27d933a7e7f5d".to_string();
        let wrong_encrypted_key: String = "ijkl".to_string();

        assert!(validate_signature(
            &raw_query_param,
            &img_key,
            &secret_salt,
            &correct_encrypted_key
        )
        .is_ok());
        assert!(validate_signature(
            &raw_query_param,
            &img_key,
            &secret_salt,
            &wrong_encrypted_key
        )
        .is_err());
    }

    use std::time::Duration;
    #[test]
    fn test_validate_expiration() {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let in_future = (now + Duration::from_secs(1000)).as_secs_f32();
        let in_past = (now - Duration::from_secs(1000)).as_secs_f32();

        assert!(validate_expiration(in_future).is_ok());
        assert!(validate_expiration(in_past).is_err());
    }
}
