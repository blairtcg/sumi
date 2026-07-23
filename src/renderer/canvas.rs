use std::{sync::Mutex, time::Instant};

use bytes::Bytes;
use itoa::Buffer;

use super::{
    PrintNumber,
    encoder::encode_webp,
    error::Result,
    pixels::{Point, RawCardImage},
    print::{draw_print_number, measure_print_number},
};

const TEXT_SIZE: f32 = 60.0;
const TEXT_PADDING_FROM_EDGE: i32 = 190;
const PADDING_BETWEEN_CARDS: u32 = 20;
const TEXT_PADDING_FROM_BOTTOM: i32 = 80;

#[inline]
fn copy_card_pixels(buffer: &mut [u8], card: &RawCardImage, total_width: u32, pos: Point<u32>) {
    let card_row_bytes = (card.size.width * 4) as usize;
    let total_row_bytes = (total_width * 4) as usize;
    assert!(total_row_bytes >= card_row_bytes, "total row must be larger than card row");

    let dest_rows = buffer.chunks_exact_mut(total_row_bytes);
    let src_rows = card.pixels.chunks_exact(card_row_bytes);

    for (dest_row, src_row) in
        dest_rows.skip(pos.y as usize).zip(src_rows).take(card.size.height as usize)
    {
        let x_offset = (pos.x * 4) as usize;
        dest_row[x_offset..x_offset + card_row_bytes].copy_from_slice(src_row);
    }
}

#[inline]
fn format_print_number(print_num: u16, buf: &mut [u8; 8]) -> &[u8] {
    let num = print_num.clamp(1, 999);
    buf[0] = b'#';
    let mut itoa = Buffer::new();
    let s = itoa.format(num);
    let len = 1 + s.len();
    buf[1..len].copy_from_slice(s.as_bytes());
    &buf[..len]
}

static DROP_POOL: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());
const MAX_POOL_BUFFERS: usize = 16;

struct BufferGuard {
    buffer: Vec<u8>,
}

impl BufferGuard {
    #[inline]
    fn new(mut buffer: Vec<u8>, required_len: usize) -> Self {
        buffer.clear();
        buffer.resize(required_len, 0);
        Self { buffer }
    }
}

impl Drop for BufferGuard {
    #[inline]
    fn drop(&mut self) {
        let mut pool = DROP_POOL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if pool.len() < MAX_POOL_BUFFERS {
            let buf = std::mem::take(&mut self.buffer);
            pool.push(buf);
        }
    }
}

impl std::ops::Deref for BufferGuard {
    type Target = Vec<u8>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl std::ops::DerefMut for BufferGuard {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}

// combine two card images and add print numbers = drop image
// we manually copy pixel rows from the card images. this is much faster
// than creating a new blank image and using a library to paste the card images to it.
pub(super) fn create_drop_image(
    left_card: &RawCardImage,
    right_card: &RawCardImage,
    left_card_print: PrintNumber,
    right_card_print: PrintNumber,
) -> Result<Bytes> {
    let start_canvas = Instant::now();

    // count the dimensions of our drop image
    let left_width = left_card.size.width;
    let right_width = right_card.size.width;

    let total_width = left_width + right_width + PADDING_BETWEEN_CARDS * 3;
    let max_card_height = left_card.size.height.max(right_card.size.height);
    let total_height = max_card_height + PADDING_BETWEEN_CARDS * 2;

    // make sure buffer big enough for image (width * height * 4 bytes per pixel)
    let required_len = (total_width * total_height * 4) as usize;

    let mut buffer = BufferGuard::new(
        DROP_POOL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop()
            .unwrap_or_default(),
        required_len,
    );

    // count starting position for the left and right card.
    let left_card_x = PADDING_BETWEEN_CARDS;
    let right_card_x = left_width + PADDING_BETWEEN_CARDS * 2;
    let card_y = PADDING_BETWEEN_CARDS;

    // copy pixels from left and right card into buffer.
    copy_card_pixels(&mut buffer, left_card, total_width, Point::new(left_card_x, card_y));
    copy_card_pixels(&mut buffer, right_card, total_width, Point::new(right_card_x, card_y));

    let mut left_print_buf = [0u8; 8];
    let left_print = format_print_number(left_card_print.value(), &mut left_print_buf);

    let mut right_print_buf = [0u8; 8];
    let right_print = format_print_number(right_card_print.value(), &mut right_print_buf);

    let canvas_time = start_canvas.elapsed();
    let start_print = Instant::now();

    // count positions for text and draw it to the image
    let left_print_width = measure_print_number(left_print);
    let right_print_width = measure_print_number(right_print);

    let ref_width = measure_print_number(b"#00");
    let right_padding = TEXT_PADDING_FROM_EDGE - ref_width;

    let left_print_x = (left_card_x + left_width).cast_signed() - right_padding - left_print_width;
    let right_print_x =
        (right_card_x + right_width).cast_signed() - right_padding - right_print_width;
    let print_y = total_height.cast_signed() - TEXT_SIZE as i32 - TEXT_PADDING_FROM_BOTTOM;

    draw_print_number(
        total_width,
        total_height,
        &mut buffer[..required_len],
        left_print,
        Point::new(left_print_x, print_y),
    )?;
    draw_print_number(
        total_width,
        total_height,
        &mut buffer[..required_len],
        right_print,
        Point::new(right_print_x, print_y),
    )?;

    let print_time = start_print.elapsed();
    let start_encode = Instant::now();

    // encode final drop image to webp
    let result = encode_webp(total_width, total_height, &buffer[..required_len]);
    let encode_time = start_encode.elapsed();

    tracing::debug!(
        "spent: paste={:.2}ms, font={:.2}ms, [encode={:.3}ms]",
        canvas_time.as_secs_f64() * 1000.0,
        print_time.as_secs_f64() * 1000.0,
        encode_time.as_secs_f64() * 1000.0
    );

    result
}
