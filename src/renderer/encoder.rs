use bytes::Bytes;
use webpx::{AlphaFilter, EncoderConfig, ImageHint, Preset, Unstoppable};

use crate::renderer::error::{RenderError, Result};

const WEBP_QUALITY: f32 = 80.0; // butteraugli, ssimu2 tested
const WEBP_SPEED: u8 = 0;
const WEBP_ALPHA_QUALITY: u8 = 85;
const WEBP_THREAD_LEVEL: u8 = 0;
const WEBP_SEGMENTS: u8 = 1;

/*
=============
encode_webp

encodes the raw canvas RGBA pixels into a compressed webp image for Blair.
https://docs.rs/webpx/latest/webpx/struct.EncoderConfig.html
=============
*/
pub(super) fn encode_webp(width: u32, height: u32, pixel_data: &[u8]) -> Result<Bytes> {
    let settings = EncoderConfig::new()
        .preset(Preset::Picture)
        .hint(ImageHint::Picture)
        .quality(WEBP_QUALITY)
        .method(WEBP_SPEED)
        .thread_level(WEBP_THREAD_LEVEL)
        .alpha_compression(true)
        .alpha_quality(WEBP_ALPHA_QUALITY)
        .alpha_filter(AlphaFilter::None)
        .low_memory(false)
        .pass(1)
        .exact(false)
        .segments(WEBP_SEGMENTS);

    let webp_data: Vec<u8> = settings
        .encode_rgba(pixel_data, width, height, Unstoppable)
        .map_err(|error| RenderError::EncodeError(error.to_string()))?;

    Ok(Bytes::from(webp_data))
}
