use std::{array, sync::LazyLock};

use fontdue::{Font, FontSettings};

use super::{error::RenderError, pixels::Point};

const TEXT_SIZE: f32 = 60.0;

struct Letter {
    coverage: Box<[u8]>,
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
            coverage: coverage.into_boxed_slice(),
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

pub(super) fn init_font() {
    LazyLock::force(&LETTERS);
}

// !!! all as usize casts are safe from sign loss !!!
// canvas_w and letter_w come from unsigned integers
// canvas_row_idx is verified positive by the bounds check
// letter_row_idx and letter_col_idx are positive offsets bounded by zero
#[allow(clippy::many_single_char_names, clippy::cast_sign_loss, clippy::similar_names)]
pub(super) fn draw_print_number(
    canvas_width: u32,
    canvas_height: u32,
    canvas_buf: &mut [u8],
    print_number: &[u8],
    pos: Point<i32>,
) -> Result<(), RenderError> {
    // put shadow 1px for visibility issues when bg too bright
    let shadow_color = [0u8, 0u8, 0u8, 160u8];
    draw_glyphs(
        canvas_width,
        canvas_height,
        canvas_buf,
        print_number,
        Point::new(pos.x + 1, pos.y + 1),
        shadow_color,
    )?;

    let white_color = [255u8, 255u8, 255u8, 255u8];
    draw_glyphs(canvas_width, canvas_height, canvas_buf, print_number, pos, white_color)?;

    Ok(())
}

#[allow(clippy::many_single_char_names, clippy::cast_sign_loss, clippy::similar_names)]
fn draw_glyphs(
    canvas_width: u32,
    canvas_height: u32,
    canvas_buf: &mut [u8],
    print_number: &[u8],
    mut pos: Point<i32>,
    color: [u8; 4],
) -> Result<(), RenderError> {
    let canvas_width = canvas_width.cast_signed();
    let canvas_height = canvas_height.cast_signed();
    let canvas_w = canvas_width as usize;

    for &b in print_number {
        let letter = match b {
            b'#' => &LETTERS.hash,
            b'0'..=b'9' => &LETTERS.digits[(b - b'0') as usize],
            _ => continue,
        };

        let letter_width = i32::from(letter.width);
        let letter_height = i32::from(letter.height);
        let letter_w = letter_width as usize;

        let draw_y = pos.y + i32::from(letter.offset_y);

        let draw_y_start = 0.max(-draw_y);
        let draw_y_end = letter_height.min(canvas_height - draw_y);

        if draw_y_start >= draw_y_end {
            pos.x += i32::from(letter.advance_width);
            continue; // offscreen vertically
        }

        let draw_x_start = 0.max(-(pos.x + i32::from(letter.offset_x)));
        let draw_x_end = letter_width.min(canvas_width - (pos.x + i32::from(letter.offset_x)));

        if draw_x_start >= draw_x_end {
            pos.x += i32::from(letter.advance_width);
            continue; // offscreen horizontally
        }

        for draw_y_offset in draw_y_start..draw_y_end {
            let canvas_y = draw_y + draw_y_offset;

            let canvas_row_idx = canvas_y as usize;
            let canvas_col_idx = (pos.x + i32::from(letter.offset_x) + draw_x_start) as usize;
            let canvas_pixel_start = (canvas_row_idx * canvas_w + canvas_col_idx) * 4;

            let letter_row_idx = draw_y_offset as usize;
            let letter_col_idx = draw_x_start as usize;
            let letter_pixel_start = letter_row_idx * letter_w + letter_col_idx;

            let count = (draw_x_end - draw_x_start) as usize;

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

            for (pixel, coverage) in
                target_pixels.chunks_exact_mut(4).zip(letter_row.iter().copied())
            {
                if coverage == 255 {
                    if color[3] == 255 {
                        pixel[0] = color[0];
                        pixel[1] = color[1];
                        pixel[2] = color[2];
                        pixel[3] = 255;
                    } else {
                        let fg_a = u32::from(color[3]);
                        let inv_fg_a = 255 - fg_a;
                        let bg_a = u32::from(pixel[3]);
                        let out_a = (fg_a * 255 + bg_a * inv_fg_a) / 255;
                        if out_a > 0 {
                            let r = u32::from(pixel[0]);
                            let g = u32::from(pixel[1]);
                            let b = u32::from(pixel[2]);
                            pixel[0] = ((u32::from(color[0]) * fg_a + r * inv_fg_a) / 255) as u8;
                            pixel[1] = ((u32::from(color[1]) * fg_a + g * inv_fg_a) / 255) as u8;
                            pixel[2] = ((u32::from(color[2]) * fg_a + b * inv_fg_a) / 255) as u8;
                            pixel[3] = out_a as u8;
                        }
                    }
                } else if coverage > 0 {
                    let fg_a = (u32::from(coverage) * u32::from(color[3])) / 255;
                    if fg_a == 0 {
                        continue;
                    }
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

                    let fg_r = u32::from(color[0]);
                    let fg_g = u32::from(color[1]);
                    let fg_b = u32::from(color[2]);

                    let fg_term_r = fg_r * 255 * fg_a;
                    let fg_term_g = fg_g * 255 * fg_a;
                    let fg_term_b = fg_b * 255 * fg_a;
                    let bg_term = bg_a * inv_fg_a;

                    pixel[0] = ((fg_term_r + r * bg_term) / out_a_times_255) as u8;
                    pixel[1] = ((fg_term_g + g * bg_term) / out_a_times_255) as u8;
                    pixel[2] = ((fg_term_b + b * bg_term) / out_a_times_255) as u8;
                    pixel[3] = out_a as u8;
                }
            }
        }

        pos.x += i32::from(letter.advance_width);
    }
    Ok(())
}

// measures how many padding needed for our print numbers
#[inline]
pub(super) fn measure_print_number(print_number: &[u8]) -> i32 {
    print_number
        .iter()
        .copied()
        .filter_map(|b| match b {
            b'#' => Some(&LETTERS.hash),
            b'0'..=b'9' => Some(&LETTERS.digits[(b - b'0') as usize]),
            _ => None,
        })
        .map(|letter| i32::from(letter.advance_width))
        .sum()
}
