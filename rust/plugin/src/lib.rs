use godot::prelude::*;

struct MyExtension;

mod gallery_client;
mod met_object;
mod met_response;
mod worker_thread;

#[gdextension]
unsafe impl ExtensionLibrary for MyExtension {}
