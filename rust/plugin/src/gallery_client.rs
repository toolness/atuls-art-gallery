use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::mpsc::{channel, Receiver, SendError, Sender, TryRecvError},
    thread::{self, JoinHandle},
};

use gallery::{
    art_object::ArtObjectId, gallery_db::get_default_gallery_db_filename, image::ImageSize,
};
use godot::{
    engine::{
        multiplayer_api::RpcMode,
        multiplayer_peer::{ConnectionStatus, TransferMode},
        FileAccess, MultiplayerPeer, OfflineMultiplayerPeer, ProjectSettings,
    },
    prelude::*,
};

use crate::{
    art_object::ArtObject,
    gallery_response::{GalleryResponse, InnerGalleryResponse},
    worker_thread::{
        work_thread, MessageFromWorker, MessageToWorker, Request, RequestBody, Response,
        ResponseBody,
    },
};

const NULL_REQUEST_ID: u32 = 0;

struct Connection {
    to_worker_tx: Sender<MessageToWorker>,
    from_worker_rx: Receiver<MessageFromWorker>,
    handler: JoinHandle<()>,
}

impl Connection {
    fn connect(root_dir: PathBuf, enable_autosync: bool) -> Self {
        godot_print!("Root dir is {}.", root_dir.display());
        let (to_worker_tx, to_worker_rx) = channel::<MessageToWorker>();
        let (from_worker_tx, from_worker_rx) = channel::<MessageFromWorker>();
        godot_print!("Spawning gallery worker thread.");
        let handler = thread::spawn(move || {
            if let Err(err) = work_thread(
                root_dir.clone(),
                enable_autosync,
                to_worker_rx,
                from_worker_tx.clone(),
            ) {
                eprintln!("Gallery worker thread errored: {err:?}");
                let _ = from_worker_tx.send(MessageFromWorker::FatalError(format!("{err:?}")));
            }
        });
        Self {
            to_worker_tx,
            from_worker_rx,
            handler,
        }
    }

    fn disconnect(self) {
        if let Err(err) = self.to_worker_tx.send(MessageToWorker::End) {
            godot_print!(
                "Error sending end signal to gallery worker thread: {:?}",
                err
            );
            return;
        }
        match self.handler.join() {
            Ok(_) => {
                godot_print!("Joined gallery worker thread.");
            }
            Err(err) => {
                godot_print!("Error joining gallery worker thread: {:?}", err);
            }
        }
    }
}

#[derive(GodotClass)]
#[class(base=Node)]
pub struct GalleryClient {
    base: Base<Node>,
    connection: Option<Connection>,
    queued_requests: Vec<(u32, RequestBody)>,
    queued_responses: VecDeque<(u32, ResponseBody)>,
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

#[godot_api]
impl INode for GalleryClient {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            connection: None,
            next_request_id: 1,
            fatal_error: None,
            queued_requests: vec![],
            queued_responses: VecDeque::new(),
        }
    }

    fn ready(&mut self) {
        self.base_mut().rpc_config(
            "proxy_request_to_server_internal".into(),
            dict! {
                "rpc_mode": RpcMode::ANY_PEER,
                "transfer_mode": TransferMode::RELIABLE,
                "call_local": false,
            }
            .to_variant(),
        );
        self.base_mut().rpc_config(
            "proxy_response_from_server_internal".into(),
            dict! {
                "rpc_mode": RpcMode::AUTHORITY,
                "transfer_mode": TransferMode::RELIABLE,
                "call_local": false,
            }
            .to_variant(),
        );
        godot_print!(
            "GalleryClient ready, is_multiplayer_client={} is_multiplayer_server={} is_offline_mode={}",
            self.is_multiplayer_client(),
            self.is_multiplayer_server(),
            self.is_offline_mode()
        );
    }
}

#[godot_api]
impl GalleryClient {
    #[func]
    fn default_db_filename(&mut self) -> GString {
        get_default_gallery_db_filename().into_godot()
    }

    #[func]
    fn connect(&mut self, root_dir: GString, enable_autosync: bool) {
        let globalized_root_dir = globalize_path(root_dir);
        self.connection = Some(Connection::connect(globalized_root_dir, enable_autosync));
    }

    fn handle_send_error(&mut self, err: SendError<MessageToWorker>) {
        if self.connection.is_some() {
            godot_error!("Sending message to gallery worker thread failed: {:?}", err);
        }
    }

    fn send(&mut self, message: MessageToWorker) {
        let Some(connection) = &self.connection else {
            return;
        };
        let result = connection.to_worker_tx.send(message);
        if let Err(err) = result {
            self.handle_send_error(err);
        }
    }

    /// This returns if the game itself is in offline mode, *not* if we're in multiplayer mode but
    /// currently disconnected from the server.
    fn is_offline_mode(&self) -> bool {
        // lol why is this so complicated
        self.base()
            .get_multiplayer()
            .map(|mut multiplayer| {
                multiplayer
                    .get_multiplayer_peer()
                    .map(|peer| peer.try_cast::<OfflineMultiplayerPeer>().is_ok())
            })
            .flatten()
            .unwrap_or(true)
    }

    fn get_multiplayer_client(&self) -> Option<Gd<MultiplayerPeer>> {
        if self.is_offline_mode() || self.base().is_multiplayer_authority() {
            return None;
        }
        let Some(multiplayer) = &mut self.base().get_multiplayer() else {
            return None;
        };
        multiplayer.get_multiplayer_peer()
    }

    fn is_multiplayer_client(&self) -> bool {
        self.get_multiplayer_client().is_some()
    }

    fn is_multiplayer_server(&self) -> bool {
        if self.is_offline_mode() || !self.base().is_multiplayer_authority() {
            return false;
        }
        let Some(multiplayer) = &mut self.base().get_multiplayer() else {
            return false;
        };
        multiplayer.has_multiplayer_peer() && self.base().is_multiplayer_authority()
    }

    #[func]
    fn proxy_request_to_server_internal(
        &mut self,
        request_id: u32,
        serialized_request_body: String,
    ) {
        if !self.is_multiplayer_server() {
            godot_error!("Non-servers cannot handle proxied requests!");
            return;
        }
        let multiplayer = &mut self.base().get_multiplayer().unwrap();
        let remote_sender_id = multiplayer.get_remote_sender_id();
        if remote_sender_id == 0 {
            godot_error!("Proxying requests must be done in an RPC context!");
            return;
        }
        let body = serde_json::from_str::<RequestBody>(&serialized_request_body);
        match body {
            Ok(body) => {
                if !body.is_proxyable_to_server() {
                    godot_error!("Proxied request is not proxyable to server: {:?}", body);
                    return;
                }
                //godot_print!("Received proxied request: {:?}", body);
                self.send(MessageToWorker::Request(Request {
                    peer_id: Some(remote_sender_id),
                    request_id,
                    body,
                }));
            }
            Err(err) => {
                godot_error!(
                    "Unable to deserialize proxied request: {}, error={:?}",
                    serialized_request_body,
                    err
                );
            }
        }
    }

    #[func]
    fn proxy_response_from_server_internal(
        &mut self,
        request_id: u32,
        serialized_response_body: String,
    ) {
        if !self.is_multiplayer_client() {
            godot_error!("Non-clients cannot handled proxied responses!");
            return;
        }
        let body = serde_json::from_str::<ResponseBody>(&serialized_response_body);
        match body {
            Ok(body) => {
                //godot_print!("Received proxied response: {:?}", body);
                self.queued_responses.push_back((request_id, body));
            }
            Err(err) => {
                godot_error!(
                    "Unable to deserialize proxied response body: {}, error={:?}",
                    serialized_response_body,
                    err
                );
            }
        }
    }

    #[func]
    fn get_art_object_url(&self, art_object_id: i64) -> String {
        ArtObjectId::from_raw_i64(art_object_id).url()
    }

    fn send_request(&mut self, body: RequestBody) -> u32 {
        let request_id = self.new_request_id();
        if body.is_proxyable_to_server() && self.is_multiplayer_client() {
            // We're being very optimistic here and assuming all requests will eventually
            // be sent.
            self.queued_requests.push((request_id, body));
            return request_id;
        }
        let Some(connection) = &self.connection else {
            return NULL_REQUEST_ID;
        };
        let result = connection
            .to_worker_tx
            .send(MessageToWorker::Request(Request {
                peer_id: None,
                request_id,
                body,
            }));
        if let Err(err) = result {
            self.handle_send_error(err);
            NULL_REQUEST_ID
        } else {
            request_id
        }
    }

    #[func]
    fn move_art_object(
        &mut self,
        art_object_id: i64,
        gallery_id: i64,
        wall_id: String,
        x: f64,
        y: f64,
    ) {
        self.send_request(RequestBody::MoveArtObject {
            art_object_id: ArtObjectId::from_raw_i64(art_object_id),
            gallery_id,
            wall_id,
            x,
            y,
        });
    }

    #[func]
    fn get_art_objects_for_gallery_wall(&mut self, gallery_id: i64, wall_id: String) -> u32 {
        self.send_request(RequestBody::GetArtObjectsForGalleryWall {
            gallery_id,
            wall_id,
        })
    }

    #[func]
    fn fetch_small_image(&mut self, object_id: i64) -> u32 {
        self.send_request(RequestBody::FetchImage {
            object_id: ArtObjectId::from_raw_i64(object_id),
            size: ImageSize::Small,
        })
    }

    #[func]
    fn fetch_large_image(&mut self, object_id: i64) -> u32 {
        self.send_request(RequestBody::FetchImage {
            object_id: ArtObjectId::from_raw_i64(object_id),
            size: ImageSize::Large,
        })
    }

    #[func]
    fn count_art_objects(&mut self, filter: String) -> u32 {
        self.send_request(RequestBody::CountArtObjects {
            filter: to_optional_string(filter),
        })
    }

    #[func]
    fn layout(&mut self, walls_json_path: GString, filter: String, dense: bool) -> u32 {
        let walls_json = FileAccess::get_file_as_string(walls_json_path).to_string();
        self.send_request(RequestBody::Layout {
            walls_json,
            filter: to_optional_string(filter),
            dense,
        })
    }

    #[func]
    fn migrate(&mut self) -> u32 {
        self.send_request(RequestBody::Migrate)
    }

    #[func]
    fn import_non_positive_layout(&mut self, json_content: String) -> u32 {
        self.send_request(RequestBody::ImportNonPositiveLayout { json_content })
    }

    #[func]
    fn export_non_positive_layout(&mut self) -> u32 {
        self.send_request(RequestBody::ExportNonPositiveLayout)
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
    fn poll(&mut self) -> Option<Gd<GalleryResponse>> {
        if !self.queued_requests.is_empty() {
            if let Some(peer) = self.get_multiplayer_client() {
                if peer.get_connection_status() == ConnectionStatus::CONNECTED {
                    let queued_requests = std::mem::take(&mut self.queued_requests);
                    for (request_id, body) in queued_requests {
                        // TODO: Consider using postcard or something else that's more space-efficient.
                        let Ok(serialized_request_body) = serde_json::to_string(&body) else {
                            godot_error!("Unable to serialize request body: {:?}", body);
                            continue;
                        };
                        //godot_print!("Proxying request to server: {}", serialized_request_body);
                        self.base_mut().rpc_id(
                            1, // Send to server only, its ID is always 1.
                            "proxy_request_to_server_internal".into(),
                            &[
                                request_id.to_variant(),
                                serialized_request_body.into_godot().to_variant(),
                            ],
                        );
                    }
                }
            }
        }

        let message: MessageFromWorker =
            if let Some((request_id, body)) = self.queued_responses.pop_front() {
                MessageFromWorker::Response(Response {
                    peer_id: None,
                    request_id,
                    body,
                })
            } else {
                let Some(connection) = &self.connection else {
                    return None;
                };
                match connection.from_worker_rx.try_recv() {
                    Ok(message) => message,
                    Err(TryRecvError::Empty) => {
                        return None;
                    }
                    Err(TryRecvError::Disconnected) => {
                        godot_print!("from_worker_rx.recv() failed, thread died!");
                        self.connection = None;
                        return None;
                    }
                }
            };

        match message {
            MessageFromWorker::Done => {
                godot_print!("Gallery worker thread exited cleanly.");
                self.connection = None;
                None
            }
            MessageFromWorker::FatalError(message) => {
                godot_error!("Gallery worker thread encountered fatal error: {message}");
                self.fatal_error = Some(message);
                self.connection = None;
                None
            }
            MessageFromWorker::Response(response) => {
                let request_id = response.request_id;
                if let Some(peer_id) = response.peer_id {
                    let Ok(serialized_response) = serde_json::to_string(&response.body) else {
                        godot_error!("Unable to serialize response: {:?}", response);
                        return None;
                    };
                    self.base_mut().rpc_id(
                        peer_id as i64, // TODO: Why do some Godot APIs think this is i32, while others think it's i64?
                        "proxy_response_from_server_internal".into(),
                        &[
                            request_id.to_variant(),
                            serialized_response.into_godot().to_variant(),
                        ],
                    );
                    None
                } else {
                    match response.body {
                        ResponseBody::Empty => Some(Gd::from_object(GalleryResponse {
                            request_id,
                            response: InnerGalleryResponse::default(),
                        })),
                        ResponseBody::Integer(int) => Some(Gd::from_object(GalleryResponse {
                            request_id,
                            response: InnerGalleryResponse::Variant(int.to_variant()),
                        })),
                        ResponseBody::String(string) => Some(Gd::from_object(GalleryResponse {
                            request_id,
                            response: InnerGalleryResponse::Variant(string.to_variant()),
                        })),
                        ResponseBody::ArtObjectsForGalleryWall(objects) => {
                            Some(Gd::from_object(GalleryResponse {
                                request_id,
                                response: InnerGalleryResponse::ArtObjects(Array::from_iter(
                                    objects.into_iter().map(|object| {
                                        Gd::from_object(ArtObject {
                                            object_id: object.object_id.to_raw_i64(),
                                            title: object.title.into_godot(),
                                            date: object.date.into_godot(),
                                            width: object.width,
                                            height: object.height,
                                            x: object.x,
                                            y: object.y,
                                            artist: object.artist.into_godot(),
                                            medium: object.medium.into_godot(),
                                            collection: object.collection.into_godot(),
                                        })
                                    }),
                                )),
                            }))
                        }
                        ResponseBody::Image(image_path) => {
                            // Note that ideally we'd load this image in a separate thread, so we wouldn't
                            // potentially cause frame skips. But there are a few things in the way, at
                            // least for doing this in Rust:
                            //
                            //   * gdext has a Cargo feature called `experimental-threads` which provides
                            //     experimental support for multithreading, but the underlying safety
                            //     rules are still being worked out as of 2024-07-25, as such there may
                            //     be unsoundness and an unstable API.
                            //
                            //     Even then, though, it looks like `Image` is !Send, so we can't simply
                            //     load the image in a separate thread and send it over a channel.
                            //
                            //   * According to the Godot docs on Multithreading [1]:
                            //
                            //     > You should avoid calling functions involving direct interaction with
                            //     > the GPU on other threads, such as creating new textures or modifying
                            //     > and retrieving image data, these operations can lead to performance
                            //     > stalls because they require synchronization with the RenderingServer,
                            //     > as data needs to be transmitted to or updated on the GPU.
                            //
                            //     Yet another part of the same document seems to contradict this:
                            //
                            //     > ... handling references on multiple threads is supported, hence
                            //     > loading resources on a thread is as well - scenes, textures, meshes,
                            //     > etc - can be loaded and manipulated on a thread and then added to the
                            //     > active scene on the main thread.
                            //
                            //     It's also unclear whether the `Image` resource is actually loaded
                            //     directly into the GPU, vs. loaded into memory. If it's just loaded into
                            //     memory, we could at least load images into memory from a different
                            //     thread, while bringing them into the GPU on the main thread.
                            //
                            //     [1] https://docs.godotengine.org/en/stable/tutorials/performance/thread_safe_apis.html#rendering
                            //
                            // Regardless, for now we're just going to pass the image path to Godot, and it
                            // can do whatever it wants with it.
                            let variant: Variant = match image_path {
                                Some(image_path) => {
                                    Variant::from(image_path.to_string_lossy().into_godot())
                                }
                                None => Variant::nil(),
                            };
                            Some(Gd::from_object(GalleryResponse {
                                request_id,
                                response: InnerGalleryResponse::Variant(variant),
                            }))
                        }
                    }
                }
            }
        }
    }
}

impl Drop for GalleryClient {
    fn drop(&mut self) {
        if let Some(connection) = self.connection.take() {
            connection.disconnect();
        }
    }
}

/// Convert a Godot URL like `user://blah.json` to an absolute path.
fn globalize_path(godot_url: GString) -> PathBuf {
    normalize_path(
        ProjectSettings::singleton()
            .globalize_path(godot_url)
            .to_string(),
    )
}

fn to_optional_string(value: String) -> Option<String> {
    if value.len() > 0 {
        Some(value)
    } else {
        None
    }
}
