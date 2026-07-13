# SAGE — Experiment Fairness Contract

This document is the "both runners run exactly 100 m" contract for every
comparative experiment in the SAGE paper. It exists to answer, concretely and
checkably, the two questions a reviewer (and our mentor) asks of any comparison:

1. **Is the baseline a real state-of-the-art (SOTA) method?**
2. **Was the experimental setup fair — did every method run under identical
   conditions?**

Everything below is generated from the actual config files and source, not
asserted in prose. Where a knob differs between experiments (e.g. RQ1 vs the
partition safety campaign), it is because the *research question* requires it,
and it is held identical **across all strategies within that experiment**.

---

## 1. The SOTA baseline and why it is SOTA

The closest published state-of-the-art to SAGE is **Cox** (Z. Li, L. Cai, W.
Qiu, F. Huang, H. Duan, C. Yuan, "Cox: a reliable runtime switching protocol for
BFT consensus algorithms," *Blockchain: Research and Applications* 7(3), art.
100345, June 2026, DOI 10.1016/j.bcra.2025.100345, open access). It is the most
recent peer-reviewed protocol for **live runtime switching of the consensus
engine** with no system downtime, which is exactly SAGE's problem class.

**Cox's own stated scope (verbatim, Sec. 3.4 + Sec. 7):** Cox is "only
applicable to BFT consensus protocols in a semi-synchronous network with clear
quorum sizes uniformly agreed upon by consensus clusters." It switches
**homogeneous** BFT↔BFT engines (demonstrated PBFT↔HotStuff), selects the single
larger quorum `q = max(q_old, q_new)` from that **uniform** quorum family, and
has **no reversibility / rollback** mechanism (its recovery sub-protocol only
catches lagging nodes *up* to the new epoch; it never reverts a switch).

**Implication for fairness.** SAGE's contribution is the **heterogeneous**
PoA→BFT crossing, where the legacy engine has an authority/honest-majority
quorum and no Byzantine quorum family at all. That case is **outside Cox's own
stated applicability**. Therefore:

- On the axis Cox *does* address (live-switch **cost**: throughput + switch
  duration under normal conditions), we run Cox-style and SAGE under **identical
  conditions** and report the result honestly (they tie — see §3).
- On the axis Cox does **not** address (heterogeneous-boundary **safety** under
  partition), we report the differential and attribute it to the `n−f` gate,
  never claiming to "beat" Cox on a problem Cox never claimed to solve.

This is the honest, defensible framing: **not "SAGE beats Cox," but "Cox ties
SAGE on Cox's own metric, and the `n−f` gate is what the heterogeneous case
additionally requires — a case Cox's authors explicitly scope out."**

---

## 2. Fairness contract — knobs held identical across ALL strategies

Every comparative experiment fixes the following knobs to a single shared value
used by **every** strategy arm (baseline / SAGE / Cox-style / hard-fork /
stop-the-world / reconfig-only). This is the "same 100 m track" guarantee.

### 2a. RQ1 migration-cost + 3-arm overhead (`config/rq1.toml`)

| Knob | Shared value | Applies to |
|---|---|---|
| Validators `n` | 20 | all arms |
| Fault bound `f` | 6 | all arms |
| Max height | 40 | all arms |
| Mean message delay | 40 µs | all arms |
| Migration schedule `(h_d, h_c, h_r)` | (10, 30, 40) | all arms |
| Stability threshold `κ` | 8 | all arms |
| Progress timeout `τ` | 1 block | all arms |
| State accounts | 16 | all arms |
| Initial balance | 1000 | all arms |
| Txs/block (overhead sweep) | {10,25,50,100,250,500} | all arms, same set |
| Seeds | {0,1,2,3,4} (5 per point) | all arms, same seeds |
| Hardware | single host, same process | all arms |
| Measurement window | full run, wall-clock | all arms |
| RNG | seeded, deterministic | all arms |

The **only** variable across arms is the migration *strategy* itself
(`StrategyKind`). Every other knob is byte-identical, read from the same
`SimConfig`. This is enforced in code: `run_overhead.rs` clones one `base_cfg`
and only mutates `cfg.strategy` per arm (see `run_case`).

### 2b. Partition safety campaign (`config/safety.toml`, testbed)

| Knob | Shared value | Applies to |
|---|---|---|
| Validators `n` | 6 | all arms |
| Fault bound `f` | 1 (BFT quorum 3) | all arms |
| Partition split | 3/3 balanced at `h_c` | all arms |
| Partition start | cutover height `h_c` | all arms |
| Trials | 10 per strategy (loopback) | all arms |
| Transport | real TCP | all arms |

Only the strategy's cutover gate differs. Hard-fork, Cox-style, and SAGE all see
the identical partition at the identical height with the identical trial count.

---

## 3. Metrics — same definitions, same instruments (Cox's own metric set)

Cox reports **(a) committed TPS before/after switch** and **(b) switch
duration**. To make the comparison fair on Cox's own terms, SAGE reports the
same two families under the same harness:

| Metric | Definition | Cox analogue |
|---|---|---|
| Committed TPS | committed txs / wall-clock seconds | Cox Fig. 5/7 |
| Cutover/switch duration | last-legacy-final → first-target-final | Cox Fig. 6 "duration" |
| CPU % | Δ(utime+stime)/wall from `/proc/self/stat` | (Cox: implicit) |
| RSS | resident set at run end | (Cox: implicit) |
| Fork rate | # runs with two finalized blocks at one height | (Cox: n/a — no partition safety experiment) |

**Honest note on absolute TPS.** Absolute wall-clock TPS is machine- and
simulator-capacity-dependent (Cox ran on 8-core/16 GB CentOS; our simulator is a
deterministic event loop). Absolute numbers are therefore **not** comparable
across papers. The scientifically stable, fair quantity is the **relative spread
across arms run on the same machine under the same config** — which is what
`tab:overhead` reports (normalized to the reconfiguration baseline = 100%).

---

## 4. The 3-arm same-conditions result (from `run_overhead`, 5 seeds/point)

All three arms (reconfiguration baseline / SAGE dual-run / Cox-style live switch)
run under the identical `config/rq1.toml` knobs above. Throughput normalized to
baseline = 100%:

| Tx/block | Baseline | SAGE (rel.) | Cox-style (rel.) | SAGE vs Cox-style |
|---|---|---|---|---|
| 10  | 100% | +8.5%  | +9.2%  | −0.6% |
| 25  | 100% | +13.5% | +10.4% | +2.8% |
| 50  | 100% | +8.7%  | +9.2%  | −0.4% |
| 100 | 100% | +6.3%  | +8.5%  | −2.0% |
| 250 | 100% | +8.4%  | +8.7%  | −0.3% |
| 500 | 100% | +1.6%  | +4.1%  | −2.5% |

**Result:** SAGE and the Cox-style SOTA arm are **statistically
indistinguishable** on throughput (SAGE within −2.5% to +2.8% of Cox-style
across all load points), under provably identical conditions. The two live-switch
designs are **cost-equivalent**; they separate only on adversarial
heterogeneous-boundary **safety** (RQ2), which is the axis Cox's design does not
address. This is the fair "both ran 100 m, and they tie on speed; SAGE
additionally clears a hurdle Cox's track never had" result.

---

## 5. Reproduction

```bash
# 3-arm overhead (fair same-config TPS/CPU/RSS)
cargo run -p sage-experiments --bin run_overhead -- --config config/rq1.toml
#   -> results/raw/dual_run_overhead.csv (90 rows: 6 loads x 5 seeds x 3 arms)
#   -> results/figures/dual_run_overhead_summary.csv (per-load 3-arm summary)
#   -> results/tables/dual_run_overhead.tex

# partition safety differential (testbed)
bash scripts/m3_partition_experiment.sh
```

Every knob above is read from the committed config files; no per-arm override
exists other than `StrategyKind`. That invariance is the fairness guarantee.
