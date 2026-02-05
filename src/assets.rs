use anyhow::Result;
use image::{self, DynamicImage, GrayImage, ImageError};
use imageproc::contrast::adaptive_threshold;
use std::path::PathBuf;

const BLOCK_RADIUS: u32 = 4;
const DELTA: i32 = 5;

pub fn apply_threshold(img: &DynamicImage) -> Result<GrayImage, ImageError> {
    let luma = img.to_luma8();
    Ok(adaptive_threshold(&luma, BLOCK_RADIUS, DELTA))
}

pub struct AssetTemplate {
    pub image: GrayImage,
    pub width: u32,
    pub height: u32,
}

impl AssetTemplate {
    pub fn from_file(file_name: &str) -> Result<Self> {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("src/assets/{}", file_name));
        let image = apply_threshold(&image::open(path)?)?;
        image.save(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(format!("after-threshold/{}", file_name)),
        )?;
        let (width, height) = image.dimensions();
        Ok(Self {
            image,
            width,
            height,
        })
    }
}
