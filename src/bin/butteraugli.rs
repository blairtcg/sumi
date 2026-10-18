use std::{env, path::Path, time::Instant};

use butteraugli::{ButteraugliParams, butteraugli};
use imgref::ImgRef;
use rgb::RGB8;
use sumi::renderer::CardCache;
use webpx::{AlphaFilter, EncoderConfig, ImageHint, Preset, Unstoppable};

const QUALITIES: [f32; 8] = [65.0, 70.0, 75.0, 78.0, 80.0, 82.0, 85.0, 90.0];
const DISCORD_BG: RGB8 = RGB8 { r: 49, g: 51, b: 56 }; // #313338

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

// blend alpha pix biar butteraugli ga ngitung pixel gak ngaruh
#[inline]
fn blend_bg(c: &[u8]) -> RGB8 {
    match c[3] {
        0 => DISCORD_BG,
        255 => RGB8::new(c[0], c[1], c[2]),
        a => {
            let alpha = u32::from(a);
            let inv_a = 255 - alpha;
            let blend = |fg: u8, bg: u8| ((u32::from(fg) * alpha + u32::from(bg) * inv_a + 127) / 255) as u8;
            RGB8::new(
                blend(c[0], DISCORD_BG.r),
                blend(c[1], DISCORD_BG.g),
                blend(c[2], DISCORD_BG.b),
            )
        }
    }
}

fn score(w: usize, h: usize, orig_rgba: &[u8], dist_rgba: &[u8]) -> f64 {
    let orig: Vec<RGB8> = orig_rgba.chunks_exact(4).map(blend_bg).collect();
    let dist: Vec<RGB8> = dist_rgba.chunks_exact(4).map(blend_bg).collect();

    butteraugli(
        ImgRef::new(&orig, w, h),
        ImgRef::new(&dist, w, h),
        &ButteraugliParams::default(),
    )
    .map_or(99.0, |res| res.score)
}

struct Stat {
    q: f32,
    avg_score: f64,
    max_score: f64,
    avg_kb: f64,
    avg_ms: f64,
}

fn find_cards(cards_dir: &Path) -> Vec<String> {
    fn scan(base: &Path, dir: &Path, acc: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return; };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                scan(base, &p, acc);
            } else if p.extension().and_then(|s| s.to_str()).map_or(false, |ext| ext.eq_ignore_ascii_case("webp")) {
                if let Ok(rel) = p.strip_prefix(base) {
                    acc.push(rel.with_extension("").to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }
    let mut cards = Vec::new();
    scan(cards_dir, cards_dir, &mut cards);
    cards
}

#[tokio::main]
async fn main() {
    let cards_dir = env::var("CARDS_DIR").unwrap_or_else(|_| "assets".to_string());
    println!("loading cards from '{cards_dir}'..");

    let cache = match CardCache::new(Path::new(&cards_dir)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to start card cache: {e}\npls set CARDS_DIR");
            std::process::exit(1);
        }
    };

    let names = find_cards(Path::new(&cards_dir));
    if names.is_empty() {
        eprintln!("? no cards found in '{cards_dir}'");
        std::process::exit(1);
    }

    let samples = names.len().min(15);
    println!("testing {samples} cards across {} qualities..\n", QUALITIES.len());

    let mut stats = Vec::with_capacity(QUALITIES.len());

    for &q in &QUALITIES {
        let mut total_score = 0.0;
        let mut max_score = 0.0f64;
        let mut total_bytes = 0;
        let mut total_ms = 0.0;

        for name in &names[..samples] {
            let card = cache.get(name).await.expect("failed to load card");
            let (w, h) = (card.size.width, card.size.height);
            let orig = &card.pixels;

            let start = Instant::now();
            let webp = encode_webp(w, h, orig, q);
            total_ms += start.elapsed().as_secs_f64() * 1000.0;
            total_bytes += webp.len();

            let (dist, _, _) = webpx::decode_rgba(&webp).expect("failed to decode webp");
            let s = score(w as usize, h as usize, orig, &dist);

            total_score += s;
            if s > max_score {
                max_score = s;
            }
        }

        let n = samples as f64;
        stats.push(Stat {
            q,
            avg_score: total_score / n,
            max_score,
            avg_kb: (total_bytes as f64 / n) / 1024.0,
            avg_ms: total_ms / n,
        });
    }

    let baseline_kb = stats
        .iter()
        .find(|s| (s.q - 85.0).abs() < f32::EPSILON)
        .map_or(147.5, |s| s.avg_kb);

    println!("\n  quality    avg score    max score     avg size    savings      encode    rating");
    println!("  {}", "─".repeat(82));

    for s in &stats {
        let savings = (1.0 - (s.avg_kb / baseline_kb)) * 100.0;
        let rating = match s.avg_score {
            score if score <= 2.85 => "elysium",
            score if score <= 3.20 => "great",
            score if score <= 3.50 => "sweet spot",
            score if score <= 3.85 => "moderate loss",
            _ => "heavy loss",
        };

        println!(
            "  Q {:<5.1}   {:>9.3}    {:>9.3}   {:>7.1} KB    {:>+5.1}%    {:>6.2} ms    {rating}",
            s.q, s.avg_score, s.max_score, s.avg_kb, savings, s.avg_ms
        );
    }
}

