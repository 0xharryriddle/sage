# SAGE — Internal Comparison Conditions and Limits

This note describes retained same-harness controls, not a fairness certificate for
full-protocol performance or an authentic external baseline. The current locked-profile
manuscript is [`paper/SAGE_ThaiCong_IEEE/main.tex`](paper/SAGE_ThaiCong_IEEE/main.tex);
the tracked [`RESPONSE_TO_REVIEWS.md`](RESPONSE_TO_REVIEWS.md) records historical
responses with a current-scope warning. Neither is a full trial ledger. Equal seed
labels and shared configuration fields do not remove source-policy rejection,
differing target quorums, or the deliberately configured stop-the-world halt in RQ1.

---

## 1. External reference versus internal controls

Cox (Z. Li et al., "Cox: a reliable runtime switching protocol for BFT
consensus algorithms," *Blockchain: Research and Applications* 7(3), art.
100345, 2026, DOI 10.1016/j.bcra.2025.100345) is a published live BFT-core
switching reference. It uses a committed configuration block, matching signed
termination checkpoints, initialization of the replacement core, and chained
epoch-change recovery for lagging nodes; its argument permits different BFT-core
quorum values. The internal `CoxStyle` cost arm executes none of that checkpoint
or recovery path and is **not an authentic Cox implementation or benchmark**.

The retained RQ1 comparison at `(n,f)=(20,6)` uses majority PoA source quorum
`q1=11` and retirement threshold `qF=14`: `qF+q1=25 <= n+f=26`, so the locked
protocol's source-policy admission condition rejects it. SAGE's target quorum
is `q=14`, while hard-fork and Cox-style controls use `q=13`; reconfiguration
does not swap engines. The approximately 64× SAGE/STW event-time gap is chiefly
the configured STW halt, not a measured full-path or client-visible advantage.
These are historical partial-path simulator controls, not an admitted,
quorum-matched comparison identifying the cost of certification.

The separate RQ2 process controls at `n=6,f=1` distinguish the threshold
effect (`q=3` versus `q=5` under a balanced `3/3` split) from the gate's
authorization effect (gated versus ungated `q=4` under a `4/2` split). The
balanced threshold contrast does not isolate a gate-fork effect, test Cox, or
establish `n-f` as the unique safe threshold.

---

## 2. Shared settings are not a one-factor comparison

The listed settings are shared within each stated contrast, but resolved target
quorum policy and the STW maintenance halt also differ. Do not infer equivalent
switching semantics or an isolated strategy effect from this table.

### 2a. RQ1 partial-path cost and separate three-arm overhead (`../config/rq1.toml`)

| Knob | Shared value | Applies to |
|---|---|---|
| Validators `n` | 20 | within each stated contrast |
| Fault bound `f` | 6 | within each stated contrast |
| Max height | 40 | within each stated contrast |
| Mean message delay | 40 µs | within each stated contrast |
| Migration schedule `(h_d, h_c, h_r)` | (10, 30, 40) | within each stated contrast |
| Stability threshold `κ` | 8 | within each stated contrast |
| Progress timeout `τ` | 1 block | within each stated contrast |
| State accounts | 16 | within each stated contrast |
| Initial balance | 1000 | within each stated contrast |
| Txs/block (overhead sweep) | {10,25,50,100,250,500} | same load points for three overhead arms |
| Retained seeds | RQ1 cost: 60/arm; overhead: 0–9 (10/load/arm) | shared labels within each campaign |
| Hardware (overhead) | one host | not independent client throughput |
| Measurement window (overhead) | full run, process wall-clock | same metric definition |

`../config/rq1.toml` provides one base configuration and seed `0`, not the
campaign seed list. The retained 180-row overhead CSV contains **ten** seed
labels at each of six loads across three arms, but this target checkout does
not contain the historical `run_overhead` driver; do not mistake the retained
CSV or its presentation rebuild for a fresh ten-seed run. The documented
driver cloned the configuration per case and set strategy, seed, load and event
budget; strategy-specific quorum selection was not made identical by sharing
the base file. Seed-label pairing is not common-random-number pairing after
strategies change event progression.

### 2b. Partition controls (`../config/safety.toml`, testbed)

| Knob | Shared value | Applies to |
|---|---|---|
| Validators `n` | 6 | stated loopback controls |
| Fault bound `f` | 1 | stated loopback controls |
| Partition split | 3/3 balanced at `h_c` | threshold contrast |
| Partition start | cutover height `h_c` | threshold contrast |
| Transport | local TCP processes | not authenticated independent hosts |

The balanced-partition threshold arms differ in target quorum (`q=3` versus
`q=5`); both gated and ungated `q=5` controls have no observed conflict in the
five-trial contrast. The gate-authorization contrast instead holds `q=4`
fixed under `4/2`. See the manuscript's RQ2 and `tab:rq2controls` for the
distinct campaigns and their retained-count boundaries.

---

## 3. Metrics — internal same-harness definitions

| Metric | Internal definition | Scope |
|---|---|---|
| Inferred validator commit rate | configured tx/block × max finalized height / full-run process wall-clock seconds | simulator-capacity proxy, not client-completed TPS |
| RQ1 maximum finalization gap | maximum successive-finalization gap in simulator event time | not client downtime or full certification pause |
| CPU % | Δ(utime+stime)/wall from `/proc/self/stat` | local process observation |
| RSS | resident set at run end | local process observation |
| Realized conflict (process tier) | distinct finalized state roots at one height | constructed schedules, not operational risk rate |

Absolute rates cannot be compared across papers or read as deployment
throughput. Even same-host relative rates here do not isolate shadow work:
SAGE uses target `q=14`, `CoxStyle` uses `q=13`, and the baseline remains on
the source. Timing and shared-host scheduling effects remain possible.

---

## 4. Retained three-arm overhead (10 seeds/load/arm)

The ten-seed retained rows use the settings above, but the arms are not
quorum-matched or a full switching-path experiment. Percentages below use
the source-only reconfiguration arm as the 100% inferred-rate reference:

| Tx/block | Baseline | SAGE (rel.) | Cox-style (rel.) | SAGE vs Cox-style |
|---|---|---|---|---|
| 10  | 100% | +4.5% | +5.0% | −0.5% |
| 25  | 100% | +4.7% | +5.5% | −0.8% |
| 50  | 100% | +4.1% | +5.4% | −1.2% |
| 100 | 100% | +4.0% | +6.1% | −2.0% |
| 250 | 100% | +4.6% | +6.3% | −1.6% |
| 500 | 100% | +4.6% | +6.8% | −2.1% |

**Result:** SAGE's inferred-rate point estimates are 0.49%–2.10% below the
internal Cox-style control and 3.96%–4.71% above the source-only baseline.
The canonical `tab:overhead` reports pointwise paired-seed bootstrap intervals;
the SAGE/Cox-style interval at 25 tx/block includes zero. These descriptive
ratios establish neither throughput parity nor isolated shadow-validation
overhead, certification cost, client throughput, or production performance.

---

## 5. Retained-input check, not an experiment rerun

From the repository root, `python3 -B docs/paper/SAGE_ThaiCong_IEEE/rebuild.py
--check-data` checks pinned presentation inputs. It does not run the overhead
campaign, recover unavailable individual process trial records, execute the
complete committee-wide certificate path, or establish submission readiness.
Historical retained inputs must not be overwritten when reviewing provenance.
