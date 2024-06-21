use godot::{engine::Os, prelude::*};

#[derive(Debug, GodotClass)]
#[class(init)]
pub struct MetObject {
    #[var]
    pub object_id: i64,
    #[var]
    pub title: GString,
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

#[godot_api]
impl MetObject {
    #[func]
    fn open_in_browser(&self) {
        Os::singleton().shell_open(
            format!(
                "https://www.metmuseum.org/art/collection/search/{}",
                self.object_id
            )
            .into_godot(),
        );
    }
}
