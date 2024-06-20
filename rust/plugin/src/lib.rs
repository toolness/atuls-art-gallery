use std::{
    path::PathBuf,
    sync::mpsc::{channel, Receiver, SendError, Sender, TryRecvError},
    thread::{self, JoinHandle},
};

use anyhow::Result;
use gallery::{
    gallery_cache::GalleryCache,
    gallery_db::{GalleryDb, LayoutRecord},
    met_api::load_met_api_record,
};
use godot::{
    engine::{Engine, Image, Os, ProjectSettings},
    prelude::*,
};
use rusqlite::Connection;

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
    next_request_id: u32,
}

enum ChannelCommand {
    End,
    MoveMetObject {
        met_object_id: u64,
        gallery_id: i64,
        wall_id: String,
        x: f64,
        y: f64,
    },
    GetMetObjectsForGalleryWall {
        request_id: u32,
        gallery_id: i64,
        wall_id: String,
    },
    FetchSmallImage {
        request_id: u32,
        object_id: u64,
    },
}

enum ChannelResponse {
    Done,
    MetObjectsForGalleryWall(u32, Vec<SimplifiedRecord>),
    Image(u32, Option<PathBuf>),
}

#[derive(Debug)]
struct SimplifiedRecord {
    object_id: u64,
    title: String,
    date: String,
    width: f64,
    height: f64,
    x: f64,
    y: f64,
}

#[derive(Default, Debug)]
enum InnerMetResponse {
    #[default]
    None,
    MetObjects(Array<Gd<MetObject>>),
    Image(Option<Gd<Image>>),
}

#[derive(Debug, GodotClass)]
#[class(init)]
struct MetResponse {
    #[var]
    request_id: u32,
    response: InnerMetResponse,
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
}

#[derive(Debug, GodotClass)]
#[class(init)]
struct MetObject {
    #[var]
    object_id: i64,
    #[var]
    title: GString,
    #[var]
    date: GString,
    #[var]
    width: f64,
    #[var]
    height: f64,
    #[var]
    x: f64,
    #[var]
    y: f64,
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

fn get_met_objects_for_gallery_wall(
    db: &mut GalleryDb,
    gallery_id: i64,
    wall_id: String,
) -> Result<Vec<SimplifiedRecord>> {
    let mut result = vec![];
    let objects = db.get_met_objects_for_gallery_wall(gallery_id, wall_id)?;
    for (object, (x, y)) in objects {
        result.push(SimplifiedRecord {
            object_id: object.object_id,
            title: object.title,
            date: object.object_date,
            width: object.width,
            height: object.height,
            x,
            y,
        });
    }
    Ok(result)
}

fn fetch_small_image(cache: &GalleryCache, met_object_id: u64) -> Option<PathBuf> {
    match load_met_api_record(&cache, met_object_id) {
        Ok(obj_record) => match obj_record.try_to_download_small_image(&cache) {
            Ok(Some((_width, _height, small_image))) => Some(cache.cache_dir().join(small_image)),
            Ok(None) => None,
            Err(err) => {
                eprintln!(
                    "Unable to download small image for met object ID {}: {:?}",
                    met_object_id, err
                );
                None
            }
        },
        Err(err) => {
            eprintln!(
                "Unable to load Met API record for met object ID {}: {:?}",
                met_object_id, err
            );
            None
        }
    }
}

fn work_thread(
    root_dir: PathBuf,
    cmd_rx: Receiver<ChannelCommand>,
    response_tx: Sender<ChannelResponse>,
) -> Result<()> {
    let cache_dir = root_dir.join("rust").join("cache");
    let cache = GalleryCache::new(cache_dir.clone());
    let db_path = cache.get_cached_path("gallery.sqlite");
    let mut db = GalleryDb::new(Connection::open(db_path)?);
    println!("work_thread waiting for command.");
    loop {
        match cmd_rx.recv() {
            Ok(ChannelCommand::End) => {
                println!("work_thread received 'end' command.");
                break;
            }
            Ok(ChannelCommand::MoveMetObject {
                met_object_id,
                gallery_id,
                wall_id,
                x,
                y,
            }) => {
                //println!("work_thread received 'MoveMetObject' command.");
                db.upsert_layout_records(&vec![LayoutRecord {
                    gallery_id,
                    wall_id,
                    met_object_id,
                    x,
                    y,
                }])?;
            }
            Ok(ChannelCommand::GetMetObjectsForGalleryWall {
                request_id,
                gallery_id,
                wall_id,
            }) => {
                //println!("work_thread received 'GetMetObjectsForGalleryWall' command, request_id={request_id}, gallery_id={gallery_id}, wall_id={wall_id}.");
                let records = get_met_objects_for_gallery_wall(&mut db, gallery_id, wall_id)?;
                if response_tx
                    .send(ChannelResponse::MetObjectsForGalleryWall(
                        request_id, records,
                    ))
                    .is_err()
                {
                    // The other end hung up, we're effectively done.
                    break;
                }
            }
            Ok(ChannelCommand::FetchSmallImage {
                request_id,
                object_id,
            }) => {
                //println!("work_thread received 'FetchSmallImage' command, request_id={request_id}, object_id={object_id}.");
                let small_image = fetch_small_image(&cache, object_id);
                if response_tx
                    .send(ChannelResponse::Image(request_id, small_image))
                    .is_err()
                {
                    // The other end hung up, we're effectively done.
                    break;
                }
            }
            Err(_) => {
                // The other end hung up, just quit.
                break;
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
        let is_running_in_editor = Engine::singleton().is_editor_hint();
        let handler = if is_running_in_editor {
            godot_print!("Running in editor, not spawning work thread.");
            None
        } else {
            Some(thread::spawn(move || {
                let root_dir = PathBuf::from(root_dir);
                if let Err(err) = work_thread(root_dir.clone(), cmd_rx, response_tx) {
                    eprintln!("Thread errored: {err:?}");
                }
            }))
        };
        Self {
            base,
            cmd_tx,
            response_rx,
            handler,
            next_request_id: 1,
        }
    }
}

const NULL_REQUEST_ID: u32 = 0;

#[godot_api]
impl MetObjectsSingleton {
    fn handle_send_error(&mut self, err: SendError<ChannelCommand>) {
        if self.handler.is_some() {
            godot_error!("sending command failed: {:?}", err);
            self.handler = None;
        }
    }

    fn send(&mut self, command: ChannelCommand) {
        let result = self.cmd_tx.send(command);
        if let Err(err) = result {
            self.handle_send_error(err);
        }
    }

    fn send_request(&mut self, request_id: u32, command: ChannelCommand) -> u32 {
        let result = self.cmd_tx.send(command);
        if let Err(err) = result {
            self.handle_send_error(err);
            request_id
        } else {
            NULL_REQUEST_ID
        }
    }

    #[func]
    fn move_met_object(
        &mut self,
        met_object_id: u64,
        gallery_id: i64,
        wall_id: String,
        x: f64,
        y: f64,
    ) {
        self.send(ChannelCommand::MoveMetObject {
            met_object_id,
            gallery_id,
            wall_id,
            x,
            y,
        });
    }

    #[func]
    fn get_met_objects_for_gallery_wall(&mut self, gallery_id: i64, wall_id: String) -> u32 {
        let request_id = self.new_request_id();
        self.send_request(
            request_id,
            ChannelCommand::GetMetObjectsForGalleryWall {
                request_id,
                gallery_id,
                wall_id,
            },
        )
    }

    #[func]
    fn fetch_small_image(&mut self, object_id: u64) -> u32 {
        let request_id = self.new_request_id();
        self.send_request(
            request_id,
            ChannelCommand::FetchSmallImage {
                request_id,
                object_id,
            },
        )
    }

    fn new_request_id(&mut self) -> u32 {
        let request_id = self.next_request_id;
        self.next_request_id += 1;
        request_id
    }

    #[func]
    fn poll(&mut self) -> Option<Gd<MetResponse>> {
        if self.handler.is_none() {
            return None;
        }
        match self.response_rx.try_recv() {
            Ok(ChannelResponse::Done) => {
                godot_print!("Work thread exited.");
                self.handler = None;
            }
            Ok(ChannelResponse::MetObjectsForGalleryWall(request_id, objects)) => {
                return Some(Gd::from_object(MetResponse {
                    request_id,
                    response: InnerMetResponse::MetObjects(Array::from_iter(
                        objects.into_iter().map(|object| {
                            Gd::from_object(MetObject {
                                object_id: object.object_id as i64,
                                title: object.title.into_godot(),
                                date: object.date.into_godot(),
                                width: object.width,
                                height: object.height,
                                x: object.x,
                                y: object.y,
                            })
                        }),
                    )),
                }));
            }
            Ok(ChannelResponse::Image(request_id, small_image)) => {
                let image = small_image
                    .map(|small_image| {
                        Image::load_from_file(GString::from(
                            small_image.to_string_lossy().into_owned(),
                        ))
                    })
                    .flatten();
                return Some(Gd::from_object(MetResponse {
                    request_id,
                    response: InnerMetResponse::Image(image),
                }));
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                godot_print!("response_rx.recv() failed, thread died!");
                self.handler = None;
            }
        }
        None
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
