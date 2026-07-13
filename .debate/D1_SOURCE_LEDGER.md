# D1 — SOTA Primary-Source Ledger (SAGE mentor-review response)

Purpose: every "does not provide X" / "outside its scope" statement SAGE makes about a
peer system must trace to a VERBATIM primary-source quote + real URL. No claim ships
without one, or it is downgraded to "does not report".

Verification legend:
- VERIFIED-PRIMARY = I (executor) read the primary source this session and confirmed.
- SUBAGENT = extracted by a delegated subagent with quote+URL; spot-verify before paper edit.
- Every row's "switch object / fault-model crossing / rollback" cell is what determines
  its placement in the D2 taxonomy.

═══════════════════════════════════════════════════════════════════════
## Row 1 — Cox  [VERIFIED-PRIMARY, cite key: cox]
═══════════════════════════════════════════════════════════════════════
- Full text read: sciencedirect PII S2096720925000727 (CC-BY), cached
  /home/harry-riddle/.hermes/cache/web/www.sciencedirect.com-236dcb169f.md
- Bibliography: paper.tex:2988 (already cited, DOI 10.1016/j.bcra.2025.100345, June 2026).
- Switch object: the CONSENSUS ENGINE (core layer), any BFT algorithm (PBFT/HotStuff demo).
- Fault-model crossing: NO. Homogeneous BFT↔BFT.
- Quorum crossing: quorum family is UNIFORM; on differing quorums selects
  q = max(q_old, q_new). VERBATIM (§3): "the larger value between the original and new
  cores is selected as [q]." Scope limit VERBATIM (§3): "it is only applicable to BFT
  consensus protocols in a semi-synchronous network with clear quorum sizes uniformly
  agreed upon by consensus clusters."
- Certified boundary: YES — StableCheckpoint (threshold-aggregated signed checkpoints).
  Structurally analogous to SAGE's CutCert. NOT intrinsically 2f+1: it is q=max from the
  uniform family; happens to be 2f+1 only because PBFT & HotStuff both use 2f+1.
- Recovery/rollback: FORWARD catch-up ONLY (§4.3 recovery sub-protocol for downstream
  nodes via chained StableCheckpoint proof). NO bounded reversibility / rollback.
- Evidence level (their paper): authors' reported measurements — real Go impl, 4-node LAN,
  Hyperbench. Switch duration 20–30 ms end-to-end (§6.2, Fig 6, config-block-commit →
  new-engine-start). NOT event-sim; NOT µs cutover-evidence latency (different construct
  from SAGE's 105.6 µs — see D3b).
- Artifact availability: no public Cox artifact FOUND after searching the paper, the
  Elsevier publisher page, and author/GitHub channels (D4_ARTIFACT_SPIKE.md; the closest
  author GitHub is a different Zhaoshuo Li). Wording MUST be "no artifact found after
  documented search" / "independent execution presently unavailable" — NOT absolute
  "infeasible" (advisor Q6). A from-scratch reimplementation is possible but out of scope.
- SAGE contribution vs Cox: SAGE targets the HETEROGENEOUS PoA→BFT crossing that Cox's
  own stated applicability excludes (PoA has no Byzantine quorum family), + adds bounded
  verifiable rollback Cox does not report. This is a SCOPE claim, not "SAGE is safer/faster."

═══════════════════════════════════════════════════════════════════════
## Row 2 — Abstract / Aliph  [VERIFIED-PRIMARY, cite key: abstract]
═══════════════════════════════════════════════════════════════════════
- Full text read: EPFL infoscience manuscript, cached
  /home/harry-riddle/.hermes/cache/web/infoscience.epfl.ch-ad5bb621e3.md
- CITATION FIX: paper.tex:2994 lists authors as "R. Guerraoui, N. Knežević, V. Quéma,
  M. Vukolić" — MISSING lead author PIERRE-LOUIS AUBLIN. Primary source title page:
  "PIERRE-LOUIS AUBLIN, INSA Lyon / RACHID GUERRAOUI, EPFL / NIKOLA KNEŽEVIĆ / VIVIEN
  QUÉMA / MARKO VUKOLIĆ". ACM TOCS vol 32 no 4 Art 12, Jan 2015, DOI 10.1145/2658994.
  (EuroSys 2010 conference version = Guerraoui/Knežević/Quéma/Vukolić, no Aublin — the
  journal version added Aublin. SAGE cites the journal ext, so must include Aublin.)
- Switch object: composes BFT protocol from abortable SMR INSTANCES; switches to
  next(i) instance on abort. Homogeneous BFT↔BFT.
- Fault-model crossing: NO. VERBATIM: "we treat a BFT protocol as a composition of
  Abstract instances." Switching monotonicity: next(i) > i, total order preserved.
- Rollback: NO bounded reversible rollback (instances abort FORWARD).
- Evidence: pen-and-paper + model checking (App A switching idempotency). Real impl
  (AZyzzyva, Aliph, R-Aliph).
- SAGE contribution vs Abstract: same as Cox — homogeneous fault model; SAGE crosses it.
  Already positioned at paper.tex:342-376. Faithful.

═══════════════════════════════════════════════════════════════════════
## Row 3 — Spectrum  [VERIFIED-PRIMARY, NOT currently cited by SAGE]
═══════════════════════════════════════════════════════════════════════
- Full text read: arXiv:1902.05873v1 (Arun, Peluso, Ravindran, Virginia Tech, 15 Feb 2019),
  cached /home/harry-riddle/.hermes/cache/web/arxiv.org-715f06becf.md
- CRITICAL SCOPE FACT: Spectrum is CRASH-FAULT, NOT BFT. It switches among Multi-Paxos,
  Raft, Caesar (leaderless CFT). Abstract names only Paxos/Raft/Caesar; no Byzantine model.
  VERBATIM (subagent, §3, to spot-verify in cached file): "Nodes may fail by crashing but
  do not behave maliciously, i.e., they are not byzantine."
- Switch object: consensus PROTOCOL via a meta-consensus layer, zero-downtime async switch,
  epoch-versioned. (Cox cites Spectrum as its switching predecessor + borrows epoch concept.)
- Fault-model crossing: NO (and not even BFT). Furthest from SAGE's problem.
- Rollback: none.
- USE IN SAGE: OPTIONAL add. Strengthens the taxonomy by showing the closest "switching
  framework" lineage (Spectrum→Cox) is CFT-or-homogeneous-BFT, never heterogeneous-boundary.
  If added: new \bibitem{spectrum}, arXiv:1902.05873. Advisor to decide add vs cite-in-prose.

═══════════════════════════════════════════════════════════════════════
## Row 4 — ADAPT  [SUBAGENT, spot-verify; NOT currently cited]
═══════════════════════════════════════════════════════════════════════
- Bahsoun, Guerraoui, Shoker, "Making BFT Protocols Really Adaptive," IEEE IPDPS 2015,
  pp 904-913, DOI 10.1109/IPDPS.2015.21. URL:
  https://repositorio.inesctec.pt/server/api/core/bitstreams/7dffde2b-cb64-479c-b404-fbe1b90baf2e/content
- Switch object: switches among BFT protocols at runtime via ML-predicted evaluation score;
  built on Aliph-style abortability. Homogeneous BFT↔BFT.
- Fault-model crossing: NO. Rollback: NO.
- Placement: adaptive-POLICY class (like AdaChain/ADACON at paper.tex:378) — decides WHICH
  homogeneous protocol; does not cross a fault-model boundary. Complementary to SAGE.

═══════════════════════════════════════════════════════════════════════
## Row 5 — REBFT  [SUBAGENT, spot-verify; NOT currently cited]
═══════════════════════════════════════════════════════════════════════
- Distler, Cachin, Kapitza, "Resource-efficient Byzantine Fault Tolerance," IEEE TC vol 65
  no 9, 2807-2819, 2016, DOI 10.1109/TC.2015.2495213. Manuscript:
  https://www4.cs.fau.de/Publications/2016/distler_16_ieeetc.pdf
- Switch object: switches MODES (resource-saving normal vs fault-handling) of the SAME BFT
  algorithm — NOT an engine switch at all.
- Fault-model crossing: NO (same engine). Rollback: NO.
- Placement: taxonomy boundary — "mode adaptation within one engine." Distinct from both
  engine-switching (Cox) AND SAGE. Useful to show SAGE is not this.

═══════════════════════════════════════════════════════════════════════
## Row 6 — AWARE  [SUBAGENT, spot-verify; NOT currently cited]
═══════════════════════════════════════════════════════════════════════
- Berger, Reiser, Sousa, Bessani, "AWARE: Adaptive Wide-Area Replication...," IEEE TDSC
  vol 19 no 3, 1605-1620, 2022, DOI 10.1109/TDSC.2020.3030605. arXiv:2011.01671.
  Open-source impl: https://github.com/bergerch/aware
- Switch object: tunes voting WEIGHTS + leader location within ONE BFT engine (BFT-SMaRt),
  WAN-optimized. NOT engine switching.
- Fault-model crossing: NO. Rollback: NO.
- Placement: within-protocol adaptation. Adjacent; cite briefly if space. Has real artifact
  but wrong problem class for a head-to-head.

═══════════════════════════════════════════════════════════════════════
## Row 7 — DA-PBFT  [SUBAGENT, paywalled — limited to IEEE metadata]
═══════════════════════════════════════════════════════════════════════
- "A Dynamic Adaptive Framework for PBFT...IoT," IEEE TC 2024, DOI 10.1109/TC.2024.3377921.
- Switch object: DRL/multi-agent PBFT TUNING (optimality-seeking within PBFT). NOT engine
  switching, NOT fault-model crossing.
- CAUTION: full text paywalled; claims limited to publisher abstract. Do NOT assert
  internal mechanism details. Placement: adaptive-policy paragraph only, if at all.

═══════════════════════════════════════════════════════════════════════
## Rows already in SAGE (reconfiguration class) — no new extraction needed
═══════════════════════════════════════════════════════════════════════
- BFT-SMaRt (bftsmart, paper.tex:3021): membership reconfig within one engine. Real open
  Java artifact — but reconfiguration-class, NOT switching. gpt-5.6-sol: wrong first
  head-to-head target. Already an analogue in RQ1.
- Sui Lutris (suilutris:2967), Abstract-reconfig (reconfig:3018): reconfiguration, cited.
- AdaChain (adachain:3030), adaptbft (:3034), ADACON (adacon:3043), threatadapt (:3038):
  adaptive-policy class, cited at paper.tex:378-396.

═══════════════════════════════════════════════════════════════════════
## Ledger conclusions (feed D2 taxonomy + mentor answer)
═══════════════════════════════════════════════════════════════════════
1. AMONG THE SYSTEMS EXAMINED HERE, none crosses a heterogeneous fault-model /
   quorum-family boundary. Cox (closest) explicitly scopes itself to UNIFORM quorum
   families [PSV verbatim]. Spectrum is CFT [PSV]. Abstract/ADAPT are homogeneous BFT.
   REBFT/AWARE/DA-PBFT don't switch engines at all.
   => Language MUST be comparison-bounded ("every system in this comparison", "among the
   systems examined here") — NOT "uncontested in the literature" / "no peer system"
   (advisor Q1: universal literature claims are not justified from a 7-system audit,
   4 of which are subagent-only [SA]). Show the mentor by taxonomy, not a performance win.
2. Among these systems, none REPORTS bounded verifiable rollback (Cox reports forward
   catch-up only [PSV]; others do not report rollback). Comparison-bounded, not universal.
3. Independent execution of Cox is presently unavailable (no artifact found after a
   documented search of paper + publisher + author/GitHub); a from-scratch reimplementation
   is possible but out of scope. NOT "infeasible" in an absolute sense (advisor Q6).
4. TWO citation-accuracy fixes surfaced: (a) add Aublin as lead author to \bibitem{abstract};
   (b) optionally add \bibitem{spectrum} to seat the Spectrum→Cox switching lineage.
5. Evidence-level column (gpt-5.6-sol G-D) populates cleanly: Cox=authors'-reported-real-impl;
   Abstract=analytical+modelcheck+real-impl; SAGE=modeled-control + independently-executed
   testbed + bounded TLC. This transparency is the trust win.

═══════════════════════════════════════════════════════════════════════
## DOI VERIFICATION (advisor correction #10 — verify [SA] before paper)
═══════════════════════════════════════════════════════════════════════
Verified via web publisher/scholarly records (NOT the denied curl; different method):
- spectrum: arXiv:1902.05873, Arun/Peluso/Ravindran, 2019 — PRIMARY-VERIFIED (arXiv PDF read).
- adapt: doi:10.1109/IPDPS.2015.21, Bahsoun/Guerraoui/Shoker, IPDPS 2015 pp.904-913 — VERIFIED
  (ACM DL dl.acm.org/doi/10.1109/IPDPS.2015.21 + ACM Comput. Surveys ref).
- rebft: doi:10.1109/TC.2015.2495213, Distler/Cachin/Kapitza, IEEE TC 65(9):2807-2819, 2016 —
  VERIFIED (IBM Research + unibe.ch crypto group + Wiley citation). Note: "ReBFT" capitalization.
- aware: doi:10.1109/TDSC.2020.3030605, Berger/Reiser/Sousa/Bessani, IEEE TDSC 19(3):1605-1620,
  2022 — VERIFIED (author FAU page + NASA ADS). Accepted Oct 2020.
- dapbft: doi:10.1109/TC.2024.3377921, Li/Qiu/Li/Liu/Zheng, IEEE TC 73:1669-1682, 2024 —
  VERIFIED (Springer citation + IEEE CS CSDL). All 5 cleared to enter bibliography.
