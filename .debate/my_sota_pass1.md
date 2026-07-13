# Independent First Pass — Kiro (claude-opus-4.8-thinking) — SOTA comparison plan

FROZEN before reading gpt-5.6-sol's pass. Grounded in paper.tex read + web research
(.debate/SOTA_RESEARCH_FINDINGS.md).

## Interpretation of the mentor's note
"Missing comparison between our Sage and SOTAs" — the paper is NOT missing Related Work
(it has a 9-row qualitative table + Cox as closest prior + Abstract/Aliph/FlexBFT/AdaChain/
Sui Lutris/Tenderbake/Merge). The defensible reading is narrower and sharper:

1. GAP-1 (empirical): SAGE's only quantitative "SOTA" comparison is a **Cox-inspired arm
   modeled on SAGE's OWN engine interface** (tab:main note c, tab:overhead). It is NOT a
   comparison against Cox's real published system/numbers, and no other SOTA appears
   quantitatively. Reviewer reads this as "compared only to itself."

2. GAP-2 (coverage): The switching-specific SOTA set is INCOMPLETE. Cox's own related work
   cites Spectrum (meta-consensus, parallel dual-run), REBFT (mode switching), ADAPT
   (ML abortable switching, Bahsoun), AWARE (weight adaptation). SAGE cites Abstract/Aliph
   and Cox but omits Spectrum/REBFT/ADAPT/AWARE — the closest switching peers.

3. GAP-3 (contribution clarity): the scientific "strength over them" is asserted in prose
   but not consolidated into ONE axis matrix that a reviewer can read in 30 seconds.

## My recommended plan addition (4 items)

### P-A: Deepen SOTA Related Work (research-backed, prose + citations)
Add Spectrum, REBFT, ADAPT, AWARE to §Runtime Consensus-Protocol Switching with PRECISE
positioning: all switch among homogeneous BFT engines / same fault model; none crosses a
fault-model boundary, none has verifiable bounded rollback, none has machine-checked
cross-boundary safety. This directly answers "figure out SOTA papers to compare."

### P-B: A single multi-axis capability matrix as the PRIMARY contribution vehicle
Rows = SOTA switching systems (Cox, Abstract/Aliph, Spectrum, REBFT, ADAPT, ADACON/DA-PBFT,
Sui Lutris reconfig, hard-fork). Columns = the axes SAGE uniquely combines:
heterogeneous fault-model crossing | quorum-family change at boundary | verifiable bounded
rollback | machine-checked cross-boundary safety | executable invariant monitor | n-f
dual-run gate. SAGE is the only all-checkmark row. This is where "show our strengths" lives.

### P-C: Honest empirical positioning (NO new heavy build)
- State PRECISELY that the Cox-style arm is a model on SAGE's engine interface, not Cox's
  Go system; keep the RQ1 tie (credits Cox, anti-straw-man) and the RQ2 fork differential.
- Add a short "why not a raw cross-system throughput race" defense: throughput races measure
  the TARGET engine, not the migration mechanism; cross-testbed numbers are apples-to-oranges.
- OPTIONAL (only if cheap): cite Cox's own reported "switching in milliseconds" and note
  SAGE's cutover-evidence latency is in the same order, as a sanity anchor — clearly labeled
  cross-paper, not head-to-head.

### P-D: Explicitly do NOT reimplement Cox for a throughput race
Scope creep + scientifically unsound (paper already argues this). Reviewers reject
apples-to-oranges cross-testbed throughput. Guard against this temptation.

## Confidence
- P-A high (research-grounded, low cost, direct answer to mentor).
- P-B high (this is the real scientific-contribution lever; matrix already partly exists at
  tab:related — extend it toward switching-specific rows + the crossing axes).
- P-C medium-high (honesty hardening; the modeled-arm caveat already partly in note c).
- P-D high (avoid the trap).

## What would change my mind
- If the mentor specifically wants a REAL head-to-head against a running SOTA implementation
  (not a matrix), then P-C escalates to "stand up a real Cox/PBFT↔HotStuff testbed and run
  it under SAGE's harness" — much larger, and I'd flag the apples-to-oranges risk hard.
- If a venue reviewer already demanded quantitative SOTA numbers, the matrix alone won't
  satisfy; we'd need at least Cox's own reported numbers reproduced or cited precisely.
