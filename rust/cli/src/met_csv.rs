use anyhow::Result;
use gallery::gallery_db::PublicDomain2DMetObjectRecord;
use regex_lite::Regex;
use serde::{de, Deserialize};

// By default, struct field names are deserialized based on the position of
// a corresponding field in the CSV data's header record.
#[derive(Debug, Deserialize)]
pub struct MetObjectCsvRecord {
    #[serde(rename = "Is Public Domain", deserialize_with = "deserialize_csv_bool")]
    pub public_domain: bool,

    #[serde(rename = "Is Highlight", deserialize_with = "deserialize_csv_bool")]
    pub highlight: bool,

    #[serde(rename = "Object ID")]
    pub object_id: u64,

    #[serde(rename = "Artist End Date", deserialize_with = "deserialize_csv_year")]
    pub artist_end_date: Option<u16>,

    #[serde(rename = "Object Wikidata URL")]
    pub object_wikidata_url: String,

    #[serde(rename = "Artist Display Name")]
    pub artist_display_name: String,

    #[serde(rename = "AccessionYear", deserialize_with = "deserialize_csv_year")]
    pub accession_year: Option<u16>,

    #[serde(rename = "Object Date")]
    pub object_date: String,

    #[serde(rename = "Culture")]
    pub culture: String,

    #[serde(rename = "Title")]
    pub title: String,

    #[serde(rename = "Medium")]
    pub medium: String,

    #[serde(rename = "Dimensions")]
    pub dimensions: String,
}

/// All artworks by artists who died before this year should definitely be
/// public domain (the criteria is that 70 years after their death, their
/// work enters PD).
const MIN_PUBLIC_DOMAIN_YEAR: u16 = 1950;

#[derive(PartialEq)]
enum PublicDomainStatus {
    Definitely,
    Probably,
    Nope,
}

impl MetObjectCsvRecord {
    fn public_domain_status(&self) -> PublicDomainStatus {
        if self.public_domain {
            return PublicDomainStatus::Definitely;
        } else if self.highlight && self.object_wikidata_url.len() > 0 {
            if let Some(year) = self.artist_end_date {
                if year <= MIN_PUBLIC_DOMAIN_YEAR {
                    return PublicDomainStatus::Probably;
                }
            }
        }
        PublicDomainStatus::Nope
    }
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
/// Also, some records have whitespace around the year, so we'll deal with that too.
fn deserialize_csv_year<'de, D>(deserializer: D) -> Result<Option<u16>, D::Error>
where
    D: de::Deserializer<'de>,
{
    let s: &str = de::Deserialize::deserialize(deserializer)?;
    let trimmed = s.trim();

    match trimmed.parse::<u16>() {
        Ok(value) => Ok(Some(value)),
        Err(_) => Ok(None),
    }
}

/// This list was obtained by running the CLI with `--all-media`, then
/// running the following SQL query on the generated DB:
///
///     select medium, count(*) as c from met_objects group by medium order by c desc limit 60;
///
/// I then ignored any medium that wasn't flat, two-dimensional art with a
/// matte surface. Examples of these are stone, glass, silk, iron, ceramic,
/// pottery, etc.
const MEDIUM_KEYWORDS: [&str; 20] = [
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
    "charcoal",
    "graphite",
    "woodblock",
    "wood block",
    "etching",
    "tempera",
    "fresco",
    "acrylic",
];

#[derive(Default)]
pub struct PublicDomain2DMetObjectOptions {
    /// Return artwork of any medium, don't return only 2D art.
    pub all_media: bool,
    /// Log warnings to stderr.
    pub warnings: bool,
}

fn try_into_public_domain_2d_met_object(
    dimension_parser: &DimensionParser,
    csv_record: MetObjectCsvRecord,
    options: &PublicDomain2DMetObjectOptions,
) -> Option<PublicDomain2DMetObjectRecord> {
    let public_domain_status = csv_record.public_domain_status();
    if public_domain_status == PublicDomainStatus::Nope {
        return None;
    }
    let Some(accession_year) = csv_record.accession_year else {
        return None;
    };
    let Some((width, height)) = dimension_parser.parse_cm(&csv_record.dimensions) else {
        return None;
    };
    let lower_medium = csv_record.medium.to_lowercase();
    for medium_keyword in MEDIUM_KEYWORDS.iter() {
        if options.all_media || lower_medium.contains(medium_keyword) {
            if public_domain_status == PublicDomainStatus::Probably {
                if options.warnings {
                    eprintln!(
                        r#"WARNING: #{}, \"{}\" by {} may actually be public domain.
  Met collection page: https://www.metmuseum.org/art/collection/search/{}
  Wikidata URL: {}"#,
                        csv_record.object_id,
                        csv_record.title,
                        csv_record.artist_display_name,
                        csv_record.object_id,
                        csv_record.object_wikidata_url
                    );
                }
                return None;
            }

            return Some(PublicDomain2DMetObjectRecord {
                object_id: csv_record.object_id,
                artist: csv_record.artist_display_name,
                accession_year,
                culture: csv_record.culture,
                object_date: csv_record.object_date,
                title: csv_record.title,
                medium: csv_record.medium,
                width: width / 100.0,   // Convert centimeters to meters
                height: height / 100.0, // Convert centimeters to meters
            });
        }
    }

    None
}

pub type MetObjectCsvResult = Result<PublicDomain2DMetObjectRecord, csv::Error>;

pub fn iter_public_domain_2d_met_csv_objects<R: std::io::Read>(
    reader: csv::Reader<R>,
    options: PublicDomain2DMetObjectOptions,
) -> impl Iterator<Item = MetObjectCsvResult> {
    let parser = DimensionParser::new();
    reader
        .into_deserialize::<MetObjectCsvRecord>()
        .filter_map(move |result| match result {
            Ok(csv_record) => {
                match try_into_public_domain_2d_met_object(&parser, csv_record, &options) {
                    Some(record) => Some(Ok(record)),
                    None => None,
                }
            }
            Err(err) => Some(Err(err)),
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

    /// Return a (width, height) tuple of the dimensions in cm. Note that this
    /// is the opposite order from the format in the data; we're using
    /// (width, height) because it's the common one in computer graphics.
    pub fn parse_cm<T: AsRef<str>>(&self, value: T) -> Option<(f64, f64)> {
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
    fn test_dimensions_parse_works() {
        let parser = DimensionParser::new();

        assert_eq!(
            parser.parse_cm("9 3/4 x 11 3/8 in. (24.8 x 28.9 cm)"),
            Some((28.9, 24.8))
        );
        assert_eq!(
            parser.parse_cm("9 3/4 x 11 3/8 in. (24 x 28.9 cm)"),
            Some((28.9, 24.0))
        );
        assert_eq!(
            parser.parse_cm("H. 2 1/2 in. (6.4 cm); Diam. 8 1/8 in. (20.6 cm)"),
            None
        );
    }
}
