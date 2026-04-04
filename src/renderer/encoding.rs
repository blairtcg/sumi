use bytes::Bytes;
use image::RgbaImage;
use webpx::{EncoderConfig, Preset, Unstoppable};

use crate::renderer::error::RenderError;

const WEBP_QUALITY: f32 = 80.0;
const WEBP_SPEED: u8 = 0;

/// we take raw pixels of two diff cards from moka cache
/// paste it into canvas and draw print nums
/// but because of that huge amount of alpha pixels and bigger image dimension
/// now if we send that raw data over to discord, lets just say the file size would be 2 MB
/// so we would suffer two things, our side and the users side of having to load that shit
/// this is why we do reencoding back to a "new" compressed .webp giving us ~500 KB final file
/// the point of using jpeg would be pointless as we need alpha support
/// the point of skipping alpha compression in webpx is also pointless - similar reason
/// using lossless is also not worth, although encoding takes 100ms instead of our current 350ms
/// it is uncompressed and our drop image dimension is huge so again file size would be 2 MB - bad.
#[inline]
pub fn encode_webp(image: &RgbaImage) -> Result<Bytes, RenderError> {
    let (width, height) = image.dimensions();
    let pixel_data = image.as_raw();

    // we keep encoding on one thread so we avoid slowing down the server.
    // this is by far the only overhead we have, it'll take ~350ms per img.
    // see all other options at https://crates.io/crates/webpx
    let settings = EncoderConfig::new()
        .preset(Preset::Default)
        .quality(WEBP_QUALITY)
        .method(WEBP_SPEED)
        .thread_level(1)
        .segments(1);

    let webp_data: Vec<u8> = settings
        .encode_rgba(pixel_data, width, height, Unstoppable)
        .map_err(|error| RenderError::EncodeError(format!("{}", error)))?;

    Ok(Bytes::from(webp_data))
}
