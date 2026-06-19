//! SAGE networking abstraction — transport traits and in-memory channel
//! for deterministic message-passing between validators.
pub mod channel;
pub mod error;
pub mod shared;
pub mod tcp;
pub mod transport;

pub use channel::InMemoryTransport;
pub use error::{NetworkError, NetworkResult};
pub use shared::SharedMemoryTransport;
pub use tcp::{Impairment, TcpTransport};
pub use transport::Transport;
