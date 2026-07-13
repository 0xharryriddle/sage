# SAGE Claim-to-Evidence Matrix (submission gate)

Purpose: every load-bearing paper claim → generating command → retained artifact →
verification status. Built after the storage-plan debate (see OUTCOME_storage_plan.md)
established the finish line is evidence reconciliation, not disk-storage engineering.

Verification legend: VERIFIED = number reduced from raw CSV this session and matches
paper; MATCH = direct CSV row equals paper table; FLAGGED = prose imprecision found,
needs edit; PENDING = needs rerun/check before submission.

## Tier 1 — Deterministic simulator (RQ1 migration cost)

| # | Claim (paper) | Location | Artifact | Command | Status |
|---|---|---|---|---|---|
| C1 | SAGE cutover-evidence latency mean **105.6 µs** | abstract:98, RQ1:1601, tab:1682 | results/raw/rq1_migration_cost.csv | run_rq1 --config config/rq1.toml | VERIFIED (60 seeds, mean 105.6 exact) |
| C2 | SAGE mean observed downtime **122.6 µs** | RQ1:1654,1700,1794 | results/raw/rq1_migration_cost.csv + rq1_stats.csv | run_rq1 | VERIFIED (122.57, rounds to 122.6) |
| C3 | SAGE vs Cox-style downtime tie: **p=1.0, effect 0** | RQ1:1654,1794 | results/raw/rq1_stats.csv | run_rq1 (stats) | MATCH (CoxStyle row: u=1800, p≈1.0, rb=0, reject=false) |
| C4 | SAGE vs HardFork tie: p≈1.0, effect 0 | RQ1 tab | rq1_stats.csv | run_rq1 | MATCH (HardFork row identical means, reject=false) |
| C5 | StopTheWorld separates: 8105.8 µs, reject | RQ1:1593 | rq1_stats.csv | run_rq1 | MATCH (mean 8105.8, p=0.0, reject=true) |
| C6 | **60** no-partition seeds/baseline, 0 safety violations | abstract:97, methodology | rq1_migration_cost.csv | run_rq1 | VERIFIED (Sage: 60 total, 60 completed, 0 violations) |

NOTE C1 vs C2: two DISTINCT metrics, not a discrepancy. rq1_stats.csv reports the
`downtime_micros` metric (122.57); 105.6 is the `cutover_latency_micros` column mean.
Both reduce from the same rq1_migration_cost.csv. Paper uses each in its own place.

## Tier 2 — Multi-process / multi-host testbed (RQ2 adversarial safety, the headline)

| # | Claim (paper) | Location | Artifact | Status |
|---|---|---|---|---|
| C7 | Same-region 6-host: SAGE **0/5**, blind hardfork **5/5** | abstract:105, tab:multihost 1963-64, RQ2:1917-24 | results/raw/multihost_fork_differential.csv | MATCH (5 sage false, 5 hardfork true) |
| C8 | WAN ~104ms RTT: SAGE **0/3**, hardfork **3/3** | abstract:106, tab:multihost 1967-68, RQ2:1933-36 | results/raw/multihost_fork_wan.csv | MATCH (3 sage false, 3 hardfork true, rtt=104) |
| C9 | Loopback testbed table: hardfork **20/20**, Cox-inspired **10/10**, Cox-threshold **20/20**, SAGE **0/20** | RQ2 tab:1820-26 | see GAP G1/G2 below | **FLAGGED-GAP** — counts do not reduce from a single retained CSV |
| C10 | Faithful (Cox-threshold) loopback forks 5/5 vs SAGE 0/5 | RQ2, coxfaithful | results/raw/coxfaithful_differential.csv | MATCH at 5-run granularity (CoxFaithful 5/5 fork_rate 1.00; SAGE 0/5 0.00) — but paper table says 20 runs (G2) |
| C11 | Wilson intervals non-overlapping | RQ2:1836-38 | coxfaithful_differential.csv carries wilson95 cols | MATCH for the 5-run rows (CoxFaithful [0.566,1.000], SAGE [-0.0,0.434]); recompute for the 20-run table rows once G1/G2 resolved |

## Tier 3 — Formal (TLA+/TLC)

| # | Claim | Location | Artifact | Status |
|---|---|---|---|---|
| C12 | Exhaustive safety + decision-uniqueness + no-abs-reversion, n∈{4,7,10} | abstract:108-110, App | formal/ + make formal | PENDING (rerun make formal, capture 8/8) |
| C13 | TLA+↔Rust conformance set-equality, 0 forks faithful | AGENTS.md, App:spec | make conformance / formal_conformance.rs | PENDING (rerun) |

## FLAGGED prose imprecisions (need edit, NOT rerun)

| # | Issue | Location | Evidence | Recommended action |
|---|---|---|---|---|
| F1 | "the **simulator** realizes the cutover certificate as an n-f Ed25519 multi-signature" — the DETERMINISTIC experiment simulator uses a MODELED SimulatedSignatureScheme (authenticates_signer()==false); real Ed25519 lives in the node testbed (`--features real-crypto`, deploy_multihost.sh:56, netns_testbed.sh:37) and the cert-size/cert-timing experiments (Makefile:138 run_cert_timing --features real-crypto). | paper.tex:2656 | verified: real-crypto gates only testbed + cert experiments, not the sim | Reword to attribute Ed25519 realization to the testbed / cert experiments (real-crypto tier), and state the deterministic simulator models signatures. Do NOT claim the sim runs Ed25519. Confirm with user before editing paper prose. |

## GAPS — provenance findings (the real finds of the matrix)

CRITICAL METHOD NOTE: the worktree has 1016 modified + many untracked files. Working-tree
CSVs are NOT reliable evidence — several are stale mid-session modifications. Every count
claim must be checked against **committed HEAD** (`git show HEAD:<path>`) or a freshly
regenerated-and-committed artifact. This section was rebuilt after that lesson bit twice.

| # | Claim | Paper says | Authoritative (HEAD) artifact | Status |
|---|---|---|---|---|
| G1 | SAGE loopback **0/20** and hardfork **20/20** | tab:1823,1826 | m3_partition_differential_20.csv: SAGE 0/20, HardFork 20/20 | NUMBERS MATCH — but artifact is **UNTRACKED** (`?? ` in git). Provenance not locked. Commit it (with generating cmd + commit hash) before submission. |
| G2 | Cox-inspired **10/10** | tab:1824 | **HEAD** results/raw/m6_sota_baseline_differential.csv: coxstyle 10/10, hardfork 10/10, sage 0/10 | **RESOLVED — FALSE ALARM.** The committed HEAD file already holds the exact 10/10. My earlier "5-run only" read was a STALE WORKING-TREE copy. See incident note below. |
| G3 | Cox-threshold **20/20** | tab:1825 | coxfaithful_differential_20.csv: CoxFaithful 20/20 [0.839,1.0], SAGE 0/20 [0,0.161] | NUMBERS MATCH — but artifact is **UNTRACKED**. Same as G1: commit with provenance before submission. |

### INCIDENT (G2) — stale-artifact misread + destructive rerun, both corrected
1. I flagged G2 "BLOCKING" after reading the WORKING-TREE m6 CSV (5 runs). The COMMITTED
   HEAD version has the full 10/10 — G2 never existed. Root cause: trusting a working-tree
   CSV in a 1016-file-dirty repo instead of `git show HEAD:`.
2. Acting on the false gap I ran `m6 ... 10 6 4`, which OVERWROTE the good file in place
   with a NON-REPRODUCIBLE result: coxstyle forked **0/10** this run (all stalled pre-cutover,
   success=0/6) — the script's own control guard failed ("coxstyle control did not trigger
   the fork detector"). This is the loopback timing fragility AGENTS.md documents (single-box
   thread starvation under partition), NOT a protocol change.
3. RECOVERED: `git show HEAD:results/raw/m6_sota_baseline_differential.csv > <path>` restored
   the authoritative 10/10 artifact; /tmp temps cleaned; file now clean vs HEAD.

LESSON (also to memory): loopback multi-process fork counts are NOT reproducible on demand on
this box; the committed artifacts are the record of a good run. Do not "refresh" them casually.
Real reruns belong on the VPS/testbed under the documented valve, then commit with provenance.

NOTE on testbed.csv: SINGLE row (validator 0, one run) — smoke/plumbing artifact, NOT the
backing for any N/M fork count. Do not cite it for run-count claims.

## Out-of-scope (debate-confirmed, do NOT gate submission)

- Durable disk-backed crash/restart recovery — explicitly future work at paper.tex:2648, :2755.
- Production HotStuff hardening (lock rules, signed votes, ancestry) — engine is a
  parameter, not the contribution; paper does not claim production HotStuff.
- FileBackend wiring into validator_proc + CutCert crash-window closure — deferred
  post-submission hardening. The window is MOOT on the current in-memory path
  (MemoryBackend put_migration_decision is a single atomic field set; process death
  loses all in-memory state regardless). It becomes real only once FileBackend is wired.

## Next actions (corrected critical path)

1. Resolve F1 (Ed25519 prose) — pending user confirm on paper edit scope.
2. Spot-check C9/C10/C11 against retained CSVs.
3. Rerun formal (C12/C13), capture pass counts.
4. VPS rerun via existing validator_proc path (no FileBackend), preserve provenance.
5. Reconcile every remaining paper number vs retained outputs; regenerate tables/figures.
6. Submission gate: fmt, workspace tests (paper feature set), verifier 18/18, formal
   conformance, negative controls, git diff --check. Archive commit + toolchain.
