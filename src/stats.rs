use std::sync::atomic::{AtomicU64, Ordering};

use tokio::fs;

#[derive(Default, Debug)]
pub struct AppStats {
    pub successful_renders: AtomicU64,
    pub failed_renders: AtomicU64,
    pub total_image_bytes: AtomicU64,
    pub total_render_time_ms: AtomicU64,
}

impl AppStats {
    pub fn record_success(&self, bytes: u64, render_time_ms: u64) {
        self.successful_renders.fetch_add(1, Ordering::Relaxed);
        self.total_image_bytes.fetch_add(bytes, Ordering::Relaxed);
        self.total_render_time_ms.fetch_add(render_time_ms, Ordering::Relaxed);
    }

    pub fn record_failure(&self) {
        self.failed_renders.fetch_add(1, Ordering::Relaxed);
    }

    pub async fn current_memory_usage_mb() -> f64 {
        if let Ok(statm) = fs::read_to_string("/proc/self/statm").await
            && let Some(rss) = statm.split_whitespace().nth(1)
            && let Ok(pages) = rss.parse::<u64>()
        {
            return (pages * 4096) as f64 / 1_048_576.0;
        }
        0.0
    }
}
