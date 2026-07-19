use std::{array, sync::LazyLock};

use fontdue::{Font, FontSettings};

use super::{error::RenderError, pixels::Point};

const TEXT_SIZE: f32 = 60.0;

struct Letter {
    coverage: Vec<u8>,
    width: u16,
    height: u16,
    advance_width: i16,
    offset_x: i16,
    offset_y: i16,
}

struct LetterSet {
    hash: Letter,
    digits: [Letter; 10],
}

static LETTERS: LazyLock<LetterSet> = LazyLock::new(|| {
    let font_data = include_bytes!("../../assets/LexendDeca-Bold.ttf") as &[u8];
    let font =
        Font::from_bytes(font_data, FontSettings::default()).expect("could not load font file");
    // safe to unwrap because font is built in and won't change during runtime
    let metrics = font.horizontal_line_metrics(TEXT_SIZE).expect("font should have line metrics");
    let ascent = metrics.ascent;

    let render_char = |c: char| -> Letter {
        let (metrics, coverage) = font.rasterize(c, TEXT_SIZE);
        Letter {
            coverage,
            width: metrics.width as u16,
            height: metrics.height as u16,
            advance_width: metrics.advance_width.round() as i16,
            offset_x: metrics.xmin as i16,
            offset_y: (ascent - metrics.ymin as f32 - metrics.height as f32).round() as i16,
        }
    };

    LetterSet {
        hash: render_char('#'),
        digits: array::from_fn(|i| render_char((b'0' + i as u8) as char)),
    }
});

pub fn init_font() {
    LazyLock::force(&LETTERS);
}

#[inline]
#[allow(clippy::many_single_char_names)]
pub fn draw_print_number(
    canvas_width: u32,
    canvas_height: u32,
    canvas_buf: &mut [u8],
    print_number: &[u8],
    mut pos: Point<i32>,
) -> Result<(), RenderError> {
    let canvas_width = canvas_width.cast_signed();
    let canvas_height = canvas_height.cast_signed();

    for &b in print_number {
        let letter = match b {
            b'#' => &LETTERS.hash,
            b'0'..=b'9' => &LETTERS.digits[(b - b'0') as usize],
            _ => continue,
        };

        let letter_width = i32::from(letter.width);
        let letter_height = i32::from(letter.height);

        let draw_y = pos.y + i32::from(letter.offset_y);

        for draw_y_offset in 0..letter_height {
            let canvas_y = draw_y + draw_y_offset;

            if canvas_y < 0 || canvas_y >= canvas_height {
                continue;
            }

            let draw_x_start = 0.max(-(pos.x + i32::from(letter.offset_x)));
            let draw_x_end = letter_width.min(canvas_width - (pos.x + i32::from(letter.offset_x)));

            if draw_x_start >= draw_x_end {
                continue;
            }

            let canvas_row_idx = usize::try_from(canvas_y).map_err(|e| {
                RenderError::Internal(format!("failed to convert canvas row index: {e}"))
            })?;
            let canvas_w = usize::try_from(canvas_width).map_err(|e| {
                RenderError::Internal(format!("failed to convert canvas width: {e}"))
            })?;
            let canvas_col_idx = usize::try_from(pos.x + i32::from(letter.offset_x) + draw_x_start)
                .map_err(|e| {
                    RenderError::Internal(format!("failed to convert canvas column index: {e}"))
                })?;
            let canvas_pixel_start = (canvas_row_idx * canvas_w + canvas_col_idx) * 4;

            let letter_row_idx = usize::try_from(draw_y_offset).map_err(|e| {
                RenderError::Internal(format!("failed to convert letter row index: {e}"))
            })?;
            let letter_w = usize::try_from(letter_width).map_err(|e| {
                RenderError::Internal(format!("failed to convert letter width: {e}"))
            })?;
            let letter_col_idx = usize::try_from(draw_x_start).map_err(|e| {
                RenderError::Internal(format!("failed to convert letter column index: {e}"))
            })?;
            let letter_pixel_start = letter_row_idx * letter_w + letter_col_idx;

            let count = usize::try_from(draw_x_end - draw_x_start).map_err(|e| {
                RenderError::Internal(format!("failed to convert pixel transfer count: {e}"))
            })?;

            let canvas_pixel_end = canvas_pixel_start + count * 4;
            let letter_pixel_end = letter_pixel_start + count;

            let target_pixels =
                canvas_buf.get_mut(canvas_pixel_start..canvas_pixel_end).ok_or_else(|| {
                    RenderError::Internal("canvas pixel range out of bounds".to_string())
                })?;
            let letter_row =
                letter.coverage.get(letter_pixel_start..letter_pixel_end).ok_or_else(|| {
                    RenderError::Internal("letter coverage range out of bounds".to_string())
                })?;

            for (pixel, &coverage) in target_pixels.chunks_exact_mut(4).zip(letter_row) {
                if coverage == 255 {
                    pixel.copy_from_slice(&[255, 255, 255, 255]);
                } else if coverage > 0 {
                    let fg_a = u32::from(coverage);
                    let inv_fg_a = 255 - fg_a;
                    let bg_a = u32::from(pixel[3]);

                    let out_a_times_255 = fg_a * 255 + bg_a * inv_fg_a;
                    let out_a = out_a_times_255 / 255;

                    if out_a == 0 {
                        continue;
                    }

                    let r = u32::from(pixel[0]);
                    let g = u32::from(pixel[1]);
                    let b = u32::from(pixel[2]);

                    pixel[0] = ((255 * fg_a * 255 + r * bg_a * inv_fg_a) / out_a_times_255) as u8;
                    pixel[1] = ((255 * fg_a * 255 + g * bg_a * inv_fg_a) / out_a_times_255) as u8;
                    pixel[2] = ((255 * fg_a * 255 + b * bg_a * inv_fg_a) / out_a_times_255) as u8;
                    pixel[3] = out_a as u8;
                }
            }
        }

        pos.x += i32::from(letter.advance_width);
    }
    Ok(())
}

// measures how many padding needed for our print numbers
pub fn measure_print_number(print_number: &[u8]) -> i32 {
    let mut width = 0;
    for &b in print_number {
        let letter = match b {
            b'#' => &LETTERS.hash,
            b'0'..=b'9' => &LETTERS.digits[(b - b'0') as usize],
            _ => continue,
        };
        width += i32::from(letter.advance_width);
    }
    width
}
