# Real-host injected-RTT 3-arm WAN-TPS: FIRST attempt (superseded — kept for provenance)

**Status: the FIRST attempt at this run was quarantined; the bug is now FIXED and
the corrected run IS cited in the paper (`tab:wanhost`). This file documents the
original flaw for provenance.**

## What the first attempt showed (2026-07-05, workload path pre-fix)
Real-host injected-RTT (~104ms) 3-arm run produced TPS numbers but with
`migrated_all=false`: 5/6 validators migrated cleanly with NO fork, but validator 0
(and, in the cox_faithful arm, validator 4) crashed with a `state root mismatch`.

## Root cause (reproduced deterministically on loopback — no VMs)
The synthetic throughput workload (`--workload-txs 200`) emitted balance-CHANGING
transfers, making the committed `state_root` height-dependent. The multi-process
testbed advances execution state only at FINALIZATION (`lib.rs::apply_block`) and
proposes from the last committed tip; it does NOT maintain a speculative per-block
state tree. Under WAN latency the pacemaker view-changes and RE-PROPOSES a height
under a new leader, so the proposer's header state_root (built from a possibly
pipelined/reordered in-flight prefix) diverged from a validator's recomputation ->
`StateRootMismatch` -> the validator FAILS CLOSED (crashes rather than finalizing
divergent state). Empty-block runs (`workload_txs=0`) never hit it because execution
state never changed; the already-cited `tab:tpshost` (workload, sub-ms RTT) never hit
it because there was no reordering.

Bisection (loopback, n=4): workload-only = clean; reordering-only = clean;
workload + reordering = crash. This is a TESTBED EXECUTION-MODEL artifact, NOT a
protocol/consensus defect and NOT a safety violation (zero fork; fail-closed).

## Fix (crates/sage-node/src/lib.rs, propose path)
The synthetic workload now emits BALANCED net-zero transfer PAIRS (a->b then b->a,
unit amount, on funded accounts 0..10). Every block is therefore state-root NEUTRAL:
after applying a block the balances return to their prior values, so the committed
state root is height-invariant and cannot diverge under reordering. The transaction
COUNT and per-tx execution cost remain real (every tx is applied), so the throughput
measurement is genuine; only the net balance delta per block is zero.

## Corrected result (2026-07-06, 3 trials, IS cited)
All three arms migrate on all six hosts, zero fork, at ~104ms real inter-VM RTT:
SAGE 230.4 TPS, hard fork 230.2 (+0.1% vs SAGE), faithful Cox 222.7 (-3.4%).
See `results/raw/multihost_wan_tps_3arm.csv` and paper `tab:wanhost`.
