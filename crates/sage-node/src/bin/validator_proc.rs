//! Single-validator process over real TCP (testbed milestone M1).
//!
//! One OS process = one validator. It binds a real `TcpTransport` listener at
//! its own address, dials peers over loopback to deliver consensus messages,
//! drives a `ProcessValidator` to `max_height` (or timeout), and prints a
//! `ProcessResult` as a single JSON line on stdout. The `spawn_testbed`
//! orchestrator launches N of these and parses one line per process.
//!
//! Unlike `run_node` (one process, many in-memory nodes), this is the real
//! distributed topology: no shared node memory, every message crosses a socket.
//!
//! Peer addresses are passed explicitly so the orchestrator controls port
//! assignment and every process agrees on the same peer map.
//!
//! Example (normally invoked by the orchestrator, not by hand):
//!   validator_proc --id 0 --n 4 --f 1 \
//!     --peers 127.0.0.1:7000,127.0.0.1:7001,127.0.0.1:7002,127.0.0.1:7003 \
//!     --max-height 8 --h-d 2 --h-c 4 --h-r 8 --strategy sage

use sage_controller::Schedule;
use sage_core::{Height, ValidatorId};
use sage_network::{Impairment, TcpTransport};
use sage_node::config::{NodeConfig, NodeStrategy};
use sage_node::ProcessValidator;
use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }
    let opts = parse_opts(&args);

    let id_num = opt_u64(&opts, "id", 0) as u32;
    let n = opt_u64(&opts, "n", 4) as u32;
    let f = opt_u64(&opts, "f", 1) as u32;
    let max_height = Height::new(opt_u64(&opts, "max-height", 8));
    let h_d = opt_u64(&opts, "h-d", 2);
    let h_c = opt_u64(&opts, "h-c", 4);
    let h_r = opt_u64(&opts, "h-r", 8);
    let kappa = opt_u64(&opts, "kappa", 1);
    let tau = opt_u64(&opts, "tau", 1);
    let max_secs = opt_u64(&opts, "max-secs", 20);
    let seed = opt_u64(&opts, "seed", 42);

    let strategy = match opts.get("strategy").map(|s| s.as_str()) {
        None | Some("sage") => NodeStrategy::Sage,
        Some("hardfork") => NodeStrategy::HardFork,
        Some("sageblind") | Some("blind") => NodeStrategy::SageBlind,
        Some("stoptheworld") | Some("stw") => NodeStrategy::StopTheWorld,
        Some("reconfig") | Some("reconfigonly") => NodeStrategy::ReconfigOnly,
        Some("coxstyle") | Some("cox") => NodeStrategy::CoxStyle,
        Some("coxfaithful") | Some("cox-faithful") => NodeStrategy::CoxFaithful,
        Some(other) => {
            eprintln!("unknown strategy '{other}'");
            std::process::exit(2);
        }
    };

    // Parse the peer address list: index i is validator i's dial address.
    let peer_str = match opts.get("peers") {
        Some(s) if !s.is_empty() => s.clone(),
        _ => {
            eprintln!("--peers is required (comma-separated addr list, index = validator id)");
            std::process::exit(2);
        }
    };
    let addrs: Vec<SocketAddr> = peer_str
        .split(',')
        .map(|s| {
            s.trim().parse::<SocketAddr>().unwrap_or_else(|e| {
                eprintln!("bad peer addr '{s}': {e}");
                std::process::exit(2);
            })
        })
        .collect();
    if addrs.len() != n as usize {
        eprintln!(
            "peer count {} does not match n {} (one addr per validator required)",
            addrs.len(),
            n
        );
        std::process::exit(2);
    }

    let peers: BTreeMap<ValidatorId, SocketAddr> = addrs
        .iter()
        .enumerate()
        .map(|(i, a)| (ValidatorId::new(i as u32), *a))
        .collect();

    let id = ValidatorId::new(id_num);
    let mut transport = match TcpTransport::bind(id, peers) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("validator {id_num} failed to bind transport: {e}");
            std::process::exit(1);
        }
    };

    // Optional WAN impairment (netem-style, no kernel privileges): per-send loss
    // and randomized delay+jitter, modeling a lossy/jittery wide-area link on top
    // of loopback. Seeded per-validator (base seed + id) so the schedule is
    // reproducible yet not identical across nodes. Unset => reliable link.
    let loss_pct = opts
        .get("loss-pct")
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0);
    let delay_mean_ms = opt_u64(&opts, "delay-ms", 0);
    let delay_jitter_ms = opt_u64(&opts, "jitter-ms", 0);
    if loss_pct > 0.0 || delay_mean_ms > 0 || delay_jitter_ms > 0 {
        transport.set_impairment(Impairment {
            delay_mean_ms,
            delay_jitter_ms,
            loss_pct,
            seed: seed.wrapping_add(id_num as u64),
        });
        eprintln!(
            "validator {id_num} impairment: loss={loss_pct} delay={delay_mean_ms}ms jitter={delay_jitter_ms}ms"
        );
    }

    let schedule = Schedule {
        h_d: Height::new(h_d),
        h_c: Height::new(h_c),
        h_r: Height::new(h_r),
        kappa,
        tau_blocks: tau,
    };

    let mut cfg = NodeConfig::local_testnet(n, f, schedule, max_height).with_strategy(strategy);
    cfg.max_runtime_secs = max_secs;
    cfg.workload_seed = seed;
    // Real-network pacemaker tuning: over a real WAN/LAN with staggered process
    // starts, the single-box defaults (view_timeout_ms=200) burn views before the
    // mesh forms. These flags let the orchestrator widen the timeouts to match the
    // real inter-host RTT and start skew. Defaults preserve single-box behavior.
    cfg.view_timeout_ms = opt_u64(&opts, "view-timeout-ms", cfg.view_timeout_ms);
    cfg.pacemaker_tick_ms = opt_u64(&opts, "pacemaker-tick-ms", cfg.pacemaker_tick_ms);
    cfg.proposal_interval_ms = opt_u64(&opts, "proposal-interval-ms", cfg.proposal_interval_ms);
    // Synthetic throughput workload: height-derived (proposer-independent) txs
    // per block so the no-fault path stays fork-free while carrying a real,
    // non-zero committed-transaction count for the TPS/finality-latency metrics.
    cfg.workload_txs_per_block = opt_u64(&opts, "workload-txs", cfg.workload_txs_per_block);

    let mut pv = ProcessValidator::new(id, cfg, transport);

    // Optional HEIGHT-TRIGGERED partition: --partition-keep is the comma-separated
    // set of validator ids THIS node can still reach (its own side, incl. itself).
    // The partition engages once next_height reaches --partition-at (default h_c),
    // NOT from start: a from-start split stalls the PoA phase (Majority quorum
    // n/2+1 is unreachable by a minority side) so no node ever reaches cutover and
    // the HotStuff fork window never opens. Engaging at the cutover boundary lets
    // both sides finalize the PoA prefix, then enter the HotStuff phase
    // partitioned, where each side can independently reach BFT quorum 2f+1.
    if let Some(keep_str) = opts.get("partition-keep") {
        if !keep_str.is_empty() {
            let keep: BTreeSet<ValidatorId> = keep_str
                .split(',')
                .filter_map(|s| s.trim().parse::<u32>().ok())
                .map(ValidatorId::new)
                .collect();
            let at = Height::new(opt_u64(&opts, "partition-at", h_c));
            eprintln!(
                "validator {id_num} partition armed at height {}: can reach {:?}",
                at.get(),
                keep.iter().map(|v| v.get()).collect::<Vec<_>>()
            );
            // Coinbase makes block content proposer-dependent so two concurrent
            // leaders on opposite partition sides mint DISTINCT boundary blocks
            // (required for the fork detector to fire). Only enabled under a
            // partition: in no-partition runs an honest view change re-proposes a
            // height under a new leader, and a proposer-dependent coinbase would
            // make that re-proposal's state root diverge and be rejected.
            pv = pv.with_coinbase().with_partition(keep, at);
        }
    }

    // Quorum-gated cutover (SAGE safety enforcement). When enabled, a SAGE
    // validator switches engines only after n-f distinct cutover attestations
    // for the same boundary, instead of on local readiness. The orchestrator
    // passes --cutover-quorum for the SAGE strategy so that under a partition a
    // minority side (< n-f) refuses to switch and cannot fork.
    if strategy == NodeStrategy::CoxFaithful {
        // Faithful Cox always gates on its 2f+1 checkpoint quorum (its actual
        // safety mechanism), regardless of --cutover-quorum, so the differential
        // exercises Cox's real threshold rather than "no gate".
        eprintln!("validator {id_num} Cox checkpoint-quorum gate ENABLED (threshold 2f+1)");
        pv = pv.with_cox_checkpoint_quorum();
    } else if opts.contains_key("cutover-quorum") {
        eprintln!("validator {id_num} cutover-quorum gate ENABLED (threshold n-f)");
        pv = pv.with_cutover_quorum();
    }

    // Optional Byzantine equivocation: when this node is the proposer at
    // --equivocate-at it mints two distinct blocks for that height and sends
    // each to a disjoint half of its peers. A correct quorum-intersecting engine
    // must still not fork; this exercises the detector against a genuine
    // Byzantine action rather than a partition.
    if let Some(h) = opts
        .get("equivocate-at")
        .and_then(|v| v.parse::<u64>().ok())
    {
        eprintln!("validator {id_num} BYZANTINE: equivocates at height {h}");
        pv = pv.with_equivocation(Height::new(h));
    }

    // Hard watchdog: the run loop checks its wall-clock valve each iteration,
    // but a single iteration can block inside real network I/O (e.g. a peer
    // socket that never returns under a partition), so the cooperative valve is
    // not sufficient to guarantee termination. This thread force-exits the
    // process max_secs + grace after start, unconditionally. Without it a hung
    // validator holds its ssh session open and blocks the orchestrator's `wait`,
    // stalling an entire multi-trial campaign (observed on the hardfork+partition
    // arm). The orchestrator treats a missing/empty JSON line as height 0 for
    // that node, which is correct: a node that could not terminate did not
    // finalize anything the fork detector should trust.
    {
        let grace_secs = opt_u64(&opts, "watchdog-grace-secs", 15);
        let deadline_secs = max_secs.saturating_add(grace_secs);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(deadline_secs));
            eprintln!(
                "validator {id_num} WATCHDOG: exceeded {deadline_secs}s (max_secs={max_secs} + grace={grace_secs}); force-exiting"
            );
            std::process::exit(2);
        });
    }

    match pv.run() {
        Ok(result) => match serde_json::to_string(&result) {
            Ok(line) => println!("{line}"),
            Err(e) => {
                eprintln!("serialize error: {e}");
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("validator {id_num} run error: {e:?}");
            std::process::exit(1);
        }
    }
}

fn parse_opts(args: &[String]) -> BTreeMap<String, String> {
    let mut opts = BTreeMap::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if let Some(key) = arg.strip_prefix("--") {
            if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                opts.insert(key.to_string(), args[i + 1].clone());
                i += 2;
            } else {
                opts.insert(key.to_string(), String::new());
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    opts
}

fn opt_u64(opts: &BTreeMap<String, String>, key: &str, default: u64) -> u64 {
    opts.get(key)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default)
}

fn print_help() {
    println!(
        "validator_proc — single-validator process over real TCP (testbed M1)\n\n\
         Required:\n\
         \x20 --id 0                      this validator's id (0-based)\n\
         \x20 --n 4                       total validators\n\
         \x20 --peers a:p,a:p,...         one socket addr per validator (index = id)\n\n\
         Optional (defaults shown):\n\
         \x20 --f 1                       faults tolerated (BFT quorum = 2f+1)\n\
         \x20 --strategy sage             sage|hardfork|sageblind|stw|reconfig\n\
         \x20 --max-height 8              target height to finalize\n\
         \x20 --h-d 2 --h-c 4 --h-r 8     migration schedule heights\n\
         \x20 --kappa 1 --tau 1           stability threshold / tau blocks\n\
         \x20 --max-secs 20               wall-clock safety valve\n\
         \x20 --seed 42                   workload seed\n\
         \x20 --partition-keep 0,1,2      ids this node can still reach (socket-level partition)\n\n\
         Prints a ProcessResult JSON line on stdout."
    );
}
