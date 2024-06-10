use crate::gallery_cache::GalleryCache;
use anyhow::{anyhow, Result};
use serde::Deserialize;

pub fn load_met_api_record(cache: &GalleryCache, object_id: u64) -> Result<MetObjectApiRecord> {
    let filename = format!("object-{}.json", object_id);
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

    #[serde(rename = "objectDate")]
    pub object_date: String,

    #[serde(rename = "objectID")]
    pub object_id: u64,

    pub title: String,
}

impl MetObjectApiRecord {
    /// Returns physical dimensions, *not* pixels.
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
                    return Some((width, height));
                }
            }
        }
        None
    }

    /// Try to download & cache the small image of the object if it's 2D artwork.
    ///
    /// If it's in the cache, returns the cached version. Otherwise, downloads and adds
    /// to cache.
    ///
    /// Returns (width, height, filename) on success. Dimensions are physical, *not* pixels.
    pub fn try_to_download_small_image(
        &self,
        cache: &GalleryCache,
    ) -> Result<Option<(f64, f64, String)>> {
        if let Some((width, height)) = self.overall_width_and_height() {
            if self.primary_image_small.ends_with(".jpg") {
                let small_image = format!("object-{}-small.jpg", self.object_id);
                cache.cache_binary_url(&self.primary_image_small, &small_image)?;
                return Ok(Some((width, height, small_image)));
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
