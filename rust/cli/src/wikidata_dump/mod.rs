use anyhow::{anyhow, Result};
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

type SerializedEntityIterator = dyn Iterator<Item = Result<(u64, String)>>;

fn iter_serialized_qids_using_cache(
    dumpfile_path: PathBuf,
    qids: Vec<u64>,
) -> Result<Box<SerializedEntityIterator>> {
    let index_path = index_path_for_dumpfile(&dumpfile_path);
    let mut index_db = IndexFileReader::new(index_path)?;
    let sledcache_path = dumpfile_path.with_extension("sledcache");
    let sledcache = sled::open(&sledcache_path)?;
    let read_sledcache = sledcache.clone();
    let (cached_qids, uncached_qids): (Vec<u64>, Vec<u64>) = qids.into_iter().partition(|qid| {
        read_sledcache
            .contains_key(qid.to_be_bytes())
            .unwrap_or(false)
    });
    let cached_iterator: Box<SerializedEntityIterator> = Box::new(cached_qids.into_iter().map(
        move |qid| match read_sledcache.get(qid.to_be_bytes()) {
            Ok(value) => match value {
                Some(value) => {
                    let buf: &[u8] = value.as_ref();
                    let value = String::from_utf8(buf.to_vec())?;
                    Ok((qid, value))
                }
                None => Err(anyhow!(
                    "sledcache does not contain a key it claimed it has"
                )),
            },
            Err(err) => Err(err.into()),
        },
    ));
    let qid_index_file_mapping = get_qid_index_file_mapping(&mut index_db, uncached_qids)?;
    if qid_index_file_mapping.qids() == 0 {
        return Ok(Box::new(cached_iterator));
    }
    println!(
        "Reading {} QIDs across {} gzipped members.",
        qid_index_file_mapping.qids(),
        qid_index_file_mapping.gzip_members()
    );
    let file = std::fs::File::open(dumpfile_path)?;
    let archive_reader = BufReader::with_capacity(BUFREADER_CAPACITY, file);
    let uncached_iterator: Box<SerializedEntityIterator> = Box::new(
        iter_serialized_qids(archive_reader, qid_index_file_mapping).map(
            move |result| match result {
                Ok((qid, value)) => {
                    sledcache.insert(qid.to_be_bytes(), value.as_bytes())?;
                    Ok((qid, value))
                }
                Err(err) => Err(err),
            },
        ),
    );
    Ok(Box::new(cached_iterator.chain(uncached_iterator)))
}

pub fn query_wikidata_dump(
    dumpfile_path: PathBuf,
    mut qids: Vec<u64>,
    csv: Option<PathBuf>,
    verbose: bool,
) -> Result<()> {
    if let Some(csv) = csv {
        parse_sparql_csv_export(csv, &mut qids)?;
    }
    let total_qids = qids.len();
    let mut count = 0;
    for result in iter_serialized_qids_using_cache(dumpfile_path, qids)? {
        count += 1;
        let percent_done = (count as f64) / (total_qids as f64) * 100.0;
        let (qid, qid_json) = result?;
        let entity: WikidataEntity = serde_json::from_str(&qid_json)?;
        if verbose {
            println!(
                "{percent_done:.1}% Q{qid}: {} - {} ({})",
                entity.label().unwrap_or_default(),
                entity.description().unwrap_or_default(),
                if entity.image_filename().is_some() {
                    "has image"
                } else {
                    "no image"
                }
            );
        } else if count % 1000 == 0 {
            println!("{percent_done:.1}% complete ({count} entities processed).");
        }
    }
    println!("Done processing {count} entities.");
    Ok(())
}
