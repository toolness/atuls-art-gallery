use std::fmt::Display;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Copy, Clone)]
pub enum ImageSize {
    Small,
    Large,
}

impl Display for ImageSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImageSize::Small => write!(f, "small"),
            ImageSize::Large => write!(f, "large"),
        }
    }
}
