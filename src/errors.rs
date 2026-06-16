use std::error::Error;
use std::fmt::{self, Display};
use std::io;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug)]
pub enum AppError {
    Cli(String),
    Io { context: String, source: io::Error },
    Iso { context: String },
    Manifest { context: String, source: io::Error },
}

impl AppError {
    pub fn io(context: impl Into<String>, source: io::Error) -> Self {
        Self::Io {
            context: context.into(),
            source,
        }
    }

    pub fn iso(context: impl Into<String>) -> Self {
        Self::Iso {
            context: context.into(),
        }
    }

    pub fn manifest(context: impl Into<String>, source: io::Error) -> Self {
        Self::Manifest {
            context: context.into(),
            source,
        }
    }

    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Cli(_) => 2,
            Self::Io { .. } | Self::Iso { .. } | Self::Manifest { .. } => 1,
        }
    }
}

impl Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cli(message) => write!(f, "{message}"),
            Self::Io { context, source } => write!(f, "{context}: {source}"),
            Self::Iso { context } => write!(f, "{context}"),
            Self::Manifest { context, source } => write!(f, "{context}: {source}"),
        }
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } | Self::Manifest { source, .. } => Some(source),
            Self::Cli(_) | Self::Iso { .. } => None,
        }
    }
}
