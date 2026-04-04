use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use image::{codecs::webp::WebPDecoder, DynamicImage, RgbaImage};
use moka::future::Cache;
use tokio::task;

use crate::renderer::error::RenderError;

// limit of images, moka will auto kick older images
// moka uses LFU algorithm so if by rng a card smh keeps getting dropped then it will protect
// and kick out other less "popular" card that was NATURALLY unlucky to not be chosen by blair.
// -- Buat lebih konteks per kartu sekitar 200kb, kalau RAM kena cap kita turunin aja
// -- Tapi kita sini pake Arcimage::RgbaImage yg belum dikompres, sekitar 3 juta byte
const MAX_CACHE_SIZE_KB: u64 = 1_000_000; // -- 1 GB limit di kilobyte

static CACHE_HITS: AtomicU64 = AtomicU64::new(0);
static CACHE_MISSES: AtomicU64 = AtomicU64::new(0);

#[inline(always)]
pub fn record_hit() {
    CACHE_HITS.fetch_add(1, Ordering::Relaxed);
}

#[inline(always)]
pub fn record_miss() {
    CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
}

#[inline]
pub fn get_stats() -> (u64, u64, f64) {
    let hits = CACHE_HITS.load(Ordering::Relaxed);
    let misses = CACHE_MISSES.load(Ordering::Relaxed);
    let total = hits + misses;
    let hit_rate = if total == 0 { 0.0 } else { (hits as f64 / total as f64) * 100.0 };
    (hits, misses, hit_rate)
}

/// hold cached image and list of file here
pub struct CardCache {
    memory: Cache<Arc<str>, Arc<RgbaImage>>,
    file_index: Arc<HashMap<Arc<str>, PathBuf>>,
}

impl CardCache {
    /// sets up the cache and finds all images
    pub fn new(cards_directory: String) -> Self {
        let cache = Cache::builder()
            .max_capacity(MAX_CACHE_SIZE_KB)
            .weigher(|_key: &Arc<str>, value: &Arc<RgbaImage>| {
                // -- Cache di kilobytes biar bisa gampang ganti ke value gede
                let size_in_bytes = value.as_raw().len();
                (size_in_bytes / 1024).max(1) as u32
            })
            .build();

        let file_index = Self::build_card_list(&cards_directory);
        log::info!("found {} card images on disk", file_index.len());

        let file_index_arc = Arc::new(file_index);

        Self { memory: cache, file_index: file_index_arc }
    }

    /// makes a list of all image files in the folder
    /// we check this list first so we dont waste time looking for missing files.
    fn build_card_list(cards_dir: &str) -> HashMap<Arc<str>, PathBuf> {
        fn find_card(dir: &Path, index: &mut HashMap<Arc<str>, PathBuf>) {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        find_card(&path, index);
                    } else if path.extension().map(|e| e == "webp").unwrap_or(false) {
                        if let Some(name) = path.file_stem() {
                            let name_str = name.to_string_lossy();
                            index.insert(name_str.as_ref().into(), path);
                        }
                    }
                }
            }
        }
        let mut index = HashMap::new();
        find_card(Path::new(cards_dir), &mut index);
        index
    }

    /// gets an image from cache.
    /// -- Sekarang klo gak ada kartu di situ, kita load dari disk. akan lebih lemot.
    pub async fn get_card(&self, name: &str) -> Result<Arc<RgbaImage>, RenderError> {
        if let Some(img) = self.memory.get(name).await {
            record_hit();
            return Ok(img);
        }

        record_miss();

        // a check on whether file exists before trying to read it
        let path = match self.file_index.get(name) {
            Some(p) => p.clone(),
            None => return Err(RenderError::CardNotFound(name.to_string())),
        };

        let name_arc: Arc<str> = name.into();
        let name_owned = name.to_string();

        self.memory
            .try_get_with(name_arc, async move {
                task::spawn_blocking(move || {
                    // read file and decode the image
                    let data = std::fs::read(&path).ok()?;

                    let decoder = WebPDecoder::new(std::io::Cursor::new(data)).ok()?;
                    DynamicImage::from_decoder(decoder)
                        .ok()
                        .map(|img| img.into_rgba8())
                        .map(Arc::new)
                })
                .await
                .unwrap_or(None)
                .ok_or(RenderError::CardNotFound(name_owned))
            })
            .await
            .map_err(|e| e.as_ref().clone())
    }
}
