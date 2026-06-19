use thiserror::Error;

#[derive(Debug, Error)]
pub enum StatsError {
    #[error("empty input")]
    EmptyInput,
    #[error("invalid confidence level: {0}")]
    InvalidConfidence(f64),
    #[error("invalid trials count: {0}")]
    InvalidTrials(u64),
}

pub type StatsResult<T> = Result<T, StatsError>;
