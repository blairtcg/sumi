use std::{
    fmt::{Debug, Formatter, Result as FmtResult},
    fs,
    path::Path,
    sync::Arc,
};

use ahash::{HashMap, RandomState};

use crate::renderer::{
    error::{RenderError, Result},
    pixels::{RawCardImage, Size},
};

// a tiny lazy card index
// we have decided to just disable our cache.
// we will likely make use of our previous moka implementation
// and use the LRU cache system for both binfo 
// and potentially bdrop (though it gets vlittle value from LRU)
pub struct CardCache {
    file_index: Arc<HashMap<Arc<str>, Arc<Path>, RandomState>>,
}

impl Debug for CardCache {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("CardCache")
            .field("file_index_len", &self.file_index.len())
            .finish_non_exhaustive()
    }
}

// helper to read and decode a webp card from disk.
fn decode_card_from_disk(path: &Path) -> Result<(Arc<RawCardImage>, u64)> {
    let file_bytes = fs::read(path).map_err(|e| {
        RenderError::Internal(format!("failed to open file '{}': {e}", path.display()))
    })?;

    if !file_bytes.starts_with(b"RIFF") || file_bytes.get(8..12) != Some(b"WEBP") {
        tracing::warn!("rejected '{}' (only webp supported)", path.display());
        return Err(RenderError::Internal(format!("'{}' is not a webp", path.display())));
    }

    let file_len = u64::try_from(file_bytes.len()).unwrap_or(u64::MAX);

    let (mut pixels, width, height) = webpx::decode_rgba(&file_bytes).map_err(|e| {
        RenderError::Internal(format!("failed to decode webp for '{}': {e:?}", path.display()))
    })?;

    // if the pixel is fully transparent, zero out the rgb
    // webpx compresses transparent borders into runs of zeroes for free
    for px in pixels.chunks_exact_mut(4) {
        if px[3] == 0 {
            px[0] = 0;
            px[1] = 0;
            px[2] = 0;
        }
    }

    if width != 725 || height != 1040 {
        tracing::warn!(
            "card '{}' dimension is {}x{} (expected 725x1040)",
            path.display(),
            width,
            height
        );
    }

    let image = RawCardImage { size: Size::new(width, height), pixels: pixels.into_boxed_slice() };

    Ok((Arc::new(image), file_len))
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

        Ok(Self { file_index: Arc::new(file_index) })
    }

    // makes a list of all image files in the folder
    // we check this list first so we dont waste time looking for missing files.
    fn build_card_list(cards_dir: &Path) -> HashMap<Arc<str>, Arc<Path>, RandomState> {
        fn find_card(
            base_dir: &Path,
            dir: &Path,
            index: &mut HashMap<Arc<str>, Arc<Path>, RandomState>,
        ) {
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
                } else if ext.eq_ignore_ascii_case("png")
                    || ext.eq_ignore_ascii_case("jpg")
                    || ext.eq_ignore_ascii_case("jpeg")
                {
                    tracing::warn!("ignored '{}' (only webp supported)", path.display());
                }
            }
        }

        let mut index = HashMap::default();
        find_card(cards_dir, cards_dir, &mut index);
        index.shrink_to_fit();
        index
    }

    // prewarm is now intentionally disabled!
    // blair's drop cmd is random enough that freezing a huge decoded cache
    // costs more than it gives back esp since its just a static cache hit rate
    pub fn start_prewarm(&self) {
        tracing::info!("! using lazy decode");
    }

    // gets decoded card image from cache memory map.
    // miss loads from disk directly
    pub async fn get(&self, name: &str) -> Result<Arc<RawCardImage>> {
        let path = self
            .file_index
            .get(name)
            .cloned()
            .ok_or_else(|| RenderError::CardNotFound(name.to_string()))?;

        let (arc_img, _) = tokio::task::spawn_blocking(move || decode_card_from_disk(&path))
            .await
            .map_err(|e| RenderError::Internal(format!("task panicked: {e}")))??;

        Ok(arc_img)
    }
}
