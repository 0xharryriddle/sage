//! Deterministic event-driven simulator for SAGE experiments.

pub mod adversary;
pub mod config;
pub mod error;
pub mod event;
pub mod metrics;
pub mod network;
pub mod simulation;
pub mod strategy;
pub mod validator;
pub mod workload;

pub use config::{AdversaryConfig, SimConfig, SimulationConfig, ValidatorsConfig, WorkloadConfig};
pub use error::{SimError, SimResult};
pub use event::{Event, EventKind};
pub use metrics::{MetricsRecorder, RunMetrics};
pub use network::NetworkModel;
pub use simulation::Simulation;
pub use strategy::StrategyKind;
pub use validator::SimValidator;
pub use workload::WorkloadGenerator;
