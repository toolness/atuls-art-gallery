use anyhow::Result;
use regex_lite::Regex;
use serde::{de, Deserialize};

// By default, struct field names are deserialized based on the position of
// a corresponding field in the CSV data's header record.
#[derive(Debug, Deserialize)]
pub struct MetObjectCsvRecord {
    #[serde(rename = "Is Public Domain", deserialize_with = "deserialize_csv_bool")]
    pub public_domain: bool,

    #[serde(rename = "Object ID")]
    pub object_id: u64,

    #[serde(rename = "AccessionYear", deserialize_with = "deserialize_csv_year")]
    pub accession_year: Option<u16>,

    #[serde(rename = "Object Date")]
    pub object_date: String,

    #[serde(rename = "Title")]
    pub title: String,

    #[serde(rename = "Medium")]
    pub medium: String,

    #[serde(rename = "Dimensions")]
    pub dimensions: String,

    pub parsed_dimensions: Option<(f64, f64)>,
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

/// A very small number of records have malformed year numbers, in such
/// cases, we'll just ignore the field instead of erroring.
fn deserialize_csv_year<'de, D>(deserializer: D) -> Result<Option<u16>, D::Error>
where
    D: de::Deserializer<'de>,
{
    match de::Deserialize::deserialize(deserializer) {
        Ok(year) => Ok(year),
        Err(_) => Ok(None),
    }
}

const MEDIUM_KEYWORDS: [&str; 12] = [
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

fn is_public_domain_2d_met_object(
    dimension_parser: &DimensionParser,
    csv_record: &mut MetObjectCsvRecord,
) -> bool {
    if !csv_record.public_domain || csv_record.accession_year.is_none() {
        return false;
    }
    let Some(dimensions) = dimension_parser.parse(&csv_record.dimensions) else {
        return false;
    };
    csv_record.parsed_dimensions = Some(dimensions);
    if !dimension_parser.can_parse(&csv_record.dimensions) {
        return false;
    }
    let lower_medium = csv_record.medium.to_lowercase();
    for medium_keyword in MEDIUM_KEYWORDS.iter() {
        if lower_medium.contains(medium_keyword) {
            return true;
        }
    }

    false
}

pub type MetObjectCsvResult = Result<MetObjectCsvRecord, csv::Error>;

pub fn iter_public_domain_2d_met_objects<R: std::io::Read>(
    reader: csv::Reader<R>,
) -> impl Iterator<Item = MetObjectCsvResult> {
    let parser = DimensionParser::new();
    reader
        .into_deserialize::<MetObjectCsvRecord>()
        .filter_map(move |result| match result {
            Ok(mut csv_record) => {
                if !is_public_domain_2d_met_object(&parser, &mut csv_record) {
                    return None;
                }
                return Some(Ok(csv_record));
            } //is_public_domain_2d_met_object(&parser, csv_record),
            Err(_) => Some(result),
        })
}

const DIMENSIONS_REGEX: &'static str = r"^.+ \(([0-9.]+) x ([0-9.]+) cm\)$";

struct DimensionParser {
    regex: Regex,
}

impl DimensionParser {
    pub fn new() -> Self {
        Self {
            regex: Regex::new(&DIMENSIONS_REGEX).unwrap(),
        }
    }

    pub fn can_parse<T: AsRef<str>>(&self, value: T) -> bool {
        self.parse(value.as_ref()).is_some()
    }

    /// Return a (width, height) tuple of the dimensions. Note that this
    /// is the opposite order from the format in the data; we're using
    /// (width, height) because it's the common one in computer graphics.
    pub fn parse<T: AsRef<str>>(&self, value: T) -> Option<(f64, f64)> {
        match self.regex.captures(value.as_ref()) {
            None => None,
            Some(caps) => {
                let height = caps[1].parse::<f64>();
                let width = caps[2].parse::<f64>();
                match (width, height) {
                    (Ok(width), Ok(height)) => Some((width, height)),
                    _ => None,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::met_csv::DimensionParser;

    #[test]
    fn test_dimensions_is_match_works() {
        let parser = DimensionParser::new();

        assert!(parser.can_parse("9 3/4 x 11 3/8 in. (24.8 x 28.9 cm)"));
        assert!(parser.can_parse("9 3/4 x 11 3/8 in. (24 x 28.9 cm)"));
        assert!(!parser.can_parse("H. 2 1/2 in. (6.4 cm); Diam. 8 1/8 in. (20.6 cm)"));
    }

    #[test]
    fn test_dimensions_parse_works() {
        let parser = DimensionParser::new();

        assert_eq!(
            parser.parse("9 3/4 x 11 3/8 in. (24.8 x 28.9 cm)"),
            Some((28.9, 24.8))
        );
        assert_eq!(
            parser.parse("9 3/4 x 11 3/8 in. (24 x 28.9 cm)"),
            Some((28.9, 24.0))
        );
        assert_eq!(
            parser.parse("H. 2 1/2 in. (6.4 cm); Diam. 8 1/8 in. (20.6 cm)"),
            None
        );
    }
}
