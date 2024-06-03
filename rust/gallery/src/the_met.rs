use crate::gallery_cache::GalleryCache;
use anyhow::{anyhow, Result};
use regex_lite::Regex;
use serde::{de, Deserialize};

pub fn load_met_object_record(cache: &GalleryCache, object_id: u64) -> Result<MetObjectApiRecord> {
    let filename = format!("object-{}.json", object_id);
    cache.cache_json_url(
        format!(
            "https://collectionapi.metmuseum.org/public/collection/v1/objects/{}",
            object_id
        ),
        &filename,
    )?;
    match serde_json::from_str(&cache.load_cached_string(&filename)?) {
        Ok(record) => Ok(record),
        Err(err) => Err(anyhow!("Failed to load {}: {}", filename, err)),
    }
}

#[derive(Debug, Deserialize)]
pub struct MetObjectApiRecord {
    pub measurements: Option<Vec<MetObjectApiMeasurements>>,

    #[serde(rename = "primaryImageSmall")]
    pub primary_image_small: String,

    #[serde(rename = "objectDate")]
    pub object_date: String,

    #[serde(rename = "objectID")]
    pub object_id: u64,

    pub title: String,
}

impl MetObjectApiRecord {
    /// Returns physical dimensions, *not* pixels.
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

    /// Try to download & cache the small image of the object if it's 2D artwork.
    ///
    /// If it's in the cache, returns the cached version. Otherwise, downloads and adds
    /// to cache.
    ///
    /// Returns (width, height, filename) on success. Dimensions are physical, *not* pixels.
    pub fn try_to_download_small_image(
        &self,
        cache: &GalleryCache,
    ) -> Result<Option<(f64, f64, String)>> {
        if let Some((width, height)) = self.overall_width_and_height() {
            if self.primary_image_small.ends_with(".jpg") {
                let small_image = format!("object-{}-small.jpg", self.object_id);
                cache.cache_binary_url(&self.primary_image_small, &small_image)?;
                return Ok(Some((width, height, small_image)));
            }
        }
        Ok(None)
    }
}

#[derive(Debug, Deserialize)]
pub struct MetObjectApiMeasurements {
    #[serde(rename = "elementName")]
    element_name: String,

    #[serde(rename = "elementMeasurements")]
    element_measurements: MetObjectApiElementMeasurements,
}

#[derive(Debug, Deserialize)]
pub struct MetObjectApiElementMeasurements {
    #[serde(rename = "Width")]
    width: Option<f64>,

    #[serde(rename = "Height")]
    height: Option<f64>,

    #[serde(rename = "Depth")]
    depth: Option<f64>,
}

// By default, struct field names are deserialized based on the position of
// a corresponding field in the CSV data's header record.
#[derive(Debug, Deserialize)]
pub struct MetObjectCsvRecord {
    #[serde(rename = "Is Public Domain", deserialize_with = "deserialize_csv_bool")]
    pub public_domain: bool,

    #[serde(rename = "Object ID")]
    pub object_id: u64,

    #[serde(rename = "Title")]
    pub title: String,

    #[serde(rename = "Medium")]
    pub medium: String,

    #[serde(rename = "Link Resource")]
    pub link_resource: String,

    #[serde(rename = "Dimensions")]
    pub dimensions: String,
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
    csv_record: &MetObjectCsvRecord,
) -> bool {
    if !csv_record.public_domain {
        return false;
    }
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
        .into_deserialize()
        .filter(move |result| match result {
            Ok(csv_record) => is_public_domain_2d_met_object(&parser, csv_record),
            Err(_) => true,
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
    use crate::the_met::DimensionParser;

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
