use aws_sdk_rekognition as rek;
use aws_sdk_s3 as s3;
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use magick_rust::MagickWand;
use rek::types::BoundingBox;

mod errors;
mod img_result;
mod params;
pub mod state;

use self::{errors::ImageError, img_result::ImgResult, params::ImgParams, state::BucketConfig};

pub async fn handle_img(
    State(BucketConfig {
        bucket_name,
        cache_bucket_name,
    }): State<BucketConfig>,
    Extension(s3_client): Extension<s3::Client>,
    Extension(rek_client): Extension<rek::Client>,
    Path(img_key): Path<String>,
    Query(params): Query<ImgParams>,
) -> impl IntoResponse {
    let img_res = get_or_cache_img(
        &s3_client,
        rek_client,
        bucket_name,
        cache_bucket_name,
        &img_key,
        params,
    )
    .await;

    match img_res {
        Ok(img) => handle_img_response(img).into_response(),
        Err(err) => handle_img_err(err).into_response(),
    }
}

async fn get_or_cache_img(
    s3_client: &s3::Client,
    rek_client: rek::Client,
    bucket_name: &str,
    cache_bucket_name: &str,
    img_key: &str,
    params: ImgParams,
) -> Result<ImgResult, ImageError> {
    let cached_key = format!("{:x}/{}", params.cacheable_param_key(), img_key);
    let cached_img = get_aws_img(s3_client, cache_bucket_name, &cached_key).await;

    match cached_img {
        Ok(img) => Ok(ImgResult {
            img_bytes: img.to_vec(),
            format: params.format.clone().unwrap_or_default(),
        }),
        Err(_) => {
            transform_cache_img(
                s3_client,
                rek_client,
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
    rek_client: rek::Client,
    bucket_name: &str,
    cache_bucket_name: &str,
    img_key: &str,
    cached_key: &str,
    params: &ImgParams,
) -> Result<ImgResult, ImageError> {
    let s3_img = get_aws_img(&s3_client, bucket_name, &img_key).await?;
    let face_bounding_box = if params.facecrop.unwrap_or(false) {
        rek_face(rek_client, bucket_name, &img_key).await
    } else {
        None
    };

    let img_result = transform_img(s3_img, params, face_bounding_box)?;

    let cloned_img = img_result.clone().img_bytes;
    let cloned_s3_client = s3_client.clone();
    let cloned_cache_bucket_name = cache_bucket_name.to_owned();
    let cloned_cached_key = cached_key.to_owned();

    tokio::spawn(async move {
        save_aws_img(
            &cloned_s3_client,
            &cloned_cache_bucket_name,
            &cloned_cached_key,
            cloned_img.into(),
        )
        .await
    });

    Ok(img_result)
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

fn transform_img(
    orig_img: Bytes,
    params: &ImgParams,
    face_bounding_box: Option<BoundingBox>,
) -> Result<ImgResult, ImageError> {
    let wand = MagickWand::new();
    let _ = wand.read_image_blob(orig_img);

    let orig_width = wand.get_image_width() as f32;
    let orig_height = wand.get_image_height() as f32;
    let orig_aspect_ratio = orig_width as f32 / orig_height as f32;

    if let Some(bounding_box) = face_bounding_box {
        let box_w: f32 = bounding_box.width().unwrap_or(1.0) * orig_width;
        let box_h: f32 = bounding_box.height().unwrap_or(1.0) * orig_height;

        let desired_aspect_ratio = params.w.unwrap_or(orig_width) / params.h.unwrap_or(orig_height);

        let (mut adjusted_w, mut adjusted_h) = if box_w > box_h {
            (box_w, box_w / desired_aspect_ratio)
        } else {
            (box_h * desired_aspect_ratio, box_h)
        };
        if adjusted_w > orig_width {
            adjusted_w = orig_width
        };
        if adjusted_h > orig_height {
            adjusted_h = orig_height
        };

        let padding = params.facepad.unwrap_or(1.0);
        let padded_w: f32 = adjusted_w * padding;
        let padded_h: f32 = adjusted_h * padding;
        let padded_x = (bounding_box.left().unwrap_or(0.0) * orig_width)
            - ((padding - 1.0) * adjusted_w / 2.0)
            - (adjusted_w - box_w) / 1.5;
        let padded_y = (bounding_box.top().unwrap_or(0.0) * orig_height)
            - ((padding - 1.0) * adjusted_h / 2.0)
            - (adjusted_h - box_h) / 1.5;

        let _ = wand.crop_image(
            padded_w as usize,
            padded_h as usize,
            padded_x as isize,
            padded_y as isize,
        );
    };

    let (new_width, new_height) = match (params.w, params.h) {
        (Some(w), None) => (w, w / orig_aspect_ratio),
        (None, Some(h)) => (h * orig_aspect_ratio, h),
        (Some(w), Some(h)) => (w, h),
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

async fn rek_face(
    rek_client: rek::Client,
    bucket_name: &str,
    img_key: &str,
) -> Option<BoundingBox> {
    let s3_obj = rek::types::S3Object::builder()
        .bucket(bucket_name)
        .name(img_key)
        .build();

    let s3_img = rek::types::Image::builder().s3_object(s3_obj).build();

    let rek_resp = rek_client
        .detect_faces()
        .image(s3_img)
        .attributes(aws_sdk_rekognition::types::Attribute::All)
        .send()
        .await;

    rek_resp.ok().and_then(|f| {
        f.face_details()
            .unwrap_or_default()
            .first()
            .and_then(|f| f.bounding_box())
            .cloned()
    })
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
