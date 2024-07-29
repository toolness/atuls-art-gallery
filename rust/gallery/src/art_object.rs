use serde::{Deserialize, Serialize};

/// Internally we represent art object IDs as an enum, but Godot and our DB
/// use i64s. This enum includes utilities to help us translate between the two.
///
/// This does, mean, however, that logging done from the Godot side won't be
/// very helpful. We should probably have Godot only log art object URLs instead
/// of raw IDs.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ArtObjectId {
    Met(i64),
    Wikidata(i64),
}

const WIKIDATA_BIT: i64 = 1 << 62;

impl ArtObjectId {
    pub fn url(&self) -> String {
        match self {
            ArtObjectId::Met(id) => {
                format!("https://www.metmuseum.org/art/collection/search/{}", id)
            }
            ArtObjectId::Wikidata(qid) => {
                format!("https://www.wikidata.org/wiki/Q{qid}")
            }
        }
    }

    pub fn to_raw_i64(&self) -> i64 {
        match self {
            ArtObjectId::Met(id) => *id,
            ArtObjectId::Wikidata(qid) => *qid | WIKIDATA_BIT,
        }
    }

    pub fn from_raw_i64(value: i64) -> Self {
        if value & WIKIDATA_BIT > 0 {
            ArtObjectId::Wikidata(value ^ WIKIDATA_BIT)
        } else {
            ArtObjectId::Met(value)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::art_object::ArtObjectId;

    #[test]
    fn test_it_converts_from_raw_i64() {
        assert_eq!(ArtObjectId::from_raw_i64(1).to_raw_i64(), 1);
    }

    #[test]
    fn test_it_converts_to_raw_i64() {
        assert_eq!(ArtObjectId::Met(1).to_raw_i64(), 1);
    }

    #[test]
    fn test_it_converts_round_trip() {
        let ids = vec![ArtObjectId::Wikidata(5), ArtObjectId::Met(5)];
        for id in ids {
            let round_tripped = ArtObjectId::from_raw_i64(id.to_raw_i64());
            assert_eq!(id, round_tripped);
        }
    }
}
