pub mod cache;
pub mod canvas;
pub mod encoder;
pub mod error;
pub mod pixels;
pub mod print;

use std::{
    num::NonZero,
    path::Path,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use bytes::Bytes;
use cache::CardCache;
use canvas::create_drop_image;
use error::RenderError;
use print::init_font;
use tokio::{sync::Semaphore, task, time::timeout, try_join};

use crate::stats::AppStats;

const TIMEOUT_SECONDS: u64 = 10;

#[derive(Debug)]
pub struct CardRenderer {
    pub card_cache: CardCache,
    pub start_time: Instant,
    pub stats: AppStats,
    cpu_semaphore: Arc<Semaphore>,
    total_permits: usize,
}

impl CardRenderer {
    pub fn new(cards_directory: impl AsRef<Path>) -> Result<Self, RenderError> {
        let cores = thread::available_parallelism().map_or(4, NonZero::get);
        tracing::info!("sumi woke up with [{cores} cpu cores]");
        init_font();

        Ok(Self {
            card_cache: CardCache::new(cards_directory.as_ref())?,
            start_time: Instant::now(),
            stats: AppStats::default(),
            cpu_semaphore: Arc::new(Semaphore::new(cores)),
            total_permits: cores,
        })
    }

    // wait for all background workers to finish
    pub async fn wait_for_tasks_to_finish(&self) {
        let active_tasks =
            self.total_permits.saturating_sub(self.cpu_semaphore.available_permits());
        if active_tasks > 0 {
            tracing::info!("finishing {} active tasks..", active_tasks);
        }
        let _ = self.cpu_semaphore.acquire_many(self.total_permits as u32).await;
        tracing::info!("all active tasks finished!");
    }

    // creates the final image.
    // if an image cant render your drop in 5 seconds, Too bad!
    // in blair-go side your cooldown wont get used. users can just try dropping again.
    pub async fn render_drop(
        &self,
        left_card_name: &str,
        right_card_name: &str,
        left_print_number: u32,
        right_print_number: u32,
    ) -> Result<Bytes, RenderError> {
        let render_future = async {
            let start_fetch = Instant::now();
            let (left_card, right_card) = try_join!(
                self.card_cache.get(left_card_name),
                self.card_cache.get(right_card_name)
            )?;
            let fetch_elapsed = start_fetch.elapsed();
            tracing::debug!("fetching cards took {:.3}ms", fetch_elapsed.as_secs_f64() * 1000.0);

            // need to acquire an owned permit so we can move it inside the blocking thread.
            // if the request times out, the thread still holds the permit until it finishes
            let permit = self
                .cpu_semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| RenderError::Internal("cpu semaphore died".to_string()))?;

            // move the heavy image work to a background thread
            let result = task::spawn_blocking(move || {
                let _lock = permit;
                create_drop_image(&left_card, &right_card, left_print_number, right_print_number)
            })
            .await
            .map_err(|e| RenderError::Internal(format!("task failed: {e}")))??;

            Ok(result)
        };

        timeout(Duration::from_secs(TIMEOUT_SECONDS), render_future)
            .await
            .map_err(|_| RenderError::Timeout)?
    }
}
