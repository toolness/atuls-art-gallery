use godot::{engine::Image, prelude::*};

use crate::art_object::ArtObject;

#[derive(Debug)]
pub enum InnerGalleryResponse {
    Variant(Variant),
    ArtObjects(Array<Gd<ArtObject>>),
    Image(Option<Gd<Image>>),
}

impl Default for InnerGalleryResponse {
    fn default() -> Self {
        InnerGalleryResponse::Variant(Variant::default())
    }
}

#[derive(Debug, GodotClass)]
#[class(init)]
pub struct GalleryResponse {
    #[var]
    pub request_id: u32,
    pub response: InnerGalleryResponse,
}

#[godot_api]
impl GalleryResponse {
    #[func]
    fn take_art_objects(&mut self) -> Array<Gd<ArtObject>> {
        match std::mem::take(&mut self.response) {
            InnerGalleryResponse::ArtObjects(response) => response,
            _ => {
                godot_error!("GalleryResponse is not ArtObjects!");
                Array::new()
            }
        }
    }

    #[func]
    fn take_optional_image(&mut self) -> Option<Gd<Image>> {
        match std::mem::take(&mut self.response) {
            InnerGalleryResponse::Image(response) => response,
            _ => {
                godot_error!("GalleryResponse is not Image!");
                None
            }
        }
    }

    #[func]
    fn take_variant(&mut self) -> Variant {
        match std::mem::take(&mut self.response) {
            InnerGalleryResponse::Variant(variant) => variant,
            _ => {
                godot_error!("GalleryResponse is not Variant!");
                Variant::nil()
            }
        }
    }
}
