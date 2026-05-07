#[derive(Debug, Clone)]
pub enum RenderError {
    CardNotFound(String),
    Timeout,
    Internal(String),
    EncodeError(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CardNotFound(s) => write!(f, "card img not found: {}", s),
            Self::Timeout => write!(f, "render timed out"),
            Self::Internal(s) => write!(f, "internal error: {}", s),
            Self::EncodeError(s) => write!(f, "encoding error: {}", s),
        }
    }
}

impl std::error::Error for RenderError {}
