use super::sledcache::{iter_and_cache_entities, sledcache_path_for_dumpfile, CachedEntityInfo};
use super::sparql_csv_export::parse_sparql_csv_export;
use anyhow::Result;
use gallery::wikidata::WikidataEntity;
use indicatif::ProgressBar;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    io::{BufReader, BufWriter},
    path::PathBuf,
};

#[derive(Serialize)]
struct PaintingToSerialize<'a> {
    pub qid: u64,
    pub artist: &'a str,
    pub title: &'a str,
    pub width: f64,
    pub height: f64,
    pub materials: String,
    pub collection: &'a str,
    pub filename: &'a str,
}

#[derive(Serialize, Deserialize)]
struct PreparedQuery {
    dumpfile: PathBuf,
    qids: Vec<u64>,
    dependency_qids: Vec<u64>,
}

fn get_dependency_label(dependencies: &HashMap<u64, WikidataEntity>, qid: Option<u64>) -> &str {
    qid.map(|qid| {
        dependencies
            .get(&qid)
            .map(|entity| entity.label())
            .flatten()
    })
    .flatten()
    .unwrap_or_default()
}

fn get_dependency_labels(dependencies: &HashMap<u64, WikidataEntity>, qids: Vec<u64>) -> Vec<&str> {
    qids.into_iter()
        .filter_map(|qid| {
            dependencies
                .get(&qid)
                .map(|entity| entity.label())
                .flatten()
        })
        .collect()
}

pub fn execute_wikidata_query(input: PathBuf, output: PathBuf, limit: Option<usize>) -> Result<()> {
    let query: PreparedQuery =
        serde_json::from_reader(BufReader::new(std::fs::File::open(input)?))?;
    let sledcache = sled::open(&sledcache_path_for_dumpfile(&query.dumpfile))?;
    let mut dependencies: HashMap<u64, WikidataEntity> =
        HashMap::with_capacity(query.dependency_qids.len());
    println!("Loading dependencies.");
    let bar = ProgressBar::new(query.dependency_qids.len() as u64);
    for qid in query.dependency_qids.iter() {
        let value = sledcache
            .get(qid.to_be_bytes())?
            .expect("dependency qid in query should exist in sledcache");
        let entity: WikidataEntity = serde_json::from_slice(value.as_ref())?;
        dependencies.insert(*qid, entity);
        bar.inc(1);
    }
    bar.finish();

    println!("Writing {}.", output.display());
    let mut writer = csv::Writer::from_path(output)?;
    let bar = ProgressBar::new(query.qids.len() as u64);
    let mut count = 0;
    for qid in query.qids.iter() {
        if let Some(limit) = limit {
            if count == limit {
                break;
            }
            count += 1;
        }

        let value = sledcache
            .get(qid.to_be_bytes())?
            .expect("qid in query should exist in sledcache");
        let entity: WikidataEntity = serde_json::from_slice(value.as_ref())?;

        // Get required fields.
        let (width, height) = entity.dimensions_in_cm().expect("dimensions should exist");
        let filename = entity
            .image_filename()
            .expect("filename should exist")
            .as_str();

        // Get optional fields.
        let title = entity.label().unwrap_or_default();
        let artist = get_dependency_label(&dependencies, entity.creator_id());
        let materials = get_dependency_labels(&dependencies, entity.material_ids());
        let collection = get_dependency_label(&dependencies, entity.collection_id());

        writer.serialize(PaintingToSerialize {
            qid: *qid,
            artist,
            title,
            width,
            height,
            materials: materials.join(", "),
            collection,
            filename,
        })?;

        bar.inc(1);
    }
    bar.finish();
    writer.flush()?;
    Ok(())
}

pub fn prepare_wikidata_query(
    output: PathBuf,
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
    let mut final_qids_with_required_fields: Vec<u64> = Vec::with_capacity(expected_total);
    let mut dependency_qids: HashSet<u64> = HashSet::new();
    let bar = ProgressBar::new(expected_total as u64);
    println!("Processing {} entities.", expected_total);
    for result in iter_and_cache_entities(dumpfile_path.clone(), qids, warnings)? {
        let CachedEntityInfo {
            entity,
            percent_done,
            count,
            qid,
        } = result?;
        total = count;
        let has_image = entity.image_filename().is_some();
        let dimensions = entity.dimensions_in_cm();
        if has_image && dimensions.is_some() {
            final_qids_with_required_fields.push(qid);
        } else if warnings {
            println!(
                "Warning: Q{qid} ({:?}) is missing required fields, image={:?}, dimensions={:?}",
                entity.label().unwrap_or_default(),
                entity.image_filename(),
                dimensions
            );
        }
        if let Some(creator_qid) = entity.creator_id() {
            dependency_qids.insert(creator_qid);
        }
        if let Some(collection_qid) = entity.collection_id() {
            dependency_qids.insert(collection_qid);
        }
        for material_qid in entity.material_ids() {
            dependency_qids.insert(material_qid);
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
        "Done processing {total} entities, {} have all required fields, {} were not found.",
        final_qids_with_required_fields.len(),
        expected_total - total
    );
    let dependency_qids =
        cache_and_get_dependency_qids(dumpfile_path.clone(), dependency_qids, verbose, warnings)?;
    let prepared_query = PreparedQuery {
        dumpfile: dumpfile_path,
        qids: final_qids_with_required_fields,
        dependency_qids,
    };
    let output_file = std::fs::File::create(output.clone())?;
    let output_writer = BufWriter::new(output_file);
    serde_json::to_writer(output_writer, &prepared_query)?;
    println!("Wrote {}.", output.display());
    Ok(())
}

fn cache_and_get_dependency_qids(
    dumpfile_path: PathBuf,
    dependency_qids: HashSet<u64>,
    verbose: bool,
    warnings: bool,
) -> Result<Vec<u64>> {
    let dependency_qids = dependency_qids.into_iter().collect::<Vec<_>>();
    let expected_total = dependency_qids.len();
    let mut final_dependency_qids: Vec<u64> = Vec::with_capacity(expected_total);
    if expected_total > 0 {
        let bar = ProgressBar::new(expected_total as u64);
        println!("Processing {} dependency entities.", expected_total);
        for result in iter_and_cache_entities(dumpfile_path, dependency_qids, warnings)? {
            let CachedEntityInfo {
                entity,
                percent_done,
                qid,
                ..
            } = result?;
            final_dependency_qids.push(qid);
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
        let total = final_dependency_qids.len();
        println!(
            "Done processing {total} dependencies, {} were not found.",
            expected_total - total
        );
    }
    Ok(final_dependency_qids)
}
