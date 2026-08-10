use std::{array, sync::LazyLock};

use fontdue::{Font, FontSettings};

use super::{error::RenderError, pixels::Point};

const TEXT_SIZE: f32 = 60.0;

#[derive(Clone, Copy)]
struct GlyphPixel {
    fg_rgb: u8,
    fg_a: u8,
    inv_a: u8,
}

struct Letter {
    width: u16,
    height: u16,
    advance_width: i16,
    offset_x: i16,
    offset_y: i16,
    shadow_pass: Box<[GlyphPixel]>,
    white_pass: Box<[GlyphPixel]>,
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

        let mut shadow_pass = Vec::with_capacity(coverage.len());
        let mut white_pass = Vec::with_capacity(coverage.len());

        for &cov in &coverage {
            white_pass.push(GlyphPixel { fg_rgb: cov, fg_a: cov, inv_a: 255 - cov });

            let shadow_a = ((u32::from(cov) * 160) / 255) as u8;
            shadow_pass.push(GlyphPixel { fg_rgb: 0, fg_a: shadow_a, inv_a: 255 - shadow_a });
        }

        Letter {
            width: metrics.width as u16,
            height: metrics.height as u16,
            advance_width: metrics.advance_width.round() as i16,
            offset_x: metrics.xmin as i16,
            offset_y: (ascent - metrics.ymin as f32 - metrics.height as f32).round() as i16,
            shadow_pass: shadow_pass.into_boxed_slice(),
            white_pass: white_pass.into_boxed_slice(),
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
// this is same for draw_pass func
#[allow(clippy::many_single_char_names, clippy::cast_sign_loss, clippy::similar_names)]
pub(super) fn draw_print_number(
    canvas_width: u32,
    canvas_height: u32,
    canvas_buf: &mut [u8],
    print_number: &[u8],
    pos: Point<i32>,
) -> Result<(), RenderError> {
    draw_pass(
        canvas_width,
        canvas_height,
        canvas_buf,
        print_number,
        Point::new(pos.x + 1, pos.y + 1),
        true,
    )?;

    draw_pass(canvas_width, canvas_height, canvas_buf, print_number, pos, false)?;

    Ok(())
}

#[allow(clippy::many_single_char_names, clippy::cast_sign_loss, clippy::similar_names)]
fn draw_pass(
    canvas_width: u32,
    canvas_height: u32,
    canvas_buf: &mut [u8],
    print_number: &[u8],
    mut pos: Point<i32>,
    is_shadow: bool,
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
            continue;
        }

        let draw_x_start = 0.max(-(pos.x + i32::from(letter.offset_x)));
        let draw_x_end = letter_width.min(canvas_width - (pos.x + i32::from(letter.offset_x)));

        if draw_x_start >= draw_x_end {
            pos.x += i32::from(letter.advance_width);
            continue;
        }

        let glyph_pass = if is_shadow { &letter.shadow_pass } else { &letter.white_pass };

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
                glyph_pass.get(letter_pixel_start..letter_pixel_end).ok_or_else(|| {
                    RenderError::Internal("letter coverage range out of bounds".to_string())
                })?;

            for (pixel, glyph) in target_pixels.chunks_exact_mut(4).zip(letter_row.iter()) {
                let fg_rgb = u32::from(glyph.fg_rgb);
                let fg_a = u32::from(glyph.fg_a);
                let inv_a = u32::from(glyph.inv_a);

                let blend = |bg: u8, fg: u32, inv: u32| -> u8 {
                    let t = u32::from(bg) * inv + 128;
                    (fg + ((t + (t >> 8)) >> 8)) as u8
                };

                pixel[0] = blend(pixel[0], fg_rgb, inv_a);
                pixel[1] = blend(pixel[1], fg_rgb, inv_a);
                pixel[2] = blend(pixel[2], fg_rgb, inv_a);
                pixel[3] = blend(pixel[3], fg_a, inv_a);
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
