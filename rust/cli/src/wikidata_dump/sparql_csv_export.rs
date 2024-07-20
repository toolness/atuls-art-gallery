use anyhow::Result;
use gallery::wikidata::deserialize_wikidata_entity_url_str;
use serde::Deserialize;
use std::{fs::File, io::BufReader, path::PathBuf};

#[derive(Debug, Deserialize)]
struct WikidataCsvRecord {
    #[serde(
        rename = "item",
        deserialize_with = "deserialize_wikidata_entity_url_str"
    )]
    pub qid: u64,
}

/// Parse a CSV export from query.wikidata.org with a single 'item' column containing entity URLs,
/// inserting the entity Q-identifiers into the given vec.
pub fn parse_sparql_csv_export(path: PathBuf, qids: &mut Vec<u64>) -> Result<()> {
    let reader = BufReader::new(File::open(path)?);
    let rdr = csv::Reader::from_reader(reader);
    for result in rdr.into_deserialize::<WikidataCsvRecord>() {
        let csv_record: WikidataCsvRecord = result?;
        qids.push(csv_record.qid);
    }
    Ok(())
}
