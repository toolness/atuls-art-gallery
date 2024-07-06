use std::fmt::Display;

use crate::gallery_cache::GalleryCache;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

const ROOT_CACHE_SUBDIR: &'static str = "met-api";

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

pub fn load_met_api_record(cache: &GalleryCache, object_id: u64) -> Result<MetObjectApiRecord> {
    let filename = format!("{ROOT_CACHE_SUBDIR}/object-{}.json", object_id);
    cache.cache_json_url(
        format!(
            "https://collectionapi.metmuseum.org/public/collection/v1/objects/{}",
            object_id
        ),
        &filename,
    )?;
    match serde_json::from_str(&cache.load_cached_string(&filename)?) {
        Ok(record) => Ok(record),
        Err(err) => Err(anyhow!("Failed to load {}: {}", filename, err)),
    }
}

#[derive(Debug, Deserialize)]
pub struct MetObjectApiRecord {
    pub measurements: Option<Vec<MetObjectApiMeasurements>>,

    #[serde(rename = "primaryImageSmall")]
    pub primary_image_small: String,

    #[serde(rename = "primaryImage")]
    pub primary_image: String,

    #[serde(rename = "objectDate")]
    pub object_date: String,

    #[serde(rename = "objectID")]
    pub object_id: u64,

    pub title: String,
}

impl MetObjectApiRecord {
    /// Returns physical dimensions in meters, *not* pixels.
    pub fn overall_width_and_height(&self) -> Option<(f64, f64)> {
        let Some(measurements) = &self.measurements else {
            return None;
        };
        for measurement in measurements {
            if &measurement.element_name == "Overall" {
                if let (Some(width), Some(height), None) = (
                    measurement.element_measurements.width,
                    measurement.element_measurements.height,
                    measurement.element_measurements.depth,
                ) {
                    // Convert centimeters to meters.
                    return Some((width / 100.0, height / 100.0));
                }
            }
        }
        None
    }

    /// Try to download & cache the an image of the object if it's 2D artwork.
    ///
    /// If it's in the cache, returns the cached version. Otherwise, downloads and adds
    /// to cache.
    ///
    /// Returns (width, height, filename) on success. Dimensions are in physical meters, *not* pixels.
    pub fn try_to_download_image(
        &self,
        cache: &GalleryCache,
        size: ImageSize,
    ) -> Result<Option<(f64, f64, String)>> {
        if let Some((width, height)) = self.overall_width_and_height() {
            let image_url = match size {
                ImageSize::Small => &self.primary_image_small,
                ImageSize::Large => &self.primary_image,
            };
            if image_url.ends_with(".jpg") {
                let image_filename =
                    format!("{ROOT_CACHE_SUBDIR}/object-{}-{size}.jpg", self.object_id);
                cache.cache_binary_url(&image_url, &image_filename)?;
                return Ok(Some((width, height, image_filename)));
            }
        }
        Ok(None)
    }
}

#[derive(Debug, Deserialize)]
pub struct MetObjectApiMeasurements {
    #[serde(rename = "elementName")]
    element_name: String,

    #[serde(rename = "elementMeasurements")]
    element_measurements: MetObjectApiElementMeasurements,
}

#[derive(Debug, Deserialize)]
pub struct MetObjectApiElementMeasurements {
    #[serde(rename = "Width")]
    width: Option<f64>,

    #[serde(rename = "Height")]
    height: Option<f64>,

    #[serde(rename = "Depth")]
    depth: Option<f64>,
}
