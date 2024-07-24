use godot::prelude::*;

struct GalleryExtension;

mod art_object;
mod gallery_client;
mod gallery_response;
mod worker_thread;

#[gdextension]
unsafe impl ExtensionLibrary for GalleryExtension {}
