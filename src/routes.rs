use std::{sync::Arc, time::Instant};

use axum::{
    Json,
    extract::{Query, State},
    http::{StatusCode, header},
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::json;

use crate::{
    error::{Error, Result},
    metrics::{ImageBytes, RenderDurationMs},
    renderer::{CardRenderer, PrintNumber},
};

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
#[tracing::instrument(skip(renderer), fields(left = %request.left, right = %request.right, left_print = request.left_print.unwrap_or(1), right_print = request.right_print.unwrap_or(1)))]
pub async fn handle_render_drop(
    State(renderer): State<Arc<CardRenderer>>,
    Query(request): Query<RenderRequest>,
) -> Result<impl IntoResponse> {
    let start = Instant::now();
    let left_print = PrintNumber(request.left_print.unwrap_or(1));
    let right_print = PrintNumber(request.right_print.unwrap_or(1));

    let image_data =
        match renderer.render_drop(&request.left, &request.right, left_print, right_print).await {
            Ok(data) => data,
            Err(err) => {
                renderer.stats.record_failure();
                return Err(Error::Render { err, left: request.left, right: request.right });
            }
        };

    let elapsed = start.elapsed();
    let bytes_sent = image_data.len();

    renderer.stats.record_success(
        ImageBytes(bytes_sent as u64),
        RenderDurationMs(elapsed.as_millis() as u64),
    );

    Ok((StatusCode::OK, [(header::CONTENT_TYPE, "image/webp")], image_data).into_response())
}

// an endpoint for sumi stats and whether sumi died or not
#[allow(clippy::unused_async)]
pub async fn handle_metrics(State(renderer): State<Arc<CardRenderer>>) -> impl IntoResponse {
    let uptime = renderer.start_time.elapsed().as_secs();

    let successful = renderer.stats.successful_renders();
    let failed = renderer.stats.failed_renders();
    let total_bytes = renderer.stats.total_image_bytes();
    let total_time_ms = renderer.stats.total_render_time_ms();

    let avg_render_time_ms =
        if successful > 0 { total_time_ms as f64 / successful as f64 } else { 0.0 };

    let avg_file_size_bytes =
        if successful > 0 { total_bytes as f64 / successful as f64 } else { 0.0 };

    let json_resp = Json(json!({
        "service": { "name": "sumi", "version": env!("CARGO_PKG_VERSION"), "uptime_seconds": uptime },
        "stats": {
            "successful_renders": successful,
            "failed_renders": failed,
            "total_image_data_sent_bytes": total_bytes,
            "average_render_time_ms": avg_render_time_ms,
            "average_file_size_bytes": avg_file_size_bytes
        }
    }));

    (StatusCode::OK, json_resp).into_response()
}
