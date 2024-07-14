use anyhow::Result;
use byteorder::{BigEndian, LittleEndian};
use flate2::bufread::GzDecoder;
use nom::{bytes::complete::tag, character::complete::digit1, sequence::preceded, IResult};
use sled::Batch;
use std::{
    io::{prelude::*, BufReader},
    path::PathBuf,
};
use zerocopy::{byteorder::U64, AsBytes, FromBytes, FromZeroes, Unaligned};

const BUFREADER_CAPACITY: usize = 1024 * 1024 * 8;

fn quick_parse_item_id(input: &str) -> IResult<&str, &str> {
    preceded(tag(r#"{"type":"item","id":"Q"#), digit1)(input)
}

#[derive(FromBytes, AsBytes, Unaligned, FromZeroes)]
#[repr(C)]
struct Key {
    qid: U64<BigEndian>,
}

#[derive(FromBytes, AsBytes, Unaligned, FromZeroes)]
#[repr(C)]
struct Value {
    gzip_member_offset: U64<LittleEndian>,
    offset_into_gzip_member: U64<LittleEndian>,
}

pub fn load_wikidata_dump(dumpfile_path: PathBuf, seek_from: Option<u64>) -> Result<()> {
    let index_path = dumpfile_path.with_extension("index");
    println!("Writing index to {}.", index_path.display());
    println!("Parsing QIDs from {}...", dumpfile_path.display());
    let index_db = sled::open(index_path)?;
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
        let bytes_read = gz.read_to_end(&mut buf)?;
        if bytes_read == 0 {
            break;
        }
        if buf[0] == b'{' && buf[buf.len() - 1] == b'}' {
            // Unfortunately, the GZip header doesn't seem to have an 'extra' block defined on it,
            // which means there's definitely no metadata that will tell us the size of the block
            // beforehand. If there was, we could have done all this decompression in parallel.
            println!("Read {bytes_read} bytes of JSON at position {gzip_member_offset}.");
            let new_qids = parse_and_upsert_qids(&buf, &index_db, gzip_member_offset);
            total += new_qids;
            println!(
                "{:.2}% done, {new_qids} QIDs parsed from gzip member ({total} total).",
                (gzip_member_offset as f64) / (total_len as f64) * 100.0
            );
        }
        let mut underlying_reader = gz.into_inner();
        gzip_member_offset = underlying_reader.stream_position().unwrap();
        gz = GzDecoder::new(underlying_reader);
    }
    println!("Done, parsed {total} QIDs.");
    Ok(())
}

fn parse_and_upsert_qids(buf: &Vec<u8>, index_db: &sled::Db, gzip_member_offset: u64) -> usize {
    let gzip_member_offset = U64::new(gzip_member_offset);
    let mut buf_reader = BufReader::new(buf.as_slice());
    let mut total = 0;
    let mut contents = String::new();
    let mut offset_into_gzip_member: u64 = 0;
    let mut batch = Batch::default();
    loop {
        contents.clear();
        let bytes_read = buf_reader
            .read_line(&mut contents)
            .expect("error reading line from buffer");
        if bytes_read == 0 {
            break;
        }
        let value = Value {
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
        let key = Key {
            qid: U64::new(qid_str.parse().unwrap()),
        };
        batch.insert(key.as_bytes(), value.as_bytes());
        total += 1;
    }

    index_db
        .apply_batch(batch)
        .expect("writing to index db failed");
    index_db.flush().expect("flushing index db failed");
    total
}
