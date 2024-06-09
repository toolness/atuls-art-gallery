use std::fs::File;
use std::process;

use anyhow::Result;
use clap::Parser;
use gallery::gallery_cache::GalleryCache;
use gallery::gallery_db::GalleryDb;
use gallery::the_met::{
    iter_public_domain_2d_met_objects, load_met_object_record, MetObjectCsvRecord,
};
use rusqlite::Connection;

use std::io::BufReader;

const TRANSACTION_BATCH_SIZE: usize = 250;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Max objects to process
    #[arg(short, long)]
    max: Option<usize>,

    /// Download objects?
    #[arg(short, long, default_value_t = false)]
    download: bool,

    /// Verbose output
    #[arg(short, long, default_value_t = false)]
    verbose: bool,
}

fn run() -> Result<()> {
    let args = Args::parse();
    let cache = GalleryCache::new("cache".into());
    let csv_file = cache.get_cached_path("MetObjects.csv");
    let reader = BufReader::new(File::open(csv_file)?);
    let rdr = csv::Reader::from_reader(reader);
    let db_path = cache.get_cached_path("gallery.sqlite");
    let mut db = GalleryDb::new(Connection::open(db_path)?);
    db.reset_met_objects_table()?;
    let mut count: usize = 0;
    let mut records_to_commit: Vec<MetObjectCsvRecord> = vec![];
    for result in iter_public_domain_2d_met_objects(rdr) {
        // Notice that we need to provide a type hint for automatic
        // deserialization.
        let csv_record: MetObjectCsvRecord = result?;
        count += 1;
        if args.verbose {
            println!(
                "#{}: medium={} title={}",
                csv_record.object_id, csv_record.medium, csv_record.title
            );
        }
        if args.download {
            let obj_record = load_met_object_record(&cache, csv_record.object_id)?;
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
        if let Some(max) = args.max {
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
