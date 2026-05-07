use std::{sync::Arc, time::Instant};

use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
};
use serde::Deserialize;

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
            renderer.stats.record(false);
            log::debug!(
                "rendered: {}/{} ({:.3}ms) - {} bytes",
                request.left,
                request.right,
                elapsed.as_secs_f64() * 1000.0,
                bytes_sent
            );
            (StatusCode::OK, [(header::CONTENT_TYPE, "image/webp")], image_data).into_response()
        }
        Err(error) => {
            // some things can happen but in theory sumi probably reached max timeout limit
            // on blair-go side we simply hardcode that sumi is busy for any error.
            // players can just retry the drop command again as it wont use up their drop cd.
            let _elapsed = start.elapsed();
            renderer.stats.record(true);

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
                log::debug!("failed: {}/{} - {}", request.left, request.right, error_msg);
            }

            let json = axum::Json(serde_json::json!({ "error": error_msg }));
            (status, json).into_response()
        }
    }
}

/// an endpoint for sumi stats and whether sumi died or not
pub async fn handle_metrics(State(renderer): State<Arc<CardRenderer>>) -> impl IntoResponse {
    let (cache_hits, cache_misses, cache_hit_rate) = renderer.card_cache.get_stats();

    let total = renderer.stats.total_requests.load(std::sync::atomic::Ordering::Relaxed);
    let errors = renderer.stats.failed_requests.load(std::sync::atomic::Ordering::Relaxed);

    let error_rate = if total > 0 { (errors as f64 / total as f64) * 100.0 } else { 0.0 };

    let uptime = renderer.start_time.elapsed().as_secs();

    // some of the metrics will be removed in next updates as we dont use these anymore except uptime
    let json = axum::Json(serde_json::json!({
        "service": { "name": "sumi", "version": "1.0.0", "uptime_seconds": uptime },
        "cache": { "hits": cache_hits, "misses": cache_misses, "hit_rate_percent": cache_hit_rate },
        "requests": { "total": total, "errors": errors, "error_rate_percent": error_rate }
    }));

    (StatusCode::OK, json).into_response()
}
