use std::{
    path::PathBuf,
    sync::mpsc::{Receiver, Sender},
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
    println!("work_thread waiting for command.");
    // TODO: We should probably either keep an internal queue of commands or
    // have a separate sync thingy that detects when the End command was sent,
    // otherwise we could spend forever joining the thread if there's a ton
    // of network requests queued up before the End command was sent.
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
