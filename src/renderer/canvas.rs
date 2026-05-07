use std::sync::LazyLock;

use crate::renderer::error::RenderError;

/// raw uncompressed rgba image
/// this will act as a container that will replace our previous `image::RgbaImage`.
pub struct RawCardImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl std::fmt::Debug for RawCardImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawCardImage")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("pixels_len", &self.pixels.len())
            .finish()
    }
}

const TEXT_SIZE: f32 = 60.0;
const TEXT_PADDING_FROM_EDGE: i32 = 190;
const PADDING_BETWEEN_CARDS: i32 = 20;
const TEXT_PADDING_FROM_BOTTOM: i32 = 80;

// normal font lib takes a while to render a letter.
// so now we do this.
struct Letter {
    coverage: Vec<u8>,
    width: u32,
    height: u32,
    advance_width: i32,
    offset_x: i32,
    offset_y: i32,
}

struct LetterSet {
    hash: Letter,
    digits: [Letter; 10],
}

// -- Load font sekali doang
static LETTERS: LazyLock<LetterSet> = LazyLock::new(|| {
    let font_data = include_bytes!("../../assets/LexendDeca-Bold.ttf") as &[u8];
    let font = fontdue::Font::from_bytes(font_data, fontdue::FontSettings::default())
        .expect("could not load font file");
    let metrics = font.horizontal_line_metrics(TEXT_SIZE).unwrap();
    let ascent = metrics.ascent;

    let render_char = |c: char| -> Letter {
        let (metrics, coverage) = font.rasterize(c, TEXT_SIZE);
        Letter {
            coverage,
            width: metrics.width as u32,
            height: metrics.height as u32,
            advance_width: metrics.advance_width.round() as i32,
            offset_x: metrics.xmin,
            offset_y: (ascent - metrics.ymin as f32 - metrics.height as f32).round() as i32,
        }
    };

    LetterSet {
        hash: render_char('#'),
        digits: [
            render_char('0'),
            render_char('1'),
            render_char('2'),
            render_char('3'),
            render_char('4'),
            render_char('5'),
            render_char('6'),
            render_char('7'),
            render_char('8'),
            render_char('9'),
        ],
    }
});

pub fn init_font() {
    let _ = &*LETTERS;
}

/// here we manually do alpha blending of the fonts to the image pixel buffer.
/// we do this manually instead of using a image processing library
/// because it is a bitty faster and avoids useless overhead.
/// we are simply manipulating the byte array for some tiny peformance gain
#[inline(always)]
fn draw_text(canvas: &mut RawCardImage, text: &[u8], mut x: i32, y: i32) {
    let canvas_width = canvas.width as i32;
    let canvas_height = canvas.height as i32;
    let canvas_buf = &mut canvas.pixels;

    for &b in text {
        // look up the letter
        // we only support 1-9 and #
        let letter = match b {
            b'#' => &LETTERS.hash,
            b'0'..=b'9' => &LETTERS.digits[(b - b'0') as usize],
            _ => continue,
        };

        // count the starting x and y coords for letter on the canvas
        let draw_y = y + letter.offset_y;

        for draw_y_offset in 0..letter.height as i32 {
            let canvas_y = draw_y + draw_y_offset;

            // skip row if its outside canvas
            if canvas_y < 0 || canvas_y >= canvas_height {
                continue;
            }

            // count and make sure we dont draw outside canvas
            let draw_x_start = if x + letter.offset_x < 0 { -(x + letter.offset_x) } else { 0 };
            let draw_x_end = if x + letter.offset_x + letter.width as i32 > canvas_width {
                canvas_width - (x + letter.offset_x)
            } else {
                letter.width as i32
            };

            // skip if the letter horizontally is outside canvas
            if draw_x_start >= draw_x_end {
                continue;
            }

            // canvas is rgba so its 4 bytes per pixel, coverage is 1 byte per pixel.
            let canvas_pixel_start =
                ((canvas_y * canvas_width + (x + letter.offset_x + draw_x_start)) * 4) as usize;
            let letter_pixel_start = (draw_y_offset * letter.width as i32 + draw_x_start) as usize;

            let count = (draw_x_end - draw_x_start) as usize;
            let target_pixels = &mut canvas_buf[canvas_pixel_start..canvas_pixel_start + count * 4];
            let glyph_row = &letter.coverage[letter_pixel_start..letter_pixel_start + count];

            for (pixel, &coverage) in target_pixels.chunks_exact_mut(4).zip(glyph_row) {
                if coverage == 255 {
                    pixel[0] = 255;
                    pixel[1] = 255;
                    pixel[2] = 255;
                    pixel[3] = 255;
                } else if coverage > 0 {
                    let alpha = coverage as u32;
                    let inv_alpha = 255 - alpha;

                    let r = pixel[0] as u32;
                    let g = pixel[1] as u32;
                    let b = pixel[2] as u32;
                    let a = pixel[3] as u32;

                    // divide by 256 using bitshift.
                    // this will make it 254 instead of 255 but is mathematically much faster.
                    // -- Fixing next version
                    pixel[0] = (alpha + ((r * inv_alpha) >> 8)) as u8;
                    pixel[1] = (alpha + ((g * inv_alpha) >> 8)) as u8;
                    pixel[2] = (alpha + ((b * inv_alpha) >> 8)) as u8;
                    pixel[3] = (alpha + ((a * inv_alpha) >> 8)) as u8;
                }
            }
        }

        // up the x coord for the next letter.
        x += letter.advance_width;
    }
}

/// combine two card images and add print numbers = drop image
/// we use thread_local buffer to make drop image, manually copying
/// pixel rows from the card images. this is much faster than creating a new
/// blank image and using a library to paste the card images to it.
pub fn create_drop_image(
    left_card: &RawCardImage,
    right_card: &RawCardImage,
    left_card_print: u32,
    right_card_print: u32,
) -> Result<bytes::Bytes, RenderError> {
    let start_canvas = std::time::Instant::now();

    // count the dimensions of our drop image
    let left_width = left_card.width;
    let left_height = left_card.height;
    let right_width = right_card.width;
    let right_height = right_card.height;

    let total_width = left_width + right_width + (PADDING_BETWEEN_CARDS as u32) * 3;
    let max_card_height = left_height.max(right_height);
    let total_height = max_card_height + (PADDING_BETWEEN_CARDS as u32) * 2;

    // make sure buffer big enough for image (width * height * 4 bytes per pixel).
    let required_len = (total_width * total_height * 4) as usize;

    // now we allocate the buffer directly since we are using mimalloc which is optimized
    // for these kinds of allocations, removing the need for Mutex buffer pool.
    let mut buffer: Vec<u8> = Vec::with_capacity(required_len);

    // -- maybeuninit 
    // 1. ask for memory from the OS (via reserve) but do not start it yet.
    // 2. manually spam zero fill ONLY the transparent padding areas using fast memset `write_bytes`
    // 3. skip the whole 3MB+ full buffer `vec![0]` iter cycle. so now the card pixel memory
    //    is left uninitialized here because it gets safely overwritten via `copy_from_slice` below
    //
    // safety guarantee:
    // - we only offset pointers strictly within the mathematically bound `required_len`.
    // - `u8` bytes require no drop glue, so leaving some uninitialized here is harmless.
    // - we commit the memory bounds with `set_len` securely at the very end of pointer work.
    unsafe {
        let ptr = buffer.as_mut_ptr();
        let row_bytes = (total_width * 4) as usize;

        for y in 0..total_height {
            let row_start = (y as usize) * row_bytes;

            // top and bottom transparent padding rows
            if y < PADDING_BETWEEN_CARDS as u32
                || y >= total_height - (PADDING_BETWEEN_CARDS as u32)
            {
                std::ptr::write_bytes(ptr.add(row_start), 0, row_bytes);
            } else {
                // transparent middle part of cards
                let mut offset = 0;
                let pad_bytes = (PADDING_BETWEEN_CARDS as usize) * 4;

                // left padding
                std::ptr::write_bytes(ptr.add(row_start + offset), 0, pad_bytes);
                offset += pad_bytes;

                // left card box
                let left_card_w_bytes = (left_width as usize) * 4;
                let card_y = y - (PADDING_BETWEEN_CARDS as u32);
                if card_y >= left_height {
                    std::ptr::write_bytes(ptr.add(row_start + offset), 0, left_card_w_bytes);
                }
                offset += left_card_w_bytes;

                // center padding
                std::ptr::write_bytes(ptr.add(row_start + offset), 0, pad_bytes);
                offset += pad_bytes;

                // right card box
                let right_card_w_bytes = (right_width as usize) * 4;
                if card_y >= right_height {
                    std::ptr::write_bytes(ptr.add(row_start + offset), 0, right_card_w_bytes);
                }
                offset += right_card_w_bytes;

                // right side padding
                std::ptr::write_bytes(ptr.add(row_start + offset), 0, pad_bytes);
            }
        }

        // finish lock in the started length so slices can index it safely
        buffer.set_len(required_len);
    }

    // count starting position for the left and right card.
    let left_card_position = PADDING_BETWEEN_CARDS as u32;
    let right_card_position = left_width + (PADDING_BETWEEN_CARDS as u32) * 2;
    let card_vertical_position = PADDING_BETWEEN_CARDS as u32;

    // count number of bytes per row for each card
    let left_row_bytes = (left_width * 4) as usize;
    let right_row_bytes = (right_width * 4) as usize;
    let total_row_bytes = (total_width * 4) as usize;

    // copy pixels from left card into buffer.
    // we do this row by row so its correctly placed in the canvas
    let left_pixels = &left_card.pixels;
    let mut card_start_index = 0;
    let mut canvas_start_index =
        ((card_vertical_position * total_width + left_card_position) * 4) as usize;
    for _ in 0..left_height {
        buffer[canvas_start_index..canvas_start_index + left_row_bytes]
            .copy_from_slice(&left_pixels[card_start_index..card_start_index + left_row_bytes]);
        card_start_index += left_row_bytes;
        canvas_start_index += total_row_bytes;
    }

    // copy pixels from right card into buffer
    let right_pixels = &right_card.pixels;
    let mut card_start_index = 0;
    let mut canvas_start_index =
        ((card_vertical_position * total_width + right_card_position) * 4) as usize;
    for _ in 0..right_height {
        buffer[canvas_start_index..canvas_start_index + right_row_bytes]
            .copy_from_slice(&right_pixels[card_start_index..card_start_index + right_row_bytes]);
        card_start_index += right_row_bytes;
        canvas_start_index += total_row_bytes;
    }

    // wrap the buffer into RawCardImage so we can pass it to the encoder etc
    // we fully consume the buffer here
    let mut final_image = RawCardImage { width: total_width, height: total_height, pixels: buffer };

    // format print numbers into string
    // a bit overengineered but we use itoa instead of normal format!()
    // -- itoa juga dipakai di segala crate jadi lebih mending diimplementasi
    // -- bedanya cuman format!() lebih simple dibaca
    let mut itoa_buf = itoa::Buffer::new();
    let left_num_str = itoa_buf.format(left_card_print);
    let mut left_text_buf = [0u8; 16];
    left_text_buf[0] = b'#';
    left_text_buf[1..1 + left_num_str.len()].copy_from_slice(left_num_str.as_bytes());
    let left_text = &left_text_buf[..1 + left_num_str.len()];

    let right_num_str = itoa_buf.format(right_card_print);
    let mut right_text_buf = [0u8; 16];
    right_text_buf[0] = b'#';
    right_text_buf[1..1 + right_num_str.len()].copy_from_slice(right_num_str.as_bytes());
    let right_text = &right_text_buf[..1 + right_num_str.len()];

    let canvas_time = start_canvas.elapsed();

    let start_text = std::time::Instant::now();
    // count positions for text and draw it to the image
    let left_text_x = left_card_position as i32 + left_width as i32 - TEXT_PADDING_FROM_EDGE;
    let right_text_x = right_card_position as i32 + right_width as i32 - TEXT_PADDING_FROM_EDGE;
    let text_y = total_height as i32 - TEXT_SIZE as i32 - TEXT_PADDING_FROM_BOTTOM;

    draw_text(&mut final_image, left_text, left_text_x, text_y);
    draw_text(&mut final_image, right_text, right_text_x, text_y);

    let text_time = start_text.elapsed();

    let start_encode = std::time::Instant::now();
    // encode final drop image to webp
    let result = super::encoding::encode_webp(&final_image);
    let encode_time = start_encode.elapsed();

    log::debug!(
        "pasting={:.3}ms, font={:.3}ms, encode={:.3}ms",
        canvas_time.as_secs_f64() * 1000.0,
        text_time.as_secs_f64() * 1000.0,
        encode_time.as_secs_f64() * 1000.0
    );

    result
}
