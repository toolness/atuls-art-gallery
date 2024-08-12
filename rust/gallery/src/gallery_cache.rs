use anyhow::{anyhow, Result};
use std::{
    fs::{create_dir_all, File},
    path::PathBuf,
    time::Duration,
};
use ureq::{Agent, AgentBuilder, Response};

const TIMEOUT_SECS: u64 = 10;

const MAX_FILE_SIZE: u64 = 10_000_000;

#[derive(Debug, PartialEq)]
pub enum CacheResult {
    NewlyCached,
    AlreadyCached,
}

pub struct GalleryCache {
    cache_dir: PathBuf,
    agent: Agent,
}

impl GalleryCache {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            agent: AgentBuilder::new()
                .timeout(Duration::from_secs(TIMEOUT_SECS))
                .build(),
        }
    }

    pub fn cache_dir(&self) -> &PathBuf {
        &self.cache_dir
    }

    pub fn get_cached_path<T: AsRef<str>>(&self, relative_pathname: T) -> PathBuf {
        let mut result = self.cache_dir.clone();
        for path_part in relative_pathname.as_ref().split("/") {
            result.push(path_part);
        }
        result
    }

    pub fn cache_binary_url<T: AsRef<str>, U: AsRef<str>>(
        &self,
        url: T,
        filename: U,
    ) -> Result<CacheResult> {
        let cached_path = self.get_cached_path(filename);
        if cached_path.exists() {
            return Ok(CacheResult::AlreadyCached);
        }
        ensure_parent_dir(&cached_path)?;
        println!("Caching {} -> {}...", url.as_ref(), cached_path.display());
        let response = self.agent.get(url.as_ref()).call()?;
        validate_response(&response)?;
        let mut response_body = response.into_reader();
        let mut outfile = File::create(cached_path.clone())?;
        // TODO: Ideally we should prevent the file from growing too large, since the
        // response may not have had a content-length header.
        match std::io::copy(&mut response_body, &mut outfile) {
            Ok(_) => Ok(CacheResult::NewlyCached),
            Err(err) => {
                // Note: I haven't actually tested this manually, hopefully it works!
                drop(outfile);
                if cached_path.exists() {
                    let _ = std::fs::remove_file(cached_path);
                }
                Err(err.into())
            }
        }
    }

    pub fn cache_json_url<T: AsRef<str>, U: AsRef<str>>(
        &self,
        url: T,
        filename: U,
    ) -> Result<CacheResult> {
        let cached_path = self.get_cached_path(filename);
        if cached_path.exists() {
            return Ok(CacheResult::AlreadyCached);
        }
        ensure_parent_dir(&cached_path)?;
        println!("Caching {} -> {}...", url.as_ref(), cached_path.display());
        let response = self.agent.get(url.as_ref()).call()?;
        validate_response(&response)?;
        if response.content_type() != "application/json" {
            return Err(anyhow!("Content type is {}", response.content_type()));
        }
        // TODO: Ideally we should prevent the response from growing too large, since the
        // response may not have had a content-length header.
        let response_body = response.into_string()?;
        let json_body: serde_json::Value = serde_json::from_str(response_body.as_ref())?;
        let pretty_printed = serde_json::to_string_pretty(&json_body)?;

        std::fs::write(cached_path, pretty_printed)?;

        Ok(CacheResult::NewlyCached)
    }

    pub fn load_cached_string<T: AsRef<str>>(&self, filename: T) -> Result<String> {
        Ok(std::fs::read_to_string(self.get_cached_path(filename))?)
    }
}

fn validate_response(response: &Response) -> Result<()> {
    if response.status() != 200 {
        return Err(anyhow!("Got HTTP {}", response.status()));
    }
    // Annoyingly, the Met API doesn't serve a content-length header, so we can't
    // parse them, hence this is optional.
    if let Some(size) = response.header("Content-Length") {
        let Ok(size) = size.parse::<u64>() else {
            return Err(anyhow!("Unable to parse content-length: {size:?}"));
        };
        if size > MAX_FILE_SIZE {
            return Err(anyhow!("Response is too large ({size} bytes)"));
        }
    };
    Ok(())
}

pub fn ensure_parent_dir(cached_path: &PathBuf) -> Result<()> {
    if let Some(parent_dir) = cached_path.parent() {
        create_dir_all(parent_dir)?;
    }
    Ok(())
}
