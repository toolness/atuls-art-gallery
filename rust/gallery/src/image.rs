use std::fmt::Display;

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

/// We only care about the ones Godot can import right now:
///
/// https://docs.godotengine.org/en/stable/tutorials/assets_pipeline/importing_images.html#supported-image-formats
const SUPPORTED_LOWERCASE_IMAGE_FORMATS: [&'static str; 4] = [".jpg", ".jpeg", ".webp", ".png"];

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
