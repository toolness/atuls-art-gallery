use anyhow::Result;
use rusqlite::{Connection, Transaction};

use crate::{
    art_object::ArtObjectId,
    filter_parser::{parse_filter, Filter},
};

pub const DEFAULT_GALLERY_DB_FILENAME: &'static str = "gallery5.sqlite";

#[derive(Default)]
pub struct ArtObjectQueryOptions {
    pub filter: Option<String>,
}

impl ArtObjectQueryOptions {
    fn order_by_clause(&self) -> String {
        format!("ORDER BY id")
    }

    pub fn where_clause(&self) -> (String, Vec<String>) {
        let mut params: Vec<String> = vec![];
        let where_clause = if let Some(filter) = &self.filter {
            if let Some(ast) = parse_filter(filter) {
                let mut query_parts = vec![];
                filter_to_sql(ast, &mut query_parts, &mut params);
                let query = query_parts.join("");
                format!("WHERE {}", query)
            } else {
                String::default()
            }
        } else {
            String::default()
        };
        (where_clause, params)
    }
}

fn filter_to_sql(filter: Filter, query_parts: &mut Vec<String>, params: &mut Vec<String>) {
    match filter {
        Filter::And(a, b) => {
            filter_to_sql(*a, query_parts, params);
            query_parts.push(" AND ".into());
            filter_to_sql(*b, query_parts, params);
        }
        Filter::Or(a, b) => {
            filter_to_sql(*a, query_parts, params);
            query_parts.push(" OR ".into());
            filter_to_sql(*b, query_parts, params);
        }
        Filter::Not(value) => {
            query_parts.push("NOT ".into());
            filter_to_sql(*value, query_parts, params);
        }
        Filter::Term(term) => {
            params.push(format!("%{term}%"));
            let num = params.len();
            query_parts.push(format!(
                "(
                    (title LIKE ?{num}) OR
                    (artist LIKE ?{num}) OR
                    (medium LIKE ?{num}) OR
                    (culture LIKE ?{num}) OR
                    (collection LIKE ?{num})
                )"
            ))
        }
    }
}

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
        // Note that conceptually, `art_object_id` is a foreign key to the art_objects
        // table, but we don't want to enforce a constraint because we want to
        // be able to blow away the art_objects table for re-importing if needed.
        tx.execute(
            "
            CREATE TABLE layout (
                gallery_id INTEGER NOT NULL,
                wall_id TEXT NOT NULL,
                art_object_id INTEGER NOT NULL UNIQUE,
                x REAL NOT NULL,
                y REAL NOT NULL
            )
            ",
            (),
        )?;
        tx.commit()?;

        Ok(())
    }

    pub fn upsert_layout_records_with_transaction<T: AsRef<str>>(
        tx: &Transaction,
        records: &Vec<LayoutRecord<T>>,
    ) -> Result<()> {
        for record in records {
            tx.execute(
            "
                INSERT INTO layout (gallery_id, wall_id, art_object_id, x, y) VALUES (?1, ?2, ?3, ?4, ?5)
                    ON CONFLICT(art_object_id) DO UPDATE SET
                        gallery_id=excluded.gallery_id,
                        wall_id=excluded.wall_id,
                        x=excluded.x,
                        y=excluded.y
                ",
        (
                    &record.gallery_id,
                    record.wall_id.as_ref(),
                    &record.art_object_id.to_raw_i64(),
                    &record.x,
                    &record.y
                ),
            )?;
        }
        Ok(())
    }

    pub fn upsert_layout_records<T: AsRef<str>>(
        &mut self,
        records: &Vec<LayoutRecord<T>>,
    ) -> Result<()> {
        let tx = self.conn.transaction()?;
        GalleryDb::upsert_layout_records_with_transaction(&tx, records)?;
        tx.commit()?;
        Ok(())
    }

    /// Clears the layout and fills it with the given records.
    pub fn set_layout_records<T: AsRef<str>>(
        &mut self,
        records: &Vec<LayoutRecord<T>>,
    ) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM layout", ())?;
        GalleryDb::upsert_layout_records_with_transaction(&tx, records)?;
        tx.commit()?;
        Ok(())
    }

    pub fn count_art_objects(&self, options: &ArtObjectQueryOptions) -> Result<usize> {
        let (where_clause, params) = options.where_clause();
        let mut statement = self.conn.prepare(&format!(
            "
            SELECT COUNT(*) FROM art_objects {where_clause}
            ",
        ))?;
        Ok(
            statement.query_row(rusqlite::params_from_iter(params.into_iter()), |row| {
                row.get(0)
            })?,
        )
    }

    pub fn get_all_art_objects_for_layout(
        &self,
        options: &ArtObjectQueryOptions,
    ) -> Result<Vec<ArtObjectLayoutInfo>> {
        let order_by_clause = options.order_by_clause();
        let (where_clause, params) = options.where_clause();
        let mut statement = self.conn.prepare(&format!(
            "
            SELECT id, width, height FROM art_objects {where_clause} {order_by_clause}
            ",
        ))?;
        let mut rows = statement.query(rusqlite::params_from_iter(params.into_iter()))?;
        let mut result: Vec<ArtObjectLayoutInfo> = Vec::new();
        while let Some(row) = rows.next()? {
            result.push(ArtObjectLayoutInfo {
                id: ArtObjectId::from_raw_i64(row.get(0)?),
                width: row.get(1)?,
                height: row.get(2)?,
            });
        }
        Ok(result)
    }

    pub fn reset_art_objects_table(&mut self) -> Result<()> {
        let tx = self.conn.transaction()?;

        tx.execute("DROP TABLE IF EXISTS art_objects", ())?;
        tx.execute(
            "
            CREATE TABLE art_objects (
                id INTEGER PRIMARY KEY,
                title TEXT NOT NULL,
                artist TEXT NOT NULL,
                culture TEXT NOT NULL,
                date TEXT NOT NULL,
                medium TEXT NOT NULL,
                width REAL NOT NULL,
                height REAL NOT NULL,
                fallback_wikidata_qid INTEGER,
                filename TEXT NOT NULL,
                collection TEXT NOT NULL
            )
            ",
            (),
        )?;

        tx.commit()?;

        Ok(())
    }

    /// Add a bunch of records in a single transaction. This is much faster than adding
    /// a single record in a single transaction.
    pub fn add_art_objects(&mut self, records: &Vec<ArtObjectRecord>) -> Result<()> {
        let tx = self.conn.transaction()?;

        for record in records {
            tx.execute(
                "
                INSERT INTO art_objects (
                    id,
                    title,
                    date,
                    medium,
                    width,
                    height,
                    artist,
                    culture,
                    fallback_wikidata_qid,
                    filename,
                    collection
                ) VALUES (
                    ?1,
                    ?2,
                    ?3,
                    ?4,
                    ?5,
                    ?6,
                    ?7,
                    ?8,
                    ?9,
                    ?10,
                    ?11
                )
                ",
                (
                    &record.object_id.to_raw_i64(),
                    &record.title,
                    &record.object_date,
                    &record.medium,
                    &record.width,
                    &record.height,
                    &record.artist,
                    &record.culture,
                    &record.fallback_wikidata_qid,
                    &record.filename,
                    &record.collection,
                ),
            )?;
        }

        tx.commit()?;

        Ok(())
    }

    pub fn get_art_object(&self, object_id: ArtObjectId) -> Result<Option<ArtObjectRecord>> {
        let mut statement = self.conn.prepare_cached(
            "
                SELECT
                    ao.title,
                    ao.date,
                    ao.medium,
                    ao.width,
                    ao.height,
                    ao.artist,
                    ao.culture,
                    ao.fallback_wikidata_qid,
                    ao.filename,
                    ao.collection
                FROM
                    art_objects AS ao
                WHERE
                    ao.id = ?1",
        )?;
        let mut rows = statement.query([object_id.to_raw_i64()])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        Ok(Some(ArtObjectRecord {
            object_id,
            title: row.get(0)?,
            object_date: row.get(1)?,
            medium: row.get(2)?,
            width: row.get(3)?,
            height: row.get(4)?,
            artist: row.get(5)?,
            culture: row.get(6)?,
            fallback_wikidata_qid: row.get(7)?,
            filename: row.get(8)?,
            collection: row.get(9)?,
        }))
    }

    pub fn get_art_objects_for_gallery_wall<T: AsRef<str>>(
        &self,
        gallery_id: i64,
        wall_id: T,
    ) -> Result<Vec<(ArtObjectRecord, (f64, f64))>> {
        let mut result = vec![];

        let mut statement = self.conn.prepare_cached(
            "
            SELECT
                layout.art_object_id,
                layout.x,
                layout.y,
                ao.title,
                ao.date,
                ao.medium,
                ao.width,
                ao.height,
                ao.artist,
                ao.culture,
                ao.fallback_wikidata_qid,
                ao.filename,
                ao.collection
            FROM
                art_objects AS ao
            INNER JOIN
                layout
            ON
                layout.art_object_id = ao.id
            WHERE
                layout.gallery_id = ?1 AND
                layout.wall_id = ?2
            ",
        )?;
        let mut rows = statement.query(rusqlite::params![&gallery_id, wall_id.as_ref()])?;
        while let Some(row) = rows.next()? {
            let id = ArtObjectId::from_raw_i64(row.get(0)?);
            let location: (f64, f64) = (row.get(1)?, row.get(2)?);
            let object = ArtObjectRecord {
                object_id: id,
                title: row.get(3)?,
                object_date: row.get(4)?,
                medium: row.get(5)?,
                width: row.get(6)?,
                height: row.get(7)?,
                artist: row.get(8)?,
                culture: row.get(9)?,
                fallback_wikidata_qid: row.get(10)?,
                filename: row.get(11)?,
                collection: row.get(12)?,
            };
            result.push((object, location));
        }

        Ok(result)
    }
}

#[derive(Debug, PartialEq)]
pub struct ArtObjectRecord {
    pub object_id: ArtObjectId,
    pub object_date: String,
    pub culture: String,
    pub artist: String,
    pub title: String,
    pub medium: String,
    pub width: f64,
    pub height: f64,
    pub fallback_wikidata_qid: Option<i64>,
    pub filename: String,
    pub collection: String,
}

#[derive(Debug, PartialEq)]
pub struct ArtObjectLayoutInfo {
    pub id: ArtObjectId,
    pub width: f64,
    pub height: f64,
}

pub struct LayoutRecord<T: AsRef<str>> {
    pub gallery_id: i64,
    pub wall_id: T,
    pub art_object_id: ArtObjectId,
    pub x: f64,
    pub y: f64,
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use crate::{
        art_object::ArtObjectId,
        gallery_db::{ArtObjectQueryOptions, LayoutRecord},
    };

    use super::{ArtObjectLayoutInfo, ArtObjectRecord, GalleryDb};

    const FUNKY_PAINTING_ID: ArtObjectId = ArtObjectId::Met(1);

    fn make_funky_painting() -> ArtObjectRecord {
        ArtObjectRecord {
            object_id: FUNKY_PAINTING_ID,
            object_date: "1864".into(),
            culture: "Martian".into(),
            artist: "Boop Jones".into(),
            title: "Funky Painting".into(),
            medium: "Oil on canvas".into(),
            width: 64.5,
            height: 28.2,
            fallback_wikidata_qid: Some(1234),
            filename: "funky-painting.jpg".into(),
            collection: "Martian Museum of Art".into(),
        }
    }

    impl Into<ArtObjectLayoutInfo> for ArtObjectRecord {
        fn into(self) -> ArtObjectLayoutInfo {
            ArtObjectLayoutInfo {
                id: self.object_id,
                width: self.width,
                height: self.height,
            }
        }
    }

    fn test_filter(db: &GalleryDb, filter: &'static str, expected: &Vec<ArtObjectLayoutInfo>) {
        let options = ArtObjectQueryOptions {
            filter: Some(filter.into()),
        };
        let actual = db.get_all_art_objects_for_layout(&options).unwrap();
        assert_eq!(&actual, expected);
        assert_eq!(db.count_art_objects(&options).unwrap(), expected.len());
    }

    #[test]
    fn test_it_works() {
        let mut db = GalleryDb::new(Connection::open_in_memory().unwrap());
        db.reset_art_objects_table().unwrap();
        db.reset_layout_table().unwrap();

        // Add an art object...
        db.add_art_objects(&vec![make_funky_painting()]).unwrap();

        // Make sure we can retrieve it.
        assert_eq!(
            db.get_art_object(FUNKY_PAINTING_ID).unwrap(),
            Some(make_funky_painting())
        );
        assert_eq!(db.get_art_object(ArtObjectId::Met(12345)).unwrap(), None);

        let funky_layout_info = vec![make_funky_painting().into()];
        let empty_layout_info = vec![];

        // Search for the art object.
        test_filter(&db, "boop", &funky_layout_info);
        test_filter(&db, "-boop", &empty_layout_info);

        // Ensure unquoted terms are ANDed together...
        test_filter(&db, "boop jones", &funky_layout_info);
        test_filter(&db, "jones boop", &funky_layout_info);

        // Ensure quoted terms are exact substring matches...
        test_filter(&db, "\"boop jones\"", &funky_layout_info);
        test_filter(&db, "\"jones boop\"", &empty_layout_info);

        // Add a painting to the layout.
        db.set_layout_records(&vec![LayoutRecord {
            gallery_id: 1,
            wall_id: "wall_02",
            art_object_id: FUNKY_PAINTING_ID,
            x: 1.2,
            y: 3.4,
        }])
        .unwrap();

        // Make sure it got placed where we placed it.
        assert_eq!(
            db.get_art_objects_for_gallery_wall(1, "wall_02").unwrap(),
            vec![(make_funky_painting(), (1.2, 3.4))]
        );

        // Make sure there's nothing in the place we want to move it to.
        assert_eq!(
            db.get_art_objects_for_gallery_wall(3, "wall_04").unwrap(),
            vec![]
        );

        // Move the painting.
        db.upsert_layout_records(&vec![LayoutRecord {
            gallery_id: 3,
            wall_id: "wall_04",
            art_object_id: FUNKY_PAINTING_ID,
            x: 5.6,
            y: 7.8,
        }])
        .unwrap();

        // Make sure there's nothing in the place we moved it from.
        assert_eq!(
            db.get_art_objects_for_gallery_wall(1, "wall_02").unwrap(),
            vec![]
        );

        // Make sure it actually got moved to where we moved it.
        assert_eq!(
            db.get_art_objects_for_gallery_wall(3, "wall_04").unwrap(),
            vec![(make_funky_painting(), (5.6, 7.8))]
        );
    }
}
