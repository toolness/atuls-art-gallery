use anyhow::Result;
use byteorder::LittleEndian;
use flate2::bufread::GzDecoder;
use gallery::wikidata::{try_to_parse_qid_from_wikidata_url, WikidataEntity};
use nom::{bytes::complete::tag, character::complete::digit1, sequence::preceded, IResult};
use serde::{de, Deserialize};
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{prelude::*, BufReader, BufWriter},
    path::PathBuf,
};
use zerocopy::{byteorder::U64, AsBytes, FromBytes, FromZeroes, Unaligned};

#[derive(Debug, Deserialize)]
struct WikidataCsvRecord {
    #[serde(rename = "item", deserialize_with = "deserialize_wikidata_entity_url")]
    pub qid: u64,
}

/// Parses a Q-identifier from a wikidata URL and returns it.
fn deserialize_wikidata_entity_url<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: de::Deserializer<'de>,
{
    let s: &str = de::Deserialize::deserialize(deserializer)?;
    match try_to_parse_qid_from_wikidata_url(s) {
        Some(qid) => Ok(qid),
        // TODO: `unknown_variant` is probably the wrong type of error to return.
        None => Err(de::Error::unknown_variant(s, &["Wikidata URL"])),
    }
}

const BUFREADER_CAPACITY: usize = 1024 * 1024 * 8;

/// Q-identifiers are *mostly* contiguous, this capacity will accommodate
/// the entire wikidata dump as of 2024-07-14.
const INDEX_FILE_CAPACITY: u64 = 300_000_000;

/// This quickly parses the item ID from a single line of a wikidata dump JSON blob,
/// without actually parsing any JSON. As can be seen from the implementation, it's
/// highly dependent on the specific serialization of wikidata, and will break if
/// it so much as introduces whitespace between JSON tokens or re-orders keys.
fn quick_parse_item_id(input: &str) -> IResult<&str, &str> {
    preceded(tag(r#"{"type":"item","id":"Q"#), digit1)(input)
}

#[derive(FromBytes, AsBytes, Unaligned, FromZeroes, Default, Debug)]
#[repr(C)]
struct IndexValue {
    /// Offset into the wikidata dump file of the gzip member containing a
    /// particular entity.
    gzip_member_offset: U64<LittleEndian>,
    /// Offset into the gzip member of the entity.
    offset_into_gzip_member: U64<LittleEndian>,
}

struct IndexFileReader {
    reader: BufReader<File>,
}

impl IndexFileReader {
    fn new(path: PathBuf) -> Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        Ok(Self { reader })
    }

    fn read(&mut self, qid: u64) -> Result<Option<IndexValue>> {
        let value_size = std::mem::size_of::<IndexValue>();
        let file_pos = qid * value_size as u64;
        self.reader.seek(std::io::SeekFrom::Start(file_pos))?;
        let mut buf: Vec<u8> = vec![0; value_size];
        let bytes_read = self.reader.read(&mut buf)?;
        if bytes_read != value_size {
            return Ok(None);
        }
        Ok(IndexValue::read_from(&buf))
    }
}

/// This struct encapsulates writing an index mapping wikidata Q-identifiers
/// to their locations in a compressed wikidata dump file.
///
/// It is stored as a simple vector, where the location of an index is just
/// the nth record into the vector, and `n` is the Q-identifier of the wikidata
/// entity.
///
/// This takes advantage of the fact that the wikidata Q-identifier space is
/// nearly contiguous, relieving us of the need to build a BTree or use more
/// sophisticated data structures.
///
/// Note that I originally used the `sled` crate for this, but insertion time
/// increased significantly as the number of indexed keys expanded into the
/// hundreds of millions. Loading the index also took several seconds. In
/// contrast, this simpler vector-based approach has constant-time
/// insertion and retrieval and is also smaller than the equivalent `sled`
/// index.
struct IndexFileWriter {
    writer: BufWriter<File>,
}

impl IndexFileWriter {
    fn new(path: PathBuf) -> Result<Self> {
        let file = OpenOptions::new().write(true).create(true).open(path)?;
        let file_size = file.metadata().unwrap().len();
        let default_value = IndexValue::default();
        let value_size = default_value.as_bytes().len() as u64;
        let capacity_in_bytes = INDEX_FILE_CAPACITY * value_size;
        let mut writer = BufWriter::new(file);
        if file_size < capacity_in_bytes {
            writer.seek(std::io::SeekFrom::End(0))?;
            let records_to_write = INDEX_FILE_CAPACITY - (file_size / value_size);
            println!(
                "Expanding index file by {} records (record size is {value_size} bytes).",
                records_to_write
            );
            for _ in 0..records_to_write {
                writer.write(&default_value.as_bytes())?;
            }
        }
        Ok(Self { writer })
    }

    fn flush(&mut self) -> Result<()> {
        self.writer.flush()?;
        Ok(())
    }

    fn write(&mut self, qid: u64, value: IndexValue) -> Result<()> {
        if qid > INDEX_FILE_CAPACITY {
            println!("Warning: INDEX_FILE_CAPACITY={INDEX_FILE_CAPACITY} but qid={qid}.")
        }
        let bytes = value.as_bytes();
        let curr_pos = self.writer.stream_position().unwrap();
        let file_pos = qid * bytes.len() as u64;
        if file_pos != curr_pos {
            self.writer.seek(std::io::SeekFrom::Start(file_pos))?;
            assert_eq!(self.writer.stream_position().unwrap(), file_pos);
        }
        self.writer.write(&bytes)?;
        Ok(())
    }
}

fn index_path_for_dumpfile(dumpfile_path: &PathBuf) -> PathBuf {
    dumpfile_path.with_extension("vecindex")
}

pub fn query_wikidata_dump(
    dumpfile_path: PathBuf,
    mut qids: Vec<u64>,
    csv: Option<String>,
) -> Result<()> {
    if let Some(csv) = csv {
        let reader = BufReader::new(File::open(csv)?);
        let rdr = csv::Reader::from_reader(reader);
        for result in rdr.into_deserialize::<WikidataCsvRecord>() {
            let csv_record: WikidataCsvRecord = result?;
            qids.push(csv_record.qid);
        }
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
