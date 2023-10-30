use std::{error::Error, fmt::Display};

use anyhow::Result;
use async_trait::async_trait;
use aws_sdk_rekognition as rek;
use aws_sdk_s3 as s3;

use vulpix::{bounding_box::BoundingBox, ImageAccess};

#[derive(Clone)]
pub struct AwsImageAccess {
    pub s3_client: s3::Client,
    pub rek_client: rek::Client,
}

#[derive(Debug)]
struct AwsError {
    tag: String,
    message: String,
}

impl Display for AwsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "error occurred while {} \n {}", self.tag, self.message)
    }
}

impl Error for AwsError {}

#[async_trait]
impl ImageAccess for AwsImageAccess {
    async fn get_img(self, tag: &str, key: &str) -> Result<Vec<u8>> {
        let aws_img = self
            .s3_client
            .get_object()
            .bucket(tag)
            .key(key)
            .send()
            .await
            .map_err(|err| AwsError {
                tag: "aws read file".into(),
                message: format!("{:?}", err),
            })?
            .body;
        let img_bytes = aws_img
            .collect()
            .await
            .map_err(|err| AwsError {
                tag: "aws byte stream conversion".into(),
                message: format!("{:?}", err),
            })?
            .into_bytes()
            .to_vec();
        Ok(img_bytes)
    }

    async fn save_img(self, tag: &str, key: &str, body: Vec<u8>) -> Result<()> {
        let body_stream = s3::primitives::ByteStream::from(body);
        let _ = self
            .s3_client
            .put_object()
            .bucket(tag)
            .key(key)
            .body(body_stream)
            .send()
            .await
            .map_err(|err| AwsError {
                tag: "aws save file".into(),
                message: format!("{:?}", err.to_string()),
            })?;
        Ok(())
    }

    async fn recog_face(self, tag: &str, key: &str) -> Option<BoundingBox> {
        let s3_obj = rek::types::S3Object::builder()
            .bucket(tag)
            .name(key)
            .build();
        let s3_img = rek::types::Image::builder().s3_object(s3_obj).build();
        let rek_resp = &self
            .rek_client
            .detect_faces()
            .image(s3_img)
            .attributes(aws_sdk_rekognition::types::Attribute::All)
            .send()
            .await;
        rek_resp.as_ref().ok().and_then(|f| {
            f.face_details()
                .unwrap_or_default()
                .first()
                .and_then(|f| f.bounding_box())
                .cloned()
                .map(|value| BoundingBox {
                    width: value.width(),
                    height: value.height(),
                    left: value.left(),
                    top: value.top(),
                })
        })
    }
}