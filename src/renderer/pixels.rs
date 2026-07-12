use std::fmt::{Debug, Formatter, Result as FmtResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point<T> {
    pub x: T,
    pub y: T,
}

impl<T> Point<T> {
    #[inline]
    pub const fn new(x: T, y: T) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size<T> {
    pub width: T,
    pub height: T,
}

impl<T> Size<T> {
    #[inline]
    pub const fn new(width: T, height: T) -> Self {
        Self { width, height }
    }
}

// pixels uncompressed rgba image
// this will act as a container that will replace our previous image::RgbaImage
pub struct RawCardImage {
    pub size: Size<u32>,
    pub pixels: Box<[u8]>,
}

impl Debug for RawCardImage {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("RawCardImage")
            .field("width", &self.size.width)
            .field("height", &self.size.height)
            .field("pixels_len", &self.pixels.len())
            .finish()
    }
}
