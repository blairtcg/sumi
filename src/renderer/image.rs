use std::{cell::RefCell, mem, sync::LazyLock};

use bytes::Bytes;
use image::RgbaImage;
use itoa;

use super::encoding::encode_webp;
use crate::renderer::error::RenderError;

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

// we recylcle our shit here
// only one buffer per tokio
thread_local! {
    static RENDER_BUFFER: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(1000 * 800 * 4));
}

/// here we manually do alpha blending of the fonts to the image pixel buffer.
/// we do this manually instead of using a image processing library
/// because it is a bitty faster and avoids useless overhead.
/// we are simply manipulating the byte array for some tiny peformance gain
#[inline(always)]
fn draw_text(canvas: &mut RgbaImage, text: &[u8], mut x: i32, y: i32) {
    let canvas_width = canvas.width() as i32;
    let canvas_height = canvas.height() as i32;
    let canvas_buf = canvas.as_mut();

    for &b in text {
        // look up the letter
        // we only support 1-9 and #
        let letter = match b {
            b'#' => &LETTERS.hash,
            b'0'..=b'9' => &LETTERS.digits[(b - b'0') as usize],
            _ => continue,
        };

        // count the starting x and y coords for letter on the canvas
        let draw_x = x + letter.offset_x;
        let draw_y = y + letter.offset_y;

        for draw_y_offset in 0..letter.height as i32 {
            let canvas_y = draw_y + draw_y_offset;

            // skip row if its outside canvas
            if canvas_y < 0 || canvas_y >= canvas_height {
                continue;
            }

            // count and make sure we dont draw outside canvas
            let mut draw_x_start = 0;
            if draw_x < 0 {
                draw_x_start = -draw_x;
            }
            let mut draw_x_end = letter.width as i32;
            if draw_x + draw_x_end > canvas_width {
                draw_x_end = canvas_width - draw_x;
            }

            // skip if the letter horizontally is outside canvas
            if draw_x_start >= draw_x_end {
                continue;
            }

            // canvas is rgba so its 4 bytes per pixel, coverage is 1 byte per pixel.
            let mut canvas_pixel_index =
                ((canvas_y * canvas_width + (draw_x + draw_x_start)) * 4) as usize;
            let mut letter_pixel_index =
                (draw_y_offset * letter.width as i32 + draw_x_start) as usize;

            for _ in draw_x_start..draw_x_end {
                let coverage = letter.coverage[letter_pixel_index];

                if coverage == 255 {
                    // full white pixel
                    canvas_buf[canvas_pixel_index] = 255;
                    canvas_buf[canvas_pixel_index + 1] = 255;
                    canvas_buf[canvas_pixel_index + 2] = 255;
                    canvas_buf[canvas_pixel_index + 3] = 255;
                } else if coverage > 0 {
                    // here its partially transparent pixel.
                    let alpha = coverage as u32;
                    let inv_alpha = 255 - alpha;

                    let r = canvas_buf[canvas_pixel_index] as u32;
                    let g = canvas_buf[canvas_pixel_index + 1] as u32;
                    let b = canvas_buf[canvas_pixel_index + 2] as u32;
                    let a = canvas_buf[canvas_pixel_index + 3] as u32;

                    // mix white with the background pixel
                    // the formula : foreground * alpha + background * (255 - alpha)) / 255
                    // we use a bit shift (>> 8) as fast approx for division by 255
                    canvas_buf[canvas_pixel_index] = (alpha + ((r * inv_alpha) >> 8)) as u8;
                    canvas_buf[canvas_pixel_index + 1] = (alpha + ((g * inv_alpha) >> 8)) as u8;
                    canvas_buf[canvas_pixel_index + 2] = (alpha + ((b * inv_alpha) >> 8)) as u8;
                    canvas_buf[canvas_pixel_index + 3] = (alpha + ((a * inv_alpha) >> 8)) as u8;
                }

                // move to next pixel
                canvas_pixel_index += 4;
                letter_pixel_index += 1;
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
    left_card_image: &RgbaImage,
    right_card_image: &RgbaImage,
    left_card_print: u32,
    right_card_print: u32,
) -> Result<Bytes, RenderError> {
    // get thread_local bufffer
    RENDER_BUFFER.with(|buf_cell| {
        let mut buffer = buf_cell.borrow_mut();

        // count the dimensions of our drop image
        let left_width = left_card_image.width();
        let left_height = left_card_image.height();
        let right_width = right_card_image.width();
        let right_height = right_card_image.height();

        let total_width = left_width + right_width + (PADDING_BETWEEN_CARDS as u32) * 3;
        let max_card_height = left_height.max(right_height);
        let total_height = max_card_height + (PADDING_BETWEEN_CARDS as u32) * 2;

        // make sure buffer big enough for image (width * height * 4 bytes per pixel).
        // we clear it first so background is transparent
        let required_len = (total_width * total_height * 4) as usize;
        buffer.clear();
        buffer.resize(required_len, 0);

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
        let left_pixels = left_card_image.as_raw();
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
        let right_pixels = right_card_image.as_raw();
        let mut card_start_index = 0;
        let mut canvas_start_index =
            ((card_vertical_position * total_width + right_card_position) * 4) as usize;
        for _ in 0..right_height {
            buffer[canvas_start_index..canvas_start_index + right_row_bytes].copy_from_slice(
                &right_pixels[card_start_index..card_start_index + right_row_bytes],
            );
            card_start_index += right_row_bytes;
            canvas_start_index += total_row_bytes;
        }

        // wrap the buffer into an RgbaImage so we can pass it to the encoder etc
        // we use std::mem::take to move the buffer for a while out of the RefCell
        let mut final_image =
            RgbaImage::from_raw(total_width, total_height, mem::take(&mut *buffer)).unwrap();

        // format print numbers into string
        // a bit overengineered but we use itoa instead of normal format!()
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

        // count positions for text and draw it to the image
        let left_text_x = left_card_position as i32 + left_width as i32 - TEXT_PADDING_FROM_EDGE;
        let right_text_x = right_card_position as i32 + right_width as i32 - TEXT_PADDING_FROM_EDGE;
        let text_y = total_height as i32 - TEXT_SIZE as i32 - TEXT_PADDING_FROM_BOTTOM;

        draw_text(&mut final_image, left_text, left_text_x, text_y);
        draw_text(&mut final_image, right_text, right_text_x, text_y);

        // encode final drop image to webp
        let result = encode_webp(&final_image);

        // return buffer to the thread_local storage so it can be reused for next reqs
        *buffer = final_image.into_raw();

        result
    })
}
