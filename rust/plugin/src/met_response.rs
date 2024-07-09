use godot::{engine::Image, prelude::*};

use crate::met_object::MetObject;

#[derive(Debug)]
pub enum InnerMetResponse {
    Variant(Variant),
    MetObjects(Array<Gd<MetObject>>),
    Image(Option<Gd<Image>>),
}

impl Default for InnerMetResponse {
    fn default() -> Self {
        InnerMetResponse::Variant(Variant::default())
    }
}

#[derive(Debug, GodotClass)]
#[class(init)]
pub struct MetResponse {
    #[var]
    pub request_id: u32,
    pub response: InnerMetResponse,
}

#[godot_api]
impl MetResponse {
    #[func]
    fn take_met_objects(&mut self) -> Array<Gd<MetObject>> {
        match std::mem::take(&mut self.response) {
            InnerMetResponse::MetObjects(response) => response,
            _ => {
                godot_error!("MetResponse is not MetObjects!");
                Array::new()
            }
        }
    }

    #[func]
    fn take_optional_image(&mut self) -> Option<Gd<Image>> {
        match std::mem::take(&mut self.response) {
            InnerMetResponse::Image(response) => response,
            _ => {
                godot_error!("MetResponse is not Image!");
                None
            }
        }
    }

    #[func]
    fn take_variant(&mut self) -> Variant {
        match std::mem::take(&mut self.response) {
            InnerMetResponse::Variant(variant) => variant,
            _ => {
                godot_error!("MetResponse is not Variant!");
                Variant::nil()
            }
        }
    }
}
