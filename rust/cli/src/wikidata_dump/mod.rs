use anyhow::Result;
use flate2::bufread::GzDecoder;
use gallery::wikidata::WikidataEntity;
use index_file::{index_path_for_dumpfile, IndexFileReader, IndexFileWriter, IndexValue};
use nom::{bytes::complete::tag, character::complete::digit1, sequence::preceded, IResult};
use sparql_csv_export::parse_sparql_csv_export;
use std::{
    collections::HashMap,
    io::{prelude::*, BufReader},
    path::PathBuf,
};
use zerocopy::byteorder::U64;

mod index_file;
mod sparql_csv_export;

const BUFREADER_CAPACITY: usize = 1024 * 1024 * 8;

/// This quickly parses the item ID from a single line of a wikidata dump JSON blob,
/// without actually parsing any JSON. As can be seen from the implementation, it's
/// highly dependent on the specific serialization of wikidata, and will break if
/// it so much as introduces whitespace between JSON tokens or re-orders keys.
fn quick_parse_item_id(input: &str) -> IResult<&str, &str> {
    preceded(tag(r#"{"type":"item","id":"Q"#), digit1)(input)
}

pub fn query_wikidata_dump(
    dumpfile_path: PathBuf,
    mut qids: Vec<u64>,
    csv: Option<PathBuf>,
) -> Result<()> {
    if let Some(csv) = csv {
        parse_sparql_csv_export(csv, &mut qids)?;
    }
    let index_path = index_path_for_dumpfile(&dumpfile_path);
    let mut index_db = IndexFileReader::new(index_path)?;
    struct QidInfo {
        qid: u64,
        offset_into_gzip_member: u64,
    }
    let mut qids_by_gzip_members = HashMap::<u64, Vec<QidInfo>>::new();
    let mut total_qids = 0;
    for qid in qids {
        let value = index_db.read(qid)?.unwrap_or_default();
        let gzip_member = value.gzip_member_offset.get();
        // Note that the very first gzip member is just an opening square bracket, i.e. no QID data,
        // so a value of 0 can _only_ mean we never populated the value when indexing.
        if gzip_member != 0 {
            total_qids += 1;
            let entry = qids_by_gzip_members.entry(gzip_member).or_default();
            let offset_into_gzip_member = value.offset_into_gzip_member.get();
            entry.push(QidInfo {
                qid,
                offset_into_gzip_member,
            });
        } else {
            println!("Warning: Q{qid} not found.");
        }
    }
    println!(
        "Reading {total_qids} QIDs across {} gzipped members.",
        qids_by_gzip_members.len()
    );
    let file = std::fs::File::open(dumpfile_path)?;
    let mut archive_reader = BufReader::with_capacity(BUFREADER_CAPACITY, file);
    for (gzip_member_offset, entries) in qids_by_gzip_members {
        println!("Decompressing gzip member at offset {gzip_member_offset}.");
        archive_reader.seek(std::io::SeekFrom::Start(gzip_member_offset))?;
        let mut gz = GzDecoder::new(archive_reader);
        let mut buf: Vec<u8> = vec![];
        gz.read_to_end(&mut buf)?;
        for QidInfo {
            qid,
            offset_into_gzip_member,
        } in entries
        {
            let slice = &buf[offset_into_gzip_member as usize..];
            let mut gzip_member_reader = BufReader::new(slice);
            let mut string = String::new();
            gzip_member_reader.read_line(&mut string)?;
            let entity: WikidataEntity = serde_json::from_str(&string[0..string.len() - 2])?;
            println!(
                "Q{qid}: {} - {} ({})",
                entity.label().unwrap_or_default(),
                entity.description().unwrap_or_default(),
                if entity.p18_image().is_some() {
                    "has image"
                } else {
                    "no image"
                }
            );
        }
        archive_reader = gz.into_inner();
    }
    Ok(())
}

pub fn index_wikidata_dump(dumpfile_path: PathBuf, seek_from: Option<u64>) -> Result<()> {
    let index_path = index_path_for_dumpfile(&dumpfile_path);
    println!("Writing index to {}.", index_path.display());
    println!("Parsing QIDs from {}...", dumpfile_path.display());
    let now = std::time::SystemTime::now();
    let mut index_db = IndexFileWriter::new(index_path)?;
    println!(
        "Opened index db in {} ms.",
        now.elapsed().unwrap().as_millis()
    );
    let file = std::fs::File::open(dumpfile_path)?;
    let total_len = file.metadata().unwrap().len();
    let mut reader = BufReader::with_capacity(BUFREADER_CAPACITY, file);
    if let Some(seek_from) = seek_from {
        reader.seek(std::io::SeekFrom::Start(seek_from))?;
    }
    let mut gz = GzDecoder::new(reader);
    let mut buf: Vec<u8> = vec![];
    let mut gzip_member_offset: u64 = seek_from.unwrap_or(0);
    let mut total = 0;
    loop {
        buf.clear();
        let now = std::time::SystemTime::now();
        let bytes_read = gz.read_to_end(&mut buf)?;
        let elapsed = now.elapsed().unwrap();
        if bytes_read == 0 {
            break;
        }
        if buf[0] == b'{' && buf[buf.len() - 1] == b'}' {
            // Unfortunately, the GZip header doesn't seem to have an 'extra' block defined on it,
            // which means there's definitely no metadata that will tell us the size of the block
            // beforehand. If there was, we could have done all this decompression in parallel.
            println!(
                "Read {bytes_read} bytes of JSON at position {gzip_member_offset} in {} ms.",
                elapsed.as_millis()
            );
            let now = std::time::SystemTime::now();
            let new_qids = parse_and_upsert_qids(&buf, &mut index_db, gzip_member_offset)?;
            let elapsed = now.elapsed().unwrap();
            total += new_qids;
            println!(
                "{:.2}% done, {new_qids} QIDs parsed from gzip member ({total} total) in {} ms.",
                (gzip_member_offset as f64) / (total_len as f64) * 100.0,
                elapsed.as_millis()
            );
        }
        let mut underlying_reader = gz.into_inner();
        gzip_member_offset = underlying_reader.stream_position().unwrap();
        if gzip_member_offset == total_len {
            break;
        }
        gz = GzDecoder::new(underlying_reader);
    }
    println!("Done, parsed {total} QIDs.");
    Ok(())
}

fn parse_and_upsert_qids(
    buf: &Vec<u8>,
    index_db: &mut IndexFileWriter,
    gzip_member_offset: u64,
) -> Result<usize> {
    let gzip_member_offset = U64::new(gzip_member_offset);
    let mut buf_reader = BufReader::new(buf.as_slice());
    let mut total = 0;
    let mut contents = String::new();
    let mut offset_into_gzip_member: u64 = 0;
    loop {
        contents.clear();
        let bytes_read = buf_reader
            .read_line(&mut contents)
            .expect("error reading line from buffer");
        if bytes_read == 0 {
            break;
        }
        let value = IndexValue {
            gzip_member_offset,
            offset_into_gzip_member: U64::new(offset_into_gzip_member),
        };
        offset_into_gzip_member += bytes_read as u64;
        let Some((_remaining, qid_str)) = quick_parse_item_id(contents.as_str()).ok() else {
            continue;
        };
        if qid_str.len() == 0 {
            continue;
        }
        index_db.write(qid_str.parse().unwrap(), value)?;
        total += 1;
    }
    index_db.flush()?;
    Ok(total)
}
