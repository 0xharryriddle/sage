# SAGE SOTA-Comparison Plan (mentor review)

> STATUS: CONSENSUS: AGREED (2-model debate, opus-4.8 + cx/gpt-5.6-sol, round 2).
> Round-1 reviewer withheld consensus on 3 points (D3 methodological trap, missing
> evidence-level column G-D, D5 quantum scope creep); all folded in; round-2 reviewer
> verified each against this file and agreed. Evidence: .debate/sol_sota_pass1.txt,
> .debate/sol_sota_pass2.txt, .debate/my_sota_pass1.md, .debate/SOTA_RESEARCH_FINDINGS.md.
> NOT yet executed — awaiting user review before touching paper.tex.


Mentor's note: "Missing comparison between our SAGE and SOTAs, must deep research
to figure out SOTA papers to compare and from that show our strengths over them
to contribute scientific[ally]."

Status: revised after 2-model debate (Hermes/opus-4.8 + cx/gpt-5.6-sol, adversarial,
in-repo grounded). This version incorporates EVERY gpt-5.6-sol P0/P1 objection.
See .debate/sol_sota_pass1.txt, .debate/my_sota_pass1.md, .debate/SOTA_RESEARCH_FINDINGS.md.

## Honest interpretation of "missing comparison" (agreed by both models)

The paper is NOT missing Related Work. It already has:
- a full Related Work section (paper.tex:272) with a dedicated runtime-switching
  subsection (:340) and a 9-row comparison table (:409);
- Cox treated as closest prior work (:345), SAGE's distinction stated as the
  heterogeneous fault-model / quorum-family boundary (:350);
- an empirical Cox-inspired baseline in RQ1/RQ2 (modeled on SAGE's engine interface).

The DEFENSIBLE reading of the mentor's note = FOUR concrete gaps:
- G-A (independent-baseline gap): SAGE compares only against controls it built on its
  OWN engine interface (Cox-inspired, hard-fork, stop-the-world, reconfig-only). A
  hostile reviewer calls these ablations/design-controls, NOT published SOTA systems.
- G-B (literature-completeness gap): recent named live-switch peers appear in Cox's own
  related work (Spectrum, ADAPT, REBFT, meta-consensus) but are not all engaged here.
  Omitting Cox's direct predecessors makes the landscape look selectively constructed.
- G-C (contribution-isolation gap): the existing feature table (:409) is checkmark-style;
  it does not cleanly isolate WHICH axis (switch object / fault-model crossing / quorum
  crossing / certified boundary / recovery / assurance) is SAGE-unique.
- G-D (evidence-level mismatch) — surfaced by gpt-5.6-sol, accepted: the paper mixes
  analytical-from-source, authors'-reported-measurement, SAGE-modeled-control, and
  independently-executed evidence in one comparison without foregrounding the tiers.

## Deliverables (revised, gated, honest scope)

### D1 — Primary-source SOTA audit (P0, submission-blocking)
Build a claim-evidence ledger. For each candidate system record: DOI/venue/version,
artifact availability + license, exact switch category, fault model, quorum assumptions,
recovery/rollback semantics, and a VERBATIM source quote backing every "does not
report / does not provide" statement SAGE makes about it. No "cannot" claim ships
without a source quote or is downgraded to "does not report".

Candidate set selected by PROBLEM PROXIMITY, not publication year:
  1. live engine switching — Cox (closest), Spectrum, ADAPT
  2. abortable/composable BFT — Abstract/Aliph (cited), REBFT
  3. adaptive selection policies — AdaChain, ADACON, DA-PBFT, AWARE (complements)
  4. within-protocol mode adaptation — REBFT
  5. membership reconfiguration — BFT-SMaRt, Sui Lutris (cited)
  6. deployed one-off upgrades — Merge, Polkadot, Cosmos, Tezos (cited)
Verify publication status of each; NEVER label a preprint peer-reviewed. Systematically
read the closest papers' own related-work + evaluation tables (esp. whether Spectrum is
the direct pre-Cox live-switching system) rather than trusting keyword recency.

### D2 — Source-backed SOTA taxonomy matrix (P0, submission-blocking)
REPLACE/EXTEND the existing :409 table — do NOT add a second repetitive checkmark table.
Columns: switch object (engine/mode/config/policy) | fault-model crossing (Y/N) |
quorum-family crossing (Y/N) | certified boundary object | recovery/rollback semantics |
assurance evidence (proof/model-check/monitor) | implementation provenance |
artifact availability. Plus the G-D EVIDENCE-LEVEL column: {analytical-from-text,
authors'-reported, SAGE-modeled-control, independently-executed}.

### D3a — Empirical-provenance labeling audit (P0, submission-blocking)
Audit EVERY RQ1/RQ2 label AND figure caption so "Cox-inspired control", "Cox-threshold
transplant", and "BFT-SMaRt-style analogue" can never be read as executions of published
systems. This is the single cheapest highest-trust fix.

### D3b — Reported-operating-points table — OMIT (FINAL, advisor Q4 both rounds)
DECISION: OMIT. This table does NOT ship to paper.tex. Placing Cox's 20-30ms end-to-end
switch beside SAGE's 105.6µs cutover-evidence latency invites the forbidden latency
comparison despite any caveat; the values differ in construct, measurement boundary,
sim-vs-real time, substrate, node count, and workload, so the caveat consumes more
scientific signal than the table provides. If Cox's 20-30ms is needed at all, use it in
contextual PROSE only — never a side-by-side table with SAGE's microseconds. See
.debate/D3b_OPERATING_POINTS.md (retained as internal decision record).

### D4 — Real SOTA artifact study (P1 feasibility spike → P2 full run, CONDITIONAL)
First a TIME-BOXED feasibility spike: code availability, license, buildability, protocol
compatibility, whether an honest common workload exists. Priority order: (1) released Cox
artifact if reproducible; (2) Spectrum or ADAPT; (3) BFT-SMaRt ONLY as a reconfiguration-
class reference (it is membership reconfig within one engine — the wrong "closest SOTA"
answer). Full integration only if the spike succeeds AND the submission claim requires it.
Submission-blocking ONLY if the venue/mentor insists on empirical SOTA performance
superiority; otherwise high-value-optional.

### D5 — Quantum-migration motivation — CUT (FINAL, advisor Q5 both rounds)
DECISION: CUT. No D5 clause ships to paper.tex. The manuscript already carries sufficient
post-quantum motivation at paper.tex:401-407 (\cite{nistpqc}); SAGE neither implements nor
evaluates a PQ-BFT target, and the "BFT Consensus in 2024" Medium review is not a strong
enough scholarly foundation for a new scope-adjacent claim. See
.debate/D5_QUANTUM_MOTIVATION.md (retained as internal decision record).

## Claims the comparison CAN and CANNOT support (agreed)
CAN: scope claim (SAGE handles a boundary outside Cox's stated applicability) · feature
claim (bounded verifiable rollback) · assurance claim (bounded TLC + executable/conformance
evidence not reported by peers) · internal-cost claim (modest overhead vs same-harness controls).
CANNOT: "SAGE outperforms/ is safer than Cox." Forks in deliberately weakened controls do
NOT prove a faithful Cox deployment forks. Defensible line: Cox does not claim this
heterogeneous crossing; transplanting its checkpoint threshold into the heterogeneous
setting does not establish the required cross-boundary quorum intersection.

## Execution order
D1 → D2 + D3a (from D1's ledger) → D3b only if metric boundaries verified → D4 spike →
D5 one-liner or cut → then paper edits (revise :409 table + Related Work + RQ labels) →
rebuild PDF → citation/claim consistency check.
