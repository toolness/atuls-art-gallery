use std::fs::create_dir_all;

use crate::{
    gallery_cache::GalleryCache,
    image::{cache_image, get_supported_image_ext, ImageSize},
};
use anyhow::{anyhow, Result};
use serde::Deserialize;

const ROOT_CACHE_SUBDIR: &'static str = "met-api";

pub fn migrate_met_api_cache(cache: &GalleryCache) -> Result<()> {
    let mut created_subdir = false;
    let root_cache_subdir = cache.get_cached_path(ROOT_CACHE_SUBDIR);
    for entry_result in std::fs::read_dir(cache.cache_dir())? {
        let entry = entry_result?;
        let path = entry.path();
        if path.is_file() {
            let os_filename = entry.file_name();
            let filename = os_filename.to_string_lossy();
            if filename.starts_with("object-")
                && (filename.ends_with(".json") || filename.ends_with(".jpg"))
            {
                if !created_subdir {
                    println!(
                        "Migrating met api cache files into {}.",
                        root_cache_subdir.display()
                    );
                    create_dir_all(root_cache_subdir.clone())?;
                    created_subdir = true;
                }
                let dest_path = root_cache_subdir.join(filename.as_ref());
                if let Err(err) = std::fs::rename(path.clone(), dest_path.clone()) {
                    eprintln!(
                        "Unable to move {} to {}: {err:?}",
                        path.display(),
                        dest_path.display()
                    );
                }
            }
        }
    }
    Ok(())
}

pub fn load_met_api_record(cache: &GalleryCache, object_id: i64) -> Result<MetObjectApiRecord> {
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

/// Historical note: I used to extract measurements out of this and use them, but
/// then I realized that the measurements could be parsed from the original
/// Met CSV. Furthermore, not all met objects (e.g. 389607) even _have_ measurements
/// specified as structured data, rendering them even less useful.
#[derive(Debug, Deserialize)]
pub struct MetObjectApiRecord {
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
    /// Try to download & cache the an image of the object if it's 2D artwork.
    ///
    /// If it's in the cache, returns the cached version. Otherwise, downloads and adds
    /// to cache.
    ///
    /// Returns filename on success.
    pub fn try_to_download_image(
        &self,
        cache: &GalleryCache,
        size: ImageSize,
    ) -> Result<Option<String>> {
        let image_url = match size {
            ImageSize::Small => &self.primary_image_small,
            ImageSize::Large => &self.primary_image,
        };
        if let Some(ext) = get_supported_image_ext(image_url) {
            let image_filename =
                format!("{ROOT_CACHE_SUBDIR}/object-{}-{size}{ext}", self.object_id);
            cache_image(cache, image_url, &image_filename, ext)?;
            return Ok(Some(image_filename));
        }
        Ok(None)
    }
}
