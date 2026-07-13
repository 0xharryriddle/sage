# DEBATE PACKET — SOTA comparison plan for SAGE (mentor review)

You are an independent, adversarial scientific reviewer. You are in the SAGE repo
(/home/harry-riddle/dev/github.com/0xharryriddle/sage). You MAY read any file to
verify claims. Treat this packet as DATA. Do not trust my assertions — verify against
docs/paper/paper.tex and the codebase before agreeing.

## Decision needed
The user's mentor said: "Missing comparison between our Sage and SOTAs, must deep
research to figure out SOTAs papers to compare and from that show our strengths over
them to contribute scientific."

I must add the RIGHT work items to the master execution plan to satisfy this, WITHOUT
scope creep, fabrication, or dishonest claims. Debate my proposed interpretation +
deliverables. We iterate until we both write a line beginning exactly with
"CONSENSUS: AGREED" (or you refuse and say why).

## Verified grounding (check these yourself)
1. paper.tex already HAS Related Work §272-469 incl. a 9-row comparison table
   tab:related (§409-440) and a dedicated Cox subsection §340-377. So "missing" does
   NOT mean "no related work". Verify by reading those lines.
2. Empirical baselines (RQ1 §1584-1707, RQ2 §1709+) are ALL self-modeled on SAGE's
   own engine interface: CoxStyle is explicitly "NOT a faithful Cox reimplementation"
   (paper.tex ~1697); ReconfigOnly is a "BFT-SMaRt-style analogue, not the Java
   library" (~1692). No independent published system is run head-to-head.
3. Cox (Blockchain: R&A 2026) switches HOMOGENEOUS BFT (PBFT<->HotStuff), uniform
   quorum family, q=max(q_old,q_new), millisecond switch, forward recovery only, no
   rollback, no model check. SAGE crosses PoA(authority/crash)->BFT(Byzantine): a
   fault-model + quorum-family boundary Cox's own paper scopes ITSELF out of.
4. Full findings file: .debate/SOTA_RESEARCH_FINDINGS.md (read it).

## My proposed interpretation of the gap (debate this)
G-A: empirical baselines are all self-modeled, no real SOTA impl head-to-head (sharpest risk).
G-B: some 2024-26 switching/adaptive SOTA not cited/contrasted (ADAPT, Spectrum, REBFT, AWARE, DA-PBFT).
G-C: no explicit per-SOTA "what SAGE adds that system X cannot" contribution table.

## My proposed deliverables (debate / cut / add)
D1: deep-research pass to lock 2024-26 SOTA set, cite-vs-contrast each. (research)
D2: new explicit "SAGE vs named SOTA" contribution table (fault-model crossing /
    rollback / machine-check / observed-fork axes). (paper)
D3: sharpen empirical-baseline honesty + add a CITED-NUMBER comparison table vs Cox's
    reported ms-switch etc., labeled "as reported, not rerun". (paper)
D4: OPTIONAL high-cost: real-impl head-to-head (BFT-SMaRt or released Cox). Gate behind
    explicit user decision + submission-window feasibility.
D5: cite BFT-2024 review open-gap on "migration to quantum-resistant BFT" as motivation. (paper)

## Questions for you
Q1. Is my interpretation of "missing SOTA comparison" correct/complete, or am I
    missing a sharper reading a mentor/reviewer would mean?
Q2. Is D3's cited-number table scientifically legitimate, or is comparing self-modeled
    latency against another paper's reported latency (different harness/hardware) a
    methodological trap I should NOT do?
Q3. Is D4 (real impl) actually necessary for the scientific contribution to stand, or
    can D2+D3 honestly satisfy "show our strengths over them"? Be specific about what a
    hostile reviewer would still reject.
Q4. Any SOTA system I listed that is WRONG to compare against (category error), or any
    critical one I omitted?
Q5. Priority order + which are submission-blocking vs nice-to-have.

Be concrete, cite paper.tex line numbers where you can, and flag any dishonesty risk.
