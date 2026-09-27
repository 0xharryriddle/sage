--------------------------- MODULE SageRollback ---------------------------
(***************************************************************************)
(* Bounded model check of SAGE's rollback correctness (paper Theorem       *)
(* `th:rollback`) and the executable invariant `check_no_absolute_reversion`*)
(* (crates/sage-controller/src/invariants.rs). Companion to Sage.tla, which *)
(* covers target-side boundary agreement + decision uniqueness under an     *)
(* assumed legacy fence. Together they back                                *)
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
    N,                      \* committee size
    F,                      \* Byzantine fault bound
    ManifestIdentity,       \* agreed MC02 complete-manifest payload identity
    AllowAbsoluteReversion, \* FALSE = faithful; TRUE = broken control (reverts absolute)
    AllowBadReplayContext,  \* FALSE = faithful fail-closed; TRUE = broken (rolls back on bad ctx)
    AllowUncertifiedSeal    \* FALSE = certificate gate; TRUE = broken auto-seal control

ASSUME Hd \in Nat /\ Hc \in Nat /\ Hr \in Nat /\ MaxH \in Nat
ASSUME N \in Nat /\ F \in Nat /\ 0 < N /\ 3 * F < N
ASSUME 0 < Hd /\ Hd < Hc /\ Hc =< Hr /\ Hr =< MaxH

Heights == 1 .. MaxH
Validators == 1 .. N
TerminalKinds == {"none", "abort", "seal"}
\* Pre-cutover heights are legacy-finalized = absolutely final.
LegacyHeights == 1 .. (Hc - 1)
\* [h_c, h_r) blocks are provisional until the seal at h_r.
ProvisionalHeights == Hc .. (Hr - 1)

VARIABLES
    h,              \* current height being processed
    phase,          \* "dual" | "v2" | "rollback" | "sealed"
    absolute,       \* subset of Heights: absolutely-final committed blocks
    provisional,    \* subset of Heights: provisionally-final committed blocks
    replayCtx,      \* "good" | "missing" | "mismatch": replay-context status at abort
    fenceIdentity,  \* "none" or the agreed complete-manifest identity
    terminalVote    \* one durable terminal payload kind per validator

vars == <<h, phase, absolute, provisional, replayCtx, fenceIdentity, terminalVote>>

SealVoters == {v \in Validators : terminalVote[v] = "seal"}
AbortVoters == {v \in Validators : terminalVote[v] = "abort"}
SealQuorum == Cardinality(SealVoters) >= N - F
AbortQuorum == Cardinality(AbortVoters) >= N - F
FenceInstalled == fenceIdentity = ManifestIdentity

TypeOK ==
    /\ h \in 0 .. MaxH
    /\ phase \in {"dual", "v2", "rollback_pending", "rollback", "sealed"}
    /\ absolute \subseteq Heights
    /\ provisional \subseteq Heights
    /\ replayCtx \in {"good", "missing", "mismatch"}
    /\ fenceIdentity \in {"none", ManifestIdentity}
    /\ terminalVote \in [Validators -> TerminalKinds]

Init ==
    /\ h = Hd
    /\ phase = "dual"
    /\ absolute = { x \in Heights : x < Hd }   \* genesis..h_d-1 already absolute
    /\ provisional = {}
    /\ replayCtx = "good"
    /\ fenceIdentity = "none"
    /\ terminalVote = [v \in Validators |-> "none"]

\* Committee-wide fence installation is a state-changing ACTION, not an
\* assumption. It installs exactly the agreed MC02 manifest identity before any
\* cutover or terminal share can be issued.
StepInstallFence ==
    /\ phase = "dual"
    /\ fenceIdentity = "none"
    /\ fenceIdentity' = ManifestIdentity
    /\ UNCHANGED <<h, phase, absolute, provisional, replayCtx, terminalVote>>

\* Dual-run: legacy finalizes height h absolutely, then advances.
StepDual ==
    /\ phase = "dual"
    /\ h < Hc
    /\ absolute' = absolute \cup {h}
    /\ h' = h + 1
    /\ UNCHANGED <<phase, provisional, replayCtx, fenceIdentity, terminalVote>>

\* Cutover at h_c requires the committee fence bound to the agreed manifest.
StepCutover ==
    /\ phase = "dual"
    /\ h = Hc
    /\ FenceInstalled
    /\ phase' = "v2"
    /\ UNCHANGED <<h, absolute, provisional, replayCtx, fenceIdentity, terminalVote>>

\* Target engine finalizes a PROVISIONAL block at h in [h_c, h_r).
StepTarget ==
    /\ phase = "v2"
    /\ h \in ProvisionalHeights
    /\ provisional' = provisional \cup {h}
    /\ h' = h + 1
    /\ UNCHANGED <<phase, absolute, replayCtx, fenceIdentity, terminalVote>>

\* A correct validator's replay context for the reversible suffix may be
\* incomplete or hash-mismatched (modeled nondeterministically).
StepCorruptCtx ==
    /\ phase = "v2"
    /\ replayCtx = "good"
    /\ replayCtx' \in {"missing", "mismatch"}
    /\ UNCHANGED <<h, phase, absolute, provisional, fenceIdentity, terminalVote>>

\* One durable terminal payload per validator. The `none` guard is the modeled
\* one-payload lock and makes abort-vs-seal mutually exclusive per validator.
StepTerminalVote(kind) ==
    /\ phase = "v2"
    /\ FenceInstalled
    /\ kind \in {"abort", "seal"}
    /\ \E v \in Validators :
           /\ terminalVote[v] = "none"
           /\ terminalVote' = [terminalVote EXCEPT ![v] = kind]
    /\ UNCHANGED <<h, phase, absolute, provisional, replayCtx, fenceIdentity>>

\* Abort is certificate-gated and can form only behind the manifest-bound fence.
\*
\* PAPER SEPARATION (Sec. III + Algorithm 1, Thm `th:rollback`): the certified
\* authority transition and the local replay-context restoration are SEPARATE
\* paths, and this model keeps them as two distinct actions:
\*
\*   StepAdoptAbortCert   : an n-f AbortCert before h_r, behind the manifest-bound
\*                          fence, returns committee authority to the legacy
\*                          engine and discards the provisional suffix.  It does
\*                          NOT require a healthy replay context -- authority is
\*                          gated by the certificate and the deadline alone.
\*                          The resulting phase is `rollback_pending`, in which
\*                          the legacy engine is authoritative but the local
\*                          anchor has not yet been restored.
\*   StepRestoreLocalAnchor : local restoration, enabled only when the replay
\*                          context is good.  It moves `rollback_pending` to
\*                          `rollback`, after which legacy finalization resumes.
\*   StepFailClosed       : a correct validator with a missing/mismatched context
\*                          stays in `rollback_pending` forever -- it never
\*                          restores and never resumes.  This is fail-closed.
\*
\* AllowBadReplayContext=TRUE is the BROKEN CONTROL: it lets restoration proceed
\* on a bad context, which TLC must catch via ReplayContextFailClosed.
StepAdoptAbortCert ==
    /\ phase = "v2"
    /\ h < Hr
    /\ FenceInstalled
    /\ AbortQuorum
    /\ phase' = "rollback_pending"
    /\ provisional' = {}
    /\ absolute' = IF AllowAbsoluteReversion
                   THEN absolute \ {x \in absolute : x = Hc - 1}
                   ELSE absolute
    /\ UNCHANGED <<h, replayCtx, fenceIdentity, terminalVote>>

\* Alias retained for the paper's guard name g_2.
StepAbort == StepAdoptAbortCert

\* Local anchor restoration: enabled only on a good replay context.
StepRestoreLocalAnchor ==
    /\ phase = "rollback_pending"
    /\ (replayCtx = "good" \/ AllowBadReplayContext)
    /\ phase' = "rollback"
    /\ UNCHANGED <<h, absolute, provisional, replayCtx, fenceIdentity, terminalVote>>

\* Fail closed: a bad context leaves the validator in `rollback_pending`.
\* Modeled as an explicit stutter so the state is not a TLC deadlock.
StepFailClosed ==
    /\ phase = "rollback_pending"
    /\ replayCtx # "good"
    /\ ~AllowBadReplayContext
    /\ UNCHANGED vars

\* Seal requires an n-f SealCert. The named broken control reproduces the old
\* deadline auto-seal and TLC must falsify NoUncertifiedAbsolutePromotion.
StepSeal ==
    /\ phase = "v2"
    /\ h >= Hr
    /\ FenceInstalled
    /\ (SealQuorum \/ AllowUncertifiedSeal)
    /\ phase' = "sealed"
    /\ absolute' = absolute \cup provisional
    /\ provisional' = {}
    /\ UNCHANGED <<h, replayCtx, fenceIdentity, terminalVote>>

\* After rollback, legacy resumes finalizing absolutely from the boundary.
StepResume ==
    /\ phase = "rollback"
    /\ h =< MaxH
    /\ absolute' = absolute \cup {h}
    /\ h' = h + 1
    /\ UNCHANGED <<phase, provisional, replayCtx, fenceIdentity, terminalVote>>

Next ==
    \/ StepInstallFence \/ StepDual \/ StepCutover \/ StepTarget
    \/ StepTerminalVote("abort") \/ StepTerminalVote("seal")
    \/ StepAdoptAbortCert \/ StepRestoreLocalAnchor \/ StepFailClosed
    \/ StepSeal \/ StepResume \/ StepCorruptCtx
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

(****************************************************************************)
(* Fail-closed replay-context invariant (P1-E; mirrors rollback.rs         *)
(* validate_replay_context + ReplayContextRootMismatch).                   *)
(*                                                                          *)
(* PAPER SEPARATION: authority adoption (StepAdoptAbortCert) is            *)
(* certificate/deadline-gated and does NOT require good replay context.    *)
(* Local anchor restoration (StepRestoreLocalAnchor) fails closed on bad  *)
(* replay context.  The broken control AllowBadReplayContext=TRUE removes  *)
(* the StepRestoreLocalAnchor guard -- TLC must catch that by allowing the *)
(* stutter action to proceed while context is bad, exposing the violation. *)
(*                                                                          *)
(* Authority adoption reaches `rollback_pending` on a certificate alone, so a  *)
(* bad-context validator IS authority-rolled-back but never locally restored.  *)
(* Restoration to `rollback` is the guarded step, so the fail-closed property  *)
(* is stated on `rollback`, and the broken control violates it directly.       *)
(****************************************************************************)
\* Authority adoption is certificate-gated regardless of local replay state.
AbortCertAuthorityIsAlwaysCertGated ==
    (phase \in {"rollback_pending", "rollback"}) => AbortQuorum

\* Local restoration happens only on a good replay context (fail-closed).
\* AllowBadReplayContext=TRUE is the broken control that violates this.
ReplayContextFailClosed ==
    (phase = "rollback") => (replayCtx = "good")

\* Certificate-gated Seal HOLD: reaching sealed is impossible unless n-f
\* validators have durably selected the seal payload. The
\* AllowUncertifiedSeal control violates this implication.
NoUncertifiedAbsolutePromotion ==
    (phase = "sealed") => SealQuorum

\* MC05: cutover and either terminal decision are unreachable until the
\* committee-wide action installed the fence bound to ManifestIdentity.
FenceGatesTerminalDecisions ==
    /\ (phase \in {"v2", "rollback_pending", "rollback", "sealed"}) => FenceInstalled
    /\ (\E v \in Validators : terminalVote[v] # "none") => FenceInstalled

===========================================================================
