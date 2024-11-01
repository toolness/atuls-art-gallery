use anyhow::{anyhow, Result};
use percent_encoding::{utf8_percent_encode, CONTROLS};
use serde::{de, Deserialize};

use crate::{
    gallery_cache::GalleryCache,
    image::{cache_image, get_supported_image_ext, ImageSize},
};

const ROOT_CACHE_SUBDIR: &'static str = "wikidata";

const WIKIDATA_URL_PREFIXES: [&'static str; 3] = [
    "http://www.wikidata.org/entity/Q",
    "https://www.wikidata.org/wiki/Q",
    // I guess this is theoretically still a URL, it's just _very_ relative. Makes it easy for
    // us to use this function to parse raw IDs like "Q1234".
    "Q",
];

const SMALL_IMAGE_WIDTH: usize = 500;

pub fn try_to_parse_qid_from_wikidata_url<T: AsRef<str>>(url: T) -> Option<u64> {
    for prefix in WIKIDATA_URL_PREFIXES {
        if url.as_ref().starts_with(prefix) {
            let slice = url.as_ref().split_at(prefix.len()).1;
            if let Ok(qid) = slice.parse::<u64>() {
                return Some(qid);
            }
        }
    }
    None
}

/// Parses a Q-identifier from a wikidata URL and returns it.
pub fn deserialize_wikidata_entity_url_str<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: de::Deserializer<'de>,
{
    let s: &str = de::Deserialize::deserialize(deserializer)?;
    match try_to_parse_qid_from_wikidata_url(s) {
        Some(qid) => Ok(qid),
        None => Err(de::Error::custom(anyhow!(
            "Unable to parse {s:?} as wikidata URL"
        ))),
    }
}

/// Parses a Q-identifier from a wikidata URL and returns it.
///
/// This version deserializes the intermediate value to an owned String instead of a
/// borrowed &str because parsing JSON apparently yields owned Strings instead of
/// borrowed ones and Serde is confusing the hell out of me and the only way around
/// it seems to be to make two separate functions that do the same thing but with
/// different types of strings.
fn deserialize_wikidata_entity_url_string<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: de::Deserializer<'de>,
{
    let s: String = de::Deserialize::deserialize(deserializer)?;
    match try_to_parse_qid_from_wikidata_url(&s) {
        Some(qid) => Ok(qid),
        None => Err(de::Error::custom(anyhow!(
            "Unable to parse {s:?} as wikidata URL"
        ))),
    }
}

/// Sometimes wikidata entities are malformed, e.g. the width of
/// https://www.wikidata.org/wiki/Q2395137, so we just return None
/// instead of erroring.
fn deserialize_wikidata_entity_url_string_forgiving<'de, D>(
    deserializer: D,
) -> Result<Option<u64>, D::Error>
where
    D: de::Deserializer<'de>,
{
    match deserialize_wikidata_entity_url_string(deserializer) {
        Ok(value) => Ok(Some(value)),
        Err(_) => Ok(None),
    }
}

/// Wikipedia serializes its floats with a leading plus sign, e.g. "+32.4", so
/// we have to specially parse it.
fn deserialize_amount_with_plus_sign<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: de::Deserializer<'de>,
{
    let s: String = de::Deserialize::deserialize(deserializer)?;
    let to_parse = if s.starts_with("+") { &s[1..] } else { &s };
    match to_parse.parse::<f64>() {
        Ok(value) => Ok(value),
        Err(err) => Err(de::Error::custom(anyhow!(
            "Unable to parse {s:?} as float with possible leading plus sign: {err:?}"
        ))),
    }
}

pub struct WikidataImageInfo {
    pub qid: i64,
    pub image_filename: String,
}

impl WikidataImageInfo {
    pub fn try_to_download_image(&self, cache: &GalleryCache, size: ImageSize) -> Result<String> {
        let image_url = get_url_for_image(&self.image_filename, size);
        let Some(ext) = get_supported_image_ext(&self.image_filename) else {
            return Err(anyhow!(
                "Invalid file extension for image: {}",
                self.image_filename
            ));
        };
        let image_filename = match size {
            ImageSize::Small => format!(
                "{ROOT_CACHE_SUBDIR}/Q{}-small-{SMALL_IMAGE_WIDTH}px{ext}",
                self.qid
            ),
            ImageSize::Large => format!("{ROOT_CACHE_SUBDIR}/Q{}{ext}", self.qid),
        };
        cache_image(cache, &image_url, &image_filename, ext)?;
        Ok(image_filename)
    }
}

#[derive(Debug, Deserialize)]
pub struct WikidataEntity {
    #[serde(deserialize_with = "deserialize_wikidata_entity_url_string")]
    pub id: u64,
    labels: Option<LocalizedValues>,
    descriptions: Option<LocalizedValues>,
    claims: Claims,
}

#[derive(Debug, Deserialize)]
pub struct WikidataEntityClaimsOnly {
    claims: Claims,
}

impl WikidataEntity {
    pub fn label(&self) -> Option<&str> {
        self.labels
            .as_ref()
            .map(|label| label.english_str())
            .flatten()
    }
    pub fn description(&self) -> Option<&str> {
        self.descriptions
            .as_ref()
            .map(|label| label.english_str())
            .flatten()
    }
    pub fn image_filename(&self) -> Option<&String> {
        self.claims.image_filename()
    }
    pub fn dimensions_in_cm(&self) -> Option<(f64, f64)> {
        if let (Some(width), Some(height)) = (
            self.claims.p2049.find_cm_amount(),
            self.claims.p2048.find_cm_amount(),
        ) {
            if width > 0.0 && height > 0.0 {
                return Some((width, height));
            }
        }
        None
    }
    pub fn creator_id(&self) -> Option<u64> {
        self.claims.p170.find(|datavalue| datavalue.entity_id())
    }
    pub fn material_ids(&self) -> Vec<u64> {
        self.claims.p186.find_all(|datavalue| match datavalue {
            Datavalue::Entity {
                value: EntityId { id },
            } => Some(*id),
            _ => None,
        })
    }
    pub fn collection_id(&self) -> Option<u64> {
        self.claims.p195.find(|datavalue| datavalue.entity_id())
    }
    pub fn inception(&self) -> Option<String> {
        self.claims.p571.find(|datavalue| match datavalue {
            Datavalue::Time { value } => value.to_string(),
            _ => None,
        })
    }
}

#[derive(Debug, Deserialize)]
struct LocalizedValues {
    en: Option<StringValue>,
}

impl LocalizedValues {
    fn english_str(&self) -> Option<&str> {
        if let Some(StringValue { value }) = &self.en {
            Some(value.as_str())
        } else {
            None
        }
    }
}

#[derive(Debug, Deserialize, Default)]
struct Statements(Vec<Statement>);

impl Statements {
    /// Iterate through all statements calling the given callback with the statement's
    /// mainsnak datavalue. Once the callback returns a `Some()` value, return it immediately.
    fn find<'a, T, F>(&'a self, callback: F) -> Option<T>
    where
        F: Fn(&'a Datavalue) -> Option<T>,
    {
        for statement in &self.0 {
            if let Some(datavalue) = &statement.mainsnak.datavalue {
                if let Some(result) = callback(datavalue) {
                    return Some(result);
                }
            }
        }
        None
    }

    fn find_cm_amount(&self) -> Option<f64> {
        self.find(|datavalue| {
            if let Datavalue::Quantity {
                value: Quantity { amount, unit },
            } = datavalue
            {
                if unit == &Some(CENTIMETRE_QID) {
                    return Some(*amount);
                }
            }
            None
        })
    }

    fn find_all<'a, T, F>(&'a self, callback: F) -> Vec<T>
    where
        T: Copy,
        F: Fn(&'a Datavalue) -> Option<T>,
    {
        let mut results = Vec::with_capacity(self.0.len());
        for statement in &self.0 {
            if let Some(datavalue) = &statement.mainsnak.datavalue {
                if let Some(result) = callback(datavalue) {
                    results.push(result);
                }
            }
        }
        results
    }
}

/// Claims. We only list the ones we care about so we don't have to worry about
/// parsing every variation of the schema, nor do we waste the device's time in
/// parsing things we don't need.
#[derive(Debug, Deserialize)]
struct Claims {
    /// P18 - Image
    #[serde(rename = "P18", default)]
    p18: Statements,

    /// P2048 - Height
    #[serde(rename = "P2048", default)]
    p2048: Statements,

    /// P2049 - Width
    #[serde(rename = "P2049", default)]
    p2049: Statements,

    /// P170 - Creator
    #[serde(rename = "P170", default)]
    p170: Statements,

    /// P186 - Made from material
    #[serde(rename = "P186", default)]
    p186: Statements,

    /// P195 - Collection
    #[serde(rename = "P195", default)]
    p195: Statements,

    /// P571 - Inception
    #[serde(rename = "P571", default)]
    p571: Statements,
}

impl Claims {
    pub fn image_filename(&self) -> Option<&String> {
        self.p18.find(|datavalue| match datavalue {
            Datavalue::String {
                value: image_filename,
            } => {
                if get_supported_image_ext(&image_filename).is_some() {
                    Some(image_filename)
                } else {
                    None
                }
            }
            _ => None,
        })
    }
}

/// https://www.wikidata.org/wiki/Q174728
const CENTIMETRE_QID: u64 = 174728;

#[derive(Debug, Deserialize)]
struct Statement {
    mainsnak: Mainsnak,
}

#[derive(Debug, Deserialize)]
struct Mainsnak {
    datavalue: Option<Datavalue>,
}

#[derive(Debug, Deserialize)]
struct StringValue {
    value: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum Datavalue {
    #[serde(rename = "string")]
    String { value: String },

    #[serde(rename = "quantity")]
    Quantity { value: Quantity },

    #[serde(rename = "wikibase-entityid")]
    Entity { value: EntityId },

    #[serde(rename = "time")]
    Time { value: Time },
}

/// This structure is documented here: https://www.wikidata.org/wiki/Special:ListDatatypes
#[derive(Debug, Deserialize)]
struct Time {
    #[serde(
        rename = "time",
        deserialize_with = "deserialize_year_from_iso_timestamp"
    )]
    year: Option<i16>,

    /// The numbers have the following meaning:
    ///   0 - billion years, 1 - hundred million years, ...,
    ///   6 - millennium, 7 - century, 8 - decade, 9 - year,
    ///   10 - month, 11 - day, 12 - hour, 13 - minute, 14 - second.
    precision: u16,
}

const PRECISION_CENTURY: u16 = 7;
const PRECISION_DECADE: u16 = 8;
const PRECISION_YEAR: u16 = 9;

impl Time {
    fn to_string(&self) -> Option<String> {
        let Some(year) = self.year else { return None };
        if self.precision == PRECISION_CENTURY {
            // TODO: This won't work for BC
            let century = (year / 100) + 1;
            // TODO: This will look weird for 1st, 2nd, 3rd century AD
            return Some(format!("{century}th century"));
        }
        // TODO: If year is BC, this will just have a negative sign in front of it,
        // which will look weird.
        if self.precision == PRECISION_DECADE {
            let decade = (year / 10) * 10;
            return Some(if decade == year {
                format!("{year}s")
            } else {
                // If the year doesn't fall on a decade, it will look weird, e.g. "1916s", so
                // instead, let's prepend "circa" to indicate that it's not exact.
                format!("ca. {year}")
            });
        }
        if self.precision >= PRECISION_YEAR {
            return Some(format!("{year}"));
        }
        None
    }
}

fn try_to_parse_year_from_iso_timestamp(value: &str) -> Option<i16> {
    let to_parse = if value.starts_with("+") {
        &value[1..5]
    } else {
        &value[0..4]
    };
    to_parse.parse::<i16>().ok()
}

fn deserialize_year_from_iso_timestamp<'de, D>(deserializer: D) -> Result<Option<i16>, D::Error>
where
    D: de::Deserializer<'de>,
{
    let s: String = de::Deserialize::deserialize(deserializer)?;
    Ok(try_to_parse_year_from_iso_timestamp(&s))
}

impl Datavalue {
    fn entity_id(&self) -> Option<u64> {
        match self {
            Datavalue::Entity {
                value: EntityId { id },
            } => Some(*id),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct EntityId {
    #[serde(rename = "numeric-id")]
    id: u64,
}

#[derive(Debug, Deserialize)]
struct Quantity {
    #[serde(deserialize_with = "deserialize_amount_with_plus_sign")]
    amount: f64,
    #[serde(deserialize_with = "deserialize_wikidata_entity_url_string_forgiving")]
    unit: Option<u64>,
}

fn get_url_for_image<T: AsRef<str>>(image_filename: T, size: ImageSize) -> String {
    // https://stackoverflow.com/a/34402875/2422398
    let spaces_replaced = image_filename.as_ref().replace(' ', "_");
    let md5_hash = format!("{:x}", md5::compute(spaces_replaced.as_bytes()));
    let a = md5_hash.get(0..1).unwrap();
    let ab = md5_hash.get(0..2).unwrap();
    let encoded_filename = utf8_percent_encode(&spaces_replaced, CONTROLS);

    match size {
        // https://phabricator.wikimedia.org/T153497
        ImageSize::Small => format!("https://upload.wikimedia.org/wikipedia/commons/thumb/{a}/{ab}/{encoded_filename}/{SMALL_IMAGE_WIDTH}px-{encoded_filename}"),
        ImageSize::Large => {
            format!("https://upload.wikimedia.org/wikipedia/commons/{a}/{ab}/{encoded_filename}")
        }
    }
}

fn parse_wikidata_claims_json(value: &str) -> Result<WikidataEntityClaimsOnly, serde_json::Error> {
    serde_json::from_str(value)
}

pub fn load_wikidata_image_info(
    cache: &GalleryCache,
    qid: i64,
) -> Result<Option<WikidataImageInfo>> {
    let filename = format!("{ROOT_CACHE_SUBDIR}/wbgetclaims-P18-Q{qid}.json");
    cache.cache_json_url(
        format!("https://www.wikidata.org/w/api.php?action=wbgetclaims&property=P18&entity=Q{qid}&format=json"),
        &filename,
    )?;

    let response = parse_wikidata_claims_json(&cache.load_cached_string(&filename)?);
    match response {
        Ok(response) => {
            let Some(image_filename) = response.claims.image_filename() else {
                return Ok(None);
            };
            Ok(Some(WikidataImageInfo {
                qid,
                image_filename: image_filename.to_string(),
            }))
        }
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        image::ImageSize,
        wikidata::{
            get_url_for_image, parse_wikidata_claims_json, try_to_parse_year_from_iso_timestamp,
            PRECISION_CENTURY, PRECISION_DECADE, PRECISION_YEAR,
        },
    };

    use super::{get_supported_image_ext, try_to_parse_qid_from_wikidata_url, Time};

    #[test]
    fn test_try_to_parse_qid_from_wikidata_url_works() {
        assert_eq!(try_to_parse_qid_from_wikidata_url("blah"), None);
        assert_eq!(
            try_to_parse_qid_from_wikidata_url("https://www.wikidata.org/wiki/Q20189849LOL"),
            None
        );
        assert_eq!(
            try_to_parse_qid_from_wikidata_url("https://www.wikidata.org/wiki/Q20189849"),
            Some(20189849)
        );
        assert_eq!(
            try_to_parse_qid_from_wikidata_url("http://www.wikidata.org/entity/Q254923"),
            Some(254923)
        )
    }

    #[test]
    fn test_get_url_for_image_small_works() {
        assert_eq!(
            get_url_for_image("", ImageSize::Small),
            "https://upload.wikimedia.org/wikipedia/commons/thumb/d/d4//500px-"
        );
        assert_eq!(
            get_url_for_image("Junior-Jaguar-Belize-Zoo.jpg", ImageSize::Small),
            "https://upload.wikimedia.org/wikipedia/commons/thumb/2/21/Junior-Jaguar-Belize-Zoo.jpg/500px-Junior-Jaguar-Belize-Zoo.jpg"
        );
        assert_eq!(
            get_url_for_image("Juan Gris - Nature morte à la nappe à carreaux.jpg", ImageSize::Small),
            "https://upload.wikimedia.org/wikipedia/commons/thumb/f/fa/Juan_Gris_-_Nature_morte_%C3%A0_la_nappe_%C3%A0_carreaux.jpg/500px-Juan_Gris_-_Nature_morte_%C3%A0_la_nappe_%C3%A0_carreaux.jpg"
        );
    }

    #[test]
    fn test_get_url_for_image_large_works() {
        assert_eq!(
            get_url_for_image("", ImageSize::Large),
            "https://upload.wikimedia.org/wikipedia/commons/d/d4/"
        );
        assert_eq!(
            get_url_for_image("Junior-Jaguar-Belize-Zoo.jpg", ImageSize::Large),
            "https://upload.wikimedia.org/wikipedia/commons/2/21/Junior-Jaguar-Belize-Zoo.jpg"
        );
        assert_eq!(
            get_url_for_image("Juan Gris - Nature morte à la nappe à carreaux.jpg", ImageSize::Large),
            "https://upload.wikimedia.org/wikipedia/commons/f/fa/Juan_Gris_-_Nature_morte_%C3%A0_la_nappe_%C3%A0_carreaux.jpg"
        );
    }

    #[test]
    fn test_get_p18_image_works() {
        let response_json = r#"{"claims":{"P18":[{"mainsnak":{"snaktype":"value","property":"P18","hash":"9c96969b48408f6aa6d208542c338cadeff2dff9","datavalue":{"value":"Juan Gris - Nature morte \u00e0 la nappe \u00e0 carreaux.jpg","type":"string"},"datatype":"commonsMedia"},"type":"statement","id":"Q20189849$5E016A60-DF33-4157-A6F0-6E1E65411428","rank":"normal"}]}}"#;
        let response = parse_wikidata_claims_json(&response_json).unwrap();
        assert_eq!(
            response.claims.image_filename(),
            Some(&"Juan Gris - Nature morte à la nappe à carreaux.jpg".to_owned())
        );
    }

    #[test]
    fn test_try_to_parse_year_from_iso_timestamp_works() {
        assert_eq!(
            try_to_parse_year_from_iso_timestamp("+1915-03-00T00:00:00Z"),
            Some(1915)
        );
        assert_eq!(try_to_parse_year_from_iso_timestamp("blah"), None);
    }

    #[test]
    fn test_parse_time_works_for_year_precision() {
        // Taken from Juan Gris painting
        let json = r#"{"time":"+1915-03-00T00:00:00Z","timezone":0,"before":0,"after":0,"precision":10,"calendarmodel":"http://www.wikidata.org/entity/Q1985727"}"#;
        let time: Time = serde_json::from_str(json).unwrap();
        assert_eq!(time.year, Some(1915));
        assert_eq!(time.precision, 10);
        assert_eq!(time.to_string(), Some("1915".into()));
    }

    #[test]
    fn test_parse_time_works_for_decade_precision() {
        // Taken from Dracula
        let json = r#"{"time":"+1890-00-00T00:00:00Z","timezone":0,"before":0,"after":0,"precision":8,"calendarmodel":"http://www.wikidata.org/entity/Q1985727"}"#;
        let time: Time = serde_json::from_str(json).unwrap();
        assert_eq!(time.year, Some(1890));
        assert_eq!(time.precision, 8);
        assert_eq!(time.to_string(), Some("1890s".into()));
    }

    #[test]
    fn test_time_to_string_works() {
        fn test_time(year: i16, precision: u16, expected: Option<&str>) {
            assert_eq!(
                Time {
                    year: Some(year),
                    precision,
                }
                .to_string(),
                expected.map(|s| s.to_string())
            );
        }

        test_time(-9999, 1, None);
        test_time(1800, PRECISION_CENTURY, Some("19th century"));
        test_time(1910, PRECISION_DECADE, Some("1910s"));
        test_time(1916, PRECISION_DECADE, Some("ca. 1916"));
        test_time(1912, PRECISION_YEAR, Some("1912"));
        test_time(2014, 14, Some("2014"));
    }

    #[test]
    fn test_get_supported_image_ext_works() {
        assert_eq!(get_supported_image_ext("boop.png"), Some(".png"));
        assert_eq!(get_supported_image_ext("boop.jpeg"), Some(".jpeg"));
        assert_eq!(get_supported_image_ext("boop.jpg"), Some(".jpg"));
        assert_eq!(get_supported_image_ext("boop.webp"), Some(".webp"));
        assert_eq!(get_supported_image_ext("boop.tiff"), None);
    }
}
