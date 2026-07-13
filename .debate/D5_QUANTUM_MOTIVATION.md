# D5 — Quantum-Migration Motivation (P2, OPTIONAL)

STATUS: CUT (FINAL DECISION — advisor Q5 both rounds). No D5 clause ships to paper.tex.
Reason: the manuscript already carries sufficient post-quantum motivation at
paper.tex:401-407 (\cite{nistpqc}); SAGE neither implements nor evaluates a PQ-BFT target,
and the "BFT Consensus in 2024" Medium review is not a strong enough scholarly foundation
for a new scope-adjacent claim. This file is retained ONLY as an internal decision record.

## Advisor's round-1 constraint (accepted)

"A review's open problem on migration to quantum-resistant BFT does not directly validate SAGE
unless the paper actually demonstrates migration to a PQ-BFT implementation... Do not call SAGE
a solution to quantum-resistant BFT migration. I would demote D5 to optional."

Verified: SAGE's evaluated target engine is HotStuff (classical Ed25519 / ordinary crypto), NOT
a post-quantum BFT engine. The paper already has generic PQ motivation at paper.tex:398-405
(\cite{nistpqc}, FIPS 203/204/205). So SAGE does NOT demonstrate PQ migration.

## Grounded external hook (from earlier research, VERIFIED)

The "BFT Consensus in 2024" literature review explicitly lists as an open research gap:
"Migration strategies for existing BFT-based blockchain systems to transition to
quantum-resistant algorithms." (medium.com/@jim380 review, Research Gaps section.)
This is a real, citable statement of the gap SAGE's MECHANISM (not its current evaluation) could
one day serve.

## RECOMMENDATION: keep as ONE restrained sentence, or cut

Option A (keep, bounded): append to the existing sovereign-motivation sentence near :398-405
ONE clause, e.g.:
  "...and the same heterogeneous-boundary transition mechanism could in principle carry a future
   migration onto a post-quantum BFT engine, a transition the community has flagged as an open
   problem [cite]; we do not evaluate a PQ target here and make no such claim for this artifact."

Option B (cut): drop entirely. The paper's existing :398-405 PQ motivation already suffices;
adding even a bounded forward-looking clause risks a reviewer reading it as scope inflation.

LEAN: Option B (cut) unless the user wants the forward-looking framing. Either way, NO claim
that SAGE solves PQ-BFT migration. This is the lowest-priority deliverable and does not gate
anything.
