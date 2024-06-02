use std::{
    fs::File,
    io::BufReader,
    path::PathBuf,
    sync::mpsc::{channel, Receiver, Sender, TryRecvError},
    thread::{self, JoinHandle},
};

use anyhow::Result;
use gallery::{
    gallery_cache::GalleryCache,
    the_met::{
        is_public_domain_2d_met_object, load_met_object_record, DimensionParser, MetObjectCsvRecord,
    },
};
use godot::{
    engine::{Engine, ProjectSettings},
    prelude::*,
};
use serde::Serialize;

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
    cmd_tx: Sender<ChannelCommand>,
    response_rx: Receiver<ChannelResponse>,
    handler: Option<JoinHandle<()>>,
}

enum ChannelCommand {
    End,
    Next,
}

enum ChannelResponse {
    Done,
    MetObject(SimplifiedRecord),
}

// TODO: Consolidate from CLI?
#[derive(Debug, Serialize)]
struct SimplifiedRecord {
    object_id: u64,
    title: String,
    date: String,
    width: f64,
    height: f64,
    small_image: String,
}

fn work_thread(
    root_dir: PathBuf,
    cmd_rx: Receiver<ChannelCommand>,
    response_tx: Sender<ChannelResponse>,
) -> Result<()> {
    let cache_dir = root_dir.join("..").join("rust").join("cache");
    let cache = GalleryCache::new(cache_dir.clone());
    let csv_file = cache.get_cached_path("MetObjects.csv");
    let reader = BufReader::new(File::open(csv_file)?);
    let rdr = csv::Reader::from_reader(reader);
    let dimension_parser = DimensionParser::new();
    let mut csv_iterator = rdr.into_deserialize();
    'outer: loop {
        println!("work_thread waiting for command.");
        match cmd_rx.recv() {
            Ok(ChannelCommand::End) => {
                println!("work_thread received 'end' command.");
                break 'outer;
            }
            Ok(ChannelCommand::Next) => {
                println!("work_thread received 'next' command.");
                // TODO: These nested loops are horrible, factor out some functions or something.
                'find_and_download_next_valid_record: loop {
                    let csv_record: MetObjectCsvRecord;
                    'find_next_valid_record: loop {
                        if let Some(maybe_result) = csv_iterator.next() {
                            let maybe_csv_record: MetObjectCsvRecord = maybe_result?;
                            if !is_public_domain_2d_met_object(&dimension_parser, &maybe_csv_record)
                            {
                                continue 'find_next_valid_record;
                            }
                            csv_record = maybe_csv_record;
                            break 'find_next_valid_record;
                        } else {
                            // We reached the end of all the records!
                            break 'outer;
                        }
                    }
                    let obj_record = load_met_object_record(&cache, csv_record.object_id)?;
                    if let Some((width, height, small_image)) =
                        obj_record.try_to_download_small_image(&cache)?
                    {
                        if response_tx
                            .send(ChannelResponse::MetObject(SimplifiedRecord {
                                object_id: obj_record.object_id,
                                title: obj_record.title,
                                date: obj_record.object_date,
                                width,
                                height,
                                small_image: cache_dir
                                    .join(small_image)
                                    .to_string_lossy()
                                    .to_string(),
                            }))
                            .is_err()
                        {
                            break 'outer;
                        }
                        break 'find_and_download_next_valid_record;
                    } else {
                        continue 'find_and_download_next_valid_record;
                    }
                }
            }
            Err(_) => {
                // The other end hung up, just quit.
                break 'outer;
            }
        }
    }

    // Ignoring result, there's not much we can do if this send fails.
    let _ = response_tx.send(ChannelResponse::Done);

    Ok(())
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
        let (cmd_tx, cmd_rx) = channel::<ChannelCommand>();
        let (response_tx, response_rx) = channel::<ChannelResponse>();
        let handler = thread::spawn(move || {
            if let Err(err) = work_thread(root_dir.into(), cmd_rx, response_tx) {
                println!("Thread errored: {:?}", err);
            }
        });
        Self {
            base,
            cmd_tx,
            response_rx,
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

    #[func]
    fn next(&mut self) {
        if self.cmd_tx.send(ChannelCommand::Next).is_err() {
            godot_print!("cmd_tx.send() failed!");
            self.handler = None;
        }
    }

    /// This returns a JSON-serialized string. It's really bad but I can't figure out any other
    /// reasonable way of returning structured data.
    #[func]
    fn poll(&mut self) -> GString {
        match self.response_rx.try_recv() {
            Ok(ChannelResponse::Done) => {
                godot_print!("No more objects!");
                self.handler = None;
            }
            Ok(ChannelResponse::MetObject(object)) => match serde_json::to_string(&object) {
                Err(err) => {
                    godot_print!("Failed to serialize result: {:?}", err);
                }
                Ok(result) => {
                    return result.into_godot();
                }
            },
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                godot_print!("response_rx.recv() failed, thread died!");
                self.handler = None;
            }
        }
        GString::default()
    }
}

impl Drop for MetObjectsSingleton {
    fn drop(&mut self) {
        godot_print!("drop MetObjectsSingleton!");
        if let Some(handler) = self.handler.take() {
            if let Err(err) = self.cmd_tx.send(ChannelCommand::End) {
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
