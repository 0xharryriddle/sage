--------------------------- MODULE SageRollback ---------------------------
(***************************************************************************)
(* Bounded model check of SAGE's rollback correctness (paper Theorem       *)
(* `th:rollback`) and the executable invariant `check_no_absolute_reversion`*)
(* (crates/sage-controller/src/invariants.rs). Companion to Sage.tla, which *)
(* covers cross-boundary safety + decision uniqueness. Together they back   *)
(* the paper App. `app:spec` claim that the bounded check reproduces        *)
(* invariant I AND the rollback property.                                   *)
(*                                                                          *)
(* TWO-TIER FINALITY (paper Sec. model): blocks finalized by the legacy     *)
(* engine before the cutover are ABSOLUTELY final; blocks finalized by the  *)
(* target engine in [h_c, h_r) are only PROVISIONALLY final until h_r, at   *)
(* which point they seal (become absolute). An abort (guard g_2) may fire   *)
(* only before h_r and discards ONLY the provisional suffix.                *)
(*                                                                          *)
(* FAITHFULNESS: the rollback action mirrors the controller (rollback.rs +  *)
(* lib.rs decide path): abort returns authority to the legacy engine at the *)
(* retained boundary state and discards provisional blocks; it never touches*)
(* an absolutely-final block. The AllowAbsoluteReversion constant toggles a *)
(* BROKEN CONTROL (FALSE = faithful; TRUE = a buggy controller that reverts *)
(* sealed/absolute blocks), which TLC MUST catch -- the falsifiability gate.*)
(***************************************************************************)
EXTENDS Naturals, FiniteSets

CONSTANTS
    Hd,                     \* dual-run start height
    Hc,                     \* cutover height
    Hr,                     \* rollback-deadline / seal height
    MaxH,                   \* last modeled height
    AllowAbsoluteReversion, \* FALSE = faithful; TRUE = broken control (reverts absolute)
    AllowBadReplayContext   \* FALSE = faithful fail-closed; TRUE = broken (rolls back on bad ctx)

ASSUME Hd \in Nat /\ Hc \in Nat /\ Hr \in Nat /\ MaxH \in Nat
ASSUME 0 < Hd /\ Hd < Hc /\ Hc =< Hr /\ Hr =< MaxH

Heights == 1 .. MaxH
\* Pre-cutover heights are legacy-finalized = absolutely final.
LegacyHeights == 1 .. (Hc - 1)
\* [h_c, h_r) blocks are provisional until the seal at h_r.
ProvisionalHeights == Hc .. (Hr - 1)

VARIABLES
    h,            \* current height being processed
    phase,        \* "dual" | "v2" | "rollback" | "sealed"
    absolute,     \* subset of Heights: absolutely-final committed blocks
    provisional,  \* subset of Heights: provisionally-final committed blocks
    replayCtx     \* "good" | "missing" | "mismatch": replay-context status at abort

vars == <<h, phase, absolute, provisional, replayCtx>>

TypeOK ==
    /\ h \in 0 .. MaxH
    /\ phase \in {"dual", "v2", "rollback", "sealed"}
    /\ absolute \subseteq Heights
    /\ provisional \subseteq Heights
    /\ replayCtx \in {"good", "missing", "mismatch"}

Init ==
    /\ h = Hd
    /\ phase = "dual"
    /\ absolute = { x \in Heights : x < Hd }   \* genesis..h_d-1 already absolute
    /\ provisional = {}
    /\ replayCtx = "good"

\* Dual-run: legacy finalizes height h absolutely, then advances.
StepDual ==
    /\ phase = "dual"
    /\ h < Hc
    /\ absolute' = absolute \cup {h}
    /\ h' = h + 1
    /\ UNCHANGED <<phase, provisional, replayCtx>>

\* Cutover at h_c: switch to target engine (gate proven by Sage.tla's
\* DecisionUniqueness; here we focus on what rollback may touch).
StepCutover ==
    /\ phase = "dual"
    /\ h = Hc
    /\ phase' = "v2"
    /\ UNCHANGED <<h, absolute, provisional, replayCtx>>

\* Target engine finalizes a PROVISIONAL block at h in [h_c, h_r).
StepTarget ==
    /\ phase = "v2"
    /\ h \in ProvisionalHeights
    /\ provisional' = provisional \cup {h}
    /\ h' = h + 1
    /\ UNCHANGED <<phase, absolute, replayCtx>>

\* A correct validator's replay context for the reversible suffix may be
\* incomplete or hash-mismatched (modeled nondeterministically). The Rust
\* controller (rollback.rs validate_replay_context) REFUSES rollback unless
\* the context is present, ordered, and hashes to the manifest-bound root.
StepCorruptCtx ==
    /\ phase = "v2"
    /\ replayCtx = "good"
    /\ replayCtx' \in {"missing", "mismatch"}
    /\ UNCHANGED <<h, phase, absolute, provisional>>

\* Abort (guard g_2): fires only before h_r. Faithful: discard ONLY the
\* provisional suffix, authority returns to legacy at the retained boundary,
\* and the rollback is FAIL-CLOSED on the replay context -- it may proceed only
\* with a "good" context. Broken controls: AllowAbsoluteReversion reverts an
\* absolute block; AllowBadReplayContext lets rollback proceed on a bad context.
\* TLC MUST catch both -- the falsifiability gate.
StepAbort ==
    /\ phase = "v2"
    /\ h < Hr
    /\ (replayCtx = "good" \/ AllowBadReplayContext)   \* fail-closed replay gate
    /\ phase' = "rollback"
    /\ provisional' = {}
    /\ absolute' = IF AllowAbsoluteReversion
                   THEN absolute \ {x \in absolute : x = Hc - 1}  \* BUG: reverts boundary
                   ELSE absolute
    /\ UNCHANGED <<h, replayCtx>>

\* Seal at h_r: provisional blocks become absolute; abort is now disabled.
StepSeal ==
    /\ phase = "v2"
    /\ h >= Hr
    /\ phase' = "sealed"
    /\ absolute' = absolute \cup provisional
    /\ provisional' = {}
    /\ UNCHANGED <<h, replayCtx>>

\* After rollback, legacy resumes finalizing absolutely from the boundary.
StepResume ==
    /\ phase = "rollback"
    /\ h =< MaxH
    /\ absolute' = absolute \cup {h}
    /\ h' = h + 1
    /\ UNCHANGED <<phase, provisional, replayCtx>>

Next ==
    \/ StepDual \/ StepCutover \/ StepTarget
    \/ StepAbort \/ StepSeal \/ StepResume \/ StepCorruptCtx
    \/ (h >= MaxH /\ UNCHANGED vars)   \* stutter at the end (no deadlock)

Spec == Init /\ [][Next]_vars /\ WF_vars(Next)

(***************************************************************************)
(* Rollback safety invariant (paper Thm `th:rollback` +                    *)
(* check_no_absolute_reversion): every legacy/sealed height that was ever   *)
(* absolutely final stays absolutely final -- no absolute block is reverted.*)
(* We track this as: every LegacyHeight, once past it, remains in absolute. *)
(***************************************************************************)

\* No absolutely-final legacy block is ever missing once it should be final.
\* A legacy height x < min(h, Hc) must be in `absolute` (never reverted).
NoAbsoluteReversion ==
    \A x \in LegacyHeights : (x < h) => (x \in absolute)

\* Provisional blocks are only ever in [h_c, h_r) before sealing.
ProvisionalBounded ==
    \A x \in provisional : x \in ProvisionalHeights

(***************************************************************************)
(* Fail-closed replay-context invariant (P1-E; mirrors rollback.rs         *)
(* validate_replay_context + ReplayContextRootMismatch). A rollback may    *)
(* only have been entered with a "good" replay context. If the faithful    *)
(* controller ever reaches phase "rollback" while replayCtx is bad, the    *)
(* fail-closed gate was bypassed -- which is exactly what the              *)
(* AllowBadReplayContext=TRUE broken control produces, and TLC must catch.  *)
(***************************************************************************)
ReplayContextFailClosed ==
    (phase = "rollback") => (replayCtx = "good")

===========================================================================
