# Inconclusive faithful-Cox real-host runs (NOT cited in the paper)

These CSVs are quarantined because they are NOT interpretable evidence and are
deliberately excluded from all paper claims:

- `multihost_coxfaithful_partition.csv`: under a real 3/3 partition, faithful
  Cox (2f+1=3 checkpoint gate) STALLED (wall_ms ~= 45000 = max-secs timeout)
  rather than cleanly forking. Reason: each partition side of 3 running
  post-cutover HotStuff with 2f+1=3 has ZERO in-side fault tolerance (all 3 must
  agree), which is liveness-fragile on a real network. So no clean fork was
  observed — but this is a LIVENESS stall, not a safety demonstration, and must
  not be read as "Cox does not fork."
- `multihost_coxfaithful_nofault.csv`: flaky (4/6 validators produced empty
  output; harness/port-cleanup race), so the TPS numbers are not trustworthy.

The faithful-Cox CODE (NodeStrategy::CoxFaithful, 2f+1 checkpoint-quorum gate)
IS implemented, sound, and gate-green; only its clean real-host empirical
demonstration is blocked by 3-node-side post-cutover liveness fragility. The
paper therefore presents the faithful-Cox contribution as (a) an implemented
stronger baseline and (b) a THEORETICAL threshold contrast (2f+1 reachable by a
3-side vs n-f=5 not), NOT as a fabricated fork count.
