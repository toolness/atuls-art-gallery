use anyhow::Result;
use flate2::bufread::GzDecoder;
use nom::{bytes::complete::tag, character::complete::digit1, sequence::preceded, IResult};
use std::{
    io::{prelude::*, BufReader},
    path::PathBuf,
};

const BUFREADER_CAPACITY: usize = 1024 * 1024 * 8;

fn quick_parse_item_id(input: &str) -> IResult<&str, &str> {
    preceded(tag(r#"{"type":"item","id":"Q"#), digit1)(input)
}

pub fn load_wikidata_dump(dumpfile_path: PathBuf, seek_from: Option<u64>) -> Result<()> {
    println!("Parsing QIDs from {}...", dumpfile_path.display());
    let file = std::fs::File::open(dumpfile_path)?;
    let mut reader = BufReader::with_capacity(BUFREADER_CAPACITY, file);
    if let Some(seek_from) = seek_from {
        reader.seek(std::io::SeekFrom::Start(seek_from))?;
    }
    let mut gz = GzDecoder::new(reader);
    let mut buf: Vec<u8> = vec![];
    let mut latest_position: u64 = seek_from.unwrap_or(0);
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
            println!("Read {bytes_read} bytes of JSON at position {latest_position}.");
            let new_qids = parse_qids(&buf);
            total += new_qids;
            println!("{new_qids} QIDs parsed from gzip member ({total} total).");
        }
        let mut underlying_reader = gz.into_inner();
        latest_position = underlying_reader.stream_position().unwrap();
        gz = GzDecoder::new(underlying_reader);
    }
    println!("Done, parsed {total} QIDs.");
    Ok(())
}

fn parse_qids(buf: &Vec<u8>) -> usize {
    let mut buf_reader = BufReader::new(buf.as_slice());
    let mut total = 0;
    let mut contents = String::new();
    loop {
        contents.clear();
        let bytes_read = buf_reader
            .read_line(&mut contents)
            .expect("error reading line from buffer");
        if bytes_read == 0 {
            break;
        }
        let Some((_remaining, qid)) = quick_parse_item_id(contents.as_str()).ok() else {
            continue;
        };
        if qid.len() == 0 {
            continue;
        }
        let _qid_number: u64 = qid.parse().unwrap();
        total += 1;
    }

    total
}
