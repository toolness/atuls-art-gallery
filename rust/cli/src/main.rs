use std::fs::File;
use std::process;

use clap::Parser;
use gallery::gallery_cache::GalleryCache;
use gallery::the_met::{
    is_public_domain_2d_met_object, load_met_object_record, DimensionParser, MetObjectCsvRecord,
};
use serde::Serialize;

use anyhow::Result;

use std::io::BufReader;

#[derive(Debug, Serialize)]
struct SimplifiedRecord {
    object_id: u64,
    title: String,
    date: String,
    width: f64,
    height: f64,
    small_image: String,
}

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
    let mut simplified_records: Vec<SimplifiedRecord> = Vec::new();
    let args = Args::parse();
    let cache = GalleryCache::new("cache".into());
    let csv_file = cache.get_cached_path("MetObjects.csv");
    let reader = BufReader::new(File::open(csv_file)?);
    let mut rdr = csv::Reader::from_reader(reader);
    let mut count: usize = 0;
    let dimension_parser = DimensionParser::new();
    for result in rdr.deserialize() {
        // Notice that we need to provide a type hint for automatic
        // deserialization.
        let csv_record: MetObjectCsvRecord = result?;
        if !is_public_domain_2d_met_object(&dimension_parser, &csv_record) {
            continue;
        }
        count += 1;
        if args.verbose {
            println!(
                "#{}: medium={} title={} {}",
                csv_record.object_id, csv_record.medium, csv_record.title, csv_record.link_resource
            );
        }
        if args.download {
            let obj_record = load_met_object_record(&cache, csv_record.object_id)?;
            if let Some((width, height, small_image)) =
                obj_record.try_to_download_small_image(&cache)?
            {
                simplified_records.push(SimplifiedRecord {
                    object_id: obj_record.object_id,
                    title: obj_record.title,
                    date: obj_record.object_date,
                    width,
                    height,
                    small_image,
                });
            }
        }
        if let Some(max) = args.max {
            if count >= max {
                println!("Reached max of {count} objects.");
                break;
            }
        }
    }
    println!("Processed {count} records.");
    if !simplified_records.is_empty() {
        let simplified_index = cache.get_cached_path("_simple-index.json");
        let pretty_printed = serde_json::to_string_pretty(&simplified_records)?;
        println!("Writing {}.", simplified_index.display());
        std::fs::write(simplified_index, pretty_printed)?;
    }
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        println!("error: {}", err);
        process::exit(1);
    }
}
