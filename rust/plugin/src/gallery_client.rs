use std::{
    path::PathBuf,
    sync::mpsc::{channel, Receiver, SendError, Sender, TryRecvError},
    thread::{self, JoinHandle},
};

use godot::{
    engine::{Image, Os, ProjectSettings},
    prelude::*,
};

use crate::{
    met_object::MetObject,
    met_response::{InnerMetResponse, MetResponse},
    worker_thread::{work_thread, ChannelCommand, ChannelResponse},
};

const NULL_REQUEST_ID: u32 = 0;

#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct GalleryClient {
    base: Base<RefCounted>,
    cmd_tx: Sender<ChannelCommand>,
    response_rx: Receiver<ChannelResponse>,
    handler: Option<JoinHandle<()>>,
    fatal_error: Option<String>,
    next_request_id: u32,
}

fn normalize_path(path: String) -> PathBuf {
    if cfg!(windows) {
        // Godot always uses '/' as a path separator. There doesn't seem to
        // be any built-in tooling to convert to an OS-specific path, so we'll
        // just do this manually. (Fortunately slashes are illegal characters in
        // Windows file names, so we don't need to worry about this accidentally
        // changing the name of a directory.)
        path.replace("/", "\\").into()
    } else {
        path.into()
    }
}

fn get_root_dir() -> PathBuf {
    let os = Os::singleton();
    if os.has_feature("editor".into()) {
        // Running from an editor binary.
        normalize_path(
            ProjectSettings::singleton()
                .globalize_path(GString::from("res://"))
                .to_string(),
        )
    } else {
        // Running from an exported project.
        let executable_path = normalize_path(os.get_executable_path().to_string());
        executable_path.parent().unwrap().to_path_buf()
    }
}

#[godot_api]
impl IRefCounted for GalleryClient {
    fn init(base: Base<RefCounted>) -> Self {
        let root_dir: PathBuf = get_root_dir();
        godot_print!("Root dir is {}.", root_dir.display());
        let (cmd_tx, cmd_rx) = channel::<ChannelCommand>();
        let (response_tx, response_rx) = channel::<ChannelResponse>();
        godot_print!("Spawning work thread.");
        let handler = thread::spawn(move || {
            if let Err(err) = work_thread(root_dir.clone(), cmd_rx, response_tx.clone()) {
                eprintln!("Thread errored: {err:?}");
                let _ = response_tx.send(ChannelResponse::FatalError(format!("{err:?}")));
            }
        });
        Self {
            base,
            cmd_tx,
            response_rx,
            handler: Some(handler),
            next_request_id: 1,
            fatal_error: None,
        }
    }
}

#[godot_api]
impl GalleryClient {
    fn handle_send_error(&mut self, err: SendError<ChannelCommand>) {
        if self.handler.is_some() {
            godot_error!("sending command failed: {:?}", err);
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
            NULL_REQUEST_ID
        } else {
            request_id
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
    fn take_fatal_error(&mut self) -> String {
        self.fatal_error.take().unwrap_or_default()
    }

    #[func]
    fn poll(&mut self) -> Option<Gd<MetResponse>> {
        if self.handler.is_none() {
            return None;
        }
        match self.response_rx.try_recv() {
            Ok(ChannelResponse::Done) => {
                godot_print!("Work thread exited cleanly.");
                self.handler = None;
            }
            Ok(ChannelResponse::FatalError(message)) => {
                godot_error!("Work thread encountered fatal error: {message}");
                self.fatal_error = Some(message);
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

impl Drop for GalleryClient {
    fn drop(&mut self) {
        godot_print!("drop GalleryClient!");
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
