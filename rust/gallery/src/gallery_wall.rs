use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct GalleryWall {
    pub width: f64,
    pub height: f64,
    pub name: String,
}
