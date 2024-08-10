use std::{fmt::Display, path::PathBuf};

use crate::gallery_cache::{CacheResult, GalleryCache};
use anyhow::Result;
use image::{codecs::jpeg::JpegEncoder, ColorType, ImageReader};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Copy, Clone)]
pub enum ImageSize {
    Small,
    Large,
}

impl Display for ImageSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImageSize::Small => write!(f, "small"),
            ImageSize::Large => write!(f, "large"),
        }
    }
}

const JPG_EXT: &'static str = ".jpg";

const JPEG_EXT: &'static str = ".jpeg";

/// We only care about the ones Godot can import right now:
///
/// https://docs.godotengine.org/en/stable/tutorials/assets_pipeline/importing_images.html#supported-image-formats
const SUPPORTED_LOWERCASE_IMAGE_FORMATS: [&'static str; 4] = [JPG_EXT, JPEG_EXT, ".webp", ".png"];

fn is_jpeg(ext: &'static str) -> bool {
    return ext == JPG_EXT || ext == JPEG_EXT;
}

/// Returns the file extension for the given image filename, if it's a supported one.
///
/// The extension will be lowercased, and will include the leading period.
pub fn get_supported_image_ext(filename: &str) -> Option<&'static str> {
    let lowercase_filename = filename.to_lowercase();
    for format in SUPPORTED_LOWERCASE_IMAGE_FORMATS {
        if lowercase_filename.ends_with(format) {
            return Some(format);
        }
    }
    None
}

pub fn cache_image(
    cache: &GalleryCache,
    image_url: &str,
    image_filename: &str,
    ext: &'static str,
) -> Result<()> {
    if cache.cache_binary_url(&image_url, &image_filename)? == CacheResult::NewlyCached {
        let full_path = cache.get_cached_path(image_filename);
        maybe_convert_image_for_loading_in_godot(&full_path, ext)?;
    }
    Ok(())
}

pub fn maybe_convert_image_for_loading_in_godot(
    filename: &PathBuf,
    ext: &'static str,
) -> Result<bool> {
    if is_jpeg(ext) {
        let img = ImageReader::open(filename)?.decode()?;
        if img.color() == ColorType::L8 {
            println!("Converting L8 JPEG image {} to RGB8.", filename.display());
            let converted = img.into_rgb8();
            let outfile = std::fs::File::create(filename)?;
            // TODO: This kind of sucks because we're re-encoding the image in a lossy format.
            let encoder = JpegEncoder::new_with_quality(outfile, 95);
            converted.write_with_encoder(encoder)?;
            return Ok(true);
        }
    }
    Ok(false)
}
