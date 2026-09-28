# M10 — gate-isolation control (`m10_gate_isolation.csv`)

## What this campaign is

A one-factor control for the RQ2 testbed differential, run at a **safe** target
commit quorum. The headline testbed arms (`tab:testbed`) were produced by binaries
that predate the per-arm commit-quorum selector: there, *every* arm including SAGE
committed at the legacy unsafe `2f+1 = 3`, so the gate was the only factor varied
but the whole table sits at an unsafe quorum. M10 re-runs the contrast with a safe
commit quorum held constant across arms, so the only remaining difference from
SAGE is the gate itself.

| arm | strategy | target commit quorum | cutover gate |
|---|---|---|---|
| `A_sage` | `sage` | safe `HotStuffNative`, `q=n-f=5` | `n-f=5` |
| `B_blind_unsafe` | `sageblind` | unsafe `2f+1`, `q=3` | none |
| `C_blind_safe` | `sageblind --force-safe-commit-quorum` | safe `HotStuffNative`, `q=n-f=5` | none |

These runs postdate the 2026-07-29 quorum work, so the engine default is
`QuorumPolicy::HotStuffNative { f }` = `n-f` = 5, **not** the generalized
`Bft{f}` = `floor((n+f)/2)+1` = 4. `--force-safe-commit-quorum` declines the
unsafe geometry and falls through to that default, so arms A and C both ran
`q=5`. (An earlier version of this file labelled them `q=4`; that predated the
`HotStuffNative` split and is corrected here.)

`--force-safe-commit-quorum` only *declines to apply* the unsafe geometry
(`NodeConfig::force_safe_target_commit_quorum`, honored at
`crates/sage-node/src/lib.rs:2998-3016`). It never lowers a threshold, so it
cannot make an unsafe configuration look safe.

## Observed result — NEGATIVE

`n=6`, `f=1`, balanced `3/3` socket-layer partition at `h_c=4`, `max_height=8`,
seeds 42-46, 5 trials per arm, 0 inconclusive:

| arm | commit quorum | gate | forked | migrated |
|---|---|---|---|---|
| `A_sage` | safe `q=5` | `n-f` | **0/5** | 0 |
| `B_blind_unsafe` | unsafe `q=3` | none | **5/5** | 30 |
| `C_blind_safe` | safe `q=5` | none | **0/5** | 0 |

**Arm C does not separate from SAGE.** The one-factor control is negative, and
this campaign therefore does **not** establish that the `n-f` gate prevents
forks.

## Why — arithmetic, not implementation

Each side of a balanced `3/3` partition holds 3 validators. Both candidate safe
thresholds exceed 3:

- `q = floor((n+f)/2)+1 = 4 > 3`
- `q = n-f = 5 > 3`

So **no** side can form a target commit quorum, with or without the gate. Only
the unsafe `q = 2f+1 = 3` is reachable by a side of three, which is why arm B is
the only arm that forks. A balanced `3/3` partition at `n=6` is structurally
incapable of isolating the cutover gate. Note the attribution is a **joint**
condition, not a ranking: arm B forks because it has an unsafe commit quorum
*and* no gate. Neither alone suffices — arm C (safe quorum, no gate) forks 0/5,
and SAGE (safe quorum, gate) forks 0/5.

Consequence for claims: gate necessity rests on Lemma 2 (`lem:necessity`) and the
bounded TLC model, **not** on this differential. The testbed evidence supports only
the narrower statement that an unsafe target commit quorum forks under partition
while a safe one does not.

## Required follow-up experiment

To isolate the gate empirically, use a topology where some side **can** reach the
safe commit quorum but **cannot** reach `n-f`: e.g. an unbalanced `4/2` split at
`n=6`, where the majority side reaches `q=4` but not `n-f=5`. Run with
`SPLIT=4 bash scripts/m10_gate_isolation.sh 6 1 4 8 "42 43 44 45 46"`. No
gate-differential claim should be made until that is executed.

## Provenance warning — this CSV was wrong twice

Both earlier versions of this file reported arm C as `5/5`, which was an
**artifact**, and that artifact was briefly written into the manuscript as
positive evidence for the gate.

1. `spawn_testbed` accepted `--force-safe-commit-quorum` but never forwarded it to
   the child `validator_proc` processes. Arm C ran `q=3` while labelled safe.
   Fixed in `crates/sage-node/src/bin/spawn_testbed.rs`.
2. After that source fix, only `--release` was rebuilt while
   `scripts/m10_gate_isolation.sh` invokes `./target/debug/spawn_testbed` behind an
   **absence-only** build guard (`if [ ! -x "$BIN" ]`). The harness therefore ran a
   stale debug binary that still dropped the flag, reproducing the same false `5/5`.

Both defects pushed the result in the direction that favoured the paper's own
claim. The absence-only guard was a class defect shared by 11 harness scripts and
has been replaced with an unconditional rebuild (debug harnesses) or a fail-closed
staleness check (release harnesses).

## Reproduce

```bash
# rebuilds unconditionally; cannot measure stale bytes
bash scripts/m10_gate_isolation.sh 6 1 4 8 "42 43 44 45 46"
```

Config wiring is pinned by
`cargo test -p sage-node --lib forced_safe_commit_quorum_wires_the_one_factor_gate_control`
(asserts which quorum policy and cutover threshold each arm receives; asserts
nothing about fork outcomes).

## Columns

`experiment,arm,strategy,commit_quorum,cutover_gate,n,f,h_c,partition_split,seed,observed_fork,max_finalized_height,migration_success,total_messages,forked_heights`

Attribution is readable from `commit_quorum` and `cutover_gate` rather than from
the arm name.

**Label caveat.** The `commit_quorum` value `safe_bft` is a *harness label string*
that predates the split of `QuorumPolicy` into `Bft` (the generalized
`floor((n+f)/2)+1`) and `HotStuffNative` (`n-f`). It denotes "the engine's own safe
default", which is now `HotStuffNative` = `q=5` at `n=6,f=1` — **not**
`QuorumPolicy::Bft` = `q=4`, despite the name. This CSV records no numeric
threshold column, so the label cannot be independently checked against the bytes
that ran; `m14b_native_gate_authorization.csv` adds `commit_q_value`/`gate_q_value`
for exactly that reason and is the pattern new fork-verdict harnesses should follow.
