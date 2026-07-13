# Advisor Challenge — SOTA comparison EXECUTION artifacts (round 3)

You are the ADVISOR in an advisor-executor loop. I (executor) built the SOTA-comparison
deliverables the plan called for. You already reached CONSENSUS: AGREED on the PLAN
(.debate/SOTA_COMPARISON_PLAN.md). Now challenge the EXECUTED artifacts before I touch paper.tex.

You are running inside the repo. VERIFY every claim against the real files — do not trust me.

## Artifacts to challenge (read them)
- .debate/D1_SOURCE_LEDGER.md      — 7-system primary-source ledger
- .debate/D2_TAXONOMY_FILLED.md    — the taxonomy table I will insert into paper.tex
- .debate/D3a_PROVENANCE_AUDIT.md  — 2 label/caption tighten edits
- .debate/D3b_OPERATING_POINTS.md  — optional non-ranking operating-point table
- .debate/D5_QUANTUM_MOTIVATION.md — quantum motivation call
- .debate/D4_ARTIFACT_SPIKE.md     — why no real head-to-head is possible

## Source-of-truth facts I claim I VERIFIED this session against primary text (check my quotes)
1. Cox (S2096720925000727) §3: "only applicable to BFT consensus protocols ... with clear
   quorum sizes uniformly agreed upon"; selects q=max(q_old,q_new) from ONE uniform family;
   Fig 6 switch duration 20-30 ms end-to-end, 4-node LAN, real Go, PBFT<->HotStuff; forward
   epoch-catch-up recovery only, no rollback.
2. Abstract (ACM TOCS 32(4) 2015, DOI 10.1145/2658994): lead author is Pierre-Louis AUBLIN.
   The SAGE bib \bibitem{abstract} (paper.tex:2994) OMITS Aublin — I claim this is a citation bug.
3. Spectrum (arXiv:1902.05873): "Nodes may fail by crashing but do not behave maliciously,
   i.e., they are not byzantine" — I claim Spectrum is CFT, switches Multi-Paxos/Raft/Caesar,
   NOT a BFT switcher. Cox cites it as a switching predecessor regardless.

## Specific questions — challenge each, cite file:line or source
Q1. Is the D2 taxonomy HONEST? Any cell that overclaims, any "cannot" not downgraded to
    "does not report", any evidence-level (AS/AR/MC/IX) mislabeled?
Q2. Abstract citation fix: should I add Aublin as lead author to \bibitem{abstract}? Verify.
Q3. Spectrum CFT reclassification: safe to state in the paper that Spectrum is crash-fault,
    not BFT? Does that strengthen or endanger any existing SAGE claim?
Q4. D3b operating-point table: KEEP (with non-ranking caption) or OMIT? Your round-1 position
    was "omit if the caveat makes it unhelpful." Decide now given Cox=20-30ms end-to-end vs
    SAGE=105.6us cutover-EVIDENCE latency (different constructs).
Q5. D5 quantum: cut entirely, or one bounded motivation clause?
Q6. Anything in D1/D2 that is NOT safe to put in front of a hostile reviewer?

End with either "CONSENSUS: AGREED" (I may proceed to edit paper.tex as scoped) or
"CONSENSUS: NOT YET" with a numbered list of exactly what must change.
