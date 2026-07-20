use std::fmt::{Display, Formatter, Result as FmtResult};

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

use crate::renderer::error::RenderError;

#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    Render { err: RenderError, left: String, right: String },
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Render { err, left, right } => {
                write!(f, "render error (left: {left}, right: {right}): {err}")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Render { err, .. } => Some(err),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

impl IntoResponse for Error {
    #[cold]
    #[inline(never)]
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
