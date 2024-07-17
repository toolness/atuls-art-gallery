use anyhow::Result;
use gallery::wikidata::WikidataEntity;
use index_file::{
    get_qid_index_file_mapping, index_path_for_dumpfile, iter_serialized_qids, IndexFileReader,
};
use sparql_csv_export::parse_sparql_csv_export;
use std::{io::BufReader, path::PathBuf};

pub use index_file::index_wikidata_dump;

mod index_file;
mod sparql_csv_export;

const BUFREADER_CAPACITY: usize = 1024 * 1024 * 8;

pub fn query_wikidata_dump(
    dumpfile_path: PathBuf,
    mut qids: Vec<u64>,
    csv: Option<PathBuf>,
) -> Result<()> {
    if let Some(csv) = csv {
        parse_sparql_csv_export(csv, &mut qids)?;
    }
    let index_path = index_path_for_dumpfile(&dumpfile_path);
    let mut index_db = IndexFileReader::new(index_path)?;
    let qid_index_file_mapping = get_qid_index_file_mapping(&mut index_db, qids)?;
    println!(
        "Reading {} QIDs across {} gzipped members.",
        qid_index_file_mapping.qids(),
        qid_index_file_mapping.gzip_members()
    );
    let file = std::fs::File::open(dumpfile_path)?;
    let archive_reader = BufReader::with_capacity(BUFREADER_CAPACITY, file);
    for result in iter_serialized_qids(archive_reader, qid_index_file_mapping) {
        let (qid, qid_json) = result?;
        let entity: WikidataEntity = serde_json::from_str(&qid_json)?;
        println!(
            "Q{qid}: {} - {} ({})",
            entity.label().unwrap_or_default(),
            entity.description().unwrap_or_default(),
            if entity.p18_image().is_some() {
                "has image"
            } else {
                "no image"
            }
        );
    }
    Ok(())
}
