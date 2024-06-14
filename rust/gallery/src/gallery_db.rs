use anyhow::Result;
use rusqlite::Connection;

pub struct GalleryDb {
    conn: Connection,
}

impl GalleryDb {
    pub fn new(conn: Connection) -> Self {
        GalleryDb { conn }
    }

    pub fn reset_layout_table(&mut self) -> Result<()> {
        let tx = self.conn.transaction()?;

        tx.execute("DROP TABLE IF EXISTS layout", ())?;
        // Note that conceptually, `met_object_id` is a foreign key to the met_objects
        // table, but we don't want to enforce a constraint because we want to
        // be able to blow away the met_objects table for re-importing if needed.
        tx.execute(
            "
            CREATE TABLE IF NOT EXISTS layout (
                gallery_id INTEGER NOT NULL,
                wall_id TEXT NOT NULL,
                met_object_id INTEGER NOT NULL,
                x REAL NOT NULL,
                y REAL NOT NULL
            )
            ",
            (),
        )?;
        tx.commit()?;

        Ok(())
    }

    pub fn add_layout_records<T: AsRef<str>>(
        &mut self,
        records: &Vec<LayoutRecord<T>>,
    ) -> Result<()> {
        let tx = self.conn.transaction()?;

        for record in records {
            tx.execute(
            "
                INSERT INTO layout (gallery_id, wall_id, met_object_id, x, y) VALUES (?1, ?2, ?3, ?4, ?5)
                ",
        (
                    &record.gallery_id,
                    record.wall_id.as_ref(),
                    &record.met_object_id,
                    &record.x,
                    &record.y
                ),
            )?;
        }

        tx.commit()?;

        Ok(())
    }

    pub fn get_all_met_objects_for_layout(&mut self) -> Result<Vec<MetObjectLayoutInfo>> {
        let mut statement = self.conn.prepare_cached(
            "
            SELECT id, width, height FROM met_objects ORDER BY id
            ",
        )?;
        let mut rows = statement.query([])?;
        let mut result: Vec<MetObjectLayoutInfo> = Vec::new();
        while let Some(row) = rows.next()? {
            result.push(MetObjectLayoutInfo {
                id: row.get(0)?,
                width: row.get(1)?,
                height: row.get(2)?,
            });
        }
        Ok(result)
    }

    pub fn reset_met_objects_table(&mut self) -> Result<()> {
        let tx = self.conn.transaction()?;

        tx.execute("DROP TABLE IF EXISTS met_objects", ())?;
        tx.execute(
            "
            CREATE TABLE met_objects (
                id INTEGER PRIMARY KEY,
                title TEXT NOT NULL,
                date TEXT NOT NULL,
                medium TEXT NOT NULL,
                width REAL NOT NULL,
                height REAL NOT NULL
            )
            ",
            (),
        )?;

        tx.commit()?;

        Ok(())
    }

    /// Add a bunch of CSV records in a single transaction. This is much faster than adding
    /// a single record in a single transaction.
    pub fn add_csv_records(&mut self, records: &Vec<PublicDomain2DMetObjectRecord>) -> Result<()> {
        let tx = self.conn.transaction()?;

        for record in records {
            tx.execute(
            "
                INSERT INTO met_objects (id, title, date, medium, width, height) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ",
        (
                    &record.object_id,
                    &record.title,
                    &record.object_date,
                    &record.medium,
                    &record.width,
                    &record.height
                ),
            )?;
        }

        tx.commit()?;

        Ok(())
    }
}

pub struct PublicDomain2DMetObjectRecord {
    pub object_id: u64,
    pub accession_year: u16,
    pub object_date: String,
    pub title: String,
    pub medium: String,
    pub width: f64,
    pub height: f64,
}

pub struct MetObjectLayoutInfo {
    pub id: u64,
    pub width: f64,
    pub height: f64,
}

pub struct LayoutRecord<T: AsRef<str>> {
    pub gallery_id: u64,
    pub wall_id: T,
    pub met_object_id: u64,
    pub x: f64,
    pub y: f64,
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader, path::PathBuf};

    use rusqlite::Connection;

    use crate::{gallery_cache::GalleryCache, met_csv::iter_public_domain_2d_met_csv_objects};

    use super::GalleryDb;

    #[test]
    fn test_it_works() {
        let mut db = GalleryDb::new(Connection::open_in_memory().unwrap());
        db.reset_met_objects_table().unwrap();
        db.reset_layout_table().unwrap();

        let manifest_dir: PathBuf = env!("CARGO_MANIFEST_DIR").into();
        let cache = GalleryCache::new(manifest_dir.join("..").join("test_data"));
        let csv_file = cache.get_cached_path("MetObjects.csv");
        let reader = BufReader::new(File::open(csv_file).unwrap());
        let rdr = csv::Reader::from_reader(reader);
        let mut records = vec![];
        for result in iter_public_domain_2d_met_csv_objects(rdr) {
            records.push(result.unwrap());
        }
        db.add_csv_records(&records).unwrap();

        let rows = db.get_all_met_objects_for_layout().unwrap();
        assert!(rows.len() > 0);
    }
}
