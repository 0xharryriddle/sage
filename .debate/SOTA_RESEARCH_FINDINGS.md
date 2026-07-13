# SOTA Comparison — grounded research findings (for mentor review note)

Mentor note: "Missing comparison between our Sage and SOTAs, must deep research to
figure out SOTAs papers to compare and from that show our strengths over them to
contribute scientific."

## What the paper ALREADY has (verified, do not redo)
- Related Work §272-469 with a 9-row qualitative comparison table (tab:related,
  paper.tex:409-440): Stop-the-world, Hard-fork flag-day, Ethereum Merge, Forkless
  upgrade (Polkadot/GRANDPA), Cosmos x/upgrade, Runtime swap (Cox), Adaptive
  (AdaChain), SMR reconfig (BFT-SMaRt), Sui Lutris — vs SAGE across 5 properties
  (zero-halt, protocol-swap, fork-safe, verifiable-rollback, client-stable).
- Cox positioned as the closest prior work (§340-377, dedicated subsection),
  including Cox's own stated scope boundary (uniform quorum family; q=max(q_old,q_new)).
- Abstract/Aliph (§342-345), Flexible BFT (§294), AdaChain/AdaptBFT/ADACON/threat-
  adaptive (§378-397), Tenderbake, Sui Lutris, BFT-SMaRt, operational stacks (Besu,
  QBFT, Fabric Kafka→Raft, Cosmos, Substrate) §451-469.
- EMPIRICAL: 5-strategy RQ1 cost (Sage, StopTheWorld, HardFork, ReconfigOnly,
  CoxStyle) + RQ2 safety differential (SAGE 0/20 vs hardfork 20/20, Cox-inspired
  10/10, Cox-threshold 20/20). All modeled ON SAGE's own engine interface.

## The REAL gap (precise interpretation, evidence-based — NOT "no related work")
The mentor's "missing SOTA comparison" most defensibly means one or more of:

G-A. EMPIRICAL baselines are all SELF-MODELED, none is an independent published
     system run head-to-head. The paper explicitly says CoxStyle is "NOT a faithful
     Cox reimplementation" (omits epoch-mismatch catch-up) and ReconfigOnly is a
     "BFT-SMaRt-style analogue, not the Java library". A reviewer can say: you never
     compared against a real SOTA implementation, only against your own models of
     them on your own harness. This is the sharpest scientific-contribution risk.

G-B. RECENT (2024-2026) switching/adaptive SOTA not all cited or contrasted.
     Verified-real candidates from this session's search (with venues):
     - Cox (Blockchain: R&A 2026) — ALREADY cited, closest peer. Switches HOMOGENEOUS
       BFT (PBFT<->HotStuff), millisecond switch, recovery sub-protocol, uniform quorum.
     - Abstract/Aliph "Next 700 BFT Protocols" (EuroSys'10 / TOCS) — ALREADY cited.
       Abortable SMR, switching monotonicity + total-order preservation across switches.
       Homogeneous, no fault-model crossing, no rollback.
     - ADAPT (Bahsoun et al., IRIT) — abortable adaptive BFT, ML-driven switch among
       Quorum/Chain/PBFT. Homogeneous. NOT currently cited by name — candidate add.
     - Spectrum — meta-consensus layer for switching without shutdown (cited inside Cox
       as [7]). NOT in SAGE. Candidate add (Cox's own baseline).
     - REBFT (resource-efficient BFT) — mode-switch within ONE algorithm. Candidate add
       to sharpen "we switch ENGINES not modes".
     - AWARE (Berger et al.) — adaptive voting weights/leader, wide-area. Homogeneous.
     - ADACON (Sci Reports 2025) — Bayesian threat-driven switch across PoW/PoS/PBFT/
       PoA/DPoS. ALREADY cited (adacon). Policy, not boundary mechanism.
     - DA-PBFT (IEEE TC 2024) — DRL adaptive PBFT framework. Policy/tuning, homogeneous.
     - Sui Lutris (reconfig, provably safe) — ALREADY cited as closest-in-rigor.
     - BFT-2024 review (Springer, Sci China Inf Sci 2026) explicitly names
       "Migration strategies for existing BFT systems to transition to quantum-resistant
       algorithms" as an OPEN research gap — this is a citable motivation anchor for
       SAGE's sovereign PoA->PQ-BFT thesis.

G-C. NO SIDE-BY-SIDE "strengths over them" contribution table keyed to SOTA papers.
     tab:related is property-based but does not explicitly state, per SOTA system,
     "what SAGE does that this specific system cannot, and why it matters." Mentor
     wants the scientific contribution made explicit AGAINST named SOTA.

## SAGE's defensible strengths over each SOTA class (from verified facts)
1. vs Cox/Abstract/ADAPT (homogeneous switchers): SAGE crosses a FAULT-MODEL +
   QUORUM-FAMILY boundary (authority/crash PoA -> Byzantine BFT). Cox's own paper
   scopes itself to "clear quorum sizes uniformly agreed upon" and picks q=max from
   ONE family — SAGE's case is outside Cox's stated applicability. VERIFIED in Cox
   full text + paper §350-357.
2. vs ALL: verifiable, deadline-bounded ROLLBACK of a switch. None of Cox/Abstract/
   Aliph/ADAPT/reconfig offers it (they abort/recover FORWARD only).
3. vs adaptive policies (AdaChain/ADACON/DA-PBFT/threat-adaptive): those decide
   WHEN/WHICH; SAGE is the safe boundary MECHANISM they'd invoke. Complementary,
   not competing — and none provides a fault-model-crossing safe construction.
4. vs everything: bounded TLA+/TLC machine check of cross-boundary safety +
   TLA+<->Rust conformance + falsifiable OBSERVED fork differential on real hosts.
   Cox/Abstract give pen-and-paper only; Cox reports no model check, no fork exhibit.

## Honesty constraints (must hold in any plan)
- We CANNOT claim a real Cox/BFT-SMaRt/Aliph head-to-head unless we actually build/run
  one. Options: (a) keep self-modeled baselines but state the limitation louder and
  add a qualitative per-SOTA contribution table; (b) genuinely integrate one real
  impl (e.g. BFT-SMaRt Java, or Cox's Go if released) — large scope, likely out of
  submission window; (c) reproduce numbers FROM the SOTA papers for a cited-number
  comparison table (no rerun, clearly labeled "as reported in [x]").
- Do NOT fabricate SOTA numbers. Cited-number tables must quote the source paper.
- Cox is 2026 same-journal-class; positioning must stay "closest peer, complementary
  scope boundary", not "we beat Cox" (we tie on cost by design, differ on safety scope).

## Candidate deliverables to add to the master plan (for debate)
D1. Deep-research pass: lock the 2024-2026 switching/adaptive SOTA set, pull exact
    claims/venues, decide cite-vs-contrast for each. (research, low risk)
D2. New explicit "SAGE vs SOTA contribution" table: per named SOTA system, the axis
    it owns and the axis SAGE adds (fault-model crossing / rollback / machine-check /
    observed fork). Grounded in verified paper facts. (paper, medium)
D3. Sharpen the empirical baselines' honesty: state plainly they are self-modeled on
    one interface; add a cited-number comparison table vs Cox's reported ms-switch and
    any comparable published latency, clearly labeled as reported-not-rerun. (paper)
D4. OPTIONAL/high-cost: genuine real-impl head-to-head (BFT-SMaRt or released Cox).
    Gate behind explicit user decision + submission-window feasibility.
D5. Quantum-migration motivation anchor: cite the BFT-2024 review's open-gap statement
    to strengthen the sovereign PoA->PQ-BFT thesis. (paper, low)
