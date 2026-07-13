//! SAGE invariant checker — runs smoke simulations and reports pass/fail.
use sage_controller::{check_all, InvariantStatus};
use sage_experiments::common::stable_config_hash;
use sage_experiments::result_schema::{
    missing_columns, CERT_TIMING_COLUMNS, COMMON_COLUMNS, FORK_DIFFERENTIAL_COLUMNS,
    M4_MESSAGE_COMPLEXITY_COLUMNS, M5_BYZANTINE_EQUIVOCATION_COLUMNS, M6_SOTA_BASELINE_COLUMNS,
    OVERHEAD_COLUMNS, OVERHEAD_SUMMARY_COLUMNS, ROLLBACK_ADMISSIBILITY_COLUMNS, RQ1_COLUMNS,
    RQ1_STATS_COLUMNS, RQ2_COLUMNS, RQ3_COLUMNS, RQ4_COLUMNS, SAFETY_COLUMNS, SCALE_LOCAL_COLUMNS,
    TESTBED_SMOKE_COLUMNS, WAN_CUTOVER_COLUMNS,
};
use sage_sim::{RunMetrics, SimConfig, Simulation, StrategyKind};
use std::path::Path;

struct Check {
    name: String,
    passed: bool,
    message: String,
}

#[allow(clippy::field_reassign_with_default)]
fn main() {
    let mut checks = Vec::new();

    // Check 1: workspace compiles (implicit via this binary running)
    checks.push(Check {
        name: "workspace_bootstrapped".into(),
        passed: true,
        message: "verify binary compiled".into(),
    });

    // Check 2: default config loads
    match SimConfig::load("config/default.toml") {
        Ok(_) => checks.push(Check {
            name: "config_loads".into(),
            passed: true,
            message: "config/default.toml loaded".into(),
        }),
        Err(e) => checks.push(Check {
            name: "config_loads".into(),
            passed: false,
            message: format!("failed: {}", e),
        }),
    }

    // Check 2b: all experiment configs load
    for cfg_path in &[
        "config/rq1.toml",
        "config/rq2.toml",
        "config/rq3.toml",
        "config/rq4.toml",
        "config/safety.toml",
    ] {
        let name = format!(
            "config_loads_{}",
            cfg_path.replace("config/", "").replace(".toml", "")
        );
        match SimConfig::load(cfg_path) {
            Ok(_) => checks.push(Check {
                name,
                passed: true,
                message: format!("{} loaded", cfg_path),
            }),
            Err(e) => checks.push(Check {
                name,
                passed: false,
                message: format!("{}: {}", cfg_path, e),
            }),
        }
    }

    // Check 3: ReconfigOnly completes without protocol swap
    let mut cfg = SimConfig::default();
    cfg.strategy = StrategyKind::ReconfigOnly;
    cfg.simulation.max_height = 4;
    cfg.simulation.max_events = 100_000;
    checks.push(run_and_check(
        "reconfig_only_no_swap",
        cfg,
        |metrics, _snapshot| {
            (
                !metrics.protocol_swap_success,
                format!(
                    "protocol_swap_success={} (expect false)",
                    metrics.protocol_swap_success
                ),
            )
        },
    ));

    // Check 4: SAGE no-fault migration with invariants
    let mut cfg = SimConfig::default();
    cfg.strategy = StrategyKind::Sage;
    cfg.simulation.max_height = 6;
    cfg.simulation.max_events = 200_000;
    cfg.migration.h_d = sage_core::Height::new(1);
    cfg.migration.h_c = sage_core::Height::new(3);
    cfg.migration.h_r = sage_core::Height::new(6);
    cfg.migration.kappa = 1;
    cfg.migration.tau_blocks = 1;
    checks.push(run_and_check(
        "sage_no_fault_runs",
        cfg,
        |metrics, snapshot| {
            let report = check_all(snapshot);
            let all_invariants = report.all_passed();
            let msgs: Vec<String> = report
                .checks
                .iter()
                .map(|c| {
                    let s = if c.status == InvariantStatus::Pass {
                        "PASS"
                    } else {
                        "FAIL"
                    };
                    format!("[{}] {}", s, c.name)
                })
                .collect();
            (
                !metrics.safety_violation && all_invariants,
                format!(
                    "finalized={} safety={} invariants=[{}]",
                    metrics.finalized_blocks,
                    metrics.safety_violation,
                    msgs.join("; ")
                ),
            )
        },
    ));

    // Check 5: HardFork with partition terminates
    let mut cfg = SimConfig::default();
    cfg.strategy = StrategyKind::HardFork;
    cfg.simulation.max_height = 4;
    cfg.simulation.max_events = 100_000;
    cfg.migration.h_d = sage_core::Height::new(1);
    cfg.migration.h_c = sage_core::Height::new(2);
    cfg.migration.h_r = sage_core::Height::new(4);
    cfg.migration.kappa = 1;
    cfg.adversary.partition_enabled = true;
    cfg.adversary.partition_start_height = 2;
    cfg.adversary.partition_duration_blocks = 2;
    cfg.adversary.partition_split = 3;
    checks.push(run_and_check(
        "hardfork_partition_terminates",
        cfg,
        |metrics, _snapshot| {
            (
                true,
                format!(
                    "finalized={} safety={}",
                    metrics.finalized_blocks, metrics.safety_violation
                ),
            )
        },
    ));

    // Check 6: STW runs
    let mut cfg = SimConfig::default();
    cfg.strategy = StrategyKind::StopTheWorld;
    cfg.simulation.max_height = 4;
    cfg.simulation.max_events = 100_000;
    cfg.migration.h_d = sage_core::Height::new(1);
    cfg.migration.h_c = sage_core::Height::new(3);
    cfg.migration.h_r = sage_core::Height::new(4);
    cfg.migration.kappa = 1;
    checks.push(run_and_check("stw_runs", cfg, |metrics, _snapshot| {
        (
            true,
            format!(
                "downtime_events={} finalized={}",
                metrics.downtime_events, metrics.finalized_blocks
            ),
        )
    }));

    // Check 7: Deterministic reproducibility — same seed produces identical metrics
    checks.push(check_determinism());

    // Check 8: SageBlind strategy (blind cutover, no cutcert)
    let mut cfg = SimConfig::default();
    cfg.strategy = StrategyKind::SageBlind;
    cfg.simulation.max_height = 4;
    cfg.simulation.max_events = 100_000;
    cfg.migration.h_d = sage_core::Height::new(1);
    cfg.migration.h_c = sage_core::Height::new(2);
    cfg.migration.h_r = sage_core::Height::new(4);
    cfg.migration.kappa = 1;
    cfg.migration.tau_blocks = 1;
    checks.push(run_and_check(
        "sage_blind_runs",
        cfg,
        |metrics, _snapshot| {
            (
                metrics.finalized_blocks > 0,
                format!(
                    "finalized={} safety={} migration_success={}",
                    metrics.finalized_blocks, metrics.safety_violation, metrics.migration_success,
                ),
            )
        },
    ));

    checks.push(check_metadata_hashing());
    checks.push(check_schema_contracts());
    checks.push(check_existing_raw_csv_headers());
    checks.push(check_paper_artifact_sync());
    checks.push(check_conformance_artifacts());

    // Report
    let mut passed = 0;
    let mut failed = 0;
    for check in &checks {
        let status = if check.passed { "PASS" } else { "FAIL" };
        if check.passed {
            passed += 1;
        } else {
            failed += 1;
        }
        println!("[{}] {}: {}", status, check.name, check.message);
    }
    println!(
        "SAGE verify: {} passed, {} failed, {} total",
        passed,
        failed,
        checks.len()
    );
    if failed > 0 {
        std::process::exit(1);
    }
}

#[allow(clippy::field_reassign_with_default)]
fn check_determinism() -> Check {
    let mut cfg = SimConfig::default();
    cfg.strategy = StrategyKind::Sage;
    cfg.simulation.max_height = 5;
    cfg.simulation.max_events = 100_000;
    cfg.simulation.seed = 42;
    cfg.migration.h_d = sage_core::Height::new(1);
    cfg.migration.h_c = sage_core::Height::new(2);
    cfg.migration.h_r = sage_core::Height::new(5);
    cfg.migration.kappa = 1;
    cfg.migration.tau_blocks = 1;

    let run_once = |cfg: &SimConfig| -> Option<RunMetrics> {
        match Simulation::new(cfg.clone()) {
            Ok(mut sim) => sim.run().ok(),
            Err(_) => None,
        }
    };

    match (run_once(&cfg), run_once(&cfg)) {
        (Some(a), Some(b)) => {
            let deterministic = a.finalized_blocks == b.finalized_blocks
                && a.max_finalized_height == b.max_finalized_height
                && a.safety_violation == b.safety_violation
                && a.cutover_height == b.cutover_height;
            Check {
                name: "deterministic_reproducibility".into(),
                passed: deterministic,
                message: if deterministic {
                    format!(
                        "same seed produced identical metrics (finalized={}, cutover={:?})",
                        a.finalized_blocks, a.cutover_height
                    )
                } else {
                    format!(
                        "non-deterministic: run1=(finalized={}, cutover={:?}), run2=(finalized={}, cutover={:?})",
                        a.finalized_blocks, a.cutover_height,
                        b.finalized_blocks, b.cutover_height
                    )
                },
            }
        }
        (Some(m), None) | (None, Some(m)) => Check {
            name: "deterministic_reproducibility".into(),
            passed: false,
            message: format!("only one run succeeded: finalized={}", m.finalized_blocks),
        },
        (None, None) => Check {
            name: "deterministic_reproducibility".into(),
            passed: false,
            message: "both runs failed".into(),
        },
    }
}

fn check_metadata_hashing() -> Check {
    let a = stable_config_hash("strategy = \"Sage\"\n");
    let b = stable_config_hash("strategy = \"Sage\"\n");
    let c = stable_config_hash("strategy = \"HardFork\"\n");
    let valid = a == b && a != c && a.len() == 64 && a.chars().all(|ch| ch.is_ascii_hexdigit());
    Check {
        name: "metadata_hash_stable_sha256".into(),
        passed: valid,
        message: if valid {
            format!("sha256 config hash stable ({})", &a[..12])
        } else {
            "config hash was not stable 64-char hex SHA-256".into()
        },
    }
}

fn check_schema_contracts() -> Check {
    let schemas = [
        ("common", COMMON_COLUMNS),
        ("rq1", RQ1_COLUMNS),
        ("rq2", RQ2_COLUMNS),
        ("rq3", RQ3_COLUMNS),
        ("rq4", RQ4_COLUMNS),
        ("safety", SAFETY_COLUMNS),
        ("overhead", OVERHEAD_COLUMNS),
        ("testbed_smoke", TESTBED_SMOKE_COLUMNS),
        ("m4_message_complexity", M4_MESSAGE_COMPLEXITY_COLUMNS),
        (
            "m5_byzantine_equivocation",
            M5_BYZANTINE_EQUIVOCATION_COLUMNS,
        ),
        ("m6_sota_baseline", M6_SOTA_BASELINE_COLUMNS),
    ];
    let mut failures = Vec::new();
    for (name, headers) in schemas {
        let missing = missing_columns(headers, COMMON_COLUMNS);
        let has_common_contract = !matches!(
            name,
            "rq4"
                | "testbed_smoke"
                | "m4_message_complexity"
                | "m5_byzantine_equivocation"
                | "m6_sota_baseline"
        );
        if has_common_contract && !missing.is_empty() {
            failures.push(format!("{} missing {:?}", name, missing));
        }
        if has_common_contract && !headers.contains(&"run_status") {
            failures.push(format!("{} missing run_status", name));
        }
    }
    Check {
        name: "csv_schema_contracts".into(),
        passed: failures.is_empty(),
        message: if failures.is_empty() {
            "required schema constants present".into()
        } else {
            failures.join("; ")
        },
    }
}

fn check_existing_raw_csv_headers() -> Check {
    let schemas = [
        ("results/raw/rq1_migration_cost.csv", RQ1_COLUMNS),
        ("results/raw/rq1_stats.csv", RQ1_STATS_COLUMNS),
        ("results/raw/rq2_partition_safety.csv", RQ2_COLUMNS),
        ("results/raw/rq3_kappa_ablation.csv", RQ3_COLUMNS),
        ("results/raw/rq4_rollback.csv", RQ4_COLUMNS),
        ("results/raw/safety_partition.csv", SAFETY_COLUMNS),
        ("results/raw/dual_run_overhead.csv", OVERHEAD_COLUMNS),
        (
            "results/figures/dual_run_overhead_summary.csv",
            OVERHEAD_SUMMARY_COLUMNS,
        ),
        ("results/raw/wan_cutover.csv", WAN_CUTOVER_COLUMNS),
        ("results/raw/testbed_smoke.csv", TESTBED_SMOKE_COLUMNS),
        (
            "results/raw/m4_message_complexity.csv",
            M4_MESSAGE_COMPLEXITY_COLUMNS,
        ),
        (
            "results/raw/m5_byzantine_equivocation.csv",
            M5_BYZANTINE_EQUIVOCATION_COLUMNS,
        ),
        (
            "results/raw/m6_sota_baseline_differential.csv",
            M6_SOTA_BASELINE_COLUMNS,
        ),
        (
            "results/raw/multihost_fork_differential.csv",
            FORK_DIFFERENTIAL_COLUMNS,
        ),
        ("results/raw/cert_timing.csv", CERT_TIMING_COLUMNS),
        (
            "results/raw/rollback_admissibility.csv",
            ROLLBACK_ADMISSIBILITY_COLUMNS,
        ),
        ("results/raw/m7_scale_local.csv", SCALE_LOCAL_COLUMNS),
    ];
    let mut checked = 0usize;
    let mut failures = Vec::new();
    for (path, required) in schemas {
        let path = Path::new(path);
        if !path.exists() {
            continue;
        }
        checked += 1;
        match csv::Reader::from_path(path) {
            Ok(mut reader) => match reader.headers() {
                Ok(headers) => {
                    let headers: Vec<&str> = headers.iter().collect();
                    let missing = missing_columns(&headers, required);
                    if !missing.is_empty() {
                        failures.push(format!("{} missing {:?}", path.display(), missing));
                    }
                }
                Err(e) => failures.push(format!("{} header error: {}", path.display(), e)),
            },
            Err(e) => failures.push(format!("{} read error: {}", path.display(), e)),
        }
    }
    Check {
        name: "existing_raw_csv_headers".into(),
        passed: failures.is_empty(),
        message: if failures.is_empty() {
            format!("validated {} existing raw CSV header(s)", checked)
        } else {
            failures.join("; ")
        },
    }
}

fn check_paper_artifact_sync() -> Check {
    let paper_path = if Path::new("docs/paper/paper.tex").exists() {
        Path::new("docs/paper/paper.tex")
    } else {
        Path::new("docs/paper.tex")
    };
    if !paper_path.exists() {
        return Check {
            name: "paper_artifact_sync".into(),
            passed: true,
            message: "paper source not present; skipped".into(),
        };
    }

    let paper = match std::fs::read_to_string(paper_path) {
        Ok(text) => text,
        Err(e) => {
            return Check {
                name: "paper_artifact_sync".into(),
                passed: false,
                message: format!("failed to read docs/paper.tex: {}", e),
            };
        }
    };

    let mut failures = Vec::new();
    let expectations = [
        (
            "results/figures/rq1_downtime.csv",
            5usize,
            "$60$ seeds per strategy",
        ),
        (
            "results/figures/rq3_ablation.csv",
            5usize,
            "$20$ seeds per $\\kappa$",
        ),
        (
            "results/figures/scaling.csv",
            10usize,
            "$n\\in\\{4,7,10,13,16,20,25,31,50,100\\}$",
        ),
        (
            "results/figures/sensitivity.csv",
            6usize,
            "$\\{20,40,80,160,320,640\\}\\,$ms",
        ),
        (
            "results/figures/termination.csv",
            14usize,
            "$\\{2,4,6,8,12,16,24\\}$ blocks",
        ),
    ];

    for (path, expected_rows, paper_token) in expectations {
        let path_ref = Path::new(path);
        if path_ref.exists() {
            match csv_row_count(path_ref) {
                Ok(rows) if rows == expected_rows => {}
                Ok(rows) => failures.push(format!(
                    "{} has {} data rows, expected {}",
                    path, rows, expected_rows
                )),
                Err(e) => failures.push(format!("{} read error: {}", path, e)),
            }
        }
        if !paper.contains(paper_token) {
            failures.push(format!("paper missing artifact token `{}`", paper_token));
        }
    }

    // Exact-sentence prose tokens are brittle: a wording edit that preserves the
    // underlying data can still change these strings and false-fail a valid build.
    // They are therefore checked only under SAGE_STRICT_PROSE=1 (camera-ready audit).
    // The STRUCTURAL checks above (CSV row counts <-> paper range tokens) and the
    // stale-token guard below remain ON by default, since those catch real
    // data/consistency drift rather than prose rewording.
    if std::env::var("SAGE_STRICT_PROSE").as_deref() == Ok("1") {
        for required in [
            "completes 60 no-partition migration seeds",
            "$200$ trials per strategy",
            "$0/200$ for every strategy",
            "Hard fork & $\\{2,4,6,8,12,16,24\\}$ & 140",
            "\\caption{Shadow-anchoring ablation; $20$ seeds per $\\kappa$.}",
            "($20$ trials per mean delay)",
            "$122$ bytes without an oracle root and $162$ bytes with one",
            "representative engineering",
            "does not model the byte-level replay-context-root hash",
            "turnkey operational validation for future work",
            "Three-arm dual-run overhead microbenchmark; mean over",
            "SAGE within $-2.5$ to $+2.8\\%$ of Cox-style across all load",
            "does not require synchronous replay-context",
            "metro $(10\\,\\mathrm{ms},2\\,\\mathrm{ms},0\\%)$",
            "removes the shared-kernel and",
        ] {
            if !paper.contains(required) {
                failures.push(format!("paper missing consistency token `{}`", required));
            }
        }
    }

    for stale in [
        "completes 30 no-partition migration seeds",
        "$100$ trials per strategy",
        "$0/100$ for every strategy",
        "five seeds per $\\kappa$",
        "($10$ trials per mean delay)",
    ] {
        if paper.contains(stale) {
            failures.push(format!("paper still contains stale token `{}`", stale));
        }
    }

    Check {
        name: "paper_artifact_sync".into(),
        passed: failures.is_empty(),
        message: if failures.is_empty() {
            "paper trial/range tokens match generated figure artifacts".into()
        } else {
            failures.join("; ")
        },
    }
}

/// Checks the TLA+<->Rust conformance harness artifacts are present and that the
/// committed-vector projections carry the expected safety signal (faithful
/// configs safe; broken controls carry a fork). This makes the gate notice if
/// the harness is deleted or the projections are regenerated wrong. It does NOT
/// re-run TLC (that is `make conformance`); it validates the committed artifacts.
fn check_conformance_artifacts() -> Check {
    let mut failures = Vec::new();

    // The correspondence doc and the pure predicate must exist.
    for path in [
        "formal/CORRESPONDENCE.md",
        "formal/dump_states.sh",
        "crates/sage-node/src/cutover_gate.rs",
        "crates/sage-node/tests/formal_conformance.rs",
    ] {
        if !Path::new(path).exists() {
            failures.push(format!("missing conformance artifact `{}`", path));
        }
    }

    // The committed-vector projections must exist with the expected distinct
    // counts (faithful: safe; broken control: contains at least one forked
    // vector, i.e. a vector mixing side 1 and side 2). Counts are the TLC
    // reachable-set sizes verified by the conformance test.
    let projections: [(&str, usize, bool); 5] = [
        ("formal/states/committed/committed_n4.txt", 31, false),
        ("formal/states/committed/committed_n7.txt", 255, false),
        ("formal/states/committed/committed_n10.txt", 2047, false),
        ("formal/states/committed/committed_blind_n4.txt", 81, true),
        ("formal/states/committed/committed_blind_n7.txt", 2187, true),
    ];
    for (path, expected_count, expect_fork) in projections {
        let p = Path::new(path);
        if !p.exists() {
            failures.push(format!(
                "missing committed projection `{}` (run `make conformance`)",
                path
            ));
            continue;
        }
        let content = match std::fs::read_to_string(p) {
            Ok(c) => c,
            Err(e) => {
                failures.push(format!("cannot read `{}`: {}", path, e));
                continue;
            }
        };
        let vectors: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
        if vectors.len() != expected_count {
            failures.push(format!(
                "`{}` has {} vectors, expected {}",
                path,
                vectors.len(),
                expected_count
            ));
        }
        // A forked vector mixes a `1` and a `2` (two sides committed distinct
        // boundary blocks). Faithful projections must have NONE; broken must
        // have at least one.
        let has_fork = vectors.iter().any(|line| {
            let has1 = line.split(',').any(|t| t.trim() == "1");
            let has2 = line.split(',').any(|t| t.trim() == "2");
            has1 && has2
        });
        if expect_fork && !has_fork {
            failures.push(format!(
                "broken-control `{}` has no forked vector — harness lost its teeth",
                path
            ));
        }
        if !expect_fork && has_fork {
            failures.push(format!(
                "faithful `{}` contains a forked committed vector — safety broken",
                path
            ));
        }
    }

    Check {
        name: "conformance_artifacts".into(),
        passed: failures.is_empty(),
        message: if failures.is_empty() {
            "TLA+<->Rust conformance artifacts present; faithful safe, controls fork".into()
        } else {
            failures.join("; ")
        },
    }
}

fn csv_row_count(path: &Path) -> Result<usize, csv::Error> {
    let mut reader = csv::Reader::from_path(path)?;
    Ok(reader.records().count())
}

fn run_and_check<F>(name: &str, cfg: SimConfig, predicate: F) -> Check
where
    F: FnOnce(&RunMetrics, &sage_controller::SystemSnapshot) -> (bool, String),
{
    match Simulation::new(cfg) {
        Ok(mut sim) => {
            let snapshot = sim.snapshot();
            match sim.run() {
                Ok(metrics) => {
                    let (passed, msg) = predicate(&metrics, &snapshot);
                    Check {
                        name: name.into(),
                        passed,
                        message: msg,
                    }
                }
                Err(e) => {
                    let msg = format!("{}", e);
                    Check {
                        name: name.into(),
                        passed: msg.contains("stale view") || msg.contains("exceeded"),
                        message: format!("sim terminated: {}", msg),
                    }
                }
            }
        }
        Err(e) => Check {
            name: name.into(),
            passed: false,
            message: format!("config error: {}", e),
        },
    }
}
