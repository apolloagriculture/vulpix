# Vulpix

> an image processing library

Vulpix is a tiny wrapper over [image magick](https://imagemagick.org/) using [magick_rust](https://crates.io/crates/magick_rust) wrapper.
It allows you process images quickly for regular tasks, like cropping, sharepenimg, brightening, chnaging formats easily. It also supports cropping on face.
A usage of this library can be found [here](https://github.com/apolloagriculture/vulpix), where vulpix is used to securely serve images from aws s3 and process them.

## installation

you need to have image magick v7 installed, check [imagemagick installation documentation](https://imagemagick.org/script/download.php)
for macos, you simply can.

```sh
brew install imagemagick
```

install vulpix in your rust project

```sh
cargo add vulpix
```

you would also need [async trait](https://github.com/dtolnay/async-trait) library

```sh
cargo add async-trait
```

## usage

initialise vulpix, you only need to do it once

```rs
vulpix::init();
```

implement ImageAceess async trait for your use case, this requires you to impletemnt 3 methods, for accesisng image, for saving an image (processed images as cache)
and detecting face in an image.

you can see a practical implementation of this trait using aws s3 and rekognation apis [here](https://github.com/apolloagriculture/vulpix/blob/main/server/src/image_access.rs)

```rs
use vulpix::{bounding_box::BoundingBox, ImageAccess};
use anyhow::Result;

struct MyImageRepo

#[async_trait]
impl ImageAccess for MyImageRepo {
    async fn get_img(self, tag: &str, key: &str) -> Result<Vec<u8>> {
      todo!("implement get_img")
    }

    async fn save_img(self, tag: &str, key: &str, body: Vec<u8>) -> Result<()> {
      todo!("implement save_img")
    }

    async fn recog_face(self, tag: &str, key: &str) -> Option<BoundingBox> {
      todo!("implement recog_face")
    }
}
```

finally we can process image by passing image params

```rs
use vulpix::params::{ImgParams, ImgFormat};

let image_params =
  ImgParams {
    w: Some(250), // image width
    h: Some(250), // image height
    format: Some(ImgFormat.Png), // image format
    blur: Some(false), // blur image
    sharpen: Some(true), // sharpen image
    enhance: Some(true), // enchance image
    facecrop: Some(true), // crops images on face if face bounding box is found
    facepad: Some(1.5), // padding around face while using facecrop
  }

let image_access = MyImageRepo {};

let processed_image_bytes = vulpix::handle_img(
      image_access,
      "my_bucket",
      "my_cache_bucket",
      "image_key",
      "cached_image_key",
      &image_params,
  )
  .await?
```
