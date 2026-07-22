use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageBytes(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderDurationMs(pub u64);

#[derive(Default, Debug)]
pub struct Metrics {
    successful_renders: AtomicU64,
    failed_renders: AtomicU64,
    total_image_bytes: AtomicU64,
    total_render_time_ms: AtomicU64,
}

impl Metrics {
    #[inline]
    pub fn record_success(&self, bytes: ImageBytes, render_time: RenderDurationMs) {
        self.successful_renders.fetch_add(1, Ordering::Relaxed);
        self.total_image_bytes.fetch_add(bytes.0, Ordering::Relaxed);
        self.total_render_time_ms.fetch_add(render_time.0, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_failure(&self) {
        self.failed_renders.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn successful_renders(&self) -> u64 {
        self.successful_renders.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn failed_renders(&self) -> u64 {
        self.failed_renders.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn total_image_bytes(&self) -> u64 {
        self.total_image_bytes.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn total_render_time_ms(&self) -> u64 {
        self.total_render_time_ms.load(Ordering::Relaxed)
    }
}
