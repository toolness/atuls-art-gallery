use super::index_file::{get_qid_index_file_mapping, index_path_for_dumpfile, IndexFileReader};
use anyhow::{anyhow, Result};
use gallery::wikidata::WikidataEntity;
use std::path::PathBuf;

use crate::wikidata_dump::index_file::par_iter_serialized_qids;

pub fn sledcache_path_for_dumpfile(dumpfile_path: &PathBuf) -> PathBuf {
    dumpfile_path.with_extension("sledcache")
}

type EntityIterator = dyn Iterator<Item = Result<WikidataEntity>>;

fn iter_and_cache_serialized_qids_without_progress_info(
    dumpfile_path: PathBuf,
    qids: Vec<u64>,
    warnings: bool,
) -> Result<Box<EntityIterator>> {
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
    let cached_iterator: Box<EntityIterator> = Box::new(
        cached_qids
            .into_iter()
            .map(move |qid| match read_sledcache.get(qid.to_be_bytes()) {
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
            })
            .map(parse_wikidata_entity_from_result),
    );
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
    let uncached_iterator: Box<EntityIterator> = Box::new(
        par_iter_serialized_qids(dumpfile_path, qid_index_file_mapping).map(move |result| {
            match result {
                Ok((qid, value)) => match parse_wikidata_entity(qid, &value) {
                    Ok(entity) => {
                        // We explicitly only cache the result if it's been successfully deserialized--otherwise
                        // there could be a bug upstream that gives us bad serialized entities, and we
                        // don't want to cache those!
                        sledcache.insert(qid.to_be_bytes(), value.as_bytes())?;
                        Ok(entity)
                    }
                    Err(err) => Err(err),
                },
                Err(err) => Err(err),
            }
        }),
    );
    Ok(Box::new(cached_iterator.chain(uncached_iterator)))
}

pub struct CachedEntityInfo {
    pub entity: WikidataEntity,
    pub count: usize,
    pub percent_done: f64,
}

/// Iterate through the given entities in the dumpfile, caching them if they are
/// not already cached.
///
/// Note that for any entities that aren't already cached, the order of iteration
/// is non-deterministic.
pub fn iter_and_cache_entities(
    dumpfile_path: PathBuf,
    qids: Vec<u64>,
    warnings: bool,
) -> Result<impl Iterator<Item = Result<CachedEntityInfo>>> {
    let total_qids = qids.len();
    let mut count = 0;
    let iterator =
        iter_and_cache_serialized_qids_without_progress_info(dumpfile_path, qids, warnings)?;
    Ok(iterator.map(move |result| {
        count += 1;
        let percent_done = (count as f64) / (total_qids as f64) * 100.0;
        let entity = result?;
        Ok(CachedEntityInfo {
            entity,
            count,
            percent_done,
        })
    }))
}

fn parse_wikidata_entity_from_result(result: Result<(u64, String)>) -> Result<WikidataEntity> {
    let (qid, qid_json) = result?;
    parse_wikidata_entity(qid, &qid_json)
}

fn parse_wikidata_entity(qid: u64, qid_json: &str) -> Result<WikidataEntity> {
    let parse_result: Result<WikidataEntity, _> = serde_json::from_str(&qid_json);
    match parse_result {
        Ok(entity) => {
            if entity.id == qid {
                Ok(entity)
            } else {
                Err(anyhow!(
                    "Deserialized Q{qid} but it claims to be {}",
                    entity.id
                ))
            }
        }
        Err(err) => Err(anyhow!(
            "Error occurred deserializing JSON for Q{qid}: {err:?}"
        )),
    }
}
