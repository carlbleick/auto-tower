use anyhow::Result;
use image;
use std::path::PathBuf;

pub struct AssetTemplate {
    pub buf: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl AssetTemplate {
    pub fn from_file(file_name: &str) -> Result<Self> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("src/assets/{}", file_name));
        let img = image::open(path)?.to_luma8();
        let (width, height) = img.dimensions();
        let buf = img.into_raw();
        Ok(Self { buf, width, height })
    }
}
