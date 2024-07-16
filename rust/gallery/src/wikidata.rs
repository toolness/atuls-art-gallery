use anyhow::Result;
use percent_encoding::{utf8_percent_encode, CONTROLS};
use serde::Deserialize;

use crate::{gallery_cache::GalleryCache, image::ImageSize};

const ROOT_CACHE_SUBDIR: &'static str = "wikidata";

const WIKIDATA_URL_PREFIX: &'static str = "https://www.wikidata.org/wiki/Q";

const SMALL_IMAGE_WIDTH: usize = 500;

pub fn try_to_parse_qid_from_wikidata_url<T: AsRef<str>>(url: T) -> Option<u64> {
    if url.as_ref().starts_with(WIKIDATA_URL_PREFIX) {
        let slice = url.as_ref().split_at(WIKIDATA_URL_PREFIX.len()).1;
        if let Ok(qid) = slice.parse::<u64>() {
            Some(qid)
        } else {
            None
        }
    } else {
        None
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
            .map(|label| label.english_value())
            .flatten()
    }
    pub fn description(&self) -> Option<&str> {
        self.descriptions
            .as_ref()
            .map(|label| label.english_value())
            .flatten()
    }
    pub fn p18_image(&self) -> Option<&str> {
        let Some(statements) = &self.claims.p18 else {
            return None;
        };
        for statement in statements {
            let image_filename = &statement.mainsnak.datavalue.value;
            if image_filename.to_lowercase().ends_with(".jpg") {
                return Some(image_filename);
            }
        }
        None
    }
}

#[derive(Debug, Deserialize)]
struct LocalizedValues {
    en: Option<Datavalue>,
}

impl LocalizedValues {
    fn english_value(&self) -> Option<&str> {
        self.en.as_ref().map(|en| en.value.as_str())
    }
}

#[derive(Debug, Deserialize)]
struct Claims {
    #[serde(rename = "P18")]
    p18: Option<Vec<Statement>>,
}

#[derive(Debug, Deserialize)]
struct Statement {
    mainsnak: Mainsnak,
}

#[derive(Debug, Deserialize)]
struct Mainsnak {
    datavalue: Datavalue,
}

#[derive(Debug, Deserialize)]
struct Datavalue {
    value: String,
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
            let Some(image_filename) = response.p18_image() else {
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
            response.p18_image(),
            Some("Juan Gris - Nature morte à la nappe à carreaux.jpg")
        );
    }
}
