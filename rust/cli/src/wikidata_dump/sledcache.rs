use super::index_file::{
    get_qid_index_file_mapping, index_path_for_dumpfile, iter_serialized_qids, IndexFileReader,
};
use anyhow::{anyhow, Result};
use gallery::wikidata::WikidataEntity;
use std::{io::BufReader, path::PathBuf};

use crate::wikidata_dump::BUFREADER_CAPACITY;

pub fn sledcache_path_for_dumpfile(dumpfile_path: &PathBuf) -> PathBuf {
    dumpfile_path.with_extension("sledcache")
}

type SerializedEntityIterator = dyn Iterator<Item = Result<(u64, String)>>;

fn iter_and_cache_serialized_qids(
    dumpfile_path: PathBuf,
    qids: Vec<u64>,
    warnings: bool,
) -> Result<Box<SerializedEntityIterator>> {
    let index_path = index_path_for_dumpfile(&dumpfile_path);
    let mut index_db = IndexFileReader::new(index_path)?;
    let sledcache_path = sledcache_path_for_dumpfile(&dumpfile_path);
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
                    let value = String::from_utf8(value.as_ref().to_vec())?;
                    Ok((qid, value))
                }
                None => Err(anyhow!(
                    "sledcache does not contain a key it claimed it has"
                )),
            },
            Err(err) => Err(err.into()),
        },
    ));
    let qid_index_file_mapping =
        get_qid_index_file_mapping(&mut index_db, uncached_qids, warnings)?;
    if qid_index_file_mapping.qids() == 0 {
        return Ok(Box::new(cached_iterator));
    }
    println!(
        "Reading {} QIDs across {} gzipped members.",
        qid_index_file_mapping.qids(),
        qid_index_file_mapping.gzip_members()
    );
    // I considered opening the dumpfile for reading in several different threads,
    // and telling each of them to retrieve the entities from a different gzip member.
    // However, this could exhaust system memory; an advantage of doing this entirely
    // sequentially is that we only ever need to hold a single decompressed gzipped
    // member in memory at any given time. It could be worth exploring more later, though.
    let file = std::fs::File::open(dumpfile_path)?;
    let dumpfile_reader = BufReader::with_capacity(BUFREADER_CAPACITY, file);
    let uncached_iterator: Box<SerializedEntityIterator> = Box::new(
        iter_serialized_qids(dumpfile_reader, qid_index_file_mapping).map(
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

pub struct CachedEntityInfo {
    pub qid: u64,
    pub entity: WikidataEntity,
    pub count: usize,
    pub percent_done: f64,
}

pub fn iter_and_cache_entities(
    dumpfile_path: PathBuf,
    qids: Vec<u64>,
    warnings: bool,
) -> Result<impl Iterator<Item = Result<CachedEntityInfo>>> {
    let total_qids = qids.len();
    let mut count = 0;
    let iterator = iter_and_cache_serialized_qids(dumpfile_path, qids, warnings)?;
    Ok(iterator.map(move |result| {
        count += 1;
        let percent_done = (count as f64) / (total_qids as f64) * 100.0;
        let (qid, qid_json) = result?;
        let entity: WikidataEntity = match serde_json::from_str(&qid_json) {
            Ok(entity) => entity,
            Err(err) => {
                return Err(anyhow!(
                    "Error occurred deserializing JSON for Q{qid}: {err:?}"
                ))
            }
        };
        Ok(CachedEntityInfo {
            qid,
            entity,
            count,
            percent_done,
        })
    }))
}
