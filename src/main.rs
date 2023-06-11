use aws_sdk_s3 as s3;
use axum::{
    body::Bytes,
    extract::{Path, Query, RawQuery, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use magick_rust::{magick_wand_genesis, MagickError, MagickWand};
use md5;
use s3::{error::SdkError, operation::get_object::GetObjectError, primitives::ByteStreamError};
use serde::{de, Deserialize, Deserializer};
use std::{
    fmt,
    net::SocketAddr,
    str::FromStr,
    sync::Once,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone)]
struct ImgState {
    s3_client: s3::Client,
    secret_salt: String,
    bucket_name: String,
}

#[tokio::main]
async fn main() {
    let aws_configuration = aws_config::load_from_env().await;
    let s3_client = s3::Client::new(&aws_configuration);
    static START: Once = Once::new();
    START.call_once(|| {
        magick_wand_genesis();
    });

    static BUCKET_NAME: &str = "";
    static SECRET_SALT: &str = "";

    let routes: Router = Router::new()
        .route("/", get(index))
        .nest(
            "/img",
            image_router().with_state(ImgState {
                s3_client,
                bucket_name: BUCKET_NAME.to_string(),
                secret_salt: SECRET_SALT.to_string(),
            }),
        )
        .fallback(handler_404);

    let addr: SocketAddr = SocketAddr::from(([127, 0, 0, 1], 6060));
    println!("Server running on {}\n", addr);

    axum::Server::bind(&addr)
        .serve(routes.into_make_service())
        .await
        .unwrap();
}

async fn index() -> impl IntoResponse {
    (StatusCode::OK, "welcome to vulpix")
}

async fn handler_404() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "nothing to see here")
}

// img module

fn image_router() -> Router<ImgState> {
    Router::new().route("/*img_key", get(handle_img))
}

#[derive(Debug, Deserialize)]
enum ImgFormat {
    #[serde(alias = "png")]
    PNG,
    #[serde(alias = "jpeg", alias = "jpg", alias = "JPG")]
    JPEG,
}

impl fmt::Display for ImgFormat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ImgFormat::PNG => write!(f, "png"),
            ImgFormat::JPEG => write!(f, "jpeg"),
        }
    }
}
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ImgParams {
    w: Option<usize>,
    h: Option<usize>,
    format: Option<ImgFormat>,
    #[serde(default, deserialize_with = "empty_string_as_true")]
    blur: Option<bool>,
    #[serde(default, deserialize_with = "empty_string_as_true")]
    enhance: Option<bool>,
    s: Option<String>,
    expires: Option<f32>,
}

fn empty_string_as_true<'de, D>(de: D) -> Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(de)?;
    match opt.as_deref() {
        None | Some("") => Ok(Some(true)),
        Some(s) => FromStr::from_str(s).map_err(de::Error::custom).map(Some),
    }
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
    let validation = match (&params.s, params.expires) {
        (Some(encrypted_key), Some(expires)) => validate_signature(
            query.unwrap_or("".to_string()),
            &img_key,
            &secret_salt,
            encrypted_key,
        )
        .and_then(|_| validate_expiration(expires)),
        (Some(encrypted_key), None) => validate_signature(
            query.unwrap_or("".to_string()),
            &img_key,
            &secret_salt,
            encrypted_key,
        ),
        (None, Some(expires)) => validate_expiration(expires),
        (None, None) => Ok(()),
    };

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

fn validate_signature(
    raw_query_param: String,
    img_key: &String,
    secret_salt: &String,
    encrypted_key: &String,
) -> Result<(), ImageError> {
    let raw_query_param_without_encryption =
        raw_query_param.replace(&format!("&s={}", encrypted_key), "");
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

fn transform_img(orig_img: Bytes, params: ImgParams) -> Result<ImgResult, ImageError> {
    let format = params.format.unwrap_or(ImgFormat::JPEG);
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

struct ImgResult {
    img_bytes: Vec<u8>,
    format: ImgFormat,
}

#[derive(Debug)]
enum ImageError {
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
    fn get_err_message(&self) -> String {
        match self {
            ImageError::AwsError(e) => e.to_string(),
            ImageError::StreamError(e) => e.to_string(),
            ImageError::MagickWandError(e) => e.to_string(),
            ImageError::EncryptionInvalid(s) => s.to_string(),
            ImageError::Expired(s) => s.to_string(),
        }
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
