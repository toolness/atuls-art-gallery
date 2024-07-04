mod layout;
mod met_csv;
mod random;

use std::fs::{self, File};
use std::path::PathBuf;
use std::process;

use anyhow::Result;
use clap::{Parser, Subcommand};
use gallery::gallery_cache::GalleryCache;
use gallery::gallery_db::{GalleryDb, LayoutRecord, PublicDomain2DMetObjectRecord};
use gallery::gallery_wall::GalleryWall;
use gallery::met_api::{load_met_api_record, ImageSize};
use layout::{place_paintings_along_wall, MetObjectLayoutFitter};
use met_csv::iter_public_domain_2d_met_csv_objects;
use random::Rng;
use rusqlite::Connection;

use std::io::BufReader;

const TRANSACTION_BATCH_SIZE: usize = 250;

const LAYOUT_START_GALLERY_ID: i64 = 1;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// Verbose output
    #[arg(short, long, default_value_t = false)]
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
    AccessionYear,
    Random,
}

#[derive(Subcommand)]
enum Commands {
    /// Import MetObjects.csv into database.
    Csv {
        /// Max objects to process
        #[arg(short, long)]
        max: Option<usize>,

        /// Download objects?
        #[arg(short, long, default_value_t = false)]
        download: bool,
    },
    /// Layout gallery walls.
    Layout {
        /// How to sort the art in the galleries. Defaults to met object ID.
        #[arg(short, long)]
        sort: Option<Sort>,

        /// Random seed to use, if sort is random. If absent, will use time since epoch, in seconds.
        #[arg(short, long)]
        random_seed: Option<u64>,
    },
    /// Show layout for the given gallery.
    ShowLayout {
        /// Gallery id to show.
        #[arg()]
        gallery_id: i64,
    },
}

fn run() -> Result<()> {
    let args = Args::parse();
    let manifest_dir: PathBuf = env!("CARGO_MANIFEST_DIR").into();
    let cache_dir = manifest_dir.join("..").join("cache");
    let cache = GalleryCache::new(cache_dir);
    let db_path = if let Some(db_path) = &args.db_path {
        db_path.clone()
    } else {
        cache.get_cached_path("gallery2.sqlite")
    };
    let db = GalleryDb::new(Connection::open(db_path)?);
    match args.command {
        Commands::Csv { max, download } => csv_command(args, cache, db, max, download),
        Commands::Layout { sort, random_seed } => layout_command(db, sort, random_seed),
        Commands::ShowLayout { gallery_id } => show_layout_command(db, gallery_id),
    }
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

fn show_layout_command(mut db: GalleryDb, gallery_id: i64) -> Result<()> {
    let walls = get_walls()?;
    for wall in walls {
        println!("Wall {}:", wall.name);
        for (object, layout) in db.get_met_objects_for_gallery_wall(gallery_id, wall.name)? {
            println!("  {:?} {:?}", object, layout);
        }
    }
    Ok(())
}

fn layout_command(mut db: GalleryDb, sort: Option<Sort>, random_seed: Option<u64>) -> Result<()> {
    let walls = get_walls()?;
    db.reset_layout_table()?;
    let mut met_objects = db.get_all_met_objects_for_layout(match sort.unwrap_or_default() {
        Sort::Id => Some("id"),
        Sort::AccessionYear => Some("accession_year, id"),
        Sort::Random => None,
    })?;
    if matches!(sort, Some(Sort::Random)) {
        let random_seed = random_seed.unwrap_or(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        );
        let mut rng = Rng::new(random_seed);
        println!("Randomizing layout using seed {random_seed}.");
        rng.shuffle(&mut met_objects);
    } else {
        met_objects.reverse();
    }
    println!(
        "Laying out {} met objects across galleries with {} walls each.",
        met_objects.len(),
        walls.len()
    );
    let mut finder = MetObjectLayoutFitter::new(met_objects);
    let mut layout_records: Vec<LayoutRecord<&str>> = vec![];
    let mut wall_idx = 0;
    let mut gallery_id = LAYOUT_START_GALLERY_ID;
    while !finder.is_empty() {
        let wall = walls.get(wall_idx).unwrap();
        place_paintings_along_wall(
            gallery_id,
            &walls,
            &wall.name,
            &mut finder,
            0.0,
            wall.width,
            wall.height,
            &mut layout_records,
        );
        wall_idx += 1;
        if wall_idx == walls.len() {
            wall_idx = 0;
            gallery_id += 1;
        }
    }
    db.upsert_layout_records(&layout_records)?;
    println!("Created a layout with {} galleries.", gallery_id);
    Ok(())
}

fn csv_command(
    args: Args,
    cache: GalleryCache,
    mut db: GalleryDb,
    max: Option<usize>,
    download: bool,
) -> Result<()> {
    let csv_file = cache.get_cached_path("MetObjects.csv");
    let reader = BufReader::new(File::open(csv_file)?);
    let rdr = csv::Reader::from_reader(reader);
    db.reset_met_objects_table()?;
    let mut count: usize = 0;
    let mut records_to_commit = vec![];
    for result in iter_public_domain_2d_met_csv_objects(rdr) {
        // Notice that we need to provide a type hint for automatic
        // deserialization.
        let csv_record: PublicDomain2DMetObjectRecord = result?;
        count += 1;
        if args.verbose {
            println!(
                "#{}: medium={} title={}",
                csv_record.object_id, csv_record.medium, csv_record.title
            );
        }
        if download {
            let obj_record = load_met_api_record(&cache, csv_record.object_id)?;
            obj_record.try_to_download_image(&cache, ImageSize::Small)?;
        }
        records_to_commit.push(csv_record);
        if records_to_commit.len() >= TRANSACTION_BATCH_SIZE {
            if args.verbose {
                println!("Committing {} records.", records_to_commit.len());
            }
            db.add_public_domain_2d_met_objects(&records_to_commit)?;
            records_to_commit.clear();
        }
        if let Some(max) = max {
            if count >= max {
                println!("Reached max of {count} objects.");
                break;
            }
        }
    }
    if records_to_commit.len() > 0 {
        if args.verbose {
            println!("Committing {} records.", records_to_commit.len());
        }
        db.add_public_domain_2d_met_objects(&records_to_commit)?;
    }
    println!("Processed {count} records.");
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

    use gallery::{gallery_cache::GalleryCache, gallery_db::LayoutRecord};
    use rusqlite::Connection;

    use crate::met_csv::iter_public_domain_2d_met_csv_objects;

    use super::GalleryDb;

    #[test]
    fn test_it_works() {
        let mut db = GalleryDb::new(Connection::open_in_memory().unwrap());
        db.reset_met_objects_table().unwrap();
        db.reset_layout_table().unwrap();

        let manifest_dir: PathBuf = env!("CARGO_MANIFEST_DIR").into();
        let cache = GalleryCache::new(manifest_dir.join("..").join("test_data"));
        let csv_file = cache.get_cached_path("MetObjects.csv");
        let reader = BufReader::new(File::open(csv_file).unwrap());
        let rdr = csv::Reader::from_reader(reader);
        let mut records = vec![];
        for result in iter_public_domain_2d_met_csv_objects(rdr) {
            records.push(result.unwrap());
        }
        db.add_public_domain_2d_met_objects(&records).unwrap();

        let rows = db.get_all_met_objects_for_layout(None).unwrap();
        assert!(rows.len() > 0);
        let met_object_id = rows.get(0).unwrap().id;

        // Add a painting to the layout.
        db.upsert_layout_records(&vec![LayoutRecord {
            gallery_id: 5,
            wall_id: "wall_1",
            met_object_id,
            x: 1.0,
            y: 6.0,
        }])
        .unwrap();

        // Make sure it got placed where we placed it.
        let (record, (x, y)) = db
            .get_met_objects_for_gallery_wall(5, "wall_1")
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(record.object_id, met_object_id);
        assert_eq!(x, 1.0);
        assert_eq!(y, 6.0);

        // Make sure there's nothing in the place we want to move it to.
        assert_eq!(
            db.get_met_objects_for_gallery_wall(6, "wall_2")
                .unwrap()
                .len(),
            0
        );

        // Move the painting.
        db.upsert_layout_records(&vec![LayoutRecord {
            gallery_id: 6,
            wall_id: "wall_2",
            met_object_id,
            x: 4.0,
            y: 9.0,
        }])
        .unwrap();

        // Make sure there's nothing in the place we moved it from.
        assert_eq!(
            db.get_met_objects_for_gallery_wall(5, "wall_1")
                .unwrap()
                .len(),
            0
        );

        // Make sure it actually got moved to where we moved it.
        let (record, (x, y)) = db
            .get_met_objects_for_gallery_wall(6, "wall_2")
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(record.object_id, met_object_id);
        assert_eq!(x, 4.0);
        assert_eq!(y, 9.0);
    }
}
