use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::mpsc::{channel, Receiver, SendError, Sender, TryRecvError},
    thread::{self, JoinHandle},
};

use godot::{
    engine::{
        multiplayer_api::RpcMode,
        multiplayer_peer::{ConnectionStatus, TransferMode},
        Image, MultiplayerPeer, OfflineMultiplayerPeer, ProjectSettings,
    },
    prelude::*,
};

use crate::{
    met_object::MetObject,
    met_response::{InnerMetResponse, MetResponse},
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
    fn connect(root_dir: PathBuf) -> Self {
        godot_print!("Root dir is {}.", root_dir.display());
        let (to_worker_tx, to_worker_rx) = channel::<MessageToWorker>();
        let (from_worker_tx, from_worker_rx) = channel::<MessageFromWorker>();
        godot_print!("Spawning work thread.");
        let handler = thread::spawn(move || {
            if let Err(err) = work_thread(root_dir.clone(), to_worker_rx, from_worker_tx.clone()) {
                eprintln!("Thread errored: {err:?}");
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
            godot_print!("Error sending end signal to thread: {:?}", err);
            return;
        }
        match self.handler.join() {
            Ok(_) => {
                godot_print!("Joined thread.");
            }
            Err(err) => {
                godot_print!("Error joining thread: {:?}", err);
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
    fn connect(&mut self, root_dir: GString) {
        let globalized_root_dir: PathBuf = normalize_path(
            ProjectSettings::singleton()
                .globalize_path(root_dir)
                .to_string(),
        );
        self.connection = Some(Connection::connect(globalized_root_dir));
    }

    fn handle_send_error(&mut self, err: SendError<MessageToWorker>) {
        if self.connection.is_some() {
            godot_error!("sending message to worker failed: {:?}", err);
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
    fn move_met_object(
        &mut self,
        met_object_id: u64,
        gallery_id: i64,
        wall_id: String,
        x: f64,
        y: f64,
    ) {
        self.send_request(RequestBody::MoveMetObject {
            met_object_id,
            gallery_id,
            wall_id,
            x,
            y,
        });
    }

    #[func]
    fn get_met_objects_for_gallery_wall(&mut self, gallery_id: i64, wall_id: String) -> u32 {
        self.send_request(RequestBody::GetMetObjectsForGalleryWall {
            gallery_id,
            wall_id,
        })
    }

    #[func]
    fn fetch_small_image(&mut self, object_id: u64) -> u32 {
        self.send_request(RequestBody::FetchSmallImage { object_id })
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
                        //godot_print!("Proxying request to server: {}", serialized_request);
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
                godot_print!("Work thread exited cleanly.");
                self.connection = None;
                None
            }
            MessageFromWorker::FatalError(message) => {
                godot_error!("Work thread encountered fatal error: {message}");
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
                        ResponseBody::MetObjectsForGalleryWall(objects) => {
                            Some(Gd::from_object(MetResponse {
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
                            }))
                        }
                        ResponseBody::Image(small_image) => {
                            let image = small_image
                                .map(|small_image| {
                                    Image::load_from_file(GString::from(
                                        small_image.to_string_lossy().into_owned(),
                                    ))
                                })
                                .flatten();
                            Some(Gd::from_object(MetResponse {
                                request_id,
                                response: InnerMetResponse::Image(image),
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
        godot_print!("drop GalleryClient!");
        if let Some(connection) = self.connection.take() {
            connection.disconnect();
        }
    }
}
