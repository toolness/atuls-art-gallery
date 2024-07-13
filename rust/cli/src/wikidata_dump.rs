use anyhow::Result;
use bzip2::bufread::MultiBzDecoder;
use nom::{bytes::complete::tag, character::complete::digit1, sequence::preceded, IResult};
use std::{
    io::{prelude::*, BufReader},
    path::PathBuf,
};

const BUFREADER_CAPACITY: usize = 1024 * 1024 * 8;

fn quick_parse_item_id(input: &str) -> IResult<&str, &str> {
    preceded(tag(r#"{"type":"item","id":"Q"#), digit1)(input)
}

pub fn load_wikidata_dump(dumpfile_path: PathBuf) -> Result<()> {
    let file = std::fs::File::open(dumpfile_path)?;
    let reader = BufReader::with_capacity(BUFREADER_CAPACITY, file);
    let multi_decompressor = MultiBzDecoder::new(reader);
    let mut buf_reader = BufReader::with_capacity(BUFREADER_CAPACITY, multi_decompressor);

    let mut contents = String::new();
    let mut total = 0;
    loop {
        contents.clear();
        let bytes_read = buf_reader.read_line(&mut contents)?;
        if bytes_read == 0 {
            return Ok(());
        }
        let Some((_remaining, qid)) = quick_parse_item_id(contents.as_str()).ok() else {
            continue;
        };
        if qid.len() == 0 {
            continue;
        }
        let _qid_number: u64 = qid.parse().unwrap();
        total += 1;
        if total % 1000 == 0 {
            println!("{total} QIDs parsed.");
        }
    }
}
