use anyhow::Result;
use byteorder::LittleEndian;
use flate2::bufread::GzDecoder;
use nom::{bytes::complete::tag, character::complete::digit1, sequence::preceded, IResult};
use std::{
    fs::{File, OpenOptions},
    io::{prelude::*, BufReader, BufWriter},
    path::PathBuf,
};
use zerocopy::{byteorder::U64, AsBytes, FromBytes, FromZeroes, Unaligned};

const BUFREADER_CAPACITY: usize = 1024 * 1024 * 8;

/// Q-identifiers are *mostly* contiguous, this capacity will accommodate
/// the entire wikidata dump as of 2024-07-14.
const INDEX_FILE_CAPACITY: u64 = 300_000_000;

fn quick_parse_item_id(input: &str) -> IResult<&str, &str> {
    preceded(tag(r#"{"type":"item","id":"Q"#), digit1)(input)
}

#[derive(FromBytes, AsBytes, Unaligned, FromZeroes, Default)]
#[repr(C)]
struct Value {
    gzip_member_offset: U64<LittleEndian>,
    offset_into_gzip_member: U64<LittleEndian>,
}

struct IndexFile {
    writer: BufWriter<File>,
}

impl IndexFile {
    fn new(path: PathBuf) -> Result<Self> {
        let file = OpenOptions::new().write(true).create(true).open(path)?;
        let file_size = file.metadata().unwrap().len();
        let default_value = Value::default();
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

    fn write(&mut self, qid: u64, value: Value) -> Result<()> {
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

pub fn load_wikidata_dump(dumpfile_path: PathBuf, seek_from: Option<u64>) -> Result<()> {
    let index_path = dumpfile_path.with_extension("vecindex");
    println!("Writing index to {}.", index_path.display());
    println!("Parsing QIDs from {}...", dumpfile_path.display());
    let now = std::time::SystemTime::now();
    let mut index_db = IndexFile::new(index_path)?;
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
    index_db: &mut IndexFile,
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
        index_db.write(qid_str.parse().unwrap(), value)?;
        total += 1;
    }
    index_db.flush()?;
    Ok(total)
}
