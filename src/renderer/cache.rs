// due to static rng cache hit rates we will use simple read only cache system
// pas startup kita warming memory sampe deket limit 2 gb terus freeze
// di runtime klo hantam hit kita serve pakai dashmap sama ahash dibanding siphash lebih cepet.
// klo miss kita decode dr disk tapi ga usah masukin memory biar minimal instruksi

use std::{
    fmt::{Debug, Formatter, Result as FmtResult},
    fs,
    num::NonZero,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    thread,
};

use ahash::{HashMap, RandomState};
use dashmap::DashMap;
use tokio::{
    fs as tokio_fs, spawn,
    task::{self, JoinSet},
};
use webpx::Decoder;

use crate::renderer::{
    error::{RenderError, Result},
    pixels::{RawCardImage, Size},
};

const MAX_CACHE_SIZE_KB: usize = 2_000_000; // -- 2 gb limit in kilobyte

// hold cached image and list of file here
pub struct CardCache {
    memory: Arc<DashMap<Arc<str>, Arc<RawCardImage>, RandomState>>,
    file_index: Arc<HashMap<Arc<str>, Arc<Path>>>,
}

impl Debug for CardCache {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("CardCache")
            .field("file_index_len", &self.file_index.len())
            .finish_non_exhaustive() // intentional for hashmap
    }
}

impl CardCache {
    // sets up the cache and finds all webp card images
    pub fn new(cards_directory: impl AsRef<Path>) -> Result<Self> {
        let file_index = Self::build_card_list(cards_directory.as_ref());

        if file_index.is_empty() {
            return Err(RenderError::Internal(
                "no cards found..? pls check or set path".to_string(),
            ));
        }

        Ok(Self {
            memory: Arc::new(DashMap::with_hasher(RandomState::new())),
            file_index: Arc::new(file_index),
        })
    }

    // makes a list of all image files in the folder
    // we check this list first so we dont waste time looking for missing files.
    // this also introduces breaking change as hashmap now saves cards as e.g. genshin/fischl_1
    fn build_card_list(cards_dir: &Path) -> HashMap<Arc<str>, Arc<Path>> {
        fn find_card(base_dir: &Path, dir: &Path, index: &mut HashMap<Arc<str>, Arc<Path>>) {
            let Ok(entries) = fs::read_dir(dir) else {
                return;
            };

            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    find_card(base_dir, &path, index);
                    continue;
                }

                let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
                    continue;
                };

                if ext.eq_ignore_ascii_case("webp") {
                    if let Ok(rel_path) = path.strip_prefix(base_dir) {
                        let key_path = rel_path.with_extension("");
                        let name_str = key_path.to_string_lossy().replace('\\', "/");
                        index.insert(name_str.into(), path.into());
                    }
                } else if matches!(ext.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg") {
                    tracing::warn!("ignored '{}' (only webp supported)", path.display());
                }
            }
        }

        let mut index = HashMap::default();
        find_card(cards_dir, cards_dir, &mut index);
        index.shrink_to_fit();
        index
    }

    // bake the entire cache up to the memory limit
    pub fn start_prewarm(&self) {
        if self.file_index.is_empty() {
            return;
        }

        let file_index_clone = self.file_index.clone();
        let memory = self.memory.clone();

        spawn(async move {
            tracing::info!("baking memory cache (dashmap/ahash)..");
            let warmed = Arc::new(AtomicUsize::new(0));
            let warmed_kb = Arc::new(AtomicU64::new(0));
            let warmed_disk_bytes = Arc::new(AtomicU64::new(0));
            let cores = thread::available_parallelism().map_or(8, NonZero::get);
            let concurrent_tasks = cores * 2;
            let mut join_set = JoinSet::new();

            for (name, path) in file_index_clone.iter() {
                // check if we reached 90% cap before spawning more work
                let current_kb = warmed_kb.load(Ordering::Relaxed);
                if current_kb > ((MAX_CACHE_SIZE_KB as u64) * 9 / 10) {
                    tracing::info!("stopped baking (reached memory cap)");
                    break;
                }

                if join_set.len() >= concurrent_tasks {
                    join_set.join_next().await;
                }

                let name = name.clone();
                let path = path.clone();
                let memory = memory.clone();
                let warmed = warmed.clone();
                let warmed_kb = warmed_kb.clone();
                let warmed_disk_bytes = warmed_disk_bytes.clone();

                join_set.spawn(async move {
                    let Ok(file_bytes) = tokio_fs::read(&path).await else {
                        return;
                    };

                    // check if its webp or not.
                    if !file_bytes.starts_with(b"RIFF") || file_bytes.get(8..12) != Some(b"WEBP") {
                        tracing::warn!("skipped '{}' (only webp supported)", path.display());
                        return;
                    }

                    let file_len = file_bytes.len() as u64;

                    let result = task::spawn_blocking(move || {
                        let decode_res =
                            Decoder::new(&file_bytes).and_then(Decoder::decode_rgba_raw);

                        decode_res.ok().map(|(pixels, width, height)| {
                            Arc::new(RawCardImage {
                                size: Size::new(width, height),
                                pixels: pixels.into_boxed_slice(),
                            })
                        })
                    })
                    .await
                    .unwrap_or(None);

                    if let Some(arc_img) = result {
                        let size_kb = arc_img.pixels.len() / 1024;
                        memory.insert(name, arc_img);
                        warmed.fetch_add(1, Ordering::Relaxed);
                        warmed_kb.fetch_add(size_kb as u64, Ordering::Relaxed);
                        warmed_disk_bytes.fetch_add(file_len, Ordering::Relaxed);
                    }
                });
            }

            while join_set.join_next().await.is_some() {}

            let total_disk_bytes = warmed_disk_bytes.load(Ordering::Relaxed);

            let mb = total_disk_bytes as f64 / 1_048_576.0;

            tracing::info!(
                "finished baking {} cards [{:.2} mb]",
                warmed.load(Ordering::Relaxed),
                mb
            );
        });
    }

    // gets decoded card image from cache memory map.
    // miss baca dari disk langsung (juga ga dimasukin ke mem map, liat line 4)
    pub async fn get(&self, name: &str) -> Result<Arc<RawCardImage>> {
        if let Some(img) = self.memory.get(name) {
            tracing::trace!("cache hit for {}", name);
            return Ok(img.value().clone());
        }

        tracing::trace!("cache miss for {}", name);

        let path = self
            .file_index
            .get(name)
            .cloned()
            .ok_or_else(|| RenderError::CardNotFound(name.to_string()))?;

        let file_bytes = tokio_fs::read(&path).await.map_err(|e| {
            RenderError::Internal(format!("failed to open file '{}': {e}", path.display()))
        })?;

        if !file_bytes.starts_with(b"RIFF") || file_bytes.get(8..12) != Some(b"WEBP") {
            tracing::warn!("rejected '{}' (only webp supported)", path.display());
            return Err(RenderError::Internal(format!("'{}' is not a webp", path.display())));
        }

        let arc_img = task::spawn_blocking(move || {
            let (pixels, width, height) = Decoder::new(&file_bytes)
                .map_err(|e| {
                    RenderError::Internal(format!(
                        "failed to start decoder for '{}': {e:?}",
                        path.display()
                    ))
                })?
                .decode_rgba_raw()
                .map_err(|e| {
                    RenderError::Internal(format!(
                        "failed to decode webp for '{}': {e:?}",
                        path.display()
                    ))
                })?;

            let image =
                RawCardImage { size: Size::new(width, height), pixels: pixels.into_boxed_slice() };
            Ok(Arc::new(image))
        })
        .await
        .map_err(|e| RenderError::Internal(format!("task panicked: {e}")))??;

        Ok(arc_img)
    }
}
