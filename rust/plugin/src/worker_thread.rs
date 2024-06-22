use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::mpsc::{Receiver, RecvError, Sender, TryRecvError},
};

use anyhow::anyhow;
use anyhow::Result;
use gallery::{
    gallery_cache::GalleryCache,
    gallery_db::{GalleryDb, LayoutRecord},
    met_api::load_met_api_record,
};
use rusqlite::Connection;

pub enum ChannelCommand {
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

pub enum ChannelResponse {
    Done,
    FatalError(String),
    MetObjectsForGalleryWall(u32, Vec<SimplifiedRecord>),
    Image(u32, Option<PathBuf>),
}

#[derive(Debug)]
pub struct SimplifiedRecord {
    pub object_id: u64,
    pub title: String,
    pub date: String,
    pub width: f64,
    pub height: f64,
    pub x: f64,
    pub y: f64,
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

fn fill_queue(
    queue: &mut VecDeque<Result<ChannelCommand, RecvError>>,
    cmd_rx: &Receiver<ChannelCommand>,
) {
    // Note that if we receive an explicit 'End' command, we push an 'End' command
    // to the front of the stack, meaning we'll ignore any other commands that had
    // been queued up. This is intentional: if the user suddenly decides to quit,
    // we want to quit ASAP, effectively aborting all in-flight requests.
    if queue.len() == 0 {
        // We don't have anything in the queue, so wait until we do.
        match cmd_rx.recv() {
            Ok(ChannelCommand::End) => {
                queue.push_front(Ok(ChannelCommand::End));
                return;
            }
            Err(RecvError) => {
                queue.push_front(Err(RecvError));
                return;
            }
            Ok(command) => {
                queue.push_back(Ok(command));
            }
        }
    }
    // Now go through anything else in the channel, without blocking. This
    // allows us to see if the client has hung up or wants us to quit ASAP.
    loop {
        match cmd_rx.try_recv() {
            Err(TryRecvError::Empty) => {
                return;
            }
            Err(TryRecvError::Disconnected) => {
                queue.push_front(Err(RecvError));
                return;
            }
            Ok(ChannelCommand::End) => {
                queue.push_front(Ok(ChannelCommand::End));
                return;
            }
            Ok(command) => {
                queue.push_back(Ok(command));
            }
        }
    }
}

pub fn work_thread(
    root_dir: PathBuf,
    cmd_rx: Receiver<ChannelCommand>,
    response_tx: Sender<ChannelResponse>,
) -> Result<()> {
    let cache = GalleryCache::new(root_dir);
    let db_path = cache.get_cached_path("gallery.sqlite");
    // Check for existence, we don't want SQLite making a zero-byte DB file.
    if !db_path.exists() {
        return Err(anyhow!("DB does not exist: {}", db_path.display()));
    }
    let mut db = GalleryDb::new(Connection::open(db_path)?);
    let mut queue = VecDeque::new();
    let send_response = |response: ChannelResponse| {
        // Ignore result, `fill_queue()` will just give us a RecvError next if we're disconnected.
        if response_tx.send(response).is_err() {
            println!("work_thread unable to send response, other end hung up.");
        };
    };
    println!("work_thread waiting for command.");
    loop {
        fill_queue(&mut queue, &cmd_rx);
        match queue.pop_front().expect("queue should not be empty") {
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
                send_response(ChannelResponse::MetObjectsForGalleryWall(
                    request_id, records,
                ));
            }
            Ok(ChannelCommand::FetchSmallImage {
                request_id,
                object_id,
            }) => {
                //println!("work_thread received 'FetchSmallImage' command, request_id={request_id}, object_id={object_id}.");
                let small_image = fetch_small_image(&cache, object_id);
                send_response(ChannelResponse::Image(request_id, small_image));
            }
            Err(RecvError) => {
                println!("work_thread client hung up prematurely.");
                break;
            }
        }
    }

    // Ignoring result, there's not much we can do if this send fails.
    let _ = response_tx.send(ChannelResponse::Done);

    Ok(())
}
