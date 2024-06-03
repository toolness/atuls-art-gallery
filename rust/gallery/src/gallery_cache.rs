use anyhow::{anyhow, Result};
use std::{fs::File, path::PathBuf};

pub struct GalleryCache {
    cache_dir: PathBuf,
}

impl GalleryCache {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    pub fn cache_dir(&self) -> &PathBuf {
        &self.cache_dir
    }

    pub fn get_cached_path<T: AsRef<str>>(&self, filename: T) -> PathBuf {
        self.cache_dir.join(filename.as_ref())
    }

    pub fn cache_binary_url<T: AsRef<str>, U: AsRef<str>>(
        &self,
        url: T,
        filename: U,
    ) -> Result<()> {
        let filename_path = self.get_cached_path(filename);
        if filename_path.exists() {
            return Ok(());
        }
        println!("Caching {} -> {}...", url.as_ref(), filename_path.display());
        let response = ureq::get(url.as_ref()).call()?;
        if response.status() != 200 {
            return Err(anyhow!("Got HTTP {}", response.status()));
        }
        let mut response_body = response.into_reader();
        let mut outfile = File::create(filename_path)?;
        std::io::copy(&mut response_body, &mut outfile)?;
        Ok(())
    }

    pub fn cache_json_url<T: AsRef<str>, U: AsRef<str>>(&self, url: T, filename: U) -> Result<()> {
        let filename_path = self.get_cached_path(filename);
        if filename_path.exists() {
            return Ok(());
        }
        println!("Caching {} -> {}...", url.as_ref(), filename_path.display());
        let response = ureq::get(url.as_ref()).call()?;
        if response.status() != 200 {
            return Err(anyhow!("Got HTTP {}", response.status()));
        }
        if response.content_type() != "application/json" {
            return Err(anyhow!("Content type is {}", response.content_type()));
        }
        let response_body = response.into_string()?;
        let json_body: serde_json::Value = serde_json::from_str(response_body.as_ref())?;
        let pretty_printed = serde_json::to_string_pretty(&json_body)?;

        std::fs::write(filename_path, pretty_printed)?;

        Ok(())
    }

    pub fn load_cached_string<T: AsRef<str>>(&self, filename: T) -> Result<String> {
        Ok(std::fs::read_to_string(self.get_cached_path(filename))?)
    }
}
