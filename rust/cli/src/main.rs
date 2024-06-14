use std::fs::{self, File};
use std::path::PathBuf;
use std::process;

use anyhow::Result;
use clap::{Parser, Subcommand};
use gallery::gallery_cache::GalleryCache;
use gallery::gallery_db::{GalleryDb, LayoutRecord, PublicDomain2DMetObjectRecord};
use gallery::gallery_wall::GalleryWall;
use gallery::met_api::load_met_api_record;
use gallery::met_csv::iter_public_domain_2d_met_csv_objects;
use rusqlite::Connection;

use std::io::BufReader;

const TRANSACTION_BATCH_SIZE: usize = 250;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// Verbose output
    #[arg(short, long, default_value_t = false)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
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
    Layout {},
}

fn run() -> Result<()> {
    let args = Args::parse();
    let manifest_dir: PathBuf = env!("CARGO_MANIFEST_DIR").into();
    let cache_dir = manifest_dir.join("..").join("cache");
    let cache = GalleryCache::new(cache_dir);
    let db_path = cache.get_cached_path("gallery.sqlite");
    let db = GalleryDb::new(Connection::open(db_path)?);
    match args.command {
        Commands::Csv { max, download } => csv_command(args, cache, db, max, download),
        Commands::Layout {} => layout_command(manifest_dir, db),
    }
}

fn layout_command(manifest_dir: PathBuf, mut db: GalleryDb) -> Result<()> {
    let walls_json_file = manifest_dir
        .join("..")
        .join("..")
        .join("Levels")
        .join("moma-gallery.walls.json");
    let walls: Vec<GalleryWall> = serde_json::from_str(&fs::read_to_string(walls_json_file)?)?;
    db.reset_layout_table()?;
    let mut met_objects = db.get_all_met_objects_for_layout()?;
    met_objects.reverse();
    println!(
        "Laying out {} met objects across galleries with {} walls each.",
        met_objects.len(),
        walls.len()
    );
    let mut layout_records: Vec<LayoutRecord<&str>> = vec![];
    let mut wall_idx = 0;
    let mut gallery_id = 1;
    loop {
        let wall = walls.get(wall_idx).unwrap();
        let Some(met_object) = met_objects.pop() else {
            break;
        };
        let x = wall.width / 2.0;
        let y = wall.height / 2.0;
        layout_records.push(LayoutRecord {
            gallery_id,
            wall_id: &wall.name,
            met_object_id: met_object.id,
            x,
            y,
        });
        wall_idx += 1;
        if wall_idx == walls.len() {
            wall_idx = 0;
            gallery_id += 1;
        }
    }
    db.add_layout_records(&layout_records)?;
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
            obj_record.try_to_download_small_image(&cache)?;
        }
        records_to_commit.push(csv_record);
        if records_to_commit.len() >= TRANSACTION_BATCH_SIZE {
            if args.verbose {
                println!("Committing {} records.", records_to_commit.len());
            }
            db.add_csv_records(&records_to_commit)?;
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
        db.add_csv_records(&records_to_commit)?;
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
