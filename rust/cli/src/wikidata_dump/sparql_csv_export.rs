use anyhow::Result;
use gallery::wikidata::try_to_parse_qid_from_wikidata_url;
use serde::{de, Deserialize};
use std::{fs::File, io::BufReader, path::PathBuf};

#[derive(Debug, Deserialize)]
struct WikidataCsvRecord {
    #[serde(rename = "item", deserialize_with = "deserialize_wikidata_entity_url")]
    pub qid: u64,
}

/// Parses a Q-identifier from a wikidata URL and returns it.
fn deserialize_wikidata_entity_url<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: de::Deserializer<'de>,
{
    let s: &str = de::Deserialize::deserialize(deserializer)?;
    match try_to_parse_qid_from_wikidata_url(s) {
        Some(qid) => Ok(qid),
        // TODO: `unknown_variant` is probably the wrong type of error to return.
        None => Err(de::Error::unknown_variant(s, &["Wikidata URL"])),
    }
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
