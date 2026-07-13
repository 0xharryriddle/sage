# D3b — Cross-Paper Operating-Point Table (CONDITIONAL, non-ranking)

STATUS: OMIT (FINAL DECISION — advisor Q4 both rounds). This table does NOT ship to
paper.tex. Reason: placing Cox's 20-30ms end-to-end switch beside SAGE's 105.6us
cutover-evidence latency invites the forbidden latency comparison despite any caveat;
the values differ in construct, measurement boundary, sim-vs-real time, substrate, node
count, and workload. The caveat consumes more scientific signal than the table provides.
This file is retained ONLY as an internal decision record. If Cox's 20-30ms is needed at
all, use it in contextual PROSE (never a side-by-side table with SAGE's microseconds).

## The methodological trap this table must avoid (round-1 advisor, verbatim concern)

"A direct numerical table comparing SAGE's 105.6us modeled cutover-evidence latency with
Cox's published millisecond switching result would be a methodological trap. The values are
not necessarily the same construct... Presenting the numbers side by side invites an invalid
inference that SAGE is faster."

Mitigation: the table records OPERATING POINTS with explicit metric definitions and
measurement boundaries, a "cross-study comparable?" column that is NO for every pair, and a
caption that FORBIDS ranking. No ratio, no normalization, no "faster/slower", no cross-study
statistics.

## The table (LaTeX-ready draft, both rows source-verified)

| System | Provenance | Metric reported | Measurement boundary | Config | Reported value | X-study comparable? |
|---|---|---|---|---|---|---|
| Cox \cite{cox} | authors' original Go impl | end-to-end switch duration | config-block-commit -> new-engine-start | 4 nodes, LAN, PBFT->HotStuff, 5-20k req/s | 20-30 ms [Fig 6, VERIFIED] | **No** |
| SAGE (this work) | SAGE deterministic simulator | cutover-EVIDENCE latency | interval to gather n-f CutCert evidence | n=20, f=6, 40 heights, 60 seeds | 105.6 us [tab:main, VERIFIED] | **No** |

## VERIFIED source anchors (both, so this table has no pending cell)

- Cox 20-30ms: Cox §6.2, Fig 6 (cached web extract line 257): "the duration of switching
  remains relatively stable, ranging from 20 ms to 30 ms, and is not affected by increasing
  workload levels." Measurement boundary Cox §6.2: "the interval between the time when the
  configuration block is committed and the time when the node successfully starts the new
  consensus algorithm." 4 full nodes + 3 load generators, LAN, real Go, PBFT->HotStuff.
- SAGE 105.6us: paper.tex:1600-1601 + tab:main:1682, reduced from rq1_migration_cost.csv this
  session (60 seeds, mean cutover_latency_micros = 105.6). It is "cutover-evidence latency",
  explicitly NOT an end-to-end switch time.

## Why they are NOT comparable (must be in the caption)

1. Different construct: Cox measures END-TO-END switch (commit->new-engine-running); SAGE's
   105.6us measures only the EVIDENCE-GATHERING interval for the n-f CutCert, a sub-phase.
2. Different substrate: Cox = real Go over LAN sockets; SAGE 105.6us = event-based DETERMINISTIC
   SIMULATOR time (paper.tex:1587 "simulator time is event based"), not wall-clock.
3. Different n, workload, crypto, hardware.
SAGE's own real-host end-to-end numbers live in the multihost tiers (tab:tpshost/tab:wanhost),
NOT here; even those must not be ranked against Cox's LAN run.

## Mandatory caption text (draft)

"Table N records published/measured OPERATING POINTS for the closest runtime-switching system
(Cox) and SAGE, with each metric's definition and measurement boundary stated explicitly.
The metrics are DIFFERENT CONSTRUCTS measured on different substrates (Cox: end-to-end switch
duration, real Go over LAN; SAGE: cutover-evidence-gathering latency, deterministic simulator
time), so the final column marks every pair as not cross-study comparable. This table MUST NOT
be read as a performance ranking; SAGE claims no latency advantage over Cox. It is included only
to situate the two systems' reported operating regimes."

## Decision recommendation to advisor + user

RECOMMEND OMIT from the paper. Rationale: the caveat is so strong (nothing comparable, no
ranking) that the table carries little scientific signal and high misread risk. The D2
mechanism/evidence taxonomy already conveys the honest contribution (scope, not speed). Keep
this draft in .debate/ as the record that we considered it and why we cut it. If the mentor
specifically wants "a quantitative comparison against a recent peer", surface THIS table with
its guardrails rather than inventing a ranking.
