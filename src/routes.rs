use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, LazyLock,
    },
    time::Instant,
};

use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;

use crate::renderer::{error::RenderError, CardRenderer};

// the data we expect when blair asks for an image.
// we need character name from its filename and also print nums
#[derive(Debug, Deserialize)]
pub struct RenderRequest {
    pub left: String,
    pub right: String,
    pub left_print: Option<u32>,
    pub right_print: Option<u32>,
}

static REQUEST_STATS: RequestStats = RequestStats::new();

static START_TIME: LazyLock<Instant> = LazyLock::new(Instant::now);

/// simple counter to keep track of how sumi is doing.
#[repr(align(64))]
#[derive(Default)]
struct RequestStats {
    total_requests: AtomicU64,
    failed_requests: AtomicU64,
    total_bytes: AtomicU64,
    total_time_ns: AtomicU64,
}

impl RequestStats {
    // this will allow static inits
    const fn new() -> Self {
        Self {
            total_requests: AtomicU64::new(0),
            failed_requests: AtomicU64::new(0),
            total_bytes: AtomicU64::new(0),
            total_time_ns: AtomicU64::new(0),
        }
    }

    /// saves details of a single request after it finishes
    /// this updates our running totals safely across multiple threads.
    #[inline(always)]
    fn record(&self, time_taken_ns: u64, bytes_sent: usize, did_fail: bool) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.total_time_ns.fetch_add(time_taken_ns, Ordering::Relaxed);
        self.total_bytes.fetch_add(bytes_sent as u64, Ordering::Relaxed);
        if did_fail {
            self.failed_requests.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// this is the main endpoint that handles requests to make our drop image.
/// it takes the left and right card details and then ask sumi to combine them,
/// and returns the drop image back to blair to the player.
pub async fn handle_render_drop(
    State(renderer): State<Arc<CardRenderer>>,
    Query(request): Query<RenderRequest>,
) -> impl IntoResponse {
    // start a timer so we know how long this request takes
    let start = Instant::now();
    let left_print = request.left_print.unwrap_or(1);
    let right_print = request.right_print.unwrap_or(1);

    match renderer.render_drop(&request.left, &request.right, left_print, right_print).await {
        Ok(image_data) => {
            // if the image was created successfully, save the stats and send it back!
            let elapsed = start.elapsed();
            let bytes_sent = image_data.len();
            REQUEST_STATS.record(elapsed.as_nanos() as u64, bytes_sent, false);
            log::info!(
                "rendered: {}/{} ({:.3}ms)",
                request.left,
                request.right,
                elapsed.as_secs_f64() * 1000.0
            );
            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, "image/webp"),
                    // -- This is unneeded -- well well well
                    (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
                ],
                image_data,
            )
                .into_response()
        }
        Err(error) => {
            // some things can happen but in theory sumi probably reached max timeout limit
            // on blair-go side we simply hardcode that sumi is busy for any error.
            // players can just retry the drop command again as it wont use up their drop cd.
            let elapsed = start.elapsed();
            REQUEST_STATS.record(elapsed.as_nanos() as u64, 0, true);

            let (status, error_msg) = match error {
                RenderError::CardNotFound(name) => {
                    (StatusCode::NOT_FOUND, format!("card not found: {}", name))
                }
                RenderError::Timeout => {
                    (StatusCode::GATEWAY_TIMEOUT, "render timed out".to_string())
                }
                _ => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
            };

            if status == StatusCode::GATEWAY_TIMEOUT {
                log::warn!("timeout: {}/{}", request.left, request.right);
            } else {
                log::error!("failed: {}/{} - {}", request.left, request.right, error_msg);
            }

            let json = Json(json!({ "error": error_msg }));
            (status, json).into_response()
        }
    }
}

/// an endpoint for sumi stats and whether sumi died or not
pub async fn handle_metrics() -> impl IntoResponse {
    let (cache_hits, cache_misses, cache_hit_rate) = CardRenderer::cache_stats();

    let total = REQUEST_STATS.total_requests.load(Ordering::Relaxed);
    let errors = REQUEST_STATS.failed_requests.load(Ordering::Relaxed);
    let bytes = REQUEST_STATS.total_bytes.load(Ordering::Relaxed);
    let time_ns = REQUEST_STATS.total_time_ns.load(Ordering::Relaxed);

    let avg_ms = if total > 0 { (time_ns as f64 / total as f64) / 1_000_000.0 } else { 0.0 };
    let error_rate = if total > 0 { (errors as f64 / total as f64) * 100.0 } else { 0.0 };

    let uptime = START_TIME.elapsed().as_secs();
    let requests_per_second = if uptime > 0 { total as f64 / uptime as f64 } else { 0.0 };

    let json = Json(json!({
        "service": { "name": "sumi", "version": "1.0.0", "uptime_seconds": uptime },
        "cache": { "hits": cache_hits, "misses": cache_misses, "hit_rate_percent": cache_hit_rate },
        "requests": { "total": total, "errors": errors, "error_rate_percent": error_rate, "avg_ms": avg_ms, "bytes": bytes, "rps": requests_per_second }
    }));

    (StatusCode::OK, json).into_response()
}
