use std::fs::File;
use std::path::{Path, PathBuf};
use std::process;

use clap::Parser;
use serde::{de, Deserialize, Serialize};

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

fn cache_binary_url<T: AsRef<str>, U: AsRef<str>>(url: T, filename: U) -> Result<()> {
    let filename_path = get_cached_path(filename);
    if filename_path.exists() {
        return Ok(());
    }
    println!("Caching {} -> {}...", url.as_ref(), filename_path.display());
    let response = ureq::get(url.as_ref()).call()?;
    if response.status() != 200 {
        return Err(anyhow!("Got HTTP {}", response.status()));
    }
    let mut response_body = response.into_reader();
    let mut outfile = File::create(filename_path)?;
    std::io::copy(&mut response_body, &mut outfile)?;
    Ok(())
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

fn load_cached_string<T: AsRef<str>>(filename: T) -> Result<String> {
    Ok(std::fs::read_to_string(get_cached_path(filename))?)
}

fn load_met_object_record(object_id: u64) -> Result<MetObjectRecord> {
    let filename = format!("object-{}.json", object_id);
    cache_json_url(
        format!(
            "https://collectionapi.metmuseum.org/public/collection/v1/objects/{}",
            object_id
        ),
        &filename,
    )?;
    match serde_json::from_str(&load_cached_string(&filename)?) {
        Ok(record) => Ok(record),
        Err(err) => Err(anyhow!("Failed to load {}: {}", filename, err)),
    }
}

#[derive(Debug, Deserialize)]
struct MetObjectRecord {
    measurements: Option<Vec<Measurements>>,

    #[serde(rename = "primaryImageSmall")]
    primary_image_small: String,

    #[serde(rename = "objectDate")]
    object_date: String,

    #[serde(rename = "objectID")]
    object_id: u64,

    title: String,
}

impl MetObjectRecord {
    pub fn overall_width_and_height(&self) -> Option<(f64, f64)> {
        let Some(measurements) = &self.measurements else {
            return None;
        };
        for measurement in measurements {
            if &measurement.element_name == "Overall" {
                if let (Some(width), Some(height), None) = (
                    measurement.element_measurements.width,
                    measurement.element_measurements.height,
                    measurement.element_measurements.depth,
                ) {
                    return Some((width, height));
                }
            }
        }
        None
    }
}

#[derive(Debug, Deserialize)]
struct Measurements {
    #[serde(rename = "elementName")]
    element_name: String,

    #[serde(rename = "elementMeasurements")]
    element_measurements: ElementMeasurements,
}

#[derive(Debug, Deserialize)]
struct ElementMeasurements {
    #[serde(rename = "Width")]
    width: Option<f64>,

    #[serde(rename = "Height")]
    height: Option<f64>,

    #[serde(rename = "Depth")]
    depth: Option<f64>,
}

#[derive(Debug, Serialize)]
struct SimplifiedRecord {
    object_id: u64,
    title: String,
    date: String,
    width: f64,
    height: f64,
    small_image: String,
}

// By default, struct field names are deserialized based on the position of
// a corresponding field in the CSV data's header record.
#[derive(Debug, Deserialize)]
struct CsvRecord {
    #[serde(rename = "Is Public Domain", deserialize_with = "deserialize_csv_bool")]
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
    let mut simplified_records: Vec<SimplifiedRecord> = Vec::new();
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
        let csv_record: CsvRecord = result?;
        if !csv_record.public_domain {
            continue;
        }
        let mut found_keyword = false;
        let lower_medium = csv_record.medium.to_lowercase();
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
                csv_record.object_id, csv_record.medium, csv_record.title, csv_record.link_resource
            );
        }
        if args.download {
            let obj_record = load_met_object_record(csv_record.object_id)?;
            if let Some((width, height)) = obj_record.overall_width_and_height() {
                if obj_record.primary_image_small.ends_with(".jpg") {
                    let small_image = format!("object-{}-small.jpg", csv_record.object_id);
                    cache_binary_url(&obj_record.primary_image_small, &small_image)?;
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
        let simplified_index = get_cached_path("_simple-index.json");
        let pretty_printed = serde_json::to_string_pretty(&simplified_records)?;
        println!("Writing {}.", simplified_index.display());
        std::fs::write(simplified_index, pretty_printed)?;
    }
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        println!("error running example: {}", err);
        process::exit(1);
    }
}
