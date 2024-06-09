use anyhow::Result;
use rusqlite::Connection;

use crate::the_met::MetObjectCsvRecord;

pub struct GalleryDb {
    conn: Connection,
}

impl GalleryDb {
    pub fn new(conn: Connection) -> Self {
        GalleryDb { conn }
    }

    pub fn create_tables(&self) -> Result<()> {
        self.conn.execute(
            "
            CREATE TABLE met_objects (
                id INTEGER PRIMARY KEY,
                title TEXT NOT NULL,
                date TEXT NOT NULL,
                medium TEXT NOT NULL,
                primary_image_small TEXT,
                width REAL,
                height REAL
            )
            ",
            (),
        )?;
        Ok(())
    }

    pub fn add_csv_record(&self, record: &MetObjectCsvRecord) -> Result<()> {
        self.conn.execute(
            "
            INSERT INTO met_objects (id, title, date, medium, width, height) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            (
                &record.object_id,
                &record.title,
                &record.object_date,
                &record.medium,
                record.parsed_dimensions.map(|r| r.0),
                record.parsed_dimensions.map(|r| r.1),
            ),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::GalleryDb;

    #[test]
    fn test_it_works() {
        let db = GalleryDb::new(Connection::open_in_memory().unwrap());
        db.create_tables().unwrap();
    }
}
