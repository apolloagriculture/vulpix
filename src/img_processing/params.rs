use serde::{de, Deserialize, Deserializer, Serialize};
use std::{fmt, str::FromStr};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ImgFormat {
    #[serde(alias = "png")]
    Png,
    #[serde(alias = "jpeg", alias = "jpg")]
    Jpeg,
}

impl fmt::Display for ImgFormat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ImgFormat::Png => write!(f, "png"),
            ImgFormat::Jpeg => write!(f, "jpeg"),
        }
    }
}

impl Default for ImgFormat {
    fn default() -> Self {
        ImgFormat::Jpeg
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImgParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub w: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<ImgFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, deserialize_with = "empty_string_as_true")]
    pub blur: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, deserialize_with = "empty_string_as_true")]
    pub sharpen: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, deserialize_with = "empty_string_as_true")]
    pub enhance: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, deserialize_with = "empty_string_as_true")]
    pub facecrop: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facepad: Option<f32>
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

impl ImgParams {
    pub fn cacheable_param_key(&self) -> md5::Digest {
        md5::compute(format!("{}", serde_json::to_string(&self).unwrap()))
    }
}
