use anyhow::{anyhow, Result};
use byteorder::{ReadBytesExt, WriteBytesExt};
use gallery::wikidata::WikidataEntity;
use index_file::{
    get_qid_index_file_mapping, index_path_for_dumpfile, iter_serialized_qids, IndexFileReader,
};
use indicatif::ProgressBar;
use sparql_csv_export::parse_sparql_csv_export;
use std::{
    collections::HashSet,
    io::{BufReader, BufWriter},
    path::PathBuf,
};
use zerocopy::LittleEndian;

pub use index_file::index_wikidata_dump;

mod index_file;
mod sparql_csv_export;

const BUFREADER_CAPACITY: usize = 1024 * 1024 * 8;

type SerializedEntityIterator = dyn Iterator<Item = Result<(u64, String)>>;

fn iter_and_cache_serialized_qids(
    dumpfile_path: PathBuf,
    qids: Vec<u64>,
    warnings: bool,
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

struct EntityInfo {
    qid: u64,
    entity: WikidataEntity,
    count: usize,
    percent_done: f64,
}

fn iter_and_cache_entities(
    dumpfile_path: PathBuf,
    qids: Vec<u64>,
    warnings: bool,
) -> Result<impl Iterator<Item = Result<EntityInfo>>> {
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
        Ok(EntityInfo {
            qid,
            entity,
            count,
            percent_done,
        })
    }))
}

fn dependency_path_for_dumpfile(dumpfile_path: &PathBuf) -> PathBuf {
    dumpfile_path.with_extension("deps.bin")
}

fn write_qid_vec_to_file(path: &PathBuf, qids: &Vec<u64>) -> Result<()> {
    let file = std::fs::File::create(path)?;
    let mut writer = BufWriter::new(file);
    writer.write_u64::<LittleEndian>(qids.len() as u64)?;
    for qid in qids {
        writer.write_u64::<LittleEndian>(*qid)?;
    }
    Ok(())
}

fn read_qid_vec_from_file(path: &PathBuf) -> Result<Vec<u64>> {
    let file = std::fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    let count = reader.read_u64::<LittleEndian>()? as usize;
    let mut qids: Vec<u64> = Vec::with_capacity(count);
    for _ in 0..count {
        qids.push(reader.read_u64::<LittleEndian>()?);
    }
    Ok(qids)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::wikidata_dump::read_qid_vec_from_file;

    use super::write_qid_vec_to_file;

    #[test]
    fn test_write_qid_vec_to_file_works() {
        let path = PathBuf::from("test_deps.bin");
        let qids: Vec<u64> = vec![1, 2, 3, 4];
        write_qid_vec_to_file(&path, &qids).unwrap();
        assert_eq!(read_qid_vec_from_file(&path).unwrap(), qids);
        std::fs::remove_file(path).unwrap();
    }
}

pub fn cache_wikidata_dump(
    dumpfile_path: PathBuf,
    mut qids: Vec<u64>,
    csv: Option<PathBuf>,
    verbose: bool,
    warnings: bool,
) -> Result<()> {
    if let Some(csv) = csv {
        parse_sparql_csv_export(csv, &mut qids)?;
    }
    let expected_total = qids.len();
    let mut total = 0;
    let mut total_with_required_fields = 0;
    let mut dependency_qids: HashSet<u64> = HashSet::new();
    let bar = ProgressBar::new(expected_total as u64);
    println!("Processing {} entities.", expected_total);
    for result in iter_and_cache_entities(dumpfile_path.clone(), qids, warnings)? {
        let EntityInfo {
            entity,
            percent_done,
            count,
            qid,
        } = result?;
        total = count;
        let has_image = entity.image_filename().is_some();
        let dimensions = entity.dimensions_in_cm();
        if has_image && dimensions.is_some() {
            total_with_required_fields += 1;
        } else if warnings {
            println!(
                "Warning: Q{qid} ({:?}) is missing required fields, image={:?}, dimensions={:?}",
                entity.label().unwrap_or_default(),
                entity.image_filename(),
                dimensions
            );
        }
        if let Some(qid) = entity.creator_id() {
            dependency_qids.insert(qid);
        }
        if verbose {
            println!(
                "{percent_done:.1}% Q{qid}: {} - {} ({}, {}, {})",
                entity.label().unwrap_or_default(),
                entity.description().unwrap_or_default(),
                if has_image { "has image" } else { "no image" },
                if let Some((width, height)) = dimensions {
                    format!("{width:.0} x {height:.0} cm")
                } else {
                    "no dimensions".to_string()
                },
                if let Some(qid) = entity.creator_id() {
                    format!("creator=Q{qid}")
                } else {
                    format!("no creator")
                }
            );
        } else {
            bar.inc(1);
        }
    }
    bar.finish();
    println!(
        "Done processing {total} entities, {total_with_required_fields} have all required fields, {} were not found.",
        expected_total - total
    );

    let dependency_qids = dependency_qids.into_iter().collect::<Vec<_>>();
    write_qid_vec_to_file(
        &dependency_path_for_dumpfile(&dumpfile_path),
        &dependency_qids,
    )?;
    let expected_total = dependency_qids.len();
    let mut total = 0;
    if expected_total > 0 {
        let bar = ProgressBar::new(expected_total as u64);
        println!("Processing {} dependency entities.", expected_total);
        for result in iter_and_cache_entities(dumpfile_path, dependency_qids, warnings)? {
            let EntityInfo {
                entity,
                percent_done,
                count,
                qid,
            } = result?;
            total = count;
            if verbose {
                println!(
                    "{percent_done:.1}% dependency Q{qid}: {} -{}",
                    entity.label().unwrap_or_default(),
                    entity.description().unwrap_or_default(),
                );
            } else {
                bar.inc(1);
            }
        }
        bar.finish();
        println!(
            "Done processing {total} depdendencies, {} were not found.",
            expected_total - total
        );
    }
    Ok(())
}
