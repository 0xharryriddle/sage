# D2 — SOTA Comparison Taxonomy (FILLED, advisor-round-2 corrected)

STATUS: cells filled from D1_SOURCE_LEDGER.md. Provenance of each cell tagged:
- [PSV] = primary-source verified this session (I read the paper text)
- [SA]  = subagent-extracted, verbatim-quote-backed, NOT yet re-verified by me
- [SAGE] = fact about our own system, from paper.tex / code

CORRECTIONS APPLIED (advisor gpt-5.6-sol round-2, sol_sota_exec_challenge.txt):
1. Exhaustiveness language bounded to "systems examined here" (no global "every prior system").
2. Cox cert threshold: n-f → q=max(q_old,q_new) (Cox picks larger of two engine quorums, NOT universally n-f).
3. Evidence-level taxonomy applied consistently; NO peer labeled IX (none independently executed);
   SAGE written as explicit prose "modeled controls + executed own artifact", not "AR".
4. Absence-based cells downgraded to "does not report X" (not categorical "NO X"), except where
   a primary source expressly states non-support.
5. Spectrum quorum cell: "no crash→Byzantine fault-model crossing" (NOT "one uniform quorum predicate";
   its CFT protocols use different classic/fast quorums).
6. Added implementation-provenance + artifact-availability columns (plan required them).
7. TABLE ACTION: REPLACE/REFACTOR the existing :409 landscape table (or materially extend it) — do NOT
   stack a second overlapping checkmark table. (Plan SOTA_COMPARISON_PLAN.md:62-68.)
8. SAGE assurance "TLA↔Rust conformance" → "cross-checked TLA+/Rust traces" unless paper establishes
   a formal refinement/equivalence (verify against formal_conformance.rs claim before printing).
9. Removed context-free Abstract/Aliph perf numbers (+360%/+30%) from Ev-level cell.

## Column legend
- Switch object: engine / mode / config / policy / membership
- Fault cross: does fault model change across switch? (the SAGE distinction)
- Quorum cross: does quorum predicate itself change?
- Cert boundary: cross-boundary certificate object, if any
- Recovery: forward-catch-up-only / bounded-verifiable-rollback / does-not-report
- Assurance: pen-paper / model-check / exec-monitor / conformance
- Impl provenance: original-artifact / analytical-only
- Artifact avail: public / none-found / n-a
- Ev-level (G-D trust column): AS=analytical-from-source, AR=authors'-reported,
  MC=SAGE-modeled-control (our harness, NOT their artifact), IX=independently-executed-peer-artifact (NONE)

## Tier 1 — direct engine-switching comparators

| System | Switch object | Fault cross | Quorum cross | Cert boundary | Recovery | Assurance | Artifact | Ev-level |
|---|---|---|---|---|---|---|---|---|
| Cox [cox] | engine BFT↔BFT [PSV] | NO — uniform BFT [PSV: "only applicable to BFT … clear quorum sizes uniformly agreed"] | NO — q=max(q_old,q_new) [PSV §3] | StableCheckpoint, q=max(q_old,q_new) agg [PSV] | forward epoch-catch-up; does not report rollback [PSV §4.3] | pen-paper Thm 1-3 [PSV §5] | none found (Go impl, not released) | AR (20-30ms end-to-end, Fig6, 4-node LAN) [PSV §6.2] |
| Spectrum [spectrum] | engine, meta-consensus layer [PSV] | NO — CFT, no Byzantine crossing [PSV: "Nodes may fail by crashing but do not behave maliciously, i.e., they are not byzantine"] | quorum rules may differ by CFT protocol; no fault-model crossing [PSV] | none identified [SA] | zero-downtime async switch; does not report rollback [PSV abstract] | authors'-reported eval [PSV] | analytical-only here | AS |
| Abstract/Aliph [abstract] | engine, abortable SMR instances [PSV] | NO — BFT↔BFT [PSV] | composition, uniform BFT [SA] | init/abort-order, switching monotonicity [SA] | abort-forward to next instance; does not report rollback [SA] | pen-paper + TLC model-check (App A) [SA] | analytical-only here | AS |
| ADAPT [adapt] | engine, ML-selected at runtime [SA] | NO — BFT↔BFT [SA] | abortable, uniform [SA] | abort-history log [SA] | abort + re-evaluate; does not report rollback [SA] | authors'-reported [SA] | analytical-only here | AS |
| **SAGE (this work)** | **engine PoA→BFT, heterogeneous** [SAGE] | **YES — authority/crash → Byzantine** [SAGE] | **YES — no-Byzantine-family → 2f+1; gate n-f** [SAGE] | **CutCert (n-f, real Ed25519 in testbed)** [SAGE] | **bounded verifiable rollback** [SAGE] | **pen-paper + bounded TLC + exec-monitor + cross-checked TLA+/Rust traces** [SAGE, verify wording] | **own artifact, executed** | **modeled controls + executed own artifact (not a peer artifact)** [SAGE] |

## Tier 2 — adjacent (mode / policy / reconfiguration): cite briefly, NOT headline rows

| System | Switch object | Fault cross | Note | Artifact | Ev-level |
|---|---|---|---|---|---|
| REBFT [rebft] | mode (same algo: normal↔fault-handling) [SA] | NO | Distler/Cachin/Kapitza, IEEE TC 2016; resource-saving mode, not engine swap [SA] | analytical-only here | AS |
| AWARE [aware] | weights/leader (same algo) [SA] | NO | Berger/Reiser/Sousa/Bessani, IEEE TDSC 2022; WAN weight tuning, NOT engine swap; public artifact exists but NOT independently run here [SA] | public, not run | AS |
| DA-PBFT [dapbft] | policy (DRL tuning of PBFT) [SA] | NO | IEEE TC 2024; deep-RL PBFT optimization, not a swap [SA] | paywalled | AS |
| AdaChain [adachain] | policy (architecture selection) | NO | already positioned at :378 as complementary policy | analytical-only | AS |
| BFT-SMaRt [bftsmart] | membership (reconfig) | NO | already at :435; reconfiguration analogue, real Java artifact not run here | public, not run | AS |

## The one honest contribution sentence this table supports (advisor-round-2 corrected framing)

"Among the runtime-switching systems examined here, each operates within one fault model —
engine↔engine BFT (Cox, Abstract/Aliph, ADAPT), crash-fault only (Spectrum), or
mode/policy/membership change (REBFT, AWARE, DA-PBFT, AdaChain, BFT-SMaRt). SAGE is the only
system in this comparison that crosses the fault-model boundary (PoA authority/crash → Byzantine
BFT), and the only one pairing that crossing with a bounded verifiable rollback and
machine-checked cross-boundary safety. We do NOT claim SAGE is faster or safer than Cox in
Cox's own setting: Cox explicitly scopes itself to uniform-quorum BFT↔BFT [PSV verbatim], the
setting SAGE's contribution lies OUTSIDE of."

## Citation fixes surfaced (both real, both strengthen us)
1. [abstract] bibitem (paper.tex:2994) MISSING lead author Aublin. Real list [PSV]:
   Aublin, Guerraoui, Knežević, Quéma, Vukolić — ACM TOCS 32(4) Art.12, 2015, doi 10.1145/2658994.
2. Spectrum is CFT not BFT [PSV] — if cited, must NOT be called a BFT-switching predecessor;
   frame as "assumes crash faults and excludes Byzantine behavior; switches among CFT protocols".

## New cite keys to add (venue/DOI from D1 ledger; [SA] rows verify venue/year before final print)
- spectrum: Arun/Peluso/Ravindran, arXiv:1902.05873, 2019 [PSV arXiv id]
- adapt: Bahsoun/Guerraoui/Shoker, IEEE IPDPS 2015, pp904-913, doi 10.1109/IPDPS.2015.21 [SA]
- rebft: Distler/Cachin/Kapitza, IEEE TC 65(9) 2016, doi 10.1109/TC.2015.2495213 [SA]
- aware: Berger/Reiser/Sousa/Bessani, IEEE TDSC 19(3) 2022, doi 10.1109/TDSC.2020.3030605 [SA]
- dapbft: IEEE TC 2024, doi 10.1109/TC.2024.3377921 [SA]

## Residual [SA] cells that MUST be primary-source-verified before entering paper.tex
(advisor correction #10 — do not print these categorically until verified)
- ADAPT: threshold / abort-history-log / "does not report rollback"
- REBFT / AWARE / DA-PBFT: all mechanism cells
- Abstract/Aliph: init/abort-order, "does not report rollback", TLC App A
- Any Tier-2 categorical assurance/rollback statement
