# multihost_fork_differential.csv — provenance and honesty notes

Same-region multi-host fork differential (6 Oracle Cloud VMs, real TCP, default
fq_codel, ~sub-millisecond inter-VM RTT), one validator per host, 3/3 partition
induced at the cutover height h=4.

## What the CSV records
Per-trial verdict (`forked` = true/false) for each arm, computed from the
per-validator committed `state_root`s observed during the trial. A trial is
`forked` when two distinct blocks (distinct state roots) are finalized at one
height across the two partition sides.

Columns: `arm,trial,n,forked,verdict`.

## Representative forked-run roots
On one inspected hardfork trial the two partition sides finalized distinct
boundary state roots at h=4: side A `eb0bcb7a...` vs side B `4b5b84b5...`.

## Retention caveat (read before auditing)
Per-trial raw roots and per-validator logs were **not retained**: the trial loop
(`scripts/multihost_fork_trials.sh`) deletes each trial's temporary output after
extracting the verdict, so this CSV holds the independent per-trial verdicts but
not the underlying per-validator committed roots. An auditor who needs the raw
per-validator roots should re-run the tier with `scripts/multihost_fork_trials.sh`
(which requires a real `config/hosts.txt`); the loopback equivalent
`make testbed` retains richer per-height detail and `SAGE_FORK_DEBUG=1` dumps
per-height state-root/engine-group detail.

## Headline result
SAGE forks 0/5 (n-f=5 cutover quorum unreachable by a side of 3); blind hard fork
forks 5/5 (each side switches engines independently). This is the real-host
witness for paper Table `tab:multihost`.
