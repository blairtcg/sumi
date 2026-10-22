mod cache;
mod canvas;
mod encoder;
mod error;
mod pixels;
mod print;

use std::{
    num::NonZero,
    path::Path,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use bytes::Bytes;
pub use cache::CardCache;
use canvas::create_drop_image;
pub use error::{RenderError, Result};
use print::init_font;
use tokio::{
    sync::Semaphore,
    task,
    time::{Instant as TokioInstant, timeout, timeout_at},
    try_join,
};

use crate::metrics::Metrics;

const TIMEOUT_SECONDS: u64 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrintNumber(u16);

impl PrintNumber {
    #[inline]
    pub fn new(val: u32) -> Self {
        Self(val.clamp(1, 999) as u16)
    }

    #[inline]
    pub const fn value(self) -> u16 {
        self.0
    }
}

#[derive(Debug)]
pub struct CardRenderer {
    pub card_cache: CardCache,
    pub start_time: Instant,
    pub stats: Metrics,
    cpu_semaphore: Arc<Semaphore>,
    total_permits: usize,
}

impl CardRenderer {
    pub fn new(cards_directory: impl AsRef<Path>) -> Result<Self> {
        let cores = thread::available_parallelism().map_or(4, NonZero::get);
        tracing::info!("sumi woke up with [{cores} cpu cores]");
        init_font();

        Ok(Self {
            card_cache: CardCache::new(cards_directory.as_ref())?,
            start_time: Instant::now(),
            stats: Metrics::default(),
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
    // if we cant render your drop in 5 seconds, Too bad!
    // in blair-go side your cooldown wont get used. users can just try dropping again.
    #[tracing::instrument(skip(self), err)]
    pub async fn render_drop(
        &self,
        left_card_name: &str,
        right_card_name: &str,
        left_print_number: PrintNumber,
        right_print_number: PrintNumber,
    ) -> Result<Bytes> {
        let deadline = TokioInstant::now() + Duration::from_secs(TIMEOUT_SECONDS);

        let permit = timeout_at(deadline, self.cpu_semaphore.clone().acquire_owned())
            .await
            .map_err(|_| RenderError::Timeout)?
            .map_err(|_| RenderError::Internal("cpu semaphore died".to_string()))?;

        let remaining = deadline.saturating_duration_since(TokioInstant::now());

        let (left_card, right_card) = timeout(remaining, async {
            try_join!(self.card_cache.get(left_card_name), self.card_cache.get(right_card_name))
        })
        .await
        .map_err(|_| RenderError::Timeout)??;

        let remaining = deadline.saturating_duration_since(TokioInstant::now());

        let result = timeout(
            remaining,
            task::spawn_blocking({
                let permit = permit;
                move || {
                    let _lock = permit;
                    create_drop_image(
                        &left_card,
                        &right_card,
                        left_print_number,
                        right_print_number,
                    )
                }
            }),
        )
        .await
        .map_err(|_| RenderError::Timeout)?
        .map_err(|e| RenderError::Internal(format!("task failed: {e}")))??;

        Ok(result)
    }
}
