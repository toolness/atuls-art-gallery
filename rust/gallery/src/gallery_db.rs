use anyhow::Result;
use rusqlite::Connection;

pub const DEFAULT_GALLERY_DB_FILENAME: &'static str = "gallery2.sqlite";

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
            CREATE TABLE layout (
                gallery_id INTEGER NOT NULL,
                wall_id TEXT NOT NULL,
                met_object_id INTEGER NOT NULL UNIQUE,
                x REAL NOT NULL,
                y REAL NOT NULL
            )
            ",
            (),
        )?;
        tx.commit()?;

        Ok(())
    }

    pub fn upsert_layout_records<T: AsRef<str>>(
        &mut self,
        records: &Vec<LayoutRecord<T>>,
    ) -> Result<()> {
        let tx = self.conn.transaction()?;

        for record in records {
            tx.execute(
            "
                INSERT INTO layout (gallery_id, wall_id, met_object_id, x, y) VALUES (?1, ?2, ?3, ?4, ?5)
                    ON CONFLICT(met_object_id) DO UPDATE SET
                        gallery_id=excluded.gallery_id,
                        wall_id=excluded.wall_id,
                        x=excluded.x,
                        y=excluded.y
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

    pub fn get_all_met_objects_for_layout(
        &mut self,
        order_by: Option<&str>,
    ) -> Result<Vec<MetObjectLayoutInfo>> {
        let mut statement = self.conn.prepare(&format!(
            "
            SELECT id, width, height FROM met_objects ORDER BY {}
            ",
            order_by.unwrap_or("id")
        ))?;
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
                artist TEXT NOT NULL,
                date TEXT NOT NULL,
                medium TEXT NOT NULL,
                width REAL NOT NULL,
                height REAL NOT NULL,
                accession_year INTEGER NOT NULL
            )
            ",
            (),
        )?;

        tx.commit()?;

        Ok(())
    }

    /// Add a bunch of records in a single transaction. This is much faster than adding
    /// a single record in a single transaction.
    pub fn add_public_domain_2d_met_objects(
        &mut self,
        records: &Vec<PublicDomain2DMetObjectRecord>,
    ) -> Result<()> {
        let tx = self.conn.transaction()?;

        for record in records {
            tx.execute(
            "
                INSERT INTO met_objects (id, title, date, medium, width, height, accession_year, artist) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ",
        (
                    &record.object_id,
                    &record.title,
                    &record.object_date,
                    &record.medium,
                    &record.width,
                    &record.height,
                    &record.accession_year,
                    &record.artist,
                ),
            )?;
        }

        tx.commit()?;

        Ok(())
    }

    pub fn get_met_objects_for_gallery_wall<T: AsRef<str>>(
        &mut self,
        gallery_id: i64,
        wall_id: T,
    ) -> Result<Vec<(PublicDomain2DMetObjectRecord, (f64, f64))>> {
        let mut result = vec![];

        let mut statement = self.conn.prepare_cached(
            "
            SELECT
                layout.met_object_id,
                layout.x,
                layout.y,
                mo.title,
                mo.date,
                mo.medium,
                mo.width,
                mo.height,
                mo.accession_year,
                mo.artist
            FROM
                met_objects AS mo
            INNER JOIN
                layout
            ON
                layout.met_object_id = mo.id
            WHERE
                layout.gallery_id = ?1 AND
                layout.wall_id = ?2
            ",
        )?;
        let mut rows = statement.query(rusqlite::params![&gallery_id, wall_id.as_ref()])?;
        while let Some(row) = rows.next()? {
            let id = row.get(0)?;
            let location: (f64, f64) = (row.get(1)?, row.get(2)?);
            let object = PublicDomain2DMetObjectRecord {
                object_id: id,
                title: row.get(3)?,
                object_date: row.get(4)?,
                medium: row.get(5)?,
                width: row.get(6)?,
                height: row.get(7)?,
                accession_year: row.get(8)?,
                artist: row.get(9)?,
            };
            result.push((object, location));
        }

        Ok(result)
    }
}

#[derive(Debug)]
pub struct PublicDomain2DMetObjectRecord {
    pub object_id: u64,
    pub accession_year: u16,
    pub object_date: String,
    pub artist: String,
    pub title: String,
    pub medium: String,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug)]
pub struct MetObjectLayoutInfo {
    pub id: u64,
    pub width: f64,
    pub height: f64,
}

pub struct LayoutRecord<T: AsRef<str>> {
    pub gallery_id: i64,
    pub wall_id: T,
    pub met_object_id: u64,
    pub x: f64,
    pub y: f64,
}
