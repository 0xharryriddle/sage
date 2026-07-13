//! Multi-process testbed orchestrator (testbed milestone M1).
//!
//! Spawns N `validator_proc` child processes, each a real OS process bound to
//! its own loopback TCP socket, hands them a shared peer map, collects one
//! `ProcessResult` JSON line per child, checks cross-process agreement (no two
//! validators may commit different block hashes at the same height), and writes
//! a CSV summary.
//!
//! This is the headline M1 artifact: N separate processes, real sockets, the
//! same `NodeValidator` consensus, crossing the PoA->HotStuff cutover with no
//! shared memory. Under the `sage` strategy the run must show zero observed
//! forks; a deliberately-unsafe strategy (hardfork/blind) is the broken control
//! that proves the cross-process fork detector can actually fire.
//!
//! Example:
//!   spawn_testbed --n 4 --strategy sage --max-height 8 --out results/raw/testbed.csv

use sage_node::ProcessResult;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::net::{SocketAddr, TcpListener};
use std::path::Path;
use std::process::{Command, Stdio};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }
    let opts = parse_opts(&args);

    let n = opt_u64(&opts, "n", 4) as u32;
    let f = opt_u64(&opts, "f", 1);
    let max_height = opt_u64(&opts, "max-height", 8);
    let h_d = opt_u64(&opts, "h-d", 2);
    let h_c = opt_u64(&opts, "h-c", 4);
    let h_r = opt_u64(&opts, "h-r", 8);
    let max_secs = opt_u64(&opts, "max-secs", 20);
    // Synthetic throughput workload forwarded to every child (0 = empty blocks,
    // preserving the fork/partition experiments' behavior).
    let workload_txs = opt_u64(&opts, "workload-txs", 0);
    // Optional WAN impairment (no-sudo, software netem-style) forwarded to every
    // child's transport: per-send loss + randomized delay/jitter. Used to model
    // cross-region latency on a single box (NOT a real geo-distributed
    // deployment). 0/absent preserves the near-zero-latency loopback behavior.
    let delay_ms = opt_u64(&opts, "delay-ms", 0);
    let jitter_ms = opt_u64(&opts, "jitter-ms", 0);
    let loss_pct = opts.get("loss-pct").cloned();
    let strategy = opts
        .get("strategy")
        .cloned()
        .unwrap_or_else(|| "sage".to_string());
    let out = opts
        .get("out")
        .cloned()
        .unwrap_or_else(|| "results/raw/testbed.csv".to_string());

    // Optional partition: --partition SIZE_A splits validators into side A =
    // {0..SIZE_A} and side B = {SIZE_A..n}. Each child is told which side it is
    // on via --partition-keep, and blocks every peer on the other side at the
    // socket layer. This is a real (no-sudo) network partition that exercises
    // the same code path a kernel-level split would: cross-side consensus
    // messages are dropped. With n=6, f=1: BFT quorum (HotStuff) = 2f+1 = 3, so
    // a 3/3 split lets EACH side reach BFT quorum independently (the fork
    // window). The SAGE cutover quorum (n-f = 5) is NOT reachable by a side of
    // 3, which is the safety asymmetry M3 demonstrates causally.
    let partition_a = opts.get("partition").and_then(|s| s.parse::<u32>().ok());
    let side_keep: Option<(String, String)> = partition_a.map(|a| {
        let side_a: Vec<String> = (0..a).map(|i| i.to_string()).collect();
        let side_b: Vec<String> = (a..n).map(|i| i.to_string()).collect();
        (side_a.join(","), side_b.join(","))
    });

    // Optional Byzantine equivocation: --byzantine ID designates one validator
    // that equivocates at --equivocate-at (default h_c), minting two distinct
    // blocks for that height and sending each to a disjoint half of its peers.
    // A correct quorum-intersecting BFT engine must STILL not fork (no two
    // 2f+1 quorums can both form on conflicting state_roots), so this exercises
    // the fork detector against a genuine Byzantine action rather than only a
    // partition — directly answering the reviewer's "structural inability to
    // fork" criticism. Equivocation requires coinbase (auto-enabled by
    // with_equivocation) so the two blocks have distinct state_roots.
    let byzantine_id = opts.get("byzantine").and_then(|s| s.parse::<u32>().ok());
    let equivocate_at = opt_u64(&opts, "equivocate-at", h_c);

    // Locate the validator_proc binary next to this one (same target dir).
    let bin = validator_proc_path();

    // Pre-allocate N free loopback ports by binding ephemeral listeners, reading
    // the OS-assigned port, then dropping them. Small TOCTOU race window, fine
    // for a loopback testbed; children rebind immediately.
    let addrs: Vec<SocketAddr> = (0..n)
        .map(|_| {
            let l = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
            l.local_addr().expect("local_addr")
        })
        .collect();
    let peers_arg = addrs
        .iter()
        .map(|a| a.to_string())
        .collect::<Vec<_>>()
        .join(",");

    eprintln!(
        "[orchestrator] spawning {n} validators, strategy={strategy}, h_c={h_c}, max_height={max_height}"
    );
    eprintln!("[orchestrator] peer map: {peers_arg}");

    if let Some((a, b)) = &side_keep {
        eprintln!(
            "[orchestrator] PARTITION: side A=[{a}] | side B=[{b}] (cross-side sockets blocked)"
        );
    }

    if let Some(bid) = byzantine_id {
        eprintln!(
            "[orchestrator] BYZANTINE: validator {bid} equivocates at height {equivocate_at} (two distinct blocks to disjoint peer halves)"
        );
    }

    // Spawn all children.
    let mut children = Vec::new();
    for id in 0..n {
        let id_s = id.to_string();
        let n_s = n.to_string();
        let f_s = f.to_string();
        let mh_s = max_height.to_string();
        let hd_s = h_d.to_string();
        let hc_s = h_c.to_string();
        let hr_s = h_r.to_string();
        let ms_s = max_secs.to_string();
        let mut args: Vec<String> = vec![
            "--id".into(),
            id_s,
            "--n".into(),
            n_s,
            "--f".into(),
            f_s,
            "--peers".into(),
            peers_arg.clone(),
            "--max-height".into(),
            mh_s,
            "--h-d".into(),
            hd_s,
            "--h-c".into(),
            hc_s,
            "--h-r".into(),
            hr_s,
            "--max-secs".into(),
            ms_s,
            "--strategy".into(),
            strategy.clone(),
            "--workload-txs".into(),
            workload_txs.to_string(),
        ];
        // Forward WAN impairment to every child, if set (no-sudo software model).
        if delay_ms > 0 {
            args.push("--delay-ms".into());
            args.push(delay_ms.to_string());
        }
        if jitter_ms > 0 {
            args.push("--jitter-ms".into());
            args.push(jitter_ms.to_string());
        }
        if let Some(loss) = &loss_pct {
            args.push("--loss-pct".into());
            args.push(loss.clone());
        }
        // Assign this validator its partition side, if any.
        if let Some((a, b)) = &side_keep {
            let keep = if id < partition_a.unwrap() { a } else { b };
            args.push("--partition-keep".into());
            args.push(keep.clone());
        }
        // SAGE enforces the quorum-gated cutover (switch only after n-f distinct
        // attestations); the unsafe baselines (hardfork/blind) do NOT, so they
        // switch blindly at h_c and serve as the broken control.
        if strategy == "sage" {
            args.push("--cutover-quorum".into());
        }
        // Designate the Byzantine equivocator, if any.
        if Some(id) == byzantine_id {
            args.push("--equivocate-at".into());
            args.push(equivocate_at.to_string());
        }
        let child = Command::new(&bin)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap_or_else(|e| {
                eprintln!("[orchestrator] failed to spawn validator {id}: {e}");
                std::process::exit(1);
            });
        children.push((id, child));
    }

    // Collect each child's stdout JSON line.
    let mut results: Vec<ProcessResult> = Vec::new();
    for (id, mut child) in children {
        let mut buf = String::new();
        if let Some(mut out) = child.stdout.take() {
            let _ = out.read_to_string(&mut buf);
        }
        let status = child.wait();
        let line = buf.lines().find(|l| l.trim_start().starts_with('{'));
        match line {
            Some(l) => match serde_json::from_str::<ProcessResult>(l) {
                Ok(r) => results.push(r),
                Err(e) => eprintln!("[orchestrator] validator {id} bad JSON ({e}): {l}"),
            },
            None => eprintln!(
                "[orchestrator] validator {id} produced no result (exit {:?})",
                status.ok().map(|s| s.code())
            ),
        }
    }

    if results.is_empty() {
        eprintln!("[orchestrator] no results collected; aborting");
        std::process::exit(1);
    }

    // Cross-process fork detection. The BFT safety property is AGREEMENT ON THE
    // DECIDED VALUE: no two validators finalize a different application state at
    // the same height. The decided value is the finalized state_root, NOT its
    // engine-tagged wire encoding. The block hash binds `engine_id`
    // (block.rs::encode_canonical), so at the PoA->HotStuff cutover boundary the
    // SAME logical block (identical txs, identical state_root, identical parent)
    // sealed under PoA on one node and under HotStuff on another hashes
    // differently while representing identical state. Comparing block hashes
    // there yields a false-positive "fork" that is purely an encoding artifact
    // (proven: at the boundary all nodes report one state_root, differing only
    // in engine tag; see SAGE_FORK_DEBUG output). We therefore judge safety at
    // the STATE level: a height with two distinct state_roots is a genuine
    // OBSERVED fork. This still catches every real fork and Byzantine
    // equivocation, because both mint distinct coinbases -> distinct state_roots
    // (the partition/equivocation experiments rely on exactly that distinction).
    let mut roots_by_height: BTreeMap<u64, BTreeSet<String>> = BTreeMap::new();
    for r in &results {
        for (h, root, _engine) in &r.committed_detail {
            roots_by_height.entry(*h).or_default().insert(root.clone());
        }
    }
    let forked_heights: Vec<u64> = roots_by_height
        .iter()
        .filter(|(_, roots)| roots.len() > 1)
        .map(|(h, _)| *h)
        .collect();
    let observed_fork = !forked_heights.is_empty();

    // Informational only: heights where the engine-tagged block HASH diverged
    // but the state_root did NOT. These are boundary-encoding artifacts, not
    // safety violations; surfaced so the distinction is auditable, never folded
    // into the `observed_fork` verdict.
    let mut hashes_by_height: BTreeMap<u64, BTreeSet<String>> = BTreeMap::new();
    for r in &results {
        for (h, hash) in &r.committed {
            hashes_by_height.entry(*h).or_default().insert(hash.clone());
        }
    }
    let encoding_divergence_heights: Vec<u64> = hashes_by_height
        .iter()
        .filter(|(h, hashes)| {
            hashes.len() > 1 && roots_by_height.get(h).map(|r| r.len() <= 1).unwrap_or(true)
        })
        .map(|(h, _)| *h)
        .collect();
    if !encoding_divergence_heights.is_empty() {
        eprintln!(
            "[orchestrator] encoding-only divergence (same state_root, different engine tag) at heights {encoding_divergence_heights:?} — NOT a fork (cutover-boundary artifact)"
        );
    }

    // Diagnostic (SAGE_FORK_DEBUG=1): for every forked height, print which
    // validator committed which hash. A genuine cross-boundary safety violation
    // shows two disjoint validator groups each holding a conflicting hash; a
    // reporting artifact (e.g. one straggler that re-proposed under a new leader
    // before catching up) shows a lopsided split. This is investigation only —
    // it never alters the detector verdict above.
    if std::env::var("SAGE_FORK_DEBUG").is_ok() {
        for h in &forked_heights {
            eprintln!("[fork-debug] height {h}:");
            let mut groups: BTreeMap<String, Vec<u32>> = BTreeMap::new();
            for r in &results {
                if let Some((_, hash)) = r.committed.iter().find(|(hh, _)| hh == h) {
                    groups.entry(hash.clone()).or_default().push(r.validator);
                }
            }
            for (hash, vals) in &groups {
                eprintln!(
                    "[fork-debug]   hash {}.. committed by {} validators: {:?}",
                    &hash[..hash.len().min(16)],
                    vals.len(),
                    vals
                );
            }
            // State-level detail: group by (state_root, engine_kind). If every
            // group shares the SAME state_root and differs only in engine_kind,
            // the divergence is a boundary-ENCODING artifact (block hash binds
            // engine_id), NOT a state-level safety violation.
            let mut state_groups: BTreeMap<(String, String), Vec<u32>> = BTreeMap::new();
            for r in &results {
                if let Some((_, root, engine)) =
                    r.committed_detail.iter().find(|(hh, _, _)| hh == h)
                {
                    state_groups
                        .entry((root.clone(), engine.clone()))
                        .or_default()
                        .push(r.validator);
                }
            }
            let distinct_roots: BTreeSet<&String> =
                state_groups.keys().map(|(root, _)| root).collect();
            for ((root, engine), vals) in &state_groups {
                eprintln!(
                    "[fork-debug]   state_root {}.. engine {} : {} validators {:?}",
                    &root[..root.len().min(16)],
                    engine,
                    vals.len(),
                    vals
                );
            }
            if distinct_roots.len() <= 1 {
                eprintln!("[fork-debug]   => SAME state_root across all groups: ENCODING artifact (engine_id in hash), NOT a state-level fork");
            } else {
                eprintln!("[fork-debug]   => DISTINCT state_roots: genuine state-level divergence at this height");
            }
        }
    }

    let success_count = results.iter().filter(|r| r.migration_success).count();
    let max_h = results
        .iter()
        .map(|r| r.max_finalized_height)
        .max()
        .unwrap_or(0);

    // Empirical message complexity: sum of real wire writes across all
    // validators. For an all-to-all BFT broadcast pattern this scales as
    // O(n^2) per round, so total_messages / max_h gives a per-round figure the
    // O(n^2) plot fits against. Reported in the CSV and summary line.
    let total_messages: u64 = results.iter().map(|r| r.messages_sent).sum();

    let quorum_threshold = u64::from(n).saturating_sub(f) as usize;
    let replay_counts =
        count_optional_strings(results.iter().map(|r| r.replay_context_root.as_deref()));
    let replay_roots: BTreeSet<String> = replay_counts.keys().cloned().collect();
    let replay_context_root_agreement = replay_roots.len() <= 1;
    let replay_context_ok = replay_counts.values().copied().max().unwrap_or(0) >= quorum_threshold;
    if replay_roots.is_empty() {
        eprintln!("[orchestrator] replay context root missing from all validators");
    } else if !replay_context_root_agreement {
        eprintln!(
            "[orchestrator] replay context root disagreement across validators: {replay_counts:?}; quorum_ok={replay_context_ok}"
        );
    }

    let manifest_counts =
        count_optional_strings(results.iter().map(|r| r.manifest_payload_hash.as_deref()));
    let manifest_hashes: BTreeSet<String> = manifest_counts.keys().cloned().collect();
    let manifest_payload_hash_agreement = manifest_hashes.len() <= 1;
    let manifest_payload_ok =
        manifest_counts.values().copied().max().unwrap_or(0) >= quorum_threshold;
    if manifest_hashes.is_empty() {
        eprintln!("[orchestrator] manifest payload hash missing from all validators");
    } else if !manifest_payload_hash_agreement {
        eprintln!(
            "[orchestrator] manifest payload hash disagreement across validators: {manifest_counts:?}; quorum_ok={manifest_payload_ok}"
        );
    }

    // Threshold CutCert verification: re-derive each validator's verifying key
    // from its id+seed, verify its ed25519 signature over the agreed cutover
    // payload hash, and count distinct valid signers. A genuine n-f quorum of
    // valid signatures is the cryptographic witness that the migration was
    // sealed by a real quorum, not a flag flip.
    let cutcert_signers = verify_cutcert(&results, f);
    let cutcert_threshold = cutcert_threshold(n, f);
    let cutcert_ok = cutcert_signers >= cutcert_threshold;
    eprintln!(
        "[orchestrator] CutCert: {cutcert_signers} valid distinct signatures (threshold n-f={cutcert_threshold}) -> {}",
        if cutcert_ok { "VERIFIED" } else { "below threshold" }
    );

    // Write CSV.
    if let Err(e) = write_csv(
        &out,
        &strategy,
        n,
        f,
        &results,
        observed_fork,
        max_h,
        total_messages,
        replay_context_root_agreement,
        replay_context_ok,
        manifest_payload_hash_agreement,
        manifest_payload_ok,
    ) {
        eprintln!("[orchestrator] failed to write {out}: {e}");
        std::process::exit(1);
    }

    eprintln!(
        "[orchestrator] {success_count}/{} migration_success, max_height={max_h}, observed_fork={observed_fork} (heights={forked_heights:?})",
        results.len()
    );
    println!(
        "{{\"strategy\":\"{strategy}\",\"n\":{n},\"f\":{f},\"results\":{},\"migration_success\":{success_count},\"max_finalized_height\":{max_h},\"observed_fork\":{observed_fork},\"cutcert_signers\":{cutcert_signers},\"cutcert_threshold\":{cutcert_threshold},\"cutcert_ok\":{cutcert_ok},\"total_messages\":{total_messages},\"replay_context_root_agreement\":{replay_context_root_agreement},\"replay_context_ok\":{replay_context_ok},\"manifest_payload_hash_agreement\":{manifest_payload_hash_agreement},\"manifest_payload_ok\":{manifest_payload_ok}}}",
        results.len()
    );
}

fn cutcert_threshold(n: u32, f: u64) -> u64 {
    u64::from(n).saturating_sub(f)
}

fn count_optional_strings<'a>(
    values: impl Iterator<Item = Option<&'a str>>,
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for value in values.flatten() {
        *counts.entry(value.to_owned()).or_insert(0) += 1;
    }
    counts
}

#[allow(clippy::too_many_arguments)]
fn write_csv(
    path: impl AsRef<Path>,
    strategy: &str,
    n: u32,
    f: u64,
    results: &[ProcessResult],
    observed_fork: bool,
    max_h: u64,
    total_messages: u64,
    replay_context_root_agreement: bool,
    replay_context_ok: bool,
    manifest_payload_hash_agreement: bool,
    manifest_payload_ok: bool,
) -> std::io::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let mut s = String::new();
    s.push_str("strategy,n,f,validator,migration_success,max_finalized_height,cutover_height,observed_fork,run_max_height,total_duration_secs,messages_sent,total_messages,replay_context_root,replay_context_root_agreement,replay_context_ok,manifest_payload_hash,manifest_payload_hash_agreement,manifest_payload_ok,total_txs,committed_tps,inter_commit_p50_micros,inter_commit_p95_micros,inter_commit_p99_micros,inter_commit_max_micros,cutover_gap_micros\n");
    for r in results {
        s.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{:.6},{},{},{},{},{},{},{},{},{},{:.3},{},{},{},{},{}\n",
            strategy,
            n,
            f,
            r.validator,
            r.migration_success,
            r.max_finalized_height,
            r.cutover_height.map(|h| h.to_string()).unwrap_or_default(),
            observed_fork,
            max_h,
            r.total_duration_secs,
            r.messages_sent,
            total_messages,
            r.replay_context_root.as_deref().unwrap_or_default(),
            replay_context_root_agreement,
            replay_context_ok,
            r.manifest_payload_hash.as_deref().unwrap_or_default(),
            manifest_payload_hash_agreement,
            manifest_payload_ok,
            r.total_txs,
            r.committed_tps,
            r.inter_commit_p50_micros,
            r.inter_commit_p95_micros,
            r.inter_commit_p99_micros,
            r.inter_commit_max_micros,
            r.cutover_gap_micros,
        ));
    }
    std::fs::write(path, s)
}

/// Re-derive each validator's deterministic ed25519 verifying key from its
/// id+seed, verify its signature over the cutover payload hash it reported, and
/// return the count of DISTINCT validators with a valid signature over a payload
/// hash agreed by the quorum. Cross-checks that every counted signer signed the
/// SAME payload hash (otherwise they are not certifying the same cutover).
#[cfg(feature = "real-crypto")]
fn verify_cutcert(results: &[ProcessResult], _f: u64) -> u64 {
    use sage_core::ValidatorId;
    use std::collections::BTreeMap;

    // Group reported payload hashes; the agreed cutover is the majority hash.
    let mut hash_votes: BTreeMap<String, u64> = BTreeMap::new();
    for r in results {
        if let Some(h) = &r.cutover_payload_hash {
            *hash_votes.entry(h.clone()).or_default() += 1;
        }
    }
    let Some((agreed_hash, _)) = hash_votes.into_iter().max_by_key(|(_, c)| *c) else {
        return 0;
    };
    let Some(payload_hash) = parse_hash32(&agreed_hash) else {
        return 0;
    };

    use sage_manifest::real_ed25519::RealEd25519Scheme;
    use sage_manifest::{SignatureEnvelope, SignatureScheme};

    let mut valid_signers = std::collections::BTreeSet::new();
    for r in results {
        let (Some(sig_hex), Some(ph)) = (&r.cutover_cert, &r.cutover_payload_hash) else {
            continue;
        };
        if ph != &agreed_hash {
            continue; // signed a different cutover view — do not count
        }
        let Some(sig_bytes) = parse_hex(sig_hex) else {
            continue;
        };
        // Re-derive this validator's deterministic keypair from id+seed (the
        // same derivation the signer used) and verify via the manifest scheme.
        let id = ValidatorId::new(r.validator);
        let mut scheme = RealEd25519Scheme::new();
        scheme.register_deterministic(id, r.seed);
        let envelope = SignatureEnvelope::Ed25519 {
            signer: id,
            payload_hash,
            signature_bytes: sig_bytes,
        };
        let signers: std::collections::BTreeSet<ValidatorId> = [id].into_iter().collect();
        if scheme.verify(&signers, payload_hash, &envelope).is_ok() {
            valid_signers.insert(r.validator);
        }
    }
    valid_signers.len() as u64
}

#[cfg(not(feature = "real-crypto"))]
fn verify_cutcert(_results: &[ProcessResult], _f: u64) -> u64 {
    0
}

#[cfg(feature = "real-crypto")]
fn parse_hash32(hex: &str) -> Option<sage_core::Hash32> {
    let bytes = parse_hex(hex)?;
    if bytes.len() != 32 {
        return None;
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Some(sage_core::Hash32::new(arr))
}

#[cfg(feature = "real-crypto")]
fn parse_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

/// Find the `validator_proc` binary in the same directory as this executable.
fn validator_proc_path() -> std::path::PathBuf {
    let exe = std::env::current_exe().expect("current_exe");
    let dir = exe.parent().expect("exe dir");
    let cand = dir.join("validator_proc");
    if cand.exists() {
        cand
    } else {
        // Fall back to PATH lookup.
        std::path::PathBuf::from("validator_proc")
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
        "spawn_testbed — multi-process testbed orchestrator (M1)\n\n\
         Spawns N validator_proc processes over real loopback TCP, collects one\n\
         ProcessResult JSON line each, checks cross-process hash agreement, and\n\
         writes a CSV summary.\n\n\
         Options (defaults shown):\n\
         \x20 --n 4                 number of validator processes\n\
         \x20 --f 1                 faults tolerated (BFT quorum = 2f+1)\n\
         \x20 --strategy sage       sage|hardfork|sageblind|stw|reconfig\n\
         \x20 --max-height 8        target height to finalize\n\
         \x20 --h-d 2 --h-c 4 --h-r 8   migration schedule heights\n\
         \x20 --max-secs 20         per-process wall-clock safety valve\n\
         \x20 --partition A         split into sides [0..A] and [A..n] at h_c\n\
         \x20 --byzantine ID        validator ID equivocates (two blocks at one height)\n\
         \x20 --equivocate-at H     height the Byzantine validator equivocates at (default h_c)\n\
         \x20 --out results/raw/testbed.csv   output CSV path\n"
    );
}

#[cfg(test)]
mod tests {
    use super::cutcert_threshold;

    #[test]
    fn cutcert_requires_n_minus_f_signers() {
        assert_eq!(cutcert_threshold(6, 1), 5);
        assert_eq!(cutcert_threshold(20, 6), 14);
    }
}
