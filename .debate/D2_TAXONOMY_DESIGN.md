# D2 — SOTA Comparison Taxonomy Design

STATUS: SKELETON. Column design + category placement are grounded design decisions
(evidence-safe). Per-system source-quote CELLS marked [PENDING-D1] until the extraction
subagents return verbatim-quote-backed facts. NO cell ships to paper.tex without a D1
ledger source quote (the round-1 advisor gate: "No 'cannot' claim ships without a source
quote or is downgraded to 'does not report'").

## Design decision: REPLACE/EXTEND the existing paper.tex:409 table, do NOT add a second one

The round-1 advisor (gpt-5.6-sol) was explicit: the existing :409 table already claims
SAGE uniquely combines five properties (Zero-halt, Protocol-swap, Fork-safe, Verif-rollback,
Client-stable). Adding a second checkmark table is repetitive. The missing scientific content
is a NARROWER mechanism-and-evidence taxonomy over the CLOSEST systems, with an evidence-level
column that the existing table lacks.

Decision: keep the existing :409 landscape table (it serves the broad "changing a live system"
framing), and ADD one focused taxonomy table in the Related-Work / runtime-switching subsection
(:340) whose rows are ONLY the direct + adjacent switching systems, not the deployed-upgrade
systems already covered at :409.

## Column design (8 columns)

1. System (+ cite key)
2. Switch object — engine / mode / config / policy / membership   (problem-proximity axis)
3. Fault-model crossing? — does the fault model change across the switch (crash/authority -> BFT)?
4. Quorum-family crossing? — does the quorum predicate itself change, or one uniform family?
5. Certified boundary object — what cross-boundary certificate, if any
6. Recovery / rollback — forward-catch-up only / bounded-verifiable-rollback / none
7. Assurance evidence — pen-paper / model-check / executable-monitor / conformance
8. **Evidence level (G-D, the trust-critical column)** — one of:
   - AS = analytical-from-source (we read their paper text)
   - AR = authors'-reported (their own measured numbers)
   - MC = SAGE-modeled-control (ran in OUR harness, NOT their artifact)
   - IX = independently-executed (we ran THEIR artifact) — NONE qualify (see D4 spike)

## Row placement (by problem proximity, NOT recency — advisor requirement)

Tier 1 — direct engine-switching comparators:
| System | Switch object | Fault cross | Quorum cross | Cert boundary | Recovery | Assurance | Ev-level |
|---|---|---|---|---|---|---|---|
| Cox \cite{cox} | engine (BFT<->BFT) | NO (uniform BFT) [D1 VERIFIED: "clear quorum sizes uniformly agreed"] | NO — q=max(q_old,q_new) one family [D1 VERIFIED] | StableCheckpoint (n-f agg) | forward epoch-catch-up only, NO rollback [D1 VERIFIED] | pen-paper (Thm 1-3) | AR (20-30ms, Fig6) |
| Spectrum \cite{?} | engine (meta-consensus layer) | [PENDING-D1] | [PENDING-D1] | [PENDING-D1] | [PENDING-D1] | [PENDING-D1] | AS |
| Abstract/Aliph \cite{abstract} | engine (abortable SMR instances) | NO (BFT<->BFT) [PENDING-D1 verbatim] | [PENDING-D1] | init/abort-order | abort-forward, NO rollback | pen-paper + model-check (App A) [PENDING-D1] | AS |
| ADAPT \cite{?} | engine (ML-selected) | [PENDING-D1] | [PENDING-D1] | abort-history | [PENDING-D1] | [PENDING-D1] | AS |
| **SAGE (this work)** | **engine (PoA->BFT, heterogeneous)** | **YES — authority/crash -> Byzantine** | **YES — no-Byzantine-family -> 2f+1; gate n-f** | **CutCert (n-f, real Ed25519 testbed)** | **bounded verifiable rollback** | **pen-paper + bounded TLC + exec monitor + TLA-Rust conformance** | **MC + AR (own tiers)** |

Tier 2 — adjacent (mode / policy / reconfiguration), cite briefly, NOT headline rows:
| REBFT \cite{?} | mode (same algo) | NO | NO | [PENDING-D1] | mode-switch | [PENDING-D1] | AS |
| AWARE \cite{?} | weights/leader (same algo) | NO | NO | n/a | n/a | [PENDING-D1] | AS |
| DA-PBFT \cite{?} | policy (DRL tuning of PBFT) | NO | NO | n/a | n/a | authors'-reported | AR |
| AdaChain \cite{adachain} | policy (architecture selection) | NO | NO | n/a | n/a | authors'-reported | AS (already at :378) |
| BFT-SMaRt \cite{bftsmart} | membership (reconfig) | NO | NO | config object | n/a | already at :435 | AS |

## The single honest contribution sentence this table supports (advisor-approved framing)

"Every prior runtime-switching system operates within ONE fault model and quorum family
(engine<->engine BFT, or mode/policy/membership change); SAGE is the only one that crosses
the fault-model + quorum-family boundary (PoA authority/crash -> Byzantine BFT), and the only
one pairing that crossing with a bounded verifiable rollback and machine-checked cross-boundary
safety. We do NOT claim SAGE is faster or safer than Cox in Cox's own setting — Cox explicitly
scopes itself to uniform-quorum BFT<->BFT [D1 VERIFIED verbatim], a setting SAGE's contribution
lies OUTSIDE of."

## Missing cite keys to add to bibliography (pending D1 verbatim venue/DOI)
- Spectrum (arXiv:1902.05873, 2019) — verify venue
- ADAPT (Bahsoun/Guerraoui/Shoker) — verify venue/year
- REBFT (Distler et al.) — verify venue/year
- AWARE (Berger/Reiser/Bessani, IEEE TDSC) — verify
- DA-PBFT (IEEE TC 2024, doi:10.1109/TC.2024.3377921) — verify
