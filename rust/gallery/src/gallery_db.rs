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

    pub fn create_visited_met_objects_table(&mut self) -> Result<()> {
        // The existence of a record with a given `id` means it's been visited.
        //
        // Note that conceptually, `id` is also a foreign key to the met_objects
        // table, but we don't want to enforce a constraint because we want to
        // be able to blow away the met_objects table for re-importing if needed.
        self.conn.execute(
            "
            CREATE TABLE IF NOT EXISTS visited_met_objects (
                id INTEGER PRIMARY KEY
            )
            ",
            (),
        )?;
        Ok(())
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
                primary_image_small TEXT,
                width REAL,
                height REAL
            )
            ",
            (),
        )?;

        tx.commit()?;

        Ok(())
    }

    /// Add a bunch of CSV records in a single transaction. This is much faster than adding
    /// a single record in a single transaction.
    pub fn add_csv_records(&mut self, records: &Vec<MetObjectCsvRecord>) -> Result<()> {
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
                    record.parsed_dimensions.map(|r| r.0),
                    record.parsed_dimensions.map(|r| r.1),
                ),
            )?;
        }

        tx.commit()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::GalleryDb;

    #[test]
    fn test_it_works() {
        let mut db = GalleryDb::new(Connection::open_in_memory().unwrap());
        db.reset_met_objects_table().unwrap();
        db.create_visited_met_objects_table().unwrap();
    }
}
