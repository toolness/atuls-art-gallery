/// Internally we represent art object IDs with a u64, but Godot uses i64s.
///
/// To Godot, an art object ID is just an opaque identifier to something that
/// only has meaning for the client, so we'll essentially just transmute between
/// the two types.
///
/// This does, mean, however, that logging done from the Godot side may show
/// different IDs than logging done from the Rust side. We should probably have
/// Godot only log art object URLs instead of raw IDs.
pub struct ArtObject(pub u64);

impl ArtObject {
    pub fn from_godot_int(value: i64) -> Self {
        Self(u64::from_le_bytes(value.to_le_bytes()))
    }

    pub fn to_godot_int(&self) -> i64 {
        i64::from_le_bytes(self.0.to_le_bytes())
    }

    pub fn url(&self) -> String {
        format!("https://www.metmuseum.org/art/collection/search/{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use crate::art_object::ArtObject;

    const NEGATIVE_ONE_AS_U64: u64 = 18446744073709551615;

    #[test]
    fn test_it_converts_from_godot_ints() {
        assert_eq!(ArtObject::from_godot_int(1).0, 1);
        assert_eq!(ArtObject::from_godot_int(-1).0, NEGATIVE_ONE_AS_U64);
    }

    #[test]
    fn test_it_converts_to_godot_ints() {
        assert_eq!(ArtObject(1).to_godot_int(), 1);
        assert_eq!(ArtObject(NEGATIVE_ONE_AS_U64).to_godot_int(), -1);
    }
}
