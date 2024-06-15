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
    gallery_db::GalleryDb,
    met_api::load_met_api_record,
    met_csv::{iter_public_domain_2d_met_csv_objects, MetObjectCsvResult},
};
use godot::{
    engine::{Engine, Image, ImageTexture, Os, ProjectSettings},
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
    GetNextCsvRecord(u32),
    GetMetObjectsForGalleryWall {
        request_id: u32,
        gallery_id: u64,
        wall_id: String,
    },
}

enum ChannelResponse {
    Done,
    NextCsvRecord(u32, Option<SimplifiedRecord>),
    MetObjectsForGalleryWall(u32, Vec<SimplifiedRecord>),
}

#[derive(Debug)]
struct SimplifiedRecord {
    object_id: u64,
    title: String,
    date: String,
    width: f64,
    height: f64,
    small_image: String,
    x: f64,
    y: f64,
}

#[derive(Debug, GodotClass)]
#[class(init)]
struct MetResponse {
    #[var]
    request_id: u32,
    response: Variant,
}

#[godot_api]
impl MetResponse {
    #[func]
    fn get(&self) -> Variant {
        self.response.clone()
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
    small_image: GString,
}

#[godot_api]
impl MetObject {
    #[func]
    fn load_small_image_texture(&self) -> Option<Gd<ImageTexture>> {
        let Some(mut image) = Image::load_from_file(self.small_image.clone()) else {
            return None;
        };
        image.generate_mipmaps();
        ImageTexture::create_from_image(image)
    }

    #[func]
    fn can_fit_in(&self, max_width: f64, max_height: f64) -> bool {
        self.width <= max_width && self.height <= max_height
    }

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

fn download_records(
    cache: &GalleryCache,
    db: &mut GalleryDb,
    gallery_id: u64,
    wall_id: String,
) -> Result<Vec<SimplifiedRecord>> {
    let mut result = vec![];
    let objects = db.get_met_objects_for_gallery_wall(gallery_id, wall_id)?;
    for (object, (x, y)) in objects {
        let obj_record = load_met_api_record(&cache, object.object_id)?;
        if let Some((width, height, small_image)) =
            obj_record.try_to_download_small_image(&cache)?
        {
            result.push(SimplifiedRecord {
                object_id: obj_record.object_id,
                title: obj_record.title,
                date: obj_record.object_date,
                width: width / 100.0,   // Convert from centimeters to meters
                height: height / 100.0, // Convert from centimeters to meters
                small_image: cache
                    .cache_dir()
                    .join(small_image)
                    .to_string_lossy()
                    .to_string(),
                x,
                y,
            });
        }
    }
    Ok(vec![])
}

fn find_and_download_next_valid_record(
    csv_iterator: &mut impl Iterator<Item = MetObjectCsvResult>,
    cache: &GalleryCache,
) -> Result<Option<SimplifiedRecord>> {
    loop {
        let Some(result) = csv_iterator.next() else {
            // We reached the end of all the records!
            return Ok(None);
        };
        let csv_record = result?;
        let obj_record = load_met_api_record(&cache, csv_record.object_id)?;
        if let Some((width, height, small_image)) =
            obj_record.try_to_download_small_image(&cache)?
        {
            return Ok(Some(SimplifiedRecord {
                object_id: obj_record.object_id,
                title: obj_record.title,
                date: obj_record.object_date,
                width: width / 100.0,   // Convert from centimeters to meters
                height: height / 100.0, // Convert from centimeters to meters
                small_image: cache
                    .cache_dir()
                    .join(small_image)
                    .to_string_lossy()
                    .to_string(),
                x: 0.0,
                y: 0.0,
            }));
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
    let csv_file = cache.get_cached_path("MetObjects.csv");
    let reader = BufReader::new(File::open(csv_file)?);
    let rdr = csv::Reader::from_reader(reader);
    let mut csv_iterator = iter_public_domain_2d_met_csv_objects(rdr);
    let db_path = cache.get_cached_path("gallery.sqlite");
    let mut db = GalleryDb::new(Connection::open(db_path)?);
    loop {
        println!("work_thread waiting for command.");
        match cmd_rx.recv() {
            Ok(ChannelCommand::End) => {
                println!("work_thread received 'end' command.");
                break;
            }
            Ok(ChannelCommand::GetMetObjectsForGalleryWall {
                request_id,
                gallery_id,
                wall_id,
            }) => {
                println!("work_thread received 'GetMetObjectsForGalleryWall' command, request_id={request_id}, gallery_id={gallery_id}, wall_id={wall_id}.");
                if response_tx
                    .send(ChannelResponse::MetObjectsForGalleryWall(
                        request_id,
                        download_records(&cache, &mut db, gallery_id, wall_id)?,
                    ))
                    .is_err()
                {
                    // The other end hung up, we're effectively done.
                    break;
                }
            }
            Ok(ChannelCommand::GetNextCsvRecord(request_id)) => {
                println!("work_thread received 'NextCsvRecord' command, request_id={request_id:}.");
                let simplified_record =
                    find_and_download_next_valid_record(&mut csv_iterator, &cache)?;
                if response_tx
                    .send(ChannelResponse::NextCsvRecord(
                        request_id,
                        simplified_record,
                    ))
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
            next_request_id: 1,
        }
    }
}

const NULL_REQUEST_ID: u32 = 0;

#[godot_api]
impl MetObjectsSingleton {
    #[func]
    fn get_met_objects_for_gallery_wall(&mut self, gallery_id: u64, wall_id: String) -> u32 {
        let request_id = self.new_request_id();
        if self
            .cmd_tx
            .send(ChannelCommand::GetMetObjectsForGalleryWall {
                request_id,
                gallery_id,
                wall_id,
            })
            .is_ok()
        {
            request_id
        } else {
            godot_print!("cmd_tx.send() failed!");
            self.handler = None;
            NULL_REQUEST_ID
        }
    }

    #[func]
    fn next_csv_record(&mut self) -> u32 {
        let request_id = self.new_request_id();
        if self
            .cmd_tx
            .send(ChannelCommand::GetNextCsvRecord(request_id))
            .is_ok()
        {
            request_id
        } else {
            godot_print!("cmd_tx.send() failed!");
            self.handler = None;
            NULL_REQUEST_ID
        }
    }

    fn new_request_id(&mut self) -> u32 {
        let request_id = self.next_request_id;
        self.next_request_id += 1;
        request_id
    }

    #[func]
    fn poll(&mut self) -> Option<Gd<MetResponse>> {
        match self.response_rx.try_recv() {
            Ok(ChannelResponse::Done) => {
                godot_print!("No more objects!");
                self.handler = None;
            }
            Ok(ChannelResponse::MetObjectsForGalleryWall(request_id, objects)) => {
                let variant_array: VariantArray = objects
                    .into_iter()
                    .map(|object| {
                        Gd::from_object(MetObject {
                            object_id: object.object_id as i64,
                            title: object.title.into_godot(),
                            date: object.date.into_godot(),
                            width: object.width,
                            height: object.height,
                            small_image: object.small_image.into_godot(),
                            x: object.x,
                            y: object.y,
                        })
                        .to_variant()
                    })
                    .collect();
                return Some(Gd::from_object(MetResponse {
                    request_id,
                    response: variant_array.to_variant(),
                }));
            }
            Ok(ChannelResponse::NextCsvRecord(request_id, object)) => {
                return Some(Gd::from_object(MetResponse {
                    request_id,
                    response: match object {
                        Some(object) => Gd::from_object(MetObject {
                            object_id: object.object_id as i64,
                            title: object.title.into_godot(),
                            date: object.date.into_godot(),
                            width: object.width,
                            height: object.height,
                            small_image: object.small_image.into_godot(),
                            x: object.x,
                            y: object.y,
                        })
                        .to_variant(),
                        None => Variant::nil(),
                    },
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
