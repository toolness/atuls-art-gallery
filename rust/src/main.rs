use std::fs::File;
use std::path::{Path, PathBuf};
use std::process;

use clap::Parser;
use serde::{de, Deserialize};

use anyhow::{anyhow, Result};

use std::io::BufReader;

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

fn get_cached_path<T: AsRef<str>>(filename: T) -> PathBuf {
    let cache_dir = Path::new("cache");
    cache_dir.join(filename.as_ref())
}

fn cache_json_url<T: AsRef<str>, U: AsRef<str>>(url: T, filename: U) -> Result<()> {
    let filename_path = get_cached_path(filename);
    if filename_path.exists() {
        return Ok(());
    }
    println!("Caching {} -> {}...", url.as_ref(), filename_path.display());
    let response = ureq::get(url.as_ref()).call()?;
    if response.status() != 200 {
        return Err(anyhow!("Got HTTP {}", response.status()));
    }
    if response.content_type() != "application/json" {
        return Err(anyhow!("Content type is {}", response.content_type()));
    }
    let response_body = response.into_string()?;
    let json_body: serde_json::Value = serde_json::from_str(response_body.as_ref())?;
    let pretty_printed = serde_json::to_string_pretty(&json_body)?;

    std::fs::write(filename_path, pretty_printed)?;

    Ok(())
}

// By default, struct field names are deserialized based on the position of
// a corresponding field in the CSV data's header record.
#[derive(Debug, Deserialize)]
struct CsvRecord {
    #[serde(rename = "Is Public Domain", deserialize_with = "deserialize_bool")]
    public_domain: bool,

    #[serde(rename = "Object ID")]
    object_id: u64,

    #[serde(rename = "Title")]
    title: String,

    #[serde(rename = "Medium")]
    medium: String,

    #[serde(rename = "Link Resource")]
    link_resource: String,
}

fn deserialize_csv_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: de::Deserializer<'de>,
{
    let s: &str = de::Deserialize::deserialize(deserializer)?;

    match s {
        "True" => Ok(true),
        "False" => Ok(false),
        _ => Err(de::Error::unknown_variant(s, &["True", "False"])),
    }
}

fn run() -> Result<()> {
    let args = Args::parse();
    let csv_file = get_cached_path("MetObjects.csv");
    let reader = BufReader::new(File::open(csv_file)?);
    let mut rdr = csv::Reader::from_reader(reader);
    let mut count = 0;
    let medium_keywords = vec![
        "watercolor",
        "lithograph",
        "oil",
        "photo",
        "drawing",
        "gouache",
        "chalk",
        "canvas",
        "ink",
        "paper",
        "print",
        "aquatint",
    ];
    for result in rdr.deserialize() {
        // Notice that we need to provide a type hint for automatic
        // deserialization.
        let record: CsvRecord = result?;
        if !record.public_domain {
            continue;
        }
        let mut found_keyword = false;
        let lower_medium = record.medium.to_lowercase();
        for medium_keyword in medium_keywords.iter() {
            if lower_medium.contains(medium_keyword) {
                found_keyword = true;
                break;
            }
        }
        if !found_keyword {
            continue;
        }
        count += 1;
        if args.verbose {
            println!(
                "#{}: medium={} title={} {}",
                record.object_id, record.medium, record.title, record.link_resource
            );
        }
        if args.download {
            cache_json_url(
                format!(
                    "https://collectionapi.metmuseum.org/public/collection/v1/objects/{}",
                    record.object_id
                ),
                format!("object-{}.json", record.object_id),
            )?;
        }
        if let Some(max) = args.max {
            if count >= max {
                println!("Reached max of {count} objects.");
                break;
            }
        }
    }
    println!("Processed {count} records.");
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        println!("error running example: {}", err);
        process::exit(1);
    }
}
