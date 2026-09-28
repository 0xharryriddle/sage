//! Generate LaTeX tables from raw experiment CSVs.
//! Reads results/raw/*.csv and outputs summary tables.
//!
//! Values are summarized from the supplied CSV rows; generating a table does not
//! establish that the CSV is verified evidence or suitable for publication.
//!
//! * Strategy arms are derived from supplied rows, never from a fixed list.
//! * `safety_violation` and disjoint-quorum exposure remain separate columns;
//!   exposure is a precondition, not a recorded violation.
use clap::Parser;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser)]
struct Cli {
    #[arg(long, default_value = "results/raw")]
    raw_dir: PathBuf,
    #[arg(long, default_value = "results/tables")]
    out_dir: PathBuf,
    /// Continue when optional inputs are missing.
    #[arg(long, default_value_t = false)]
    allow_missing: bool,
}

#[derive(Debug, Deserialize)]
struct Rq1Row {
    strategy: String,
    #[allow(dead_code)]
    seed: u64,
    run_status: String,
    finalized_blocks: u64,
    safety_violation: bool,
    downtime_micros: u64,
    finality_gap: u64,
}

#[derive(Debug, Deserialize)]
struct Rq2Row {
    strategy: String,
    partition_split: Option<u32>,
    partition_duration_blocks: Option<u64>,
    safety_violation: bool,
    migration_success: bool,
    disjoint_quorum_windows: u64,
    run_status: String,
}

#[derive(Debug, Deserialize)]
struct Rq3Row {
    kappa: u64,
    safety_violation: bool,
    migration_success: bool,
    cutover_latency_micros: u64,
    run_status: String,
}

#[derive(Debug, Deserialize)]
struct Rq4Row {
    abort_height: u64,
    rollback_occurred: bool,
    rollback_latency_micros: u64,
    absolute_reversion_rejected: bool,
    safety_violation: bool,
    run_status: String,
}

#[derive(Debug, Deserialize)]
struct ManifestRow {
    test_name: String,
    passed: bool,
    #[allow(dead_code)]
    message: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    fs::create_dir_all(&cli.out_dir)?;

    let rq1_path = cli.raw_dir.join("rq1_migration_cost.csv");
    require_input(&rq1_path, cli.allow_missing)?;
    if rq1_path.exists() {
        let rows = load::<Rq1Row>(&rq1_path)?;
        write_rq1_table(&cli.out_dir, &rows)?;
    }

    let rq2_path = cli.raw_dir.join("rq2_partition_safety.csv");
    require_input(&rq2_path, cli.allow_missing)?;
    if rq2_path.exists() {
        let rows = load::<Rq2Row>(&rq2_path)?;
        write_rq2_table(&cli.out_dir, &rows)?;
    }

    let rq3_path = cli.raw_dir.join("rq3_kappa_ablation.csv");
    require_input(&rq3_path, cli.allow_missing)?;
    if rq3_path.exists() {
        let rows = load::<Rq3Row>(&rq3_path)?;
        write_rq3_table(&cli.out_dir, &rows)?;
    }

    let rq4_path = cli.raw_dir.join("rq4_rollback.csv");
    require_input(&rq4_path, cli.allow_missing)?;
    if rq4_path.exists() {
        let rows = load::<Rq4Row>(&rq4_path)?;
        write_rq4_table(&cli.out_dir, &rows)?;
    }

    let manifest_path = cli.raw_dir.join("manifest_tests.csv");
    require_input(&manifest_path, cli.allow_missing)?;
    if manifest_path.exists() {
        let rows = load::<ManifestRow>(&manifest_path)?;
        write_manifest_table(&cli.out_dir, &rows)?;
    }

    let safety_path = cli.raw_dir.join("safety_partition.csv");
    require_input(&safety_path, cli.allow_missing)?;
    if safety_path.exists() {
        println!("Safety CSV found at {}", safety_path.display());
        // The dedicated adversarial-safety sweep is reported through the RQ2
        // table above; requiring the input here prevents silent paper/artifact
        // drift if that sweep is dropped from a run.
    }

    Ok(())
}

fn require_input(path: &Path, allow_missing: bool) -> Result<(), Box<dyn Error>> {
    if !path.exists() && !allow_missing {
        return Err(format!(
            "required input {} is missing (pass --allow-missing to continue)",
            path.display()
        )
        .into());
    }
    if !path.exists() {
        eprintln!("warning: missing {}, skipping", path.display());
    }
    Ok(())
}

fn load<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Vec<T>, Box<dyn Error>> {
    let mut reader = csv::Reader::from_path(path)?;
    let mut rows = Vec::new();
    for result in reader.deserialize() {
        rows.push(result?);
    }
    Ok(rows)
}

/// Wilson score interval at 95%. Used for every rate reported as a fraction of
/// trials, so a 0/N result carries an honest upper bound instead of reading as
/// a proven zero.
fn wilson_ci(successes: usize, total: usize) -> (f64, f64) {
    if total == 0 {
        return (0.0, 1.0);
    }
    let z = 1.96_f64;
    let n = total as f64;
    let p = successes as f64 / n;
    let denom = 1.0 + z * z / n;
    let center = (p + z * z / (2.0 * n)) / denom;
    let half = z * ((p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt()) / denom;
    ((center - half).max(0.0), (center + half).min(1.0))
}

/// Escape the characters LaTeX treats specially. Experiment identifiers are
/// snake_case, so an unescaped `_` is a math-mode subscript error that breaks
/// the build (or silently renders wrong) rather than a cosmetic issue.
fn latex_escape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '_' | '%' | '$' | '&' | '#' | '{' | '}' => {
                out.push('\\');
                out.push(ch);
            }
            '\\' => out.push_str("\\textbackslash{}"),
            '^' => out.push_str("\\textasciicircum{}"),
            '~' => out.push_str("\\textasciitilde{}"),
            _ => out.push(ch),
        }
    }
    out
}

fn table_header(caption: &str, label: &str, colspec: &str) -> String {
    let mut out = String::new();
    out.push_str("% Auto-generated by generate_tables -- do not edit manually\n");
    out.push_str("\\begin{table}[ht]\n");
    out.push_str("\\centering\n");
    out.push_str(&format!("\\caption{{{}}}\n", caption));
    out.push_str(&format!("\\label{{{}}}\n", label));
    out.push_str(&format!("\\begin{{tabular}}{{{}}}\n", colspec));
    out.push_str("\\toprule\n");
    out
}

fn table_footer() -> String {
    let mut out = String::new();
    out.push_str("\\bottomrule\n");
    out.push_str("\\end{tabular}\n");
    out.push_str("\\end{table}\n");
    out
}

fn write_out(out_dir: &Path, name: &str, body: String) -> Result<(), Box<dyn Error>> {
    let path = out_dir.join(name);
    fs::write(&path, body)?;
    println!("Wrote {}", path.display());
    Ok(())
}

/// Distinct strategy arms present in the data, ordered for display.
///
/// Arms are derived from observed rows; preferred names only control display
/// order. Unknown strategy names remain visible rather than being silently
/// dropped from the generated table.
const PREFERRED_ORDER: [&str; 6] = [
    "Sage",
    "StopTheWorld",
    "HardFork",
    "ReconfigOnly",
    "CoxStyle",
    "SageBlind",
];

fn ordered_strategies(observed: impl Iterator<Item = String>) -> Vec<String> {
    let mut seen: Vec<String> = observed.collect();
    seen.sort();
    seen.dedup();
    let mut ordered: Vec<String> = PREFERRED_ORDER
        .iter()
        .filter(|name| seen.iter().any(|s| s == *name))
        .map(|name| name.to_string())
        .collect();
    for name in seen {
        if !PREFERRED_ORDER.contains(&name.as_str()) {
            ordered.push(name);
        }
    }
    ordered
}

fn write_rq1_table(out_dir: &Path, rows: &[Rq1Row]) -> Result<(), Box<dyn Error>> {
    let mut out = table_header(
        "Migration cost against baselines (RQ1), summarized from supplied completed CSV rows.",
        "tab:rq1",
        "lrrrr",
    );
    out.push_str(
        "Strategy & Finalized Blocks & Downtime (ms) & Finality Gap & Safety Violation \\\\\n",
    );
    out.push_str("\\midrule\n");

    for strategy in ordered_strategies(rows.iter().map(|r| r.strategy.clone())) {
        let group: Vec<&Rq1Row> = rows
            .iter()
            .filter(|r| r.strategy == strategy && r.run_status == "completed")
            .collect();
        if group.is_empty() {
            continue;
        }
        let n = group.len();
        let mean_blocks: f64 =
            group.iter().map(|r| r.finalized_blocks as f64).sum::<f64>() / n as f64;
        // Observed downtime reported in milliseconds for readability.
        let mean_downtime_ms: f64 =
            group.iter().map(|r| r.downtime_micros as f64).sum::<f64>() / n as f64 / 1000.0;
        let mean_gap: f64 = group.iter().map(|r| r.finality_gap as f64).sum::<f64>() / n as f64;
        let violation_count = group.iter().filter(|r| r.safety_violation).count();

        out.push_str(&format!(
            "{} & {:.1} & {:.3} & {:.1} & {}/{} \\\\\n",
            latex_escape(&strategy),
            mean_blocks,
            mean_downtime_ms,
            mean_gap,
            violation_count,
            n
        ));
    }

    out.push_str(&table_footer());
    write_out(out_dir, "rq1_table.tex", out)
}

/// Partition safety (RQ2).
///
/// Recorded safety violations and disjoint-quorum exposure are separate. An
/// exposure window is a precondition for a fork, not an observed conflict.
/// Supplied simulator CSV rows do not establish a cross-process safety result.
fn write_rq2_table(out_dir: &Path, rows: &[Rq2Row]) -> Result<(), Box<dyn Error>> {
    let mut out = table_header(
        "Partition safety (RQ2), summarized from supplied completed simulator rows. \\emph{Forks} counts recorded safety violations; \\emph{exposure} counts trials with a recorded disjoint-quorum window, not observed forks. The simulator does not establish cross-process fork behavior. Intervals are 95\\% Wilson.",
        "tab:rq2",
        "lrrrrr",
    );
    out.push_str(
        "Strategy & Trials & Forks & Exposure & Exposure rate (95\\% CI) & Migrated \\\\\n",
    );
    out.push_str("\\midrule\n");

    for strategy in ordered_strategies(rows.iter().map(|r| r.strategy.clone())) {
        // Fail-closed: a row whose run did not complete yields no verdict and is
        // excluded from the denominator rather than counted as a no-fork
        // observation. Counting an unparsed run as success manufactures evidence.
        let group: Vec<&Rq2Row> = rows
            .iter()
            .filter(|r| r.strategy == strategy && r.run_status == "ok")
            .collect();
        let inconclusive = rows
            .iter()
            .filter(|r| r.strategy == strategy && r.run_status != "ok")
            .count();
        if group.is_empty() {
            continue;
        }
        let n = group.len();
        let forks = group.iter().filter(|r| r.safety_violation).count();
        let exposed = group
            .iter()
            .filter(|r| r.disjoint_quorum_windows > 0)
            .count();
        let migrated = group.iter().filter(|r| r.migration_success).count();
        let (lo, hi) = wilson_ci(exposed, n);

        let mut label = latex_escape(&strategy);
        if inconclusive > 0 {
            label.push_str(&format!(" ({} inconclusive)", inconclusive));
        }
        out.push_str(&format!(
            "{} & {} & {}/{} & {}/{} & {:.3} [{:.3}, {:.3}] & {}/{} \\\\\n",
            label,
            n,
            forks,
            n,
            exposed,
            n,
            exposed as f64 / n as f64,
            lo,
            hi,
            migrated,
            n
        ));
    }

    // Partition geometry actually executed, so a reader can tell which splits
    // the rates above cover instead of assuming the full grid ran.
    let mut splits: Vec<u32> = rows.iter().filter_map(|r| r.partition_split).collect();
    splits.sort_unstable();
    splits.dedup();
    let mut durations: Vec<u64> = rows
        .iter()
        .filter_map(|r| r.partition_duration_blocks)
        .collect();
    durations.sort_unstable();
    durations.dedup();
    out.push_str("\\midrule\n");
    out.push_str(&format!(
        "\\multicolumn{{6}}{{l}}{{\\footnotesize Splits executed: {:?}; durations (blocks): {:?}}} \\\\\n",
        splits, durations
    ));

    out.push_str(&table_footer());
    write_out(out_dir, "rq2_table.tex", out)
}

/// Kappa ablation (RQ3): how the shadow-agreement streak requirement trades
/// against cutover latency. Unsafe-cutover rate is observed, never inferred.
fn write_rq3_table(out_dir: &Path, rows: &[Rq3Row]) -> Result<(), Box<dyn Error>> {
    let mut out = table_header(
        "Readiness-threshold ($\\kappa$) ablation (RQ3), summarized from supplied completed simulator rows. Safety-violation rates use recorded CSV fields. Intervals are 95\\% Wilson.",
        "tab:rq3",
        "rrrrr",
    );
    out.push_str(
        "$\\kappa$ & Trials & Unsafe cutover (95\\% CI) & Migrated & Mean cutover latency ($\\mu$s) \\\\\n",
    );
    out.push_str("\\midrule\n");

    let mut groups: BTreeMap<u64, Vec<&Rq3Row>> = BTreeMap::new();
    for row in rows.iter().filter(|r| r.run_status == "completed") {
        groups.entry(row.kappa).or_default().push(row);
    }
    for (kappa, group) in groups {
        let n = group.len();
        let unsafe_count = group.iter().filter(|r| r.safety_violation).count();
        let migrated = group.iter().filter(|r| r.migration_success).count();
        let (lo, hi) = wilson_ci(unsafe_count, n);
        let mean_latency: f64 = group
            .iter()
            .map(|r| r.cutover_latency_micros as f64)
            .sum::<f64>()
            / n as f64;
        out.push_str(&format!(
            "{} & {} & {}/{} [{:.3}, {:.3}] & {}/{} & {:.1} \\\\\n",
            kappa, n, unsafe_count, n, lo, hi, migrated, n, mean_latency
        ));
    }

    out.push_str(&table_footer());
    write_out(out_dir, "rq3_table.tex", out)
}

/// Rollback sweep (RQ4), grouped by abort height. The rejection column reflects
/// the recorded flag in the supplied CSV; it is not independent proof of refusal.
fn write_rq4_table(out_dir: &Path, rows: &[Rq4Row]) -> Result<(), Box<dyn Error>> {
    let mut out = table_header(
        "Rollback sweep (RQ4), grouped by abort height, summarized from supplied completed simulator rows. \\emph{Absolute reversion rejected} reflects the recorded CSV flag, not an independently verified finality result.",
        "tab:rq4",
        "rrrrrr",
    );
    out.push_str(
        "Abort height & Trials & Rolled back & Abs. reversion rejected & Safety violations & Mean rollback latency ($\\mu$s) \\\\\n",
    );
    out.push_str("\\midrule\n");

    let mut groups: BTreeMap<u64, Vec<&Rq4Row>> = BTreeMap::new();
    for row in rows.iter().filter(|r| r.run_status == "ok") {
        groups.entry(row.abort_height).or_default().push(row);
    }
    for (abort_height, group) in groups {
        let n = group.len();
        let rolled = group.iter().filter(|r| r.rollback_occurred).count();
        let rejected = group
            .iter()
            .filter(|r| r.absolute_reversion_rejected)
            .count();
        let violations = group.iter().filter(|r| r.safety_violation).count();
        // Latency is meaningful only for trials that actually rolled back;
        // averaging in the zeros of refused aborts would understate it.
        let latencies: Vec<f64> = group
            .iter()
            .filter(|r| r.rollback_occurred)
            .map(|r| r.rollback_latency_micros as f64)
            .collect();
        let mean_latency = if latencies.is_empty() {
            0.0
        } else {
            latencies.iter().sum::<f64>() / latencies.len() as f64
        };
        out.push_str(&format!(
            "{} & {} & {}/{} & {}/{} & {}/{} & {:.1} \\\\\n",
            abort_height, n, rolled, n, rejected, n, violations, n, mean_latency
        ));
    }

    out.push_str(&table_footer());
    write_out(out_dir, "rq4_table.tex", out)
}

/// Manifest test rows from supplied CSV. The expectation column is inferred from
/// the test name and is not independent verification of its actual assertion.
fn write_manifest_table(out_dir: &Path, rows: &[ManifestRow]) -> Result<(), Box<dyn Error>> {
    let mut out = table_header(
        "Manifest test outcomes from supplied CSV rows. Expectation is inferred from each test name; the outcome reflects the recorded passed flag.",
        "tab:manifest",
        "lll",
    );
    out.push_str("Test & Expectation & Outcome \\\\\n");
    out.push_str("\\midrule\n");

    for row in rows {
        // The producer names negative cases with "rejected"; display that
        // name-implied expectation separately from the recorded passed flag.
        let expectation = if row.test_name.contains("rejected") {
            "reject"
        } else {
            "accept"
        };
        let outcome = if row.passed { "pass" } else { "FAIL" };
        out.push_str(&format!(
            "{} & {} & {} \\\\\n",
            latex_escape(&row.test_name),
            expectation,
            outcome
        ));
    }

    let passed = rows.iter().filter(|r| r.passed).count();
    out.push_str("\\midrule\n");
    out.push_str(&format!(
        "\\multicolumn{{3}}{{l}}{{\\footnotesize {}/{} manifest tests passed}} \\\\\n",
        passed,
        rows.len()
    ));

    out.push_str(&table_footer());
    write_out(out_dir, "manifest_table.tex", out)
}
