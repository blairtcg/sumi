use std::{
    sync::{Arc, atomic::Ordering},
    time::Instant,
};

use axum::{
    Json,
    extract::{Query, State},
    http::{StatusCode, header},
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::json;

use crate::{error::AppError, renderer::CardRenderer, stats::AppStats};

// the data we expect when blair asks for an image.
// we need character name from its filename and also print nums
#[derive(Debug, Deserialize)]
pub struct RenderRequest {
    pub left: String,
    pub right: String,
    pub left_print: Option<u32>,
    pub right_print: Option<u32>,
}

// this is the main endpoint that handles requests to make our drop image.
// it takes the left and right card details and then ask sumi to combine them,
// and returns the drop image back to blair to the player.
pub async fn handle_render_drop(
    State(renderer): State<Arc<CardRenderer>>,
    Query(request): Query<RenderRequest>,
) -> Result<impl IntoResponse, AppError> {
    // start a timer so we know how long this request takes
    let start = Instant::now();
    let left_print = request.left_print.unwrap_or(1);
    let right_print = request.right_print.unwrap_or(1);

    tracing::debug!(
        "starting render..\n      left: {} (#{})\n      right: {} (#{})",
        request.left,
        left_print,
        request.right,
        right_print
    );

    let image_data =
        match renderer.render_drop(&request.left, &request.right, left_print, right_print).await {
            Ok(data) => data,
            Err(err) => {
                renderer.stats.record_failure();
                return Err(AppError::Render { err, left: request.left, right: request.right });
            }
        };

    // if the image was created successfully, send it back!
    let elapsed = start.elapsed();
    let bytes_sent = image_data.len();

    renderer.stats.record_success(bytes_sent as u64, elapsed.as_millis() as u64);

    tracing::debug!(
        "rendered successfully\n      left: {}\n      right: {}\n      elapsed: {:.3}ms\n      size: {} bytes",
        request.left,
        request.right,
        elapsed.as_secs_f64() * 1000.0,
        bytes_sent
    );

    Ok((StatusCode::OK, [(header::CONTENT_TYPE, "image/webp")], image_data).into_response())
}

// an endpoint for sumi stats and whether sumi died or not
pub async fn handle_metrics(State(renderer): State<Arc<CardRenderer>>) -> impl IntoResponse {
    let uptime = renderer.start_time.elapsed().as_secs();

    let successful = renderer.stats.successful_renders.load(Ordering::Relaxed);
    let failed = renderer.stats.failed_renders.load(Ordering::Relaxed);
    let total_bytes = renderer.stats.total_image_bytes.load(Ordering::Relaxed);
    let total_time_ms = renderer.stats.total_render_time_ms.load(Ordering::Relaxed);

    let avg_render_time_ms =
        if successful > 0 { total_time_ms as f64 / successful as f64 } else { 0.0 };

    let avg_file_size_bytes =
        if successful > 0 { total_bytes as f64 / successful as f64 } else { 0.0 };

    let mem_usage_mb = AppStats::current_memory_usage_mb().await;

    let json_resp = Json(json!({
        "service": { "name": "sumi", "version": "1.3.0", "uptime_seconds": uptime },
        "stats": {
            "successful_renders": successful,
            "failed_renders": failed,
            "total_image_data_sent_bytes": total_bytes,
            "average_render_time_ms": avg_render_time_ms,
            "average_file_size_bytes": avg_file_size_bytes,
            "memory_usage_mb": mem_usage_mb
        }
    }));

    (StatusCode::OK, json_resp).into_response()
}
