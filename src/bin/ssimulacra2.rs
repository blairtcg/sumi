use std::{env, path::Path, sync::LazyLock, time::Instant};

use fast_ssim2::{LinearRgbImage, compute_ssimulacra2};
use rgb::RGB8;
use sumi::renderer::CardCache;
use webpx::{AlphaFilter, EncoderConfig, ImageHint, Preset, Unstoppable};

const QUALITIES: [f32; 8] = [65.0, 70.0, 75.0, 78.0, 80.0, 82.0, 85.0, 90.0];
const DISCORD_BG: RGB8 = RGB8 { r: 49, g: 51, b: 56 }; // #313338

const CARDS: [&str; 2] = ["IMG_8376", "IMG_8377"];

fn encode_webp(w: u32, h: u32, rgba: &[u8], quality: f32) -> Vec<u8> {
    EncoderConfig::new()
        .preset(Preset::Picture)
        .hint(ImageHint::Picture)
        .quality(quality)
        .method(0)
        .alpha_compression(true)
        .alpha_quality(85)
        .alpha_filter(AlphaFilter::None)
        .low_memory(false)
        .pass(1)
        .exact(false)
        .segments(1)
        .encode_rgba(rgba, w, h, Unstoppable)
        .expect("webp encode failed")
}

// blend alpha pix biar ssimulacra2 ga ngitung pixel gak ngaruh
#[inline]
fn blend_bg(c: &[u8]) -> RGB8 {
    match c[3] {
        0 => DISCORD_BG,
        255 => RGB8::new(c[0], c[1], c[2]),
        a => {
            let alpha = u32::from(a);
            let inv_a = 255 - alpha;
            let blend = |fg: u8, bg: u8| {
                ((u32::from(fg) * alpha + u32::from(bg) * inv_a + 127) / 255) as u8
            };
            RGB8::new(
                blend(c[0], DISCORD_BG.r),
                blend(c[1], DISCORD_BG.g),
                blend(c[2], DISCORD_BG.b),
            )
        }
    }
}

static SRGB_LUT: LazyLock<[f32; 256]> = LazyLock::new(|| {
    let mut lut = [0.0f32; 256];
    for (i, entry) in lut.iter_mut().enumerate() {
        let v = i as f32 / 255.0;
        *entry = if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        };
    }
    lut
});

#[inline]
fn srgb_to_linear(b: u8) -> f32 {
    SRGB_LUT[b as usize]
}

fn score(w: u32, h: u32, orig_rgba: &[u8], dist_rgba: &[u8]) -> f64 {
    let orig_linear: Vec<[f32; 3]> = orig_rgba
        .chunks_exact(4)
        .map(blend_bg)
        .map(|c| [srgb_to_linear(c.r), srgb_to_linear(c.g), srgb_to_linear(c.b)])
        .collect();

    let dist_linear: Vec<[f32; 3]> = dist_rgba
        .chunks_exact(4)
        .map(blend_bg)
        .map(|c| [srgb_to_linear(c.r), srgb_to_linear(c.g), srgb_to_linear(c.b)])
        .collect();

    let orig_img = LinearRgbImage::new(orig_linear, w as usize, h as usize);
    let dist_img = LinearRgbImage::new(dist_linear, w as usize, h as usize);

    compute_ssimulacra2(orig_img, dist_img).unwrap_or(0.0)
}

struct Stat {
    q: f32,
    avg_score: f64,
    min_score: f64,
    avg_kb: f64,
    avg_ms: f64,
}

#[tokio::main]
async fn main() {
    let cards_dir = env::var("CARDS_DIR").unwrap_or_else(|_| "assets".to_string());
    println!("loading cards from '{cards_dir}'..");

    let cache = match CardCache::new(Path::new(&cards_dir)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to init card cache: {e}\npls set CARDS_DIR");
            std::process::exit(1);
        }
    };

    println!("testing 2 fixed cards ({:?}) across {} qualities..\n", CARDS, QUALITIES.len());

    let mut stats = Vec::with_capacity(QUALITIES.len());

    for &q in &QUALITIES {
        let mut total_score = 0.0;
        let mut min_score = 100.0f64;
        let mut total_bytes = 0;
        let mut total_ms = 0.0;

        for name in &CARDS {
            let card = cache.get(name).await.unwrap_or_else(|e| panic!("failed to load {name}: {e}"));
            let (w, h) = (card.size.width, card.size.height);
            let orig = &card.pixels;

            let start = Instant::now();
            let webp = encode_webp(w, h, orig, q);
            total_ms += start.elapsed().as_secs_f64() * 1000.0;
            total_bytes += webp.len();

            let (dist, _, _) = webpx::decode_rgba(&webp).expect("failed to decode webp");
            let s = score(w, h, orig, &dist);

            total_score += s;
            if s < min_score {
                min_score = s;
            }
        }

        let n = CARDS.len() as f64;
        stats.push(Stat {
            q,
            avg_score: total_score / n,
            min_score,
            avg_kb: (total_bytes as f64 / n) / 1024.0,
            avg_ms: total_ms / n,
        });
    }

    let baseline_kb = stats
        .iter()
        .find(|s| (s.q - 85.0).abs() < f32::EPSILON)
        .map_or(147.5, |s| s.avg_kb);

    println!("\n  quality    avg score    min score     avg size    savings      encode    rating");
    println!("  {}", "─".repeat(82));

    for s in &stats {
        let savings = (1.0 - (s.avg_kb / baseline_kb)) * 100.0;
        let rating = match s.avg_score {
            score if score >= 90.0 => "visually lossless",
            score if score >= 85.0 => "elysium",
            score if score >= 78.0 => "great",
            score if score >= 70.0 => "sweet spot",
            score if score >= 55.0 => "moderate loss",
            _ => "heavy loss",
        };

        println!(
            "  Q {:<5.1}   {:>9.3}    {:>9.3}   {:>7.1} KB    {:>+5.1}%    {:>6.2} ms    {rating}",
            s.q, s.avg_score, s.min_score, s.avg_kb, savings, s.avg_ms
        );
    }
}
