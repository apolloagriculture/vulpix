use aws_sdk_s3 as s3;
use axum::{
    body::Bytes,
    extract::{Path, Query, RawQuery, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use magick_rust::MagickWand;
use md5;
use std::time::{SystemTime, UNIX_EPOCH};

mod errors;
mod img_result;
mod params;
pub mod state;

use self::{errors::ImageError, img_result::ImgResult, params::ImgParams, state::ImgState};

pub async fn handle_img(
    State(ImgState {
        secret_salt,
        bucket_name,
        cache_bucket_name,
    }): State<ImgState>,
    Extension(s3_client): Extension<s3::Client>,
    Path(img_key): Path<String>,
    Query(params): Query<ImgParams>,
    RawQuery(query): RawQuery,
) -> impl IntoResponse {
    let validation = validate_params(
        &params,
        &query.unwrap_or(String::from("")),
        &img_key,
        secret_salt,
    );

    let img_res = match validation {
        Ok(()) => {
            get_or_cache_img(
                &s3_client,
                bucket_name,
                cache_bucket_name,
                &img_key,
                &params,
            )
            .await
        }
        Err(err) => Err(err),
    };

    match img_res {
        Ok(img) => handle_img_response(img).into_response(),
        Err(err) => handle_img_err(err).into_response(),
    }
}

async fn get_or_cache_img(
    s3_client: &s3::Client,
    bucket_name: &str,
    cache_bucket_name: &str,
    img_key: &str,
    params: &ImgParams,
) -> Result<ImgResult, ImageError> {
    let cached_key = format!("{}{}", img_key, params.cacheable_param_key());
    let cached_img = get_aws_img(s3_client, cache_bucket_name, &cached_key).await;

    match cached_img {
        Ok(img) => Ok(ImgResult {
            img_bytes: img.to_vec(),
            format: params.format.clone().unwrap_or_default(),
        }),
        Err(_) => {
            transform_cache_img(
                s3_client,
                bucket_name,
                cache_bucket_name,
                img_key,
                &cached_key,
                &params,
            )
            .await
        }
    }
}

async fn transform_cache_img(
    s3_client: &s3::Client,
    bucket_name: &str,
    cache_bucket_name: &str,
    img_key: &str,
    cached_key: &str,
    params: &ImgParams,
) -> Result<ImgResult, ImageError> {
    let s3_img = get_aws_img(&s3_client, bucket_name, &img_key).await;

    match s3_img {
        Ok(img) => {
            let cloned_img = img.clone();
            let cloned_s3_client = s3_client.clone();
            let cloned_cache_bucket_name = cache_bucket_name.to_owned();
            let cloned_cached_key = cached_key.to_owned();

            tokio::spawn(async move {
                save_aws_img(
                    &cloned_s3_client,
                    &cloned_cache_bucket_name,
                    &cloned_cached_key,
                    img,
                )
                .await
            });
            transform_img(cloned_img, params)
        }
        Err(err) => Err(err),
    }
}

fn validate_params(
    params: &ImgParams,
    raw_query_param: &str,
    img_key: &str,
    secret_salt: &str,
) -> Result<(), ImageError> {
    validate_signature(raw_query_param, img_key, secret_salt, &params.s)
        .and_then(|_| validate_expiration(params.expires))
}

fn validate_signature(
    raw_query_param: &str,
    img_key: &str,
    secret_salt: &str,
    encrypted_key: &str,
) -> Result<(), ImageError> {
    let raw_query_param_without_encryption = raw_query_param
        .replace(&format!("&s={}", encrypted_key), "")
        .replace(&format!("?s={}", encrypted_key), "");
    let md5_digest = md5::compute(format!(
        "{}/{}?{}",
        secret_salt, img_key, raw_query_param_without_encryption
    ));
    if format!("{:x}", md5_digest) == encrypted_key {
        Ok(())
    } else {
        Err(ImageError::EncryptionInvalid)
    }
}

fn validate_expiration(expiration: f32) -> Result<(), ImageError> {
    let current_epech_in_seconds = SystemTime::now().duration_since(UNIX_EPOCH);
    match current_epech_in_seconds {
        Ok(t) if t.as_secs_f32() < expiration as f32 => Ok(()),
        _ => Err(ImageError::Expired),
    }
}

async fn get_aws_img(
    s3_client: &s3::Client,
    bucket_name: &str,
    img_key: &str,
) -> Result<Bytes, ImageError> {
    let aws_img = s3_client
        .get_object()
        .bucket(bucket_name)
        .key(img_key)
        .send()
        .await?;
    let img_bytes = aws_img.body.collect().await?.into_bytes();
    Ok(img_bytes)
}

async fn save_aws_img(
    s3_client: &s3::Client,
    bucket_name: &str,
    img_key: &str,
    body: Bytes,
) -> Result<(), ImageError> {
    let body_stream = s3::primitives::ByteStream::from(body);
    let _ = s3_client
        .put_object()
        .bucket(bucket_name)
        .key(img_key)
        .body(body_stream)
        .send()
        .await?;
    Ok(())
}

fn transform_img(orig_img: Bytes, params: &ImgParams) -> Result<ImgResult, ImageError> {
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

    if params.sharpen.unwrap_or(false) {
        let _ = wand.sharpen_image(0.0, 10.0);
    }

    let format = params.format.clone().unwrap_or_default();
    wand.write_image_blob(&format!("{}", format))
        .map_err(ImageError::MagickWandError)
        .map(|img_bytes| ImgResult { img_bytes, format })
}

fn handle_img_response(ImgResult { img_bytes, format }: ImgResult) -> impl IntoResponse {
    let content_type = format!("image/{}", format);
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
            serde_json::json!({"err": "an error occured while processing image", "msg": format!("{}", err)}),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_signature() {
        let raw_query_param = "foo=1&bar=2";
        let img_key = "xyz";
        let secret_salt = "abcd";
        // md5 of "abcd/xyz?foo=1&bar=2"
        let correct_key = "d4459a3f3836da68fdf27d933a7e7f5d";
        let wrong_key = "ijkl";

        assert!(validate_signature(raw_query_param, img_key, secret_salt, correct_key).is_ok());
        assert!(validate_signature(raw_query_param, img_key, secret_salt, wrong_key).is_err());
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
