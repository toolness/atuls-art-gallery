use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::mpsc::{Receiver, RecvError, Sender, TryRecvError},
};

use anyhow::anyhow;
use anyhow::Result;
use gallery::{
    art_object::ArtObjectId,
    gallery_cache::GalleryCache,
    gallery_db::{get_default_gallery_db_filename, ArtObjectQueryOptions, GalleryDb, LayoutRecord},
    gallery_db_migration::migrate_gallery_db,
    gallery_wall::GalleryWall,
    image::ImageSize,
    layout::layout,
    met_api::{load_met_api_record, migrate_met_api_cache},
    wikidata::{load_wikidata_image_info, WikidataImageInfo},
};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct Request {
    pub peer_id: Option<i32>,
    pub request_id: u32,
    pub body: RequestBody,
}

// We need to support serialization here to allow other godot clients
// to proxy requests to and from servers.
#[derive(Debug, Deserialize, Serialize)]
pub enum RequestBody {
    MoveArtObject {
        art_object_id: ArtObjectId,
        gallery_id: i64,
        wall_id: String,
        x: f64,
        y: f64,
    },
    GetArtObjectsForGalleryWall {
        gallery_id: i64,
        wall_id: String,
    },
    FetchImage {
        object_id: ArtObjectId,
        size: ImageSize,
    },
    Layout {
        walls_json: String,
        filter: Option<String>,
        dense: bool,
    },
    CountArtObjects {
        filter: Option<String>,
    },
    Migrate,
}

#[derive(Debug)]
pub struct Response {
    pub peer_id: Option<i32>,
    pub request_id: u32,
    pub body: ResponseBody,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum ResponseBody {
    ArtObjectsForGalleryWall(Vec<SimplifiedRecord>),
    Image(Option<PathBuf>),
    Empty,
    Integer(i64),
}

pub enum MessageToWorker {
    End,
    Request(Request),
}

impl RequestBody {
    // TODO: It'd be nice to have a completely separate enum for proxyable requests,
    // so we can get exhaustiveness checking.
    pub fn is_proxyable_to_server(&self) -> bool {
        // Right now we don't allow _any_ requests to be proxied to the server, as
        // everything is called directly by the server itself.
        false
    }
}

pub enum MessageFromWorker {
    Done,
    FatalError(String),
    Response(Response),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SimplifiedRecord {
    pub object_id: ArtObjectId,
    pub artist: String,
    pub medium: String,
    pub title: String,
    pub date: String,
    pub width: f64,
    pub height: f64,
    pub x: f64,
    pub y: f64,
    pub collection: String,
}

fn get_art_objects_for_gallery_wall(
    db: &mut GalleryDb,
    gallery_id: i64,
    wall_id: String,
) -> Result<Vec<SimplifiedRecord>> {
    let mut result = vec![];
    let objects = db.get_art_objects_for_gallery_wall(gallery_id, wall_id)?;
    for (object, (x, y)) in objects {
        result.push(SimplifiedRecord {
            object_id: object.object_id,
            title: object.title,
            date: object.object_date,
            width: object.width,
            height: object.height,
            artist: object.artist,
            medium: object.medium,
            collection: object.collection,
            x,
            y,
        });
    }
    Ok(result)
}

fn fetch_met_api_image(
    cache: &GalleryCache,
    met_object_id: i64,
    size: ImageSize,
) -> Option<PathBuf> {
    match load_met_api_record(&cache, met_object_id) {
        Ok(obj_record) => match obj_record.try_to_download_image(&cache, size) {
            Ok(Some((_width, _height, image))) => Some(cache.cache_dir().join(image)),
            Ok(None) => None,
            Err(err) => {
                eprintln!(
                    "Unable to download {size} image for met object ID {}: {:?}",
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

fn try_to_download_wikidata_image(
    db: &GalleryDb,
    cache: &GalleryCache,
    object_id: ArtObjectId,
    size: ImageSize,
) -> Result<Option<PathBuf>> {
    if let Some(record) = db.get_art_object(object_id)? {
        if let ArtObjectId::Wikidata(qid) = object_id {
            Ok(fetch_wikidata_image_from_qid_and_filename(
                cache,
                WikidataImageInfo {
                    qid,
                    image_filename: record.filename,
                },
                size,
            ))
        } else if let Some(qid) = record.fallback_wikidata_qid {
            Ok(fetch_wikidata_image_from_qid_only(&cache, qid, size))
        } else {
            Ok(None)
        }
    } else {
        println!("WARNING: Could not find {:?} in the database.", object_id);
        Ok(None)
    }
}

fn fetch_wikidata_image_from_qid_and_filename(
    cache: &GalleryCache,
    info: WikidataImageInfo,
    size: ImageSize,
) -> Option<PathBuf> {
    match info.try_to_download_image(&cache, size) {
        Ok(filename) => Some(cache.cache_dir().join(filename)),
        Err(err) => {
            eprintln!(
                "Unable to fetch wikidata image for Q{}: {:?}",
                info.qid, err
            );
            None
        }
    }
}

fn fetch_wikidata_image_from_qid_only(
    cache: &GalleryCache,
    qid: i64,
    size: ImageSize,
) -> Option<PathBuf> {
    match load_wikidata_image_info(&cache, qid) {
        Ok(Some(info)) => fetch_wikidata_image_from_qid_and_filename(cache, info, size),
        Ok(None) => {
            eprintln!("Wikidata has no image info for Q{qid}.");
            None
        }
        Err(err) => {
            eprintln!("Unable to fetch wikidata image info for Q{qid}: {:?}", err);
            None
        }
    }
}

fn fill_queue(
    queue: &mut VecDeque<Result<MessageToWorker, RecvError>>,
    to_worker_rx: &Receiver<MessageToWorker>,
) {
    // Note that if we receive an explicit 'End' message, we push an 'End' message
    // to the front of the stack, meaning we'll ignore any other messages that had
    // been queued up. This is intentional: if the user suddenly decides to quit,
    // we want to quit ASAP, effectively aborting all in-flight requests.
    if queue.len() == 0 {
        // We don't have anything in the queue, so wait until we do.
        match to_worker_rx.recv() {
            Ok(MessageToWorker::End) => {
                queue.push_front(Ok(MessageToWorker::End));
                return;
            }
            Err(RecvError) => {
                queue.push_front(Err(RecvError));
                return;
            }
            Ok(message) => {
                queue.push_back(Ok(message));
            }
        }
    }
    // Now go through anything else in the channel, without blocking. This
    // allows us to see if the client has hung up or wants us to quit ASAP.
    loop {
        match to_worker_rx.try_recv() {
            Err(TryRecvError::Empty) => {
                return;
            }
            Err(TryRecvError::Disconnected) => {
                queue.push_front(Err(RecvError));
                return;
            }
            Ok(MessageToWorker::End) => {
                queue.push_front(Ok(MessageToWorker::End));
                return;
            }
            Ok(message) => {
                queue.push_back(Ok(message));
            }
        }
    }
}

pub fn work_thread(
    root_dir: PathBuf,
    to_worker_rx: Receiver<MessageToWorker>,
    from_worker_tx: Sender<MessageFromWorker>,
) -> Result<()> {
    let cache = GalleryCache::new(root_dir);
    migrate_met_api_cache(&cache)?;
    let db_path = cache.get_cached_path(get_default_gallery_db_filename());
    // Check for existence, we don't want SQLite making a zero-byte DB file.
    if !db_path.exists() {
        return Err(anyhow!("DB does not exist: {}", db_path.display()));
    }
    let mut db = GalleryDb::new(Connection::open(db_path)?);
    let mut queue = VecDeque::new();
    let send_message = |response: MessageFromWorker| {
        // Ignore result, `fill_queue()` will just give us a RecvError next if we're disconnected.
        if from_worker_tx.send(response).is_err() {
            println!("work_thread unable to send response, other end hung up.");
        };
    };
    println!("work_thread waiting for message.");
    loop {
        fill_queue(&mut queue, &to_worker_rx);
        match queue.pop_front().expect("queue should not be empty") {
            Ok(MessageToWorker::End) => {
                println!("work_thread received 'end' message.");
                break;
            }
            Ok(MessageToWorker::Request(request)) => {
                let peer_id = request.peer_id;
                let request_id = request.request_id;
                let send_response = |body: ResponseBody| {
                    send_message(MessageFromWorker::Response(Response {
                        peer_id,
                        request_id,
                        body,
                    }));
                };
                //println!("work_thread received request: {:?}", request.body);
                match request.body {
                    RequestBody::Migrate => {
                        migrate_gallery_db(&cache)?;
                        send_response(ResponseBody::Empty);
                    }
                    RequestBody::Layout {
                        walls_json,
                        filter,
                        dense,
                    } => {
                        let walls: Vec<GalleryWall> = serde_json::from_str(&walls_json)?;
                        let options = ArtObjectQueryOptions {
                            filter,
                            ..Default::default()
                        };
                        let art_objects = db.get_all_art_objects_for_layout(&options)?;
                        let gallery_start_id = 1;
                        let except_art_object_ids =
                            db.get_art_object_ids_in_non_positive_galleries()?;
                        let (galleries_created, layout_records) = layout(
                            dense,
                            gallery_start_id,
                            &walls,
                            art_objects,
                            &except_art_object_ids,
                            false,
                        )?;
                        db.set_layout_records_in_positive_galleries(&layout_records)?;
                        println!(
                            "Created layout across {} galleries with {} walls each, dense={dense}.",
                            galleries_created,
                            walls.len()
                        );
                        send_response(ResponseBody::Empty);
                    }
                    RequestBody::CountArtObjects { filter } => {
                        let options = ArtObjectQueryOptions {
                            filter,
                            ..Default::default()
                        };
                        let count = db.count_art_objects(&options)?;
                        send_response(ResponseBody::Integer(count as i64))
                    }
                    RequestBody::MoveArtObject {
                        art_object_id,
                        gallery_id,
                        wall_id,
                        x,
                        y,
                    } => {
                        db.upsert_layout_records(&vec![LayoutRecord {
                            gallery_id,
                            wall_id,
                            art_object_id,
                            x,
                            y,
                        }])?;
                    }
                    RequestBody::GetArtObjectsForGalleryWall {
                        gallery_id,
                        wall_id,
                    } => {
                        let objects =
                            get_art_objects_for_gallery_wall(&mut db, gallery_id, wall_id)?;
                        send_response(ResponseBody::ArtObjectsForGalleryWall(objects));
                    }
                    RequestBody::FetchImage { object_id, size } => match object_id {
                        ArtObjectId::Met(met_object_id) => {
                            let mut image_path = fetch_met_api_image(&cache, met_object_id, size);
                            if image_path.is_none() {
                                image_path =
                                    try_to_download_wikidata_image(&db, &cache, object_id, size)?;
                            }
                            send_response(ResponseBody::Image(image_path));
                        }
                        ArtObjectId::Wikidata(_qid) => {
                            let image_path =
                                try_to_download_wikidata_image(&db, &cache, object_id, size)?;
                            send_response(ResponseBody::Image(image_path));
                        }
                    },
                }
            }
            Err(RecvError) => {
                println!("work_thread client hung up prematurely.");
                break;
            }
        }
    }

    // Ignoring result, there's not much we can do if this send fails.
    let _ = from_worker_tx.send(MessageFromWorker::Done);

    Ok(())
}
