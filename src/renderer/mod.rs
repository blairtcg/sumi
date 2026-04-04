pub mod cache;
pub mod encoding;
pub mod error;
pub mod image;

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use bytes::Bytes;
use error::RenderError;
use tokio::{task, time::timeout, try_join};

const TIMEOUT_SECONDS: u64 = 5;

#[derive(Clone)]
pub struct CardRenderer {
    card_cache: Arc<cache::CardCache>,
}

impl CardRenderer {
    pub fn new(cards_directory: String) -> Self {
        Self { card_cache: Arc::new(cache::CardCache::new(cards_directory)) }
    }

    /// creates the final image.
    /// if an image cant render your drop in 5 seconds, Too bad!
    /// in blair-go side your cooldown wont get used. users can just try dropping again.
    /// -- Bantuy check lagi...
    pub async fn render_drop(
        &self,
        left_card_name: &str,
        right_card_name: &str,
        left_print_number: u32,
        right_print_number: u32,
    ) -> Result<Bytes, RenderError> {
        let render_future = async {
            let (left_card, right_card) = try_join!(
                self.card_cache.get_card(left_card_name),
                self.card_cache.get_card(right_card_name)
            )?;

            let queued_at = Instant::now();

            // move the heavy image work to a background thread
            let result = task::spawn_blocking(move || {
                // kill if too long
                if queued_at.elapsed() > Duration::from_secs(TIMEOUT_SECONDS) {
                    return Err(RenderError::Timeout);
                }

                image::create_drop_image(
                    &left_card,
                    &right_card,
                    left_print_number,
                    right_print_number,
                )
            })
            .await
            .map_err(|e| RenderError::Internal(format!("task failed: {}", e)))??;

            Ok(result)
        };

        timeout(Duration::from_secs(TIMEOUT_SECONDS), render_future)
            .await
            .map_err(|_| RenderError::Timeout)?
    }

    pub fn cache_stats() -> (u64, u64, f64) {
        cache::get_stats()
    }
}
