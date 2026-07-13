//! Assumption 3 admissibility study (Track D / P1-F).
//!
//! Assumption 3 (paper `as:metadata`) requires that transactions finalized in the
//! reversible window `[h_c, h_r)` execute as a pure function of the ordered tx set
//! and parent state -- i.e. they do NOT consume non-deterministic block metadata
//! (block.timestamp, block.prevrandao, oracle freshness) that could differ between
//! the target attempt and a legacy replay. Under the fail-closed policy, txs that
//! touch such metadata are deferred to absolute finality rather than admitted.
//!
//! This binary QUANTIFIES how restrictive that constraint is on a realistic EVM
//! workload, replacing the paper's hand-waved "checkable constraint".
//!
//! HONESTY LABEL: this is a MODELED study, not a live-trace replay. It combines
//! (a) a published transaction-category distribution and (b) conservative,
//! literature-grounded per-category rates at which each category consumes
//! reversible-window-unsafe metadata. A live mainnet-trace replay requires an
//! archive node and is documented as a remaining gap (single-machine limitation).
//!
//! Distribution sources (recorded in the per-class CSV `source` notes):
//!   - Glassnode "Ethereum Transaction Type Breakdown" (22 May 2026): Vanilla ETH
//!     transfers ~48.6%, Stablecoins ~16.8%, ERC20 ~5.5%, DeFi ~2.1%, NFTs ~4.9%,
//!     Bridges ~0.1%, Other ~22.0% of transaction count.
//!   - Pirlea, "Ethereum smart contract usage" (4.5M tx): 30.7% transfers,
//!     51.6% single calls (81.9% of which are ERC20 direct transfers), 17.6% multi.
//!
//! Per-category metadata-dependence rates are conservative engineering estimates
//! (documented inline); the binary recomputes admissibility from them so a reviewer
//! can substitute their own rates and re-derive A.
//!
//! Run: cargo run -p sage-experiments --bin run_assumption3
use sage_experiments::common::write_csv;
use sage_experiments::error::ExperimentResult;
use serde::Serialize;
use std::path::PathBuf;

/// One transaction category: its share of total tx count and the fraction of
/// txs in the category that consume reversible-window-unsafe block metadata
/// (block.timestamp / prevrandao / live oracle reads).
struct TxClass {
    name: &'static str,
    population_share: f64,
    /// Fraction of this category that touches non-deterministic metadata and is
    /// therefore DEFERRED under the fail-closed policy.
    metadata_dependence: f64,
    rationale: &'static str,
}

/// Default model. Shares sum to ~1.0 (Glassnode May-2026 normalized). Metadata
/// rates are conservative (rounded UP toward "unsafe" where uncertain).
fn default_model() -> Vec<TxClass> {
    vec![
        TxClass {
            name: "vanilla_eth_transfer",
            population_share: 0.486,
            metadata_dependence: 0.00,
            rationale: "pure EOA->EOA value move; no contract code, no block metadata",
        },
        TxClass {
            name: "stablecoin_transfer",
            population_share: 0.168,
            metadata_dependence: 0.01,
            rationale: "ERC20 balance update; deterministic except rare timestamped hooks",
        },
        TxClass {
            name: "erc20_transfer",
            population_share: 0.055,
            metadata_dependence: 0.02,
            rationale: "ERC20 transfer/approve; deterministic balance/allowance state",
        },
        TxClass {
            name: "nft",
            population_share: 0.049,
            metadata_dependence: 0.50,
            rationale:
                "mint paths often use prevrandao/timestamp; marketplace transfers deterministic",
        },
        TxClass {
            name: "defi",
            population_share: 0.021,
            metadata_dependence: 0.80,
            rationale: "DEX swaps use block.timestamp deadlines; lending reads live price oracles",
        },
        TxClass {
            name: "bridge",
            population_share: 0.001,
            metadata_dependence: 0.40,
            rationale: "mixed; some lock/mint paths timestamp-bound, some plain transfers",
        },
        TxClass {
            name: "other",
            population_share: 0.220,
            metadata_dependence: 0.30,
            rationale: "routers/multicall frequently carry deadlines; remainder deterministic",
        },
    ]
}

#[derive(Debug, Serialize)]
struct ClassRow {
    experiment: String,
    class: String,
    population_share: f64,
    metadata_dependence: f64,
    admissible_share: f64,
    deferred_share: f64,
    rationale: String,
}

#[derive(Debug, Serialize)]
struct SummaryRow {
    experiment: String,
    total_population: f64,
    admissible_rate: f64,
    deferred_rate: f64,
    contingency_branch: String,
    model_kind: String,
}

fn contingency_branch(a: f64) -> &'static str {
    if a >= 0.80 {
        "A>=80%: keep fail-closed default; defer the unsafe tail to absolute finality"
    } else if a >= 0.50 {
        "50%<=A<80%: add PinEnvironment deterministic-wrapper policy; report with/without"
    } else {
        "A<50%: implement replay-context handling OR re-scope to deterministic chains"
    }
}

fn main() -> ExperimentResult<()> {
    let out_dir = PathBuf::from("results/raw");
    std::fs::create_dir_all(&out_dir)?;

    let model = default_model();
    let total: f64 = model.iter().map(|c| c.population_share).sum();

    let mut class_rows = Vec::new();
    let mut admissible_mass = 0.0;
    for c in &model {
        let admissible = c.population_share * (1.0 - c.metadata_dependence);
        let deferred = c.population_share * c.metadata_dependence;
        admissible_mass += admissible;
        class_rows.push(ClassRow {
            experiment: "assumption3_admissibility".into(),
            class: c.name.into(),
            population_share: c.population_share,
            metadata_dependence: c.metadata_dependence,
            admissible_share: admissible,
            deferred_share: deferred,
            rationale: c.rationale.into(),
        });
    }

    // Admissibility A = admissible mass / total population mass.
    let a = admissible_mass / total;
    let class_path = out_dir.join("assumption3_admissibility.csv");
    write_csv(&class_path, &class_rows)?;

    let summary = vec![SummaryRow {
        experiment: "assumption3_admissibility".into(),
        total_population: total,
        admissible_rate: a,
        deferred_rate: 1.0 - a,
        contingency_branch: contingency_branch(a).into(),
        model_kind: "modeled_published_distribution_not_live_trace".into(),
    }];
    let summary_path = out_dir.join("assumption3_summary.csv");
    write_csv(&summary_path, &summary)?;

    println!(
        "Assumption 3 admissibility A = {:.1}% (deferred {:.1}%) over {} categories",
        a * 100.0,
        (1.0 - a) * 100.0,
        model.len()
    );
    println!("Contingency branch: {}", contingency_branch(a));
    println!(
        "Wrote {} and {}",
        class_path.display(),
        summary_path.display()
    );
    Ok(())
}
