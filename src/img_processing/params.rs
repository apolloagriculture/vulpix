use serde::{de, Deserialize, Deserializer};
use std::{fmt, str::FromStr};

#[derive(Debug, Deserialize, Clone)]
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

#[derive(Debug, Deserialize)]
pub struct ImgParams {
    pub w: Option<usize>,
    pub h: Option<usize>,
    pub format: Option<ImgFormat>,
    #[serde(default, deserialize_with = "empty_string_as_true")]
    pub blur: Option<bool>,
    #[serde(default, deserialize_with = "empty_string_as_true")]
    pub sharpen: Option<bool>,
    #[serde(default, deserialize_with = "empty_string_as_true")]
    pub enhance: Option<bool>,
    pub s: String,
    pub expires: f32,
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
    pub fn cacheable_param_key(&self) -> String {
        format!(
            "w={:?}&h={:?}&format={:?}&blur={:?}&sharpen={:?}&enhance={:?}",
            self.w.unwrap_or_default(),
            self.h.unwrap_or_default(),
            self.format.clone().unwrap_or_default(),
            self.blur.unwrap_or_default(),
            self.sharpen.unwrap_or_default(),
            self.enhance.unwrap_or_default()
        )
    }
}
