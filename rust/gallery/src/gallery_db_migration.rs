use anyhow::Result;
use rusqlite::Connection;

use crate::{
    gallery_cache::GalleryCache,
    gallery_db::{
        get_default_gallery_db_filename, get_gallery_db_filename, GalleryDb,
        LATEST_GALLERY_DB_VERSION,
    },
};

const OLDEST_SUPPORTED_GALLERY_DB_VERSION_TO_TRIVIALLY_MIGRATE: usize = 5;

pub fn migrate_gallery_db(cache: &GalleryCache) -> Result<bool> {
    for version in
        (OLDEST_SUPPORTED_GALLERY_DB_VERSION_TO_TRIVIALLY_MIGRATE..LATEST_GALLERY_DB_VERSION).rev()
    {
        let from_db_path = cache.get_cached_path(get_gallery_db_filename(version));
        if from_db_path.exists() {
            // Rather than migrating the database schema, which is
            // how migrations conventionally work, we're going to pull the
            // small amount of user data that we want to migrate out of the
            // old DB and into the new DB.
            let to_db_path = cache.get_cached_path(get_default_gallery_db_filename());
            println!(
                "Migrating layout records from {} to {}.",
                from_db_path.display(),
                to_db_path.display()
            );
            let mut from_db = GalleryDb::new(Connection::open(from_db_path)?);
            let layout_records = from_db.get_layout_records_in_non_positive_galleries()?;
            println!("Found {} layout records to migrate.", layout_records.len());
            let mut to_db = GalleryDb::new(Connection::open(to_db_path)?);
            to_db.upsert_layout_records(&layout_records)?;
            println!("Migrated {} layout records.", layout_records.len());
            // TODO: Delete the old DB since we don't need it anymore?
            return Ok(true);
        }
    }
    Ok(false)
}
