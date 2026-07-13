# D3a — Empirical Provenance Label Audit (RQ1 + RQ2)

Goal: every RQ1/RQ2 label + caption must make it impossible to read
"Cox-inspired control", "Cox-threshold transplant", "BFT-SMaRt-style analogue",
or "reconfig-only" as an EXECUTION OF THE PUBLISHED SYSTEM. Audit prose AND captions.

Legend: OK = already provenance-honest; TIGHTEN = wording could be misread, needs edit;
GAP = missing disclaimer.

## RQ1 (migration cost) — paper.tex ~1584-1707

| Loc | Current wording | Verdict |
|---|---|---|
| :1596 | "reconfig-only is the BFT-SMaRt-style membership-reconfiguration analogue" | OK — "analogue" explicit |
| :1602-08 | "a Cox-style live homogeneous switch" + "We compare against Cox because it is the closest recent peer-reviewed protocol" | TIGHTEN — "compare against Cox" can read as comparing against Cox's ARTIFACT; it is a Cox-STYLE arm on SAGE's engine. Add "(modeled on our engine interface, not Cox's implementation)" at first use. |
| :1686 tab:main note c | "A Cox-inspired live switch modeled on our engine interface … This is NOT a Cox-threshold reimplementation" | OK — footnote is explicit and strong |
| :1692 tab:main note b | "models a BFT-SMaRt-style dynamic reconfiguration … an analogue, not a reimplementation of the BFT-SMaRt Java library" | OK — explicit |
| :1654 | "SAGE versus the Cox-inspired live switch" | OK — "Cox-inspired" |

## RQ2 (adversarial safety) — paper.tex ~1709-1841

| Loc | Current wording | Verdict |
|---|---|---|
| :1783-85 | "A Cox-inspired live switch—modeled on our engine interface with a quorum-certified boundary checkpoint but without SAGE's n-f dual-run cutover gate—also forks in every run (10/10)" | OK — inline "modeled on our engine interface" |
| :1795-97 | "This is still a Cox-inspired checkpoint-switch baseline, not a Cox-threshold implementation with epoch-mismatch catch-up" | OK — explicit |
| :1808-12 tab:testbed caption | "The Cox-threshold arm gates on Cox's own 2f+1 checkpoint quorum (its actual StableCheckpoint threshold, omitting only the epoch-mismatch catch-up sub-protocol) … this does not test Cox within its homogeneous fault model" | OK but VERIFY: caption says "Cox's own 2f+1 checkpoint quorum … its actual StableCheckpoint threshold". Primary-source check (D1): Cox does NOT fix 2f+1 — it selects q=max(q_old,q_new) from a UNIFORM family (Cox §3, line 118). For PBFT↔HotStuff both are 2f+1, so 2f+1 is correct FOR THAT PAIR, but "Cox's own threshold" overstates generality. TIGHTEN: "Cox's StableCheckpoint threshold for the 2f+1-quorum BFT engines it targets". |
| :1823-25 tab rows | "Cox-inspired / checkpoint, no n-f" and "Cox-threshold / 2f+1 (=3)" | OK — gate column names the mechanism, not an execution |
| :1776 | "sign the cutover certificate with real Ed25519 keys" | OK (real-crypto testbed) — consistent with F1 fix elsewhere |

## Key D1-dependent finding (feeds D2 + D3a caption tighten)
Cox's quorum is NOT intrinsically 2f+1. Cox §3 (verbatim, line 118):
"it is only applicable to BFT consensus protocols in a semi-synchronous network with
clear quorum sizes uniformly agreed upon by consensus clusters" and selects
q = max(q_old, q_new). The paper's "Cox-threshold = 2f+1" is correct ONLY because
the PBFT/HotStuff pair Cox demonstrates both use 2f+1. The heterogeneous PoA→BFT
boundary (PoA has NO Byzantine quorum family) is precisely outside Cox's stated scope
— which STRENGTHENS SAGE's contribution and is already argued at :350-356. The
tab:testbed caption should not imply 2f+1 is Cox's universal threshold.

## D3a verdict
Mostly OK — the paper is already unusually careful (footnotes b/c, inline "modeled on
our engine interface"). Two TIGHTEN edits only:
1. :1602-08 RQ1 — add "(modeled on our engine interface, not Cox's implementation)"
   at first "compare against Cox".
2. :1808-12 RQ2 caption — qualify "Cox's own 2f+1" to "Cox's StableCheckpoint
   threshold for the 2f+1-quorum engines it targets (PBFT/HotStuff)".
No GAP found. No label currently claims an execution of a published system.
