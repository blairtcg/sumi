use std::{array, sync::LazyLock};

use fontdue::{Font, FontSettings};

use super::pixels::Point;

pub(super) const TEXT_SIZE: f32 = 60.0;

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
    ambient_shadow_pass: Box<[GlyphPixel]>,
    contact_shadow_pass: Box<[GlyphPixel]>,
    white_pass: Box<[GlyphPixel]>,
}

struct LetterSet {
    hash: Letter,
    digits: [Letter; 10],
    ref_width: i32,
}

static LETTERS: LazyLock<LetterSet> = LazyLock::new(|| {
    let font_data = include_bytes!("../../assets/LexendDeca-Bold.ttf") as &[u8];
    let font =
        Font::from_bytes(font_data, FontSettings::default()).expect("could not load font file");
    // safe to unwrap because font is built in and won't change during runtime.
    let metrics = font.horizontal_line_metrics(TEXT_SIZE).expect("font should have line metrics");
    let ascent = metrics.ascent;

    let render_char = |c: char| -> Letter {
        let (metrics, coverage) = font.rasterize(c, TEXT_SIZE);

        let mut ambient_shadow_pass = Vec::with_capacity(coverage.len());
        let mut contact_shadow_pass = Vec::with_capacity(coverage.len());
        let mut white_pass = Vec::with_capacity(coverage.len());

        for &cov in &coverage {
            white_pass.push(GlyphPixel { fg_rgb: cov, fg_a: cov, inv_a: 255 - cov });
            
            // make the shadow lighter (90/255) for the ambient pass.
            let ambient_a = ((u32::from(cov) * 90) / 255) as u8;
            ambient_shadow_pass.push(GlyphPixel {
                fg_rgb: 0,
                fg_a: ambient_a,
                inv_a: 255 - ambient_a,
            });

            // make the shadow darker (190/255) for the contact pass.
            let contact_a = ((u32::from(cov) * 190) / 255) as u8;
            contact_shadow_pass.push(GlyphPixel {
                fg_rgb: 0,
                fg_a: contact_a,
                inv_a: 255 - contact_a,
            });
        }

        Letter {
            width: metrics.width as u16,
            height: metrics.height as u16,
            advance_width: metrics.advance_width.round() as i16,
            offset_x: metrics.xmin as i16,
            offset_y: (ascent - metrics.ymin as f32 - metrics.height as f32).round() as i16,
            ambient_shadow_pass: ambient_shadow_pass.into_boxed_slice(),
            contact_shadow_pass: contact_shadow_pass.into_boxed_slice(),
            white_pass: white_pass.into_boxed_slice(),
        }
    };

    let hash = render_char('#');
    let digits = array::from_fn(|i| render_char((b'0' + i as u8) as char));
    // calculate the width of "#00" to use as a reference for spacing.
    let ref_width = i32::from(hash.advance_width) + i32::from(digits[0].advance_width) * 2;

    LetterSet { hash, digits, ref_width }
});

/*
=============
init_font
=============
*/
pub(super) fn init_font() {
    LazyLock::force(&LETTERS);
}

/*
=============
ref_number_width
=============
*/
#[inline]
pub(super) fn ref_number_width() -> i32 {
    LETTERS.ref_width
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PassKind {
    AmbientShadow,
    ContactShadow,
    Foreground,
}

/*
=============
draw_print_number
=============
*/
#[allow(clippy::many_single_char_names, clippy::cast_sign_loss, clippy::similar_names)]
pub(super) fn draw_print_number(
    canvas_width: u32,
    canvas_height: u32,
    canvas_buf: &mut [u8],
    print_number: &[u8],
    pos: Point<i32>,
) {
    // draw the shadows first so they appear underneath the white text.
    draw_pass(
        canvas_width,
        canvas_height,
        canvas_buf,
        print_number,
        Point::new(pos.x + 2, pos.y + 2),
        PassKind::AmbientShadow,
    );

    draw_pass(
        canvas_width,
        canvas_height,
        canvas_buf,
        print_number,
        Point::new(pos.x + 1, pos.y + 1),
        PassKind::ContactShadow,
    );

    draw_pass(canvas_width, canvas_height, canvas_buf, print_number, pos, PassKind::Foreground);
}

/*
=============
draw_pass
=============
*/
#[allow(clippy::many_single_char_names, clippy::cast_sign_loss, clippy::similar_names)]
fn draw_pass(
    canvas_width: u32,
    canvas_height: u32,
    canvas_buf: &mut [u8],
    print_number: &[u8],
    mut pos: Point<i32>,
    pass_kind: PassKind,
) {
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

        // clip the drawing area if it goes outside the canvas boundaries.
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

        let glyph_pass = match pass_kind {
            PassKind::AmbientShadow => &letter.ambient_shadow_pass,
            PassKind::ContactShadow => &letter.contact_shadow_pass,
            PassKind::Foreground => &letter.white_pass,
        };

        for draw_y_offset in draw_y_start..draw_y_end {
            let canvas_y = draw_y + draw_y_offset;
    
            // calculate the exact memory address for the pixel row.
            let canvas_row_idx = canvas_y as usize;
            let canvas_col_idx = (pos.x + i32::from(letter.offset_x) + draw_x_start) as usize;
            let canvas_pixel_start = (canvas_row_idx * canvas_w + canvas_col_idx) * 4;

            let letter_row_idx = draw_y_offset as usize;
            let letter_col_idx = draw_x_start as usize;
            let letter_pixel_start = letter_row_idx * letter_w + letter_col_idx;

            let count = (draw_x_end - draw_x_start) as usize;

            let canvas_pixel_end = canvas_pixel_start + count * 4;
            let letter_pixel_end = letter_pixel_start + count;

            let target_pixels = &mut canvas_buf[canvas_pixel_start..canvas_pixel_end];
            let letter_row = &glyph_pass[letter_pixel_start..letter_pixel_end];

            for (pixel, glyph) in target_pixels.chunks_exact_mut(4).zip(letter_row.iter()) {
                // skip fully transparent pixels to save time.
                if glyph.fg_a == 0 {
                    continue;
                }

                // if the pixel is fully opaque white, we just copy it directly.
                if pass_kind == PassKind::Foreground && glyph.fg_a == 255 {
                    pixel.copy_from_slice(&[255, 255, 255, 255]);
                    continue;
                }

                if pass_kind != PassKind::Foreground {
                    let inv_a = u32::from(glyph.inv_a);
                    let fg_a = u32::from(glyph.fg_a);

                    // blend the shadow color with the background.
                    let scale = |bg: u8| -> u8 {
                        let t = u32::from(bg) * inv_a + 128;
                        ((t + (t >> 8)) >> 8) as u8
                    };

                    pixel[0] = scale(pixel[0]);
                    pixel[1] = scale(pixel[1]);
                    pixel[2] = scale(pixel[2]);
                    let t_alpha = u32::from(pixel[3]) * inv_a + 128;
                    pixel[3] = (fg_a + ((t_alpha + (t_alpha >> 8)) >> 8)) as u8;
                    continue;
                }

                let fg_rgb = u32::from(glyph.fg_rgb);
                let fg_a = u32::from(glyph.fg_a);
                let inv_a = u32::from(glyph.inv_a);

                // result = background * (1 - alpha) + foreground * alpha
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
}

/*
=============
measure_print_number

measures how many padding is needed for the card's print numbers.
=============
*/
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
