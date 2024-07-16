use anyhow::Result;
use byteorder::LittleEndian;
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{prelude::*, BufReader, BufWriter},
    path::PathBuf,
};
use zerocopy::{byteorder::U64, AsBytes, FromBytes, FromZeroes, Unaligned};

/// Q-identifiers are *mostly* contiguous, this capacity will accommodate
/// the entire wikidata dump as of 2024-07-14.
const INDEX_FILE_CAPACITY: u64 = 300_000_000;

#[derive(FromBytes, AsBytes, Unaligned, FromZeroes, Default, Debug)]
#[repr(C)]
pub struct IndexValue {
    /// Offset into the wikidata dump file of the gzip member containing a
    /// particular entity.
    pub gzip_member_offset: U64<LittleEndian>,
    /// Offset into the gzip member of the entity.
    pub offset_into_gzip_member: U64<LittleEndian>,
}

pub struct IndexFileReader {
    reader: BufReader<File>,
}

impl IndexFileReader {
    pub fn new(path: PathBuf) -> Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        Ok(Self { reader })
    }

    pub fn read(&mut self, qid: u64) -> Result<Option<IndexValue>> {
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

pub struct QidIndexFileMapping {
    /// Mapping from gzip members, identified by their byte offset, to details about the location of individual
    /// entities within each gzip member. This makes it easy for us to decompress each gzip member only once
    /// to retrieve all the data we need from it.
    pub qids_by_gzip_members: HashMap<u64, Vec<QidGzipMemberInfo>>,
    pub total_qids: usize,
}

pub struct QidGzipMemberInfo {
    pub qid: u64,
    pub offset_into_gzip_member: u64,
}

pub fn get_qid_index_file_mapping(
    reader: &mut IndexFileReader,
    qids: Vec<u64>,
) -> Result<QidIndexFileMapping> {
    let mut qids_by_gzip_members = HashMap::<u64, Vec<QidGzipMemberInfo>>::new();
    let mut total_qids = 0;
    for qid in qids {
        let value = reader.read(qid)?.unwrap_or_default();
        let gzip_member = value.gzip_member_offset.get();
        // Note that the very first gzip member is just an opening square bracket, i.e. no QID data,
        // so a value of 0 can _only_ mean we never populated the value when indexing.
        if gzip_member != 0 {
            total_qids += 1;
            let entry = qids_by_gzip_members.entry(gzip_member).or_default();
            let offset_into_gzip_member = value.offset_into_gzip_member.get();
            entry.push(QidGzipMemberInfo {
                qid,
                offset_into_gzip_member,
            });
        } else {
            println!("Warning: Q{qid} not found.");
        }
    }

    Ok(QidIndexFileMapping {
        qids_by_gzip_members,
        total_qids,
    })
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
pub struct IndexFileWriter {
    writer: BufWriter<File>,
}

impl IndexFileWriter {
    pub fn new(path: PathBuf) -> Result<Self> {
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

    pub fn flush(&mut self) -> Result<()> {
        self.writer.flush()?;
        Ok(())
    }

    pub fn write(&mut self, qid: u64, value: IndexValue) -> Result<()> {
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

pub fn index_path_for_dumpfile(dumpfile_path: &PathBuf) -> PathBuf {
    dumpfile_path.with_extension("vecindex")
}
