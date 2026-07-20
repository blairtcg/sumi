use std::{
    error::Error,
    fmt::{Display, Formatter, Result as FmtResult},
};

#[derive(Debug, Clone)]
pub enum RenderError {
    CardNotFound(String),
    Timeout,
    Internal(String),
    EncodeError(String),
}

impl Display for RenderError {
    #[cold]
    #[inline(never)]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::CardNotFound(s) => write!(f, "card not found {s}"),
            Self::Timeout => write!(f, "render timed out"),
            Self::Internal(s) => write!(f, "{s}"),
            Self::EncodeError(s) => write!(f, "encoding failed {s}"),
        }
    }
}

impl Error for RenderError {}
