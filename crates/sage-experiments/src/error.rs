//! Shared error types for experiment binaries.
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExperimentError {
    #[error("IO error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("TOML parse error at {path}: {source}")]
    Toml {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("CSV error at {path}: {source}")]
    Csv {
        path: PathBuf,
        #[source]
        source: csv::Error,
    },

    #[error("Simulation error: {0}")]
    Simulation(String),

    #[error("missing config file: {path}")]
    MissingConfig { path: PathBuf },

    #[error("empty seed list")]
    EmptySeeds,

    #[error("no valid runs produced")]
    NoValidRuns,
}

impl From<std::io::Error> for ExperimentError {
    fn from(err: std::io::Error) -> Self {
        ExperimentError::Io {
            path: std::path::PathBuf::new(),
            source: err,
        }
    }
}

pub type ExperimentResult<T> = Result<T, ExperimentError>;
