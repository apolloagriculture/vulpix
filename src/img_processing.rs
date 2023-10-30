use async_trait::async_trait;
use magick_rust::{MagickWand, magick_wand_genesis};

pub mod errors;
pub mod img_result;
pub mod params;
pub mod bounding_box;

use self::{errors::ImageError, img_result::ImgResult, params::ImgParams, bounding_box::BoundingBox};

#[async_trait]
pub trait ImageAccess
where
    Self: Clone + std::marker::Send + 'static,
{
    async fn get_img(self, tag: &str, key: &str) -> Result<Vec<u8>, ImageError>;
   
    async fn save_img(self, tag: &str, key: &str, body: Vec<u8>) -> Result<(), ImageError>;

    async fn recog_face(self, tag: &str, key: &str) -> Option<BoundingBox>;
}

pub fn init() {
    magick_wand_genesis();
}

pub async fn handle_img(
    image_access: impl ImageAccess,
    tag_name: &str,
    cache_tag_name: &str,
    img_key: &str,
    cached_key: &str,
    params: &ImgParams,
) -> Result<ImgResult, ImageError> {
    let cached_img = image_access.clone().get_img(&cache_tag_name, &cached_key).await;

    match cached_img {
        Ok(img) => Ok(ImgResult {
            img_bytes: img.to_vec(),
            format: params.format.clone().unwrap_or_default(),
        }),
        Err(_) => {
            transform_cache_img(
                image_access,
                &tag_name,
                &cache_tag_name,
                &img_key,
                &cached_key,
                &params,
            )
            .await
        }
    }
}

async fn transform_cache_img(
    image_access: impl ImageAccess,
    tag_name: &str,
    cache_tag_name: &str,
    img_key: &str,
    cached_key: &str,
    params: &ImgParams,
) -> Result<ImgResult, ImageError> {
    let s3_img = image_access.clone().get_img(tag_name, &img_key).await?;
    let face_bounding_box = if params.facecrop.unwrap_or(false) {
        image_access.clone().recog_face(tag_name, &img_key).await
    } else {
        None
    };

    let img_result = transform_img(s3_img, params, face_bounding_box)?;

    let cloned_image_access = image_access.clone();
    let cloned_img = img_result.clone().img_bytes;
    let cloned_cache_tag_name = cache_tag_name.to_owned();
    let cloned_cached_key = cached_key.to_owned();

    tokio::spawn(async move {
        cloned_image_access
            .save_img(
                &cloned_cache_tag_name,
                &cloned_cached_key,
                cloned_img.into(),
            )
            .await
    });

    Ok(img_result)
}

fn transform_img(
    orig_img: Vec<u8>,
    params: &ImgParams,
    face_bounding_box: Option<BoundingBox>,
) -> Result<ImgResult, ImageError> {
    let wand = MagickWand::new();
    let _ = wand.read_image_blob(orig_img);

    let orig_width = wand.get_image_width() as f32;
    let orig_height = wand.get_image_height() as f32;
    let orig_aspect_ratio = orig_width as f32 / orig_height as f32;

    if let Some(bounding_box) = face_bounding_box {
        let box_w: f32 = bounding_box.width.unwrap_or(1.0) * orig_width;
        let box_h: f32 = bounding_box.height.unwrap_or(1.0) * orig_height;

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
        let padded_x = (bounding_box.left.unwrap_or(0.0) * orig_width)
            - ((padding - 1.0) * adjusted_w / 2.0)
            - (adjusted_w - box_w) / 1.5;
        let padded_y = (bounding_box.top.unwrap_or(0.0) * orig_height)
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
