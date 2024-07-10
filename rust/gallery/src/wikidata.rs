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

#[cfg(test)]
mod tests {
    use super::try_to_parse_qid_from_wikidata_url;

    #[test]
    fn test_it_works() {
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
}
