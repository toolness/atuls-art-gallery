use anyhow::{anyhow, Result};
use percent_encoding::{utf8_percent_encode, CONTROLS};
use serde::{de, Deserialize};

use crate::{gallery_cache::GalleryCache, image::ImageSize};

const ROOT_CACHE_SUBDIR: &'static str = "wikidata";

const WIKIDATA_URL_PREFIXES: [&'static str; 2] = [
    "http://www.wikidata.org/entity/Q",
    "https://www.wikidata.org/wiki/Q",
];

/// We only care about the ones Godot can import right now:
///
/// https://docs.godotengine.org/en/stable/tutorials/assets_pipeline/importing_images.html#supported-image-formats
const SUPPORTED_LOWERCASE_IMAGE_FORMATS: [&'static str; 4] = [".jpg", ".jpeg", ".webp", ".png"];

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
    qid: u64,
    image_filename: String,
}

impl WikidataImageInfo {
    pub fn try_to_download_image(&self, cache: &GalleryCache, size: ImageSize) -> Result<String> {
        let image_url = get_url_for_image(&self.image_filename, size);
        let image_filename = match size {
            ImageSize::Small => format!(
                "{ROOT_CACHE_SUBDIR}/Q{}-small-{SMALL_IMAGE_WIDTH}px.jpg",
                self.qid
            ),
            ImageSize::Large => format!("{ROOT_CACHE_SUBDIR}/Q{}.jpg", self.qid),
        };
        cache.cache_binary_url(&image_url, &image_filename)?;
        Ok(image_filename)
    }
}

#[derive(Debug, Deserialize)]
pub struct WikidataEntity {
    labels: Option<LocalizedValues>,
    descriptions: Option<LocalizedValues>,
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
        self.claims.p18.find(|datavalue| match datavalue {
            Datavalue::String {
                value: image_filename,
            } => {
                let lowercase_filename = image_filename.to_lowercase();
                for format in SUPPORTED_LOWERCASE_IMAGE_FORMATS {
                    if lowercase_filename.ends_with(format) {
                        return Some(image_filename);
                    }
                }
                None
            }
            _ => None,
        })
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
        self.claims.p170.find_and_copy(|datavalue| match datavalue {
            Datavalue::Entity {
                value: EntityId { id },
            } => Some(id),
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
    fn find<T, F>(&self, callback: F) -> Option<&T>
    where
        F: Fn(&Datavalue) -> Option<&T>,
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

    fn find_and_copy<T, F>(&self, callback: F) -> Option<T>
    where
        T: Copy,
        F: Fn(&Datavalue) -> Option<&T>,
    {
        self.find(callback).copied()
    }

    fn find_cm_amount(&self) -> Option<f64> {
        self.find_and_copy(|datavalue| {
            if let Datavalue::Quantity {
                value: Quantity { amount, unit },
            } = datavalue
            {
                if unit == &Some(CENTIMETRE_QID) {
                    return Some(amount);
                }
            }
            None
        })
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

pub fn load_wikidata_image_info(
    cache: &GalleryCache,
    qid: u64,
) -> Result<Option<WikidataImageInfo>> {
    let filename = format!("{ROOT_CACHE_SUBDIR}/wbgetclaims-P18-Q{qid}.json");
    cache.cache_json_url(
        format!("https://www.wikidata.org/w/api.php?action=wbgetclaims&property=P18&entity=Q{qid}&format=json"),
        &filename,
    )?;

    let response = serde_json::from_str::<WikidataEntity>(&cache.load_cached_string(&filename)?);
    match response {
        Ok(response) => {
            let Some(image_filename) = response.image_filename() else {
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
    use crate::{image::ImageSize, wikidata::get_url_for_image};

    use super::{try_to_parse_qid_from_wikidata_url, WikidataEntity};

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
        let response: WikidataEntity = serde_json::from_str(&response_json).unwrap();
        assert_eq!(
            response.image_filename(),
            Some(&"Juan Gris - Nature morte à la nappe à carreaux.jpg".to_owned())
        );
    }
}
