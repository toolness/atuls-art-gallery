use std::{
    sync::mpsc::{channel, Sender},
    thread::{self, JoinHandle},
};

use godot::{
    engine::{Engine, ProjectSettings},
    prelude::*,
};

struct MyExtension;

const SINGLETON_NAME: &'static str = "RustMetObjects";

#[gdextension]
unsafe impl ExtensionLibrary for MyExtension {
    fn on_level_init(level: InitLevel) {
        if level == InitLevel::Scene {
            Engine::singleton().register_singleton(
                StringName::from(SINGLETON_NAME),
                MetObjectsSingleton::new_alloc().upcast(),
            );
        }
    }

    fn on_level_deinit(level: InitLevel) {
        if level == InitLevel::Scene {
            let mut engine = Engine::singleton();
            let singleton_name = StringName::from(SINGLETON_NAME);

            let singleton = engine
                .get_singleton(singleton_name.clone())
                .expect("Cannot retrieve the singleton");

            engine.unregister_singleton(singleton_name);
            singleton.free();
        }
    }
}

#[derive(GodotClass)]
#[class(base=Object)]
struct MetObjectsSingleton {
    base: Base<Object>,
    tx: Sender<ChannelMesssage>,
    handler: Option<JoinHandle<()>>,
}

enum ChannelMesssage {
    End,
}

#[godot_api]
impl IObject for MetObjectsSingleton {
    fn init(base: Base<Object>) -> Self {
        let project_settings = ProjectSettings::singleton();
        let mut root_dir = project_settings
            .globalize_path(GString::from("res://"))
            .to_string();
        if cfg!(windows) {
            // Godot always uses '/' as a path separator. There doesn't seem to
            // be any built-in tooling to convert to an OS-specific path, so we'll
            // just do this manually. (Fortunately slashes are illegal characters in
            // Windows file names, so we don't need to worry about this accidentally
            // changing the name of a directory.)
            root_dir = root_dir.replace("/", "\\");
        }
        let (tx, rx) = channel::<ChannelMesssage>();
        let handler = thread::spawn(move || {
            loop {
                match rx.recv() {
                    Ok(ChannelMesssage::End) => {
                        break;
                    }
                    Err(_) => {
                        // The other end hung up, just quit.
                        return;
                    }
                }
            }
        });

        godot_print!("init MetObjectsSingleton, root dir is: {}", root_dir);
        Self {
            base,
            tx,
            handler: Some(handler),
        }
    }
}

#[godot_api]
impl MetObjectsSingleton {
    #[func]
    fn add(&self, a: i32, b: i32) -> i32 {
        godot_print!("ADD {a} + {b}!?");
        a + b
    }
}

impl Drop for MetObjectsSingleton {
    fn drop(&mut self) {
        godot_print!("drop MetObjectsSingleton!");
        if let Some(handler) = self.handler.take() {
            if let Err(err) = self.tx.send(ChannelMesssage::End) {
                godot_print!("Error sending end signal to thread: {:?}", err);
                return;
            }
            match handler.join() {
                Ok(_) => {
                    godot_print!("Joined thread.");
                }
                Err(err) => {
                    godot_print!("Error joining thread: {:?}", err);
                }
            }
        }
    }
}
