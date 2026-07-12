use bytes::Bytes;
use zenwebp::{EncodeRequest, LossyConfig, PixelLayout, Preset};

use crate::renderer::error::RenderError;

const WEBP_QUALITY: f32 = 85.0;
const WEBP_SPEED: u8 = 0;
const WEBP_ALPHA_QUALITY: u8 = 80;
const WEBP_SEGMENTS: u8 = 1;

/// we take raw pixels of two diff cards from cache
/// paste it into canvas and draw print nums
/// but because of that huge amount of alpha pixels and bigger image dimension
/// now if we send that pixels data over to discord, lets just say the file size would be 2 MB
/// so we would suffer two things, our side and the users side of having to load that shit
/// this is why we do reencoding back to a "new" compressed .webp giving us ~400 KB final file
/// the point of using jpeg would be pointless as we need alpha support
/// the point of skipping alpha compression in zenwebp is also pointless - similar reason
/// using lossless is also not worth, although encoding takes 100ms instead of our current 350ms
/// it is uncompressed and our drop image dimension is huge so again file size would be 2 MB - bad.
pub fn encode_webp(width: u32, height: u32, pixel_data: &[u8]) -> Result<Bytes, RenderError> {
    // we keep encoding on one thread so we avoid slowing down the server.
    // this is by far the only overhead we have, it'll take ~100ms per img.
    let settings = LossyConfig::new()
        // preset needs more testing to check which one is best for our cards
        .with_preset_value(Preset::Picture)
        .with_quality(WEBP_QUALITY)
        .with_method(WEBP_SPEED)
        .with_alpha_quality(WEBP_ALPHA_QUALITY)
		// 3 = 8 partitions, which theoretically should unlock parallel decoding for discord users
		// though i dont really know what goes on inside discord as they do their own weird shit
		// but more partitions essentially means more file size which is bad for our I/O
        // .partitions(3)
        .with_segments(WEBP_SEGMENTS);

    let webp_data: Vec<u8> = EncodeRequest::lossy(&settings, pixel_data, PixelLayout::Rgba8, width, height)
        .encode()
        .map_err(|error| RenderError::EncodeError(error.to_string()))?;

    Ok(Bytes::from(webp_data))
}
