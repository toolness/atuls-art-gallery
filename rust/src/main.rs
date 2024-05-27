use std::{error::Error, io, process};

use serde::{de, Deserialize};

// By default, struct field names are deserialized based on the position of
// a corresponding field in the CSV data's header record.
#[derive(Debug, Deserialize)]
struct Record {
    #[serde(rename = "Object Number")]
    object_number: String,

    #[serde(rename = "Is Public Domain", deserialize_with="deserialize_bool")]
    public_domain: bool,

    #[serde(rename = "Object ID")]
    object_id: u64,

    #[serde(rename = "Object Name")]
    object_name: String,

    #[serde(rename = "Title")]
    title: String,

    #[serde(rename = "Medium")]
    medium: String,

    #[serde(rename = "Link Resource")]
    link_resource: String,

    #[serde(rename = "Dimensions")]
    dimensions: String
}

fn deserialize_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
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

fn example() -> Result<(), Box<dyn Error>> {
    let mut rdr = csv::Reader::from_reader(io::stdin());
    let mut count = 0;
    let medium_keywords = vec!["watercolor", "lithograph", "oil", "photo", "drawing", "gouache", "chalk", "canvas", "ink", "paper", "print", "aquatint"];
    for result in rdr.deserialize() {
        // Notice that we need to provide a type hint for automatic
        // deserialization.
        let record: Record = result?;
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
        println!("medium={} title={} {}", record.medium, record.title, record.link_resource);
        //println!("{:?}", record);
    }
    println!("Found {count} records.");
    Ok(())
}

fn main() {
    if let Err(err) = example() {
        println!("error running example: {}", err);
        process::exit(1);
    }
}
