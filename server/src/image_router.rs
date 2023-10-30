use aws_sdk_rekognition as rek;
use aws_sdk_s3 as s3;
use axum::{
    extract::{Path, Query, State},
    routing::get,
    Extension, Router,
};
use vulpix::params::ImgParams;

use crate::{
    app_error::AppError, image_access::AwsImageAccess, image_result::ImageResult,
    settings::ImgSource, state::BucketConfig,
};

pub fn create_image_router(
    img_sources: Vec<ImgSource>,
    s3_client: aws_sdk_s3::Client,
    rek_client: aws_sdk_rekognition::Client,
) -> Router {
    let mut img_router = Router::new();

    for img_source in img_sources.iter() {
        img_router = img_router.route(
            &format!("/{}/*img_key", img_source.path.clone()),
            get(handle_img).with_state(BucketConfig {
                bucket_name: img_source.bucket.clone(),
                cache_bucket_name: img_source.cache_bucket.clone(),
            }),
        );
    }

    img_router
        .layer(Extension(s3_client))
        .layer(Extension(rek_client))
}

async fn handle_img(
    State(BucketConfig {
        bucket_name,
        cache_bucket_name,
    }): State<BucketConfig>,
    Extension(s3_client): Extension<s3::Client>,
    Extension(rek_client): Extension<rek::Client>,
    Path(img_key): Path<String>,
    Query(params): Query<ImgParams>,
) -> Result<ImageResult, AppError> {
    let cached_key = format!("{:x}/{}", params.cacheable_param_key(), img_key);
    let image_access = AwsImageAccess {
        s3_client: s3_client,
        rek_client: rek_client,
    };
    let image_bytes = vulpix::handle_img(
        image_access,
        &bucket_name,
        &cache_bucket_name,
        &img_key,
        &cached_key,
        &params,
    )
    .await?;

    Ok(ImageResult {
        image_bytes,
        format: params.format.clone().unwrap_or_default(),
    })
}
