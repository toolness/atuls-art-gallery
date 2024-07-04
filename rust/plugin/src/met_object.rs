use godot::prelude::*;

#[derive(Debug, GodotClass)]
#[class(init)]
pub struct MetObject {
    #[var]
    pub object_id: i64,
    #[var]
    pub title: GString,
    #[var]
    pub artist: GString,
    #[var]
    pub date: GString,
    #[var]
    pub width: f64,
    #[var]
    pub height: f64,
    #[var]
    pub x: f64,
    #[var]
    pub y: f64,
}
