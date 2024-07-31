mod met_csv;
mod wikidata_dump;

use std::collections::HashSet;
use std::fs::{self, File};
use std::path::PathBuf;
use std::process;

use anyhow::Result;
use clap::{Parser, Subcommand};
use gallery::art_object::ArtObjectId;
use gallery::gallery_cache::GalleryCache;
use gallery::gallery_db::{
    ArtObjectQueryOptions, ArtObjectRecord, GalleryDb, DEFAULT_GALLERY_DB_FILENAME,
};
use gallery::gallery_wall::GalleryWall;
use gallery::layout::layout;
use gallery::random::Rng;
use indicatif::{ProgressBar, ProgressStyle};
use met_csv::{iter_public_domain_2d_met_csv_objects, PublicDomain2DMetObjectOptions};
use rusqlite::Connection;
use wikidata_dump::{
    execute_wikidata_query, index_wikidata_dump, iter_wikidata_objects, prepare_wikidata_query,
};

use std::io::BufReader;

const TRANSACTION_BATCH_SIZE: usize = 1000;

const LAYOUT_START_GALLERY_ID: i64 = 1;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// Verbose output
    #[arg(short, long, default_value_t = false, global = true)]
    verbose: bool,

    /// Path to database
    #[arg(short, long)]
    db_path: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Copy, Clone, Default, clap::ValueEnum)]
enum Sort {
    #[default]
    Id,
    Random,
}

#[derive(Subcommand)]
enum Commands {
    /// Import MetObjects.csv into database.
    Csv {
        /// Path to met objects CSV
        #[arg(short, long)]
        met_objects_path: Option<PathBuf>,

        /// Path to wikidata objects CSV
        #[arg(short, long)]
        wikidata_objects_path: Option<PathBuf>,

        /// Max objects to process
        #[arg(short, long)]
        max: Option<usize>,

        /// Normally we filter to ensure that only art that is flat and matte
        /// is in the gallery. This disables the filter, which will result in
        /// more photos of artifacts that are in the collection showing up
        /// in your gallery.
        #[arg(long, default_value_t = false)]
        met_objects_all_media: bool,

        /// Log warnings about whether e.g. something that claims to not be
        /// public domain is actually public domain.
        #[arg(long, default_value_t = false)]
        warnings: bool,
    },
    /// Layout gallery walls.
    Layout {
        /// How to sort the art in the galleries. Defaults to art object ID.
        #[arg(short, long)]
        sort: Option<Sort>,

        /// Random seed to use, if sort is random. If absent, will use time since epoch, in seconds.
        #[arg(short, long)]
        random_seed: Option<u64>,

        /// Filter artwork to only those matching this value.
        #[arg(short, long)]
        filter: Option<String>,

        /// Whether to use a dense layout (stack some art vertically).
        #[arg(long = "dense", default_value_t = false)]
        use_dense_layout: bool,

        /// Log warnings about whether e.g. a painting won't fit in a gallery.
        #[arg(long, default_value_t = false)]
        warnings: bool,
    },
    /// Show layout for the given gallery.
    ShowLayout {
        /// Gallery id to show.
        #[arg()]
        gallery_id: i64,
    },
    /// Index QIDs in wikidata dump file.
    WikidataIndex {
        #[arg()]
        dumpfile: PathBuf,

        #[arg(short, long)]
        seek_from: Option<u64>,
    },
    /// Prepare a query for later execution.
    WikidataPrepare {
        #[arg()]
        dumpfile: PathBuf,

        #[arg()]
        qids: Vec<u64>,

        /// CSV export from query.wikidata.org with a single 'item' column containing entity URLs.
        #[arg(long)]
        csv: Option<PathBuf>,

        /// JSON filename to store the prepared query in.
        #[arg(short, long, required = true)]
        output: PathBuf,

        /// Log warnings about whether e.g. an item doesn't have required fields, or doesn't exist.
        #[arg(long, default_value_t = false)]
        warnings: bool,
    },
    /// Execute a prepared wikidata query.
    WikidataExecute {
        #[arg()]
        input: PathBuf,

        #[arg()]
        output: PathBuf,

        #[arg(short, long)]
        limit: Option<usize>,
    },
    /// Export layout for non-positive galleries.
    ExportLayout {},
}

fn run() -> Result<()> {
    let args = Args::parse();
    let manifest_dir: PathBuf = env!("CARGO_MANIFEST_DIR").into();
    let cache_dir = manifest_dir.join("..").join("cache");
    let cache = GalleryCache::new(cache_dir);
    let db_path = if let Some(db_path) = &args.db_path {
        db_path.clone()
    } else {
        cache.get_cached_path(DEFAULT_GALLERY_DB_FILENAME)
    };
    let db = GalleryDb::new(Connection::open(db_path)?);
    match args.command {
        Commands::Csv {
            met_objects_path,
            wikidata_objects_path,
            max,
            met_objects_all_media,
            warnings,
        } => csv_command(
            args.verbose,
            met_objects_path,
            wikidata_objects_path,
            cache,
            db,
            max,
            met_objects_all_media,
            warnings,
        ),
        Commands::Layout {
            sort,
            random_seed,
            use_dense_layout,
            filter,
            warnings,
        } => layout_command(
            db,
            sort,
            random_seed,
            use_dense_layout,
            filter,
            args.verbose,
            warnings,
        ),
        Commands::ShowLayout { gallery_id } => show_layout_command(db, gallery_id),
        Commands::WikidataIndex {
            dumpfile,
            seek_from,
        } => index_wikidata_dump(dumpfile, seek_from),
        Commands::WikidataPrepare {
            output,
            dumpfile,
            qids,
            csv,
            warnings,
        } => prepare_wikidata_query(output, dumpfile, qids, csv, args.verbose, warnings),
        Commands::WikidataExecute {
            input,
            output,
            limit,
        } => execute_wikidata_query(input, output, limit),
        Commands::ExportLayout {} => export_layout(db),
    }
}

fn export_layout(mut db: GalleryDb) -> Result<()> {
    let records = db.get_layout_records_in_non_positive_galleries()?;
    let json = serde_json::to_string_pretty(&records)?;
    println!("{}", json);
    Ok(())
}

fn get_walls() -> Result<Vec<GalleryWall>> {
    let manifest_dir: PathBuf = env!("CARGO_MANIFEST_DIR").into();
    let walls_json_file = manifest_dir
        .join("..")
        .join("..")
        .join("Levels")
        .join("moma-gallery.walls.json");
    let walls: Vec<GalleryWall> = serde_json::from_str(&fs::read_to_string(walls_json_file)?)?;
    Ok(walls)
}

fn show_layout_command(db: GalleryDb, gallery_id: i64) -> Result<()> {
    let walls = get_walls()?;
    for wall in walls {
        println!("Wall {}:", wall.name);
        for (object, layout) in db.get_art_objects_for_gallery_wall(gallery_id, wall.name)? {
            println!("  {:?} {:?}", object, layout);
        }
    }
    Ok(())
}

fn layout_command(
    mut db: GalleryDb,
    sort: Option<Sort>,
    random_seed: Option<u64>,
    use_dense_layout: bool,
    filter: Option<String>,
    verbose: bool,
    warnings: bool,
) -> Result<()> {
    let walls = get_walls()?;
    db.reset_layout_table()?;

    let options = ArtObjectQueryOptions {
        filter,
        ..Default::default()
    };

    if verbose && options.filter.is_some() {
        let (query, params) = options.where_clause();
        println!("Filter SQL: {query}");
        for (id, param) in params.iter().enumerate() {
            println!("Param #{}: {:?}", id + 1, param)
        }
    }

    let mut art_objects = db.get_all_art_objects_for_layout(&options)?;
    if matches!(sort, Some(Sort::Random)) {
        let mut rng = Rng::new(random_seed);
        println!("Randomizing layout using seed {}.", rng.seed);
        rng.shuffle(&mut art_objects);
    }
    println!(
        "Laying out {} art objects across galleries with {} walls each.",
        art_objects.len(),
        walls.len()
    );

    let (galleries_created, layout_records) = layout(
        use_dense_layout,
        LAYOUT_START_GALLERY_ID,
        &walls,
        art_objects,
        &HashSet::new(),
        warnings,
    )?;

    db.set_layout_records_in_positive_galleries(&layout_records)?;
    println!("Created a layout with {} galleries.", galleries_created);

    Ok(())
}

fn csv_command(
    verbose: bool,
    met_objects_path: Option<PathBuf>,
    wikidata_objects_path: Option<PathBuf>,
    cache: GalleryCache,
    mut db: GalleryDb,
    max: Option<usize>,
    met_objects_all_media: bool,
    warnings: bool,
) -> Result<()> {
    let met_csv_file = met_objects_path.unwrap_or(cache.get_cached_path("MetObjects.csv"));
    println!("Loading met objects from {}.", met_csv_file.display());
    let wikidata_csv_file =
        wikidata_objects_path.unwrap_or(cache.get_cached_path("WikidataObjects.csv"));
    println!("Loading wikidata objects from {}.", met_csv_file.display());
    let met_reader = BufReader::new(File::open(met_csv_file)?);
    let met_csv_reader = csv::Reader::from_reader(met_reader);
    let wikidata_reader = BufReader::new(File::open(wikidata_csv_file)?);
    let wikidata_objects_iterator =
        iter_wikidata_objects(csv::Reader::from_reader(wikidata_reader));
    db.reset_art_objects_table()?;
    let mut count: usize = 0;
    let mut records_to_commit = vec![];
    let met_objects_iterator = iter_public_domain_2d_met_csv_objects(
        met_csv_reader,
        PublicDomain2DMetObjectOptions {
            all_media: met_objects_all_media,
            warnings,
            ..Default::default()
        },
    );
    let bar = ProgressBar::new_spinner();
    bar.set_style(ProgressStyle::with_template("[{elapsed_precise}] {spinner} {msg}").unwrap());
    let mut fallback_wikidata_qids: HashSet<i64> = HashSet::new();

    // We should always put wikidata last, as we want to know what wikidata fallback QIDs
    // from the other collections we've processed so we can skip the same ones in the
    // wikidata to avoid duplicates.
    let combined_iterator =
        Box::new(met_objects_iterator).chain(Box::new(wikidata_objects_iterator));

    for result in combined_iterator {
        // Notice that we need to provide a type hint for automatic
        // deserialization.
        let csv_record: ArtObjectRecord = result?;
        if let Some(qid) = csv_record.fallback_wikidata_qid {
            fallback_wikidata_qids.insert(qid);
        } else if let ArtObjectId::Wikidata(qid) = csv_record.object_id {
            if fallback_wikidata_qids.contains(&qid) {
                // This wikidata item is already the fallback for an item from another CSV
                // we've processed. Skip it, since we don't want duplicates.
                continue;
            }
        }
        if csv_record.height <= 0.0 || csv_record.width <= 0.0 {
            if warnings {
                println!(
                    "Skipping {:?} due to invalid dimensions.",
                    csv_record.object_id
                );
            }
            continue;
        }
        count += 1;
        if verbose {
            println!(
                "#{:?}: medium={} title={}",
                csv_record.object_id, csv_record.medium, csv_record.title
            );
        }
        records_to_commit.push(csv_record);
        if records_to_commit.len() >= TRANSACTION_BATCH_SIZE {
            if verbose {
                println!("Committing {} records.", records_to_commit.len());
            }
            db.add_art_objects(&records_to_commit)?;
            records_to_commit.clear();
            bar.tick();
            bar.set_message(format!("Processed {count} records."));
        }
        if let Some(max) = max {
            if count >= max {
                println!("Reached max of {count} objects.");
                break;
            }
        }
    }
    if records_to_commit.len() > 0 {
        if verbose {
            println!("Committing {} records.", records_to_commit.len());
        }
        db.add_art_objects(&records_to_commit)?;
    }
    bar.set_message(format!("Processed {count} records."));
    bar.finish();
    println!("Done.");
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        println!("error: {}", err);
        process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader, path::PathBuf};

    use gallery::gallery_cache::GalleryCache;
    use rusqlite::Connection;

    use crate::met_csv::iter_public_domain_2d_met_csv_objects;

    use super::GalleryDb;

    #[test]
    fn test_it_works() {
        let mut db = GalleryDb::new(Connection::open_in_memory().unwrap());
        db.reset_art_objects_table().unwrap();
        db.reset_layout_table().unwrap();

        let manifest_dir: PathBuf = env!("CARGO_MANIFEST_DIR").into();
        let cache = GalleryCache::new(manifest_dir.join("..").join("test_data"));
        let csv_file = cache.get_cached_path("MetObjects.csv");
        let reader = BufReader::new(File::open(csv_file).unwrap());
        let rdr = csv::Reader::from_reader(reader);
        let mut records = vec![];
        for result in iter_public_domain_2d_met_csv_objects(rdr, Default::default()) {
            records.push(result.unwrap());
        }
        db.add_art_objects(&records).unwrap();

        let rows = db
            .get_all_art_objects_for_layout(&Default::default())
            .unwrap();
        assert!(rows.len() > 0);
    }
}
