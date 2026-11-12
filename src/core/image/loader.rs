use std::fmt;
use std::path::Path;

use image::DynamicImage;

#[derive(Debug)]
pub enum LoadImageError {
    Io(std::io::Error),
    Decode(image::ImageError),
}

impl fmt::Display for LoadImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadImageError::Io(e) => write!(f, "IO error: {}", e),
            LoadImageError::Decode(e) => write!(f, "Image decode error: {}", e),
        }
    }
}

impl std::error::Error for LoadImageError {}

impl From<std::io::Error> for LoadImageError {
    fn from(e: std::io::Error) -> Self {
        LoadImageError::Io(e)
    }
}

impl From<image::ImageError> for LoadImageError {
    fn from(e: image::ImageError) -> Self {
        LoadImageError::Decode(e)
    }
}

pub struct LoadedImage {
    pub dynamic: DynamicImage,
    pub width: u32,
    pub height: u32,
}

impl LoadedImage {
    pub fn aspect_ratio(&self) -> f64 {
        if self.height == 0 {
            1.0
        } else {
            self.width as f64 / self.height as f64
        }
    }
}

pub fn load_image(path: &Path) -> Result<LoadedImage, LoadImageError> {
    let dynamic = image::open(path)?;
    let width = dynamic.width();
    let height = dynamic.height();
    Ok(LoadedImage {
        dynamic,
        width,
        height,
    })
}