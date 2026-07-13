# D4 — Real-Artifact Feasibility Spike (SOTA head-to-head)

STATUS: spike executed (read-only web research). Verdict: independent execution
of the CLOSEST comparator (Cox) is PRESENTLY UNAVAILABLE (no artifact found after
a documented search); available real artifacts test a DIFFERENT problem class.
Full D4 experiment should be CUT or heavily deprioritized. The contribution stands
on D1/D2/D3 (analytical + evidence-tier), exactly as gpt-5.6-sol argued in round-1 Q3.
WORDING GUARD (advisor Q6): say "no artifact found after documented search" /
"independent execution presently unavailable" — NOT absolute "infeasible". A
from-scratch reimplementation is possible but out of scope.

## Artifact availability audit (evidence-first)

| System | Class | Public artifact? | Evidence | Usable for SAGE head-to-head? |
|---|---|---|---|---|
| **Cox** \cite{cox} | live engine switching (HOMOGENEOUS BFT) — the closest comparator | **NO** | Paper says "implemented in the GO language" (Cox §6.2) but no repo cited in the paper, none on ScienceDirect, none locatable. GitHub `mli0603` = a *different* Zhaoshuo Li (NVIDIA Cosmos / surgical-robotics vision researcher), NOT the Zhejiang blockchain author. | **NO** — cannot run the closest system. This is the decisive finding. |
| **Spectrum** | pre-Cox switching via "meta-consensus" layer | arXiv:1902.05873 (2019); code status UNVERIFIED | Cox cites it as the stop-and-start / parallel-run baseline | Unlikely; verify only if D4-full is pursued. Homogeneous. |
| **BFTBrain / BFTGym** \cite{adaptbft} | RL-adaptive SELECTION among homogeneous BFT protocols | **YES** — github.com/JeffersonQin/BFTBrain (NSDI'25 / VLDB'24) | Real, maintained | **NO for the boundary claim** — it is adaptive-policy selection among homogeneous engines, not a heterogeneous fault-model crossing. It is a POLICY (already framed as complementary at paper.tex:378). |
| **BFT-SMaRt** \cite{bftsmart} | membership RECONFIGURATION, one engine | **YES** — github.com/bft-smart/library (Java) | Real, widely used | **NO** — reconfiguration-class, not protocol-switching. gpt-5.6-sol round-1: "wrong first real implementation." Already a paper analogue (reconfig-only). |
| **Abstract/Aliph** \cite{abstract} | abortable-SMR composition, homogeneous | research proto (EuroSys'10); no maintained artifact | — | NO — conceptual predecessor only. |

## Why a real head-to-head does NOT gate the contribution (grounds the Q3 consensus)

1. The closest comparator (Cox) has no runnable artifact, so even an ideal
   engineering effort cannot produce a faithful Cox-vs-SAGE benchmark. Building
   our own "Cox" would just reproduce the Cox-inspired / Cox-threshold CONTROLS
   the paper already has (and already labels as SAGE-native, per D3a).
2. The artifacts that DO exist (BFTBrain, BFT-SMaRt) are different problem
   classes (homogeneous adaptive selection; membership reconfiguration). Running
   them would measure the target/engine or a non-competing mechanism, not the
   heterogeneous-boundary safety claim — the same "throughput race measures the
   target engine, not the migration" logic the paper already makes at :288-293.
3. The honest scientific claim is therefore SCOPE + ASSURANCE, not empirical
   superiority: SAGE handles a boundary OUTSIDE Cox's stated applicability
   (verbatim Cox §3 limitation, see D1 ledger), adds bounded verifiable rollback,
   and supplies bounded model checking + executable conformance that Cox does not
   report. NOTE (advisor Q6): do NOT imply Abstract lacks machine-checked assurance —
   Abstract DOES report TLC model checking (App A switching idempotency). The SAGE
   distinction vs Abstract is the implementation↔model linkage (TLA+↔Rust conformance
   over the real cutover_gate) applied to a HETEROGENEOUS boundary, not "Abstract has
   no model check." This is defensible without outperforming anyone.

## Recommendation to fold into the plan

- D4-full (run a real SOTA system head-to-head): **CUT** as a submission gate.
  Record this feasibility verdict in the paper's evaluation-scope / limitations
  discussion so a reviewer sees the infeasibility was assessed, not ignored.
- Keep the honest framing: the Cox-inspired and Cox-threshold arms are
  SAGE-native controls that transplant Cox's checkpoint/threshold onto our engine
  interface (D3a), used to ISOLATE the n-f gate — not executions of Cox.
- If a reviewer later insists on an empirical peer, the only feasible target is a
  from-scratch faithful Cox reimplementation, which is a multi-month effort and a
  separate contribution; note it as future work, do not gate this submission.

## Open item (only if D4-full is ever pursued)
- Verify Spectrum artifact (arXiv:1902.05873) code availability + license.
- Verify whether BFTGym could host a common-workload harness — but it would still
  only compare homogeneous adaptive selection, not the heterogeneous boundary.
