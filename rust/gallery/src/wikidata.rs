use anyhow::Result;
use percent_encoding::{utf8_percent_encode, CONTROLS};
use serde::Deserialize;

use crate::gallery_cache::GalleryCache;

const ROOT_CACHE_SUBDIR: &'static str = "wikidata";

const WIKIDATA_URL_PREFIX: &'static str = "https://www.wikidata.org/wiki/Q";

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

#[derive(Debug, Deserialize)]
struct WbGetClaimsResponse {
    claims: Claims,
}

impl WbGetClaimsResponse {
    fn get_p18_image(&self) -> Option<&String> {
        for statement in &self.claims.p18 {
            let image_filename = &statement.mainsnak.datavalue.value;
            if image_filename.to_lowercase().ends_with(".jpg") {
                return Some(image_filename);
            }
        }
        None
    }
}

#[derive(Debug, Deserialize)]
struct Claims {
    #[serde(rename = "P18")]
    p18: Vec<Statement>,
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

fn get_url_for_image<T: AsRef<str>>(image_filename: T) -> String {
    // https://stackoverflow.com/a/34402875/2422398
    let spaces_replaced = image_filename.as_ref().replace(' ', "_");
    let md5_hash = format!("{:x}", md5::compute(spaces_replaced.as_bytes()));
    let a = md5_hash.get(0..1).unwrap();
    let ab = md5_hash.get(0..2).unwrap();
    format!(
        "https://upload.wikimedia.org/wikipedia/commons/{a}/{ab}/{}",
        utf8_percent_encode(&spaces_replaced, CONTROLS)
    )
}

pub fn try_to_get_wikidata_image_url(cache: &GalleryCache, qid: u64) -> Result<Option<String>> {
    let filename = format!("{ROOT_CACHE_SUBDIR}/wbgetclaims-P18-Q{qid}");
    cache.cache_json_url(
        format!("https://www.wikidata.org/w/api.php?action=wbgetclaims&property=P18&entity=Q{qid}&format=json"),
        &filename,
    )?;

    let response =
        serde_json::from_str::<WbGetClaimsResponse>(&cache.load_cached_string(&filename)?);
    match response {
        Ok(record) => Ok(record.get_p18_image().map(get_url_for_image)),
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use crate::wikidata::get_url_for_image;

    use super::{try_to_parse_qid_from_wikidata_url, WbGetClaimsResponse};

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
    fn test_get_url_for_image_works() {
        assert_eq!(
            get_url_for_image(""),
            "https://upload.wikimedia.org/wikipedia/commons/d/d4/"
        );
        assert_eq!(
            get_url_for_image("Junior-Jaguar-Belize-Zoo.jpg"),
            "https://upload.wikimedia.org/wikipedia/commons/2/21/Junior-Jaguar-Belize-Zoo.jpg"
        );
        assert_eq!(
            get_url_for_image("Juan Gris - Nature morte à la nappe à carreaux.jpg"),
            "https://upload.wikimedia.org/wikipedia/commons/f/fa/Juan_Gris_-_Nature_morte_%C3%A0_la_nappe_%C3%A0_carreaux.jpg"
        );
    }

    #[test]
    fn test_get_p18_image_works() {
        let response_json = r#"{"claims":{"P18":[{"mainsnak":{"snaktype":"value","property":"P18","hash":"9c96969b48408f6aa6d208542c338cadeff2dff9","datavalue":{"value":"Juan Gris - Nature morte \u00e0 la nappe \u00e0 carreaux.jpg","type":"string"},"datatype":"commonsMedia"},"type":"statement","id":"Q20189849$5E016A60-DF33-4157-A6F0-6E1E65411428","rank":"normal"}]}}"#;
        let response: WbGetClaimsResponse = serde_json::from_str(&response_json).unwrap();
        assert_eq!(
            response.get_p18_image(),
            Some(&"Juan Gris - Nature morte à la nappe à carreaux.jpg".to_owned())
        );
    }
}
