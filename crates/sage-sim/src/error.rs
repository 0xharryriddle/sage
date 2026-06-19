use sage_core::ValidatorId;

pub type SimResult<T> = Result<T, SimError>;

#[derive(Debug, thiserror::Error)]
pub enum SimError {
    #[error("configuration error: {0}")]
    Config(String),
    #[error("unknown validator {0}")]
    UnknownValidator(ValidatorId),
    #[error("core error: {0}")]
    Core(#[from] sage_core::CoreError),
    #[error("consensus error: {0}")]
    Consensus(#[from] sage_consensus::ConsensusError),
    #[error("toml decode error: {0}")]
    TomlDecode(#[from] toml::de::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
