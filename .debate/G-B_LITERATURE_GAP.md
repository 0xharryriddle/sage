# G-B — Literature-Completeness Gap (CONCRETE, verified)

VERIFIED 2026-07-13 by grep of docs/paper/paper.tex:
- `\bibitem{abstract}` (line 2994) exists but OMITS lead author Aublin.
  Current: "R. Guerraoui, N. Knežević, V. Quéma, and M. Vukolić"
  Primary source (verified, ACM TOCS 2015): "Pierre-Louis Aublin, Rachid
  Guerraoui, Nikola Knežević, Vivien Quéma, Marko Vukolić". FIX: add Aublin as lead.
- ZERO matches for: Spectrum, Aublin, REBFT/ReBFT, AWARE, DA-PBFT anywhere in paper.tex.
  => These switching-specific systems are ENTIRELY ABSENT from SAGE's related work.

## Why this is the sharpest reading of the mentor's note

The mentor said "missing comparison between SAGE and SOTAs." The paper is NOT missing
related work in general (9-row table, Cox, Abstract, AdaChain, Sui Lutris, etc.).
What it IS missing is the runtime-consensus-SWITCHING predecessor cluster that Cox
itself cites as its own baselines:
  - Spectrum (arXiv:1902.05873, 2019) — meta-consensus switching layer (CFT, verified)
  - ADAPT (IPDPS 2015) — ML-selected abortable BFT switching
  - REBFT (IEEE TC 2016) — mode switching within one BFT algorithm

A hostile reviewer (or mentor) sees SAGE cite Cox but NOT Cox's own predecessors, and
concludes the landscape is selectively constructed to make SAGE look more novel. Adding
these — correctly classified by problem proximity, with the evidence-level column —
is the concrete scientific-contribution sharpening the note asks for.

## What this does NOT mean
- Does NOT mean SAGE must beat them empirically (D4 spike: no runnable artifact for
  the closest system anyway).
- The contribution remains SCOPE (heterogeneous fault-model/quorum crossing) + rollback
  + machine-checked assurance — properties the whole switching cluster lacks.

## Fixes queued for paper-update (after advisor consensus)
1. Bibliography: fix `abstract` author list (add Aublin); add net-new bibitems for
   Spectrum, ADAPT, REBFT (+ AWARE/DA-PBFT if kept as adjacent cites).
2. Related-Work :340 (runtime-switching subsection): add the switching taxonomy table
   (D2) whose rows include Spectrum/ADAPT/REBFT, with the honest classification that
   Spectrum is CFT (not even BFT) and ADAPT/REBFT are homogeneous.
