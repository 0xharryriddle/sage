# local_client_r2 — QUARANTINED probe campaigns (2026-07-27)

**These directories are NOT evidence. Do not cite any number in them.**

Both campaigns are self-marked `status=invalid` in their own
`campaign_manifest.json`, and the harness refused to seal either one. They are
retained only for provenance of the R2 pipeline unblock, not for results.

## What they were for

R2 (client saturation curve + p50/p95/p99) per
`docs/reviews/2026-07-27-blocked-items-resolution-plan.md`. These runs were
diagnostic probes to (a) prove the client-finality verifier blocker was fixed
(see `docs/reviews/2026-07-27-client-finality-verifier-scheme-drift.md`) and
(b) locate the saturation knee before spending seeds on confidence intervals.

## Why they are quarantined: host co-tenancy

The measurements were taken while several LLM subagent processes were running on
the same 12-core box. Observed during the sweep:

- `loadavg` 9.66 (1-min peak 15.70) on 12 cores;
- browser, chat, and four `hermes` processes each consuming 5–27% CPU;
- subagent transcript files being actively written at 18:30 and 18:33, i.e.
  inside the `r2_knee` measurement window.

The resulting curve is not a saturation curve. It is non-monotonic in a way that
no offered-load model explains:

| rate (tps) | finalized | scheduled | success | campaign |
|---|---|---|---|---|
| 200 | 2000 | 2000 | **1.000** | r2_ladder |
| 200 | 921 | 2000 | **0.461** | r2_knee |
| 300 | 370 | 3000 | 0.123 | r2_knee |
| 400 | 2114 | 4000 | 0.528 | r2_knee |
| 500 | 1958 | 5000 | 0.392 | r2_knee |
| 800 | 1499 | 8000 | 0.187 | r2_ladder |

The same offered load (200 tps), same seed (42), same committee (n=4, f=1), and
byte-identical `contract.json` produced success 1.000 and 0.461 in two runs. A
factor-of-two swing at identical parameters is measurement noise from CPU
contention, not protocol behavior. The 300→400 tps *increase* in success
fraction is the same artifact.

`r2_knee` additionally terminated fail-closed at rate=600 with "one or more
validators did not complete migration" — a starved validator missing its
migration window under contention, which is exactly the fail-closed behavior the
harness is supposed to exhibit. It is not a protocol defect and not a fork.

## What was genuinely established

1. The client-finality verifier scheme-drift blocker is fixed: a full trial now
   passes independent cryptographic verification (1000/1000 finalized,
   success_fraction 1.0, 1400 request rows validated by
   `scripts/verify_client_results.py`).
2. The R2 pipeline runs end-to-end: validators boot, clients drive load,
   receipts verify, and the campaign sealer enforces its overload-point
   requirement.
3. Order-of-magnitude only: the knee is somewhere below 800 tps offered on this
   box. This is a range hint for planning the real sweep, NOT a measurement.

## Requirement for a citable R2 result

A publishable saturation curve needs a quiet host:

- no LLM subagents, browsers, or other tenants during measurement;
- `loadavg` below ~1.0 before each trial starts;
- >=5 seeds per rate point, trial-level (not validator-within-trial) CIs;
- monotonic success fraction, else re-measure rather than report;
- `scripts/verify_open_loop_schedule.py` plus the campaign sealer both passing,
  yielding `status=complete` and >=2 genuine overload points.

Until such a run exists, the paper's position stands unchanged: client-observed
throughput is future work (`docs/paper/paper.tex` records this explicitly). No
number from this directory may enter the paper.
