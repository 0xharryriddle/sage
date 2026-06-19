//! Real TCP transport over loopback (or any reachable address).
//!
//! Unlike `InMemoryTransport`, this moves consensus messages across real
//! sockets with length-prefixed JSON framing. It is the foundation of the
//! multi-process local testbed: each validator binds a listener, and peers
//! dial it to deliver messages. Inbound messages land in a shared queue that
//! `recv` drains, matching the `Transport` trait contract.
//!
//! Framing: each message is `[u32 big-endian length][JSON bytes]`. This is a
//! deliberately simple, debuggable wire format — not a performance-optimized
//! one. The testbed's job is to exhibit real network behavior (connection
//! setup, reordering across peers, partition via address blocking), not to
//! maximize throughput.

use crate::error::{NetworkError, NetworkResult};
use crate::transport::Transport;
use sage_consensus::MessageEnvelope;
use sage_core::ValidatorId;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

type InboundQueue = Arc<Mutex<VecDeque<MessageEnvelope>>>;

/// In-process software network impairment ("netem-style"), applied per-send.
///
/// This models a lossy, jittery link without any kernel privileges (no `tc
/// qdisc netem`, no `sudo`): the transport itself drops a fraction of messages
/// and delays the rest by a randomized amount before they hit the wire. Because
/// each send's delay is drawn independently, messages can overtake one another,
/// which reproduces real-world reordering on top of loss and latency.
///
/// All randomness is driven by a seeded LCG (see [`TcpTransport::set_impairment`]),
/// so a given seed yields a reproducible loss/delay schedule — important for a
/// research artifact where runs must be explainable.
#[derive(Debug, Clone, Copy)]
pub struct Impairment {
    /// Mean per-message delay before the write hits the socket, in ms.
    pub delay_mean_ms: u64,
    /// Symmetric uniform jitter around the mean: the actual delay is
    /// `delay_mean_ms + uniform(-delay_jitter_ms ..= +delay_jitter_ms)`,
    /// clamped at zero. Independent per send, so it induces reordering.
    pub delay_jitter_ms: u64,
    /// Probability in `[0.0, 1.0]` that a given message is dropped outright
    /// (packet loss). A dropped send returns `Ok(())` without writing.
    pub loss_pct: f64,
    /// Seed for the deterministic LCG that drives loss/jitter decisions.
    pub seed: u64,
}

impl Default for Impairment {
    /// A no-op impairment: no delay, no jitter, no loss. Equivalent to leaving
    /// impairment unset (the transport behaves like a plain reliable link).
    fn default() -> Self {
        Self {
            delay_mean_ms: 0,
            delay_jitter_ms: 0,
            loss_pct: 0.0,
            seed: 0,
        }
    }
}

/// A TCP-backed transport for one validator.
pub struct TcpTransport {
    local_id: ValidatorId,
    peers: BTreeMap<ValidatorId, SocketAddr>,
    inbound: InboundQueue,
    listener_addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    listener_handle: Option<JoinHandle<()>>,
    /// Peers this node currently refuses to dial, modeling a partition.
    blocked: BTreeSet<ValidatorId>,
    /// Active software impairment, if any. `None` preserves the original
    /// reliable, synchronous send path (used by every existing test).
    impairment: Option<Impairment>,
    /// Mutable LCG state advanced once per loss/jitter draw. Seeded from
    /// `Impairment::seed` in `set_impairment`.
    rng_state: u64,
    /// Count of messages this transport actually wrote to the wire (a real
    /// socket send that was neither partition-blocked nor impairment-dropped).
    /// A `broadcast` to a quorum increments this once per delivered peer, so the
    /// per-validator total scales with the protocol's per-round fan-out. The
    /// orchestrator sums these to plot empirical O(n^2) message complexity.
    sent_count: u64,
}

impl TcpTransport {
    /// Bind a listener for `local_id` and start accepting inbound frames.
    /// `peers` maps every validator (including self) to its dial address.
    pub fn bind(
        local_id: ValidatorId,
        peers: BTreeMap<ValidatorId, SocketAddr>,
    ) -> NetworkResult<Self> {
        let own = *peers
            .get(&local_id)
            .ok_or(NetworkError::UnknownPeer(local_id))?;
        let listener = TcpListener::bind(own).map_err(|e| NetworkError::Io(e.to_string()))?;
        let listener_addr = listener
            .local_addr()
            .map_err(|e| NetworkError::Io(e.to_string()))?;
        listener
            .set_nonblocking(true)
            .map_err(|e| NetworkError::Io(e.to_string()))?;

        let inbound: InboundQueue = Arc::new(Mutex::new(VecDeque::new()));
        let shutdown = Arc::new(AtomicBool::new(false));

        let inbound_for_thread = Arc::clone(&inbound);
        let shutdown_for_thread = Arc::clone(&shutdown);
        let handle = std::thread::spawn(move || {
            accept_loop(listener, inbound_for_thread, shutdown_for_thread);
        });

        Ok(Self {
            local_id,
            peers,
            inbound,
            listener_addr,
            shutdown,
            listener_handle: Some(handle),
            blocked: BTreeSet::new(),
            impairment: None,
            rng_state: 0,
            sent_count: 0,
        })
    }

    /// Address this transport is actually listening on (useful when the caller
    /// binds to port 0 and lets the OS pick a free port).
    pub fn local_addr(&self) -> SocketAddr {
        self.listener_addr
    }

    /// Total messages this transport has actually written to the wire (sends
    /// that were neither partition-blocked nor impairment-dropped). The testbed
    /// sums this across validators to plot empirical message complexity vs n.
    pub fn sent_count(&self) -> u64 {
        self.sent_count
    }

    /// Block (drop) all sends to `peer`, modeling one side of a partition.
    pub fn block_peer(&mut self, peer: ValidatorId) {
        self.blocked.insert(peer);
    }

    /// Restore connectivity to `peer` (partition healing).
    pub fn unblock_peer(&mut self, peer: ValidatorId) {
        self.blocked.remove(&peer);
    }

    /// Engage a software network impairment (loss + delay + jitter) on this
    /// transport's send path, modeling WAN conditions without kernel `tc netem`.
    /// Passing `Impairment::default()` (or clearing) restores a reliable link.
    pub fn set_impairment(&mut self, imp: Impairment) {
        self.rng_state = imp.seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        self.impairment = Some(imp);
    }

    /// Clear any active impairment, restoring the reliable synchronous path.
    pub fn clear_impairment(&mut self) {
        self.impairment = None;
    }

    /// Advance the seeded LCG and return a uniform `f64` in `[0.0, 1.0)`.
    /// Deterministic given the seed, so an impaired run is fully reproducible.
    fn next_unit(&mut self) -> f64 {
        // Numerical Recipes LCG constants.
        self.rng_state = self
            .rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        // Top 53 bits -> [0,1).
        ((self.rng_state >> 11) as f64) / ((1u64 << 53) as f64)
    }

    /// Compute the (loss?, delay_ms) decision for one send under the active
    /// impairment, advancing the RNG. Returns `(true, _)` if the message should
    /// be dropped. With no impairment set, never drops and adds no delay.
    fn impair_decision(&mut self) -> (bool, u64) {
        let imp = match self.impairment {
            Some(imp) => imp,
            None => return (false, 0),
        };
        if imp.loss_pct > 0.0 && self.next_unit() < imp.loss_pct {
            return (true, 0);
        }
        let delay = if imp.delay_jitter_ms == 0 {
            imp.delay_mean_ms
        } else {
            // Uniform in [-jitter, +jitter] added to the mean, clamped at 0.
            let span = (2 * imp.delay_jitter_ms + 1) as f64;
            let offset = (self.next_unit() * span) as i64 - imp.delay_jitter_ms as i64;
            (imp.delay_mean_ms as i64 + offset).max(0) as u64
        };
        (false, delay)
    }

    fn dial_and_write(&self, to: ValidatorId, bytes: &[u8]) -> NetworkResult<()> {
        let addr = self.peers.get(&to).ok_or(NetworkError::UnknownPeer(to))?;
        let mut stream = TcpStream::connect_timeout(addr, Duration::from_millis(500))
            .map_err(|e| NetworkError::Io(e.to_string()))?;
        stream
            .set_write_timeout(Some(Duration::from_millis(500)))
            .map_err(|e| NetworkError::Io(e.to_string()))?;
        let len = (bytes.len() as u32).to_be_bytes();
        stream
            .write_all(&len)
            .map_err(|e| NetworkError::Io(e.to_string()))?;
        stream
            .write_all(bytes)
            .map_err(|e| NetworkError::Io(e.to_string()))?;
        stream
            .flush()
            .map_err(|e| NetworkError::Io(e.to_string()))?;
        Ok(())
    }
}

impl Drop for TcpTransport {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(handle) = self.listener_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Transport for TcpTransport {
    fn broadcast(&mut self, from: ValidatorId, message: MessageEnvelope) -> NetworkResult<()> {
        let peers: Vec<ValidatorId> = self.peers.keys().copied().collect();
        for id in peers {
            if id != from && id != self.local_id {
                // A blocked peer silently drops, exactly like a partition.
                let _ = self.send(from, id, message.clone());
            }
        }
        Ok(())
    }

    fn send(
        &mut self,
        _from: ValidatorId,
        to: ValidatorId,
        message: MessageEnvelope,
    ) -> NetworkResult<()> {
        if self.blocked.contains(&to) {
            return Ok(());
        }
        // Software impairment: a dropped message returns Ok without writing
        // (packet loss), and a surviving message is delayed before it hits the
        // socket. Independent per-send delays induce reordering across peers.
        let (dropped, delay_ms) = self.impair_decision();
        if dropped {
            return Ok(());
        }
        if delay_ms > 0 {
            std::thread::sleep(Duration::from_millis(delay_ms));
        }
        let bytes = serde_json::to_vec(&message).map_err(|e| NetworkError::Serde(e.to_string()))?;
        let res = self.dial_and_write(to, &bytes);
        if res.is_ok() {
            self.sent_count += 1;
        }
        res
    }

    fn recv(&mut self, _validator: ValidatorId) -> NetworkResult<Vec<MessageEnvelope>> {
        let mut q = self
            .inbound
            .lock()
            .map_err(|_| NetworkError::Io("inbound queue poisoned".to_string()))?;
        Ok(q.drain(..).collect())
    }

    fn peers(&self) -> BTreeSet<ValidatorId> {
        self.peers.keys().copied().collect()
    }

    /// Drop connectivity to every peer not in `keep` (and not self), modeling a
    /// partition where this node can only reach `keep`. Cross-side sends are
    /// silently dropped at `send`, exactly like a kernel-level split.
    fn partition_to(&mut self, keep: &BTreeSet<ValidatorId>) {
        for id in self.peers.keys().copied().collect::<Vec<_>>() {
            if id == self.local_id {
                continue;
            }
            if keep.contains(&id) {
                self.blocked.remove(&id);
            } else {
                self.blocked.insert(id);
            }
        }
    }

    /// Clear all partition state (network healing).
    fn heal(&mut self) {
        self.blocked.clear();
    }

    fn sent_count(&self) -> u64 {
        self.sent_count
    }
}

/// Accept connections and decode length-prefixed JSON frames until shutdown.
fn accept_loop(listener: TcpListener, inbound: InboundQueue, shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _addr)) => {
                if let Some(msg) = read_frame(stream) {
                    if let Ok(mut q) = inbound.lock() {
                        q.push_back(msg);
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(_) => {
                std::thread::sleep(Duration::from_millis(2));
            }
        }
    }
}

/// Read one length-prefixed JSON frame from a freshly accepted stream.
fn read_frame(mut stream: TcpStream) -> Option<MessageEnvelope> {
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .ok()?;
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).ok()?;
    let len = u32::from_be_bytes(len_buf) as usize;
    // Guard against absurd allocations from a malformed/hostile frame.
    if len > 8 * 1024 * 1024 {
        return None;
    }
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).ok()?;
    serde_json::from_slice::<MessageEnvelope>(&buf).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sage_consensus::{ConsensusMessage, HotStuffMessage};
    use sage_core::{
        Block, BlockHash, BlockHeader, ChainId, ConfigId, EngineGeneration, EngineId, EngineKind,
        Epoch, FinalityTier, Height, StateRoot, View,
    };

    fn dummy(from: ValidatorId, to: Option<ValidatorId>) -> MessageEnvelope {
        let block = Block {
            header: BlockHeader {
                chain_id: ChainId::new("test"),
                epoch: Epoch::new(1),
                config_id: ConfigId::new(1),
                height: Height::new(1),
                parent_hash: BlockHash::new([0; 32]),
                state_root: StateRoot::new([0; 32]),
                engine_id: EngineId::new(EngineKind::Poa, EngineGeneration::new(1)),
                finality_tier: FinalityTier::None,
                manifest_hash: None,
            },
            txs: vec![],
        };
        MessageEnvelope {
            from,
            to,
            chain_id: ChainId::new("test"),
            epoch: Epoch::new(1),
            config_id: ConfigId::new(1),
            message: ConsensusMessage::HotStuff(HotStuffMessage::Vote {
                view: View::new(0),
                block,
                block_hash: BlockHash::new([1; 32]),
            }),
        }
    }

    /// Bind two transports on OS-chosen loopback ports, rebind their peer maps
    /// to the actual addresses, and return the live pair.
    fn pair() -> (TcpTransport, TcpTransport) {
        let a = ValidatorId::new(0);
        let b = ValidatorId::new(1);
        let lo = "127.0.0.1:0".parse::<SocketAddr>().unwrap();
        // First bind to learn real ports.
        let ta0 = TcpTransport::bind(a, BTreeMap::from([(a, lo)])).unwrap();
        let tb0 = TcpTransport::bind(b, BTreeMap::from([(b, lo)])).unwrap();
        let addr_a = ta0.local_addr();
        let addr_b = tb0.local_addr();
        drop(ta0);
        drop(tb0);
        // Rebind with the full peer map pointing at the learned ports.
        let peers = BTreeMap::from([(a, addr_a), (b, addr_b)]);
        let ta = TcpTransport::bind(a, peers.clone()).unwrap();
        let tb = TcpTransport::bind(b, peers).unwrap();
        (ta, tb)
    }

    fn poll_recv(t: &mut TcpTransport, who: ValidatorId) -> Vec<MessageEnvelope> {
        for _ in 0..50 {
            let msgs = t.recv(who).unwrap();
            if !msgs.is_empty() {
                return msgs;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Vec::new()
    }

    #[test]
    fn message_crosses_real_socket() {
        let a = ValidatorId::new(0);
        let b = ValidatorId::new(1);
        let (mut ta, mut tb) = pair();
        ta.send(a, b, dummy(a, Some(b))).unwrap();
        let got = poll_recv(&mut tb, b);
        assert_eq!(got.len(), 1, "message should arrive over the socket");
        assert_eq!(got[0].from, a);
    }

    #[test]
    fn blocked_peer_drops_silently() {
        let a = ValidatorId::new(0);
        let b = ValidatorId::new(1);
        let (mut ta, mut tb) = pair();
        ta.block_peer(b);
        // send returns Ok but nothing is delivered (partition).
        ta.send(a, b, dummy(a, Some(b))).unwrap();
        let got = poll_recv(&mut tb, b);
        assert!(got.is_empty(), "blocked peer must not receive the message");
        // Healing restores delivery.
        ta.unblock_peer(b);
        ta.send(a, b, dummy(a, Some(b))).unwrap();
        let got2 = poll_recv(&mut tb, b);
        assert_eq!(got2.len(), 1, "healed peer should receive again");
    }

    #[test]
    fn broadcast_reaches_other_peer() {
        let a = ValidatorId::new(0);
        let b = ValidatorId::new(1);
        let (mut ta, mut tb) = pair();
        ta.broadcast(a, dummy(a, None)).unwrap();
        let got = poll_recv(&mut tb, b);
        assert_eq!(got.len(), 1, "broadcast should reach the peer");
    }

    #[test]
    fn impairment_full_loss_drops_every_message() {
        let a = ValidatorId::new(0);
        let b = ValidatorId::new(1);
        let (mut ta, mut tb) = pair();
        // 100% loss: nothing should ever arrive, but send still returns Ok.
        ta.set_impairment(Impairment {
            delay_mean_ms: 0,
            delay_jitter_ms: 0,
            loss_pct: 1.0,
            seed: 7,
        });
        for _ in 0..5 {
            ta.send(a, b, dummy(a, Some(b))).unwrap();
        }
        let got = poll_recv(&mut tb, b);
        assert!(
            got.is_empty(),
            "full-loss impairment must drop all messages"
        );
    }

    #[test]
    fn impairment_loss_is_seed_deterministic() {
        // Same seed + same loss probability => identical drop schedule, so a
        // research run is reproducible. We compare the drop decisions of two
        // freshly-seeded transports over a fixed number of draws.
        let a = ValidatorId::new(0);
        let mk = || {
            let lo = "127.0.0.1:0".parse::<SocketAddr>().unwrap();
            let mut t = TcpTransport::bind(a, BTreeMap::from([(a, lo)])).unwrap();
            t.set_impairment(Impairment {
                delay_mean_ms: 0,
                delay_jitter_ms: 0,
                loss_pct: 0.5,
                seed: 42,
            });
            t
        };
        let mut t1 = mk();
        let mut t2 = mk();
        let s1: Vec<bool> = (0..32).map(|_| t1.impair_decision().0).collect();
        let s2: Vec<bool> = (0..32).map(|_| t2.impair_decision().0).collect();
        assert_eq!(s1, s2, "same seed must yield identical loss schedule");
        assert!(s1.iter().any(|&d| d), "0.5 loss should drop some");
        assert!(s1.iter().any(|&d| !d), "0.5 loss should keep some");
    }

    #[test]
    fn impairment_clear_restores_reliable_link() {
        let a = ValidatorId::new(0);
        let b = ValidatorId::new(1);
        let (mut ta, mut tb) = pair();
        ta.set_impairment(Impairment {
            delay_mean_ms: 0,
            delay_jitter_ms: 0,
            loss_pct: 1.0,
            seed: 1,
        });
        ta.clear_impairment();
        ta.send(a, b, dummy(a, Some(b))).unwrap();
        let got = poll_recv(&mut tb, b);
        assert_eq!(got.len(), 1, "cleared impairment must deliver again");
    }
}
