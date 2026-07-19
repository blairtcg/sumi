use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

use crate::renderer::error::RenderError;

pub enum Error {
    Render { err: RenderError, left: String, right: String },
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, error_msg) = match self {
            Self::Render { err, left, right } => {
                let (status, msg) = match &err {
                    RenderError::CardNotFound(name) => {
                        (StatusCode::NOT_FOUND, format!("card not found: {name}"))
                    }
                    RenderError::Timeout => {
                        (StatusCode::GATEWAY_TIMEOUT, "render timed out".to_string())
                    }
                    _ => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
                };

                if status == StatusCode::GATEWAY_TIMEOUT {
                    tracing::warn!(
                        "render timed out\n      left: {}\n      right: {}",
                        left,
                        right
                    );
                } else {
                    tracing::debug!(
                        "render failed\n      left: {}\n      right: {}\n      reason: {}",
                        left,
                        right,
                        msg
                    );
                }

                (status, msg)
            }
        };

        let json_resp = Json(json!({ "error": error_msg }));
        (status, json_resp).into_response()
    }
}
