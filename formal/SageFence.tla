--------------------------- MODULE SageFence ---------------------------
(***************************************************************************)
(* Bounded model check of CROSS-ENGINE AUTHORITY EXCLUSION under PARTIAL    *)
(* transition knowledge (paper Theorem `th:authority-exclusion`).           *)
(*                                                                         *)
(* Sage.tla deliberately ASSUMES an installed committee fence and checks    *)
(* only the cutover DECISION. That abstraction cannot discover a residual   *)
(* source quorum, which is exactly the composition failure raised in the    *)
(* technical review. This module removes the assumption and models the      *)
(* per-validator state the review asked for:                               *)
(*                                                                         *)
(*     cutcertSeen[v], fenceDurable[v], targetActive[v], generation[v]      *)
(*                                                                         *)
(* plus manifestAdopted[v], fenceShared[v], and sourceAuthorized[v], with   *)
(* arbitrary message delivery and crash/restart transitions.                *)
(*                                                                         *)
(* THREAT MODEL (decisive for the review's counterexample): a Byzantine     *)
(* validator MAY participate in BOTH protocol domains during partial        *)
(* activation. It may release a fence share it never durably installed AND  *)
(* still authorize source finalization at or above the boundary. It need    *)
(* not equivocate between two legacy blocks to do so.                       *)
(*                                                                         *)
(* CORRECT-VALIDATOR OBLIGATIONS (Definition `def:fencecert`):              *)
(*  1. adopt the complete manifest before installing the fence;            *)
(*  2. install the fence durably only if it has authorized no source        *)
(*     action at or above the boundary;                                    *)
(*  3. release its fence share only AFTER that durable write;              *)
(*  4. expose target authority only after manifest + durable local fence +  *)
(*     a completed FenceCert.                                              *)
(*                                                                         *)
(* CRASH ORDERING: Restart(v) clears the VOLATILE projections              *)
(* (manifestAdopted, targetActive) but preserves the DURABLE fence record   *)
(* and the record of prior source authorization, so the model explores      *)
(* crash points between every pair of durable writes.                       *)
(*                                                                         *)
(* FALSIFIABILITY. Three constants switch the SAME model between the        *)
(* faithful protocol and deliberately broken controls TLC MUST catch:       *)
(*  - Q1/QF violating the admission rule QF + Q1 > N + F reproduces the     *)
(*    review's (n,f,q_1,q_F) = (7,2,4,5) execution;                        *)
(*  - AllowShareBeforeInstall = TRUE drops obligation 3;                    *)
(*  - AllowUngatedActivation = TRUE drops obligation 4.                    *)
(*                                                                         *)
(* SYMMETRY. Correct and Byzantine identities are supplied as two disjoint  *)
(* sets of model values. No action or invariant distinguishes members       *)
(* WITHIN either set, so TLC may quotient the state space by               *)
(* Permutations(CorrectVals) \cup Permutations(ByzVals). This keeps the     *)
(* n=7 check exhaustive rather than truncated.                             *)
(***************************************************************************)
EXTENDS Naturals, FiniteSets, TLC

CONSTANTS
    CorrectVals,              \* set of correct validator identities
    ByzVals,                  \* set of Byzantine validator identities
    Q1,                       \* admitted source-finality quorum
    QF,                       \* source-fence certificate quorum
    AllowShareBeforeInstall,  \* FALSE = faithful install-before-share ordering
    AllowUngatedActivation    \* FALSE = faithful manifest+fence+FenceCert gate

Validators == CorrectVals \cup ByzVals
N == Cardinality(Validators)
F == Cardinality(ByzVals)

\* Symmetry group offered to TLC via the configuration file.
Perms == Permutations(CorrectVals) \cup Permutations(ByzVals)

ASSUME CorrectVals \cap ByzVals = {}
ASSUME 3 * F < N
ASSUME Q1 \in Nat /\ QF \in Nat /\ 0 < Q1 /\ 0 < QF
ASSUME QF =< N - F          \* fence availability against F silent validators

VARIABLES
    cutcertSeen,        \* [Validators -> BOOLEAN] : has v received the CutCert
    manifestAdopted,    \* [Validators -> BOOLEAN] : durable complete-manifest adoption
    fenceDurable,       \* [Validators -> BOOLEAN] : durable local fence installation
    fenceShared,        \* [Validators -> BOOLEAN] : has v released its fence share
    targetActive,       \* [Validators -> BOOLEAN] : is target authority exposed on v
    sourceAuthorized,   \* [Validators -> BOOLEAN] : has v authorized a source action >= h*
    generation          \* [Validators -> 0..1] : 0 = source generation g1, 1 = renewed g1'

vars == <<cutcertSeen, manifestAdopted, fenceDurable, fenceShared,
          targetActive, sourceAuthorized, generation>>

FenceSigners == {v \in Validators : fenceShared[v]}
SourceVoters == {v \in Validators : sourceAuthorized[v]}

\* A FenceCert exists once QF validators have released matching shares.
FenceCertFormed == Cardinality(FenceSigners) >= QF
\* A residual source quorum exists once Q1 validators have authorized a source
\* action at or above the boundary height.
SourceQuorumFormed == Cardinality(SourceVoters) >= Q1

TypeOK ==
    /\ cutcertSeen \in [Validators -> BOOLEAN]
    /\ manifestAdopted \in [Validators -> BOOLEAN]
    /\ fenceDurable \in [Validators -> BOOLEAN]
    /\ fenceShared \in [Validators -> BOOLEAN]
    /\ targetActive \in [Validators -> BOOLEAN]
    /\ sourceAuthorized \in [Validators -> BOOLEAN]
    /\ generation \in [Validators -> 0 .. 1]

Init ==
    /\ cutcertSeen = [v \in Validators |-> FALSE]
    /\ manifestAdopted = [v \in Validators |-> FALSE]
    /\ fenceDurable = [v \in Validators |-> FALSE]
    /\ fenceShared = [v \in Validators |-> FALSE]
    /\ targetActive = [v \in Validators |-> FALSE]
    /\ sourceAuthorized = [v \in Validators |-> FALSE]
    /\ generation = [v \in Validators |-> 0]

(***************************************************************************)
(* Arbitrary delivery. Nothing forces the CutCert to reach everyone, which  *)
(* is precisely the partial-delivery execution the review constructed.      *)
(***************************************************************************)
DeliverCutCert(v) ==
    /\ ~cutcertSeen[v]
    /\ cutcertSeen' = [cutcertSeen EXCEPT ![v] = TRUE]
    /\ UNCHANGED <<manifestAdopted, fenceDurable, fenceShared,
                   targetActive, sourceAuthorized, generation>>

AdoptManifest(v) ==
    /\ cutcertSeen[v]
    /\ ~manifestAdopted[v]
    /\ manifestAdopted' = [manifestAdopted EXCEPT ![v] = TRUE]
    /\ UNCHANGED <<cutcertSeen, fenceDurable, fenceShared,
                   targetActive, sourceAuthorized, generation>>

\* Obligations 1 and 2: manifest first, and never after authorizing the source
\* at or above the boundary.
InstallFence(v) ==
    /\ manifestAdopted[v]
    /\ ~fenceDurable[v]
    /\ ~sourceAuthorized[v]
    /\ fenceDurable' = [fenceDurable EXCEPT ![v] = TRUE]
    /\ UNCHANGED <<cutcertSeen, manifestAdopted, fenceShared,
                   targetActive, sourceAuthorized, generation>>

\* Obligation 3: a CORRECT validator releases its share only after the durable
\* write. The broken control drops the conjunct.
ShareFence(v) ==
    /\ ~fenceShared[v]
    /\ IF AllowShareBeforeInstall THEN manifestAdopted[v] ELSE fenceDurable[v]
    /\ fenceShared' = [fenceShared EXCEPT ![v] = TRUE]
    /\ UNCHANGED <<cutcertSeen, manifestAdopted, fenceDurable,
                   targetActive, sourceAuthorized, generation>>

\* A Byzantine validator participates in BOTH domains: it can emit a fence
\* share with no durable install of its own.
ByzShareFence(v) ==
    /\ v \in ByzVals
    /\ ~fenceShared[v]
    /\ fenceShared' = [fenceShared EXCEPT ![v] = TRUE]
    /\ UNCHANGED <<cutcertSeen, manifestAdopted, fenceDurable,
                   targetActive, sourceAuthorized, generation>>

\* Obligation 4: target authority requires the local durable fence, the adopted
\* manifest, and a completed FenceCert. The broken control activates on the
\* CutCert alone, which is the pre-revision runtime behaviour.
ActivateTarget(v) ==
    /\ ~targetActive[v]
    /\ IF AllowUngatedActivation
         THEN cutcertSeen[v]
         ELSE /\ manifestAdopted[v]
              /\ fenceDurable[v]
              /\ FenceCertFormed
    /\ targetActive' = [targetActive EXCEPT ![v] = TRUE]
    /\ UNCHANGED <<cutcertSeen, manifestAdopted, fenceDurable,
                   fenceShared, sourceAuthorized, generation>>

\* A correct validator may still authorize a source action while it has no
\* durable fence -- that is the whole hazard of partial delivery. A Byzantine
\* validator may do so unconditionally, in either generation.
AuthorizeSource(v) ==
    /\ ~sourceAuthorized[v]
    /\ generation[v] = 0
    /\ (v \in ByzVals \/ ~fenceDurable[v])
    /\ sourceAuthorized' = [sourceAuthorized EXCEPT ![v] = TRUE]
    /\ UNCHANGED <<cutcertSeen, manifestAdopted, fenceDurable,
                   fenceShared, targetActive, generation>>

\* Crash/restart: volatile projections are lost, durable records survive.
Restart(v) ==
    /\ manifestAdopted[v] \/ targetActive[v]
    /\ manifestAdopted' = [manifestAdopted EXCEPT ![v] = FALSE]
    /\ targetActive' = [targetActive EXCEPT ![v] = FALSE]
    /\ UNCHANGED <<cutcertSeen, fenceDurable, fenceShared,
                   sourceAuthorized, generation>>

\* Certified abort binds a FRESH source generation; the fenced generation is
\* never resumed, so the durable fence is retained.
RenewGeneration(v) ==
    /\ fenceDurable[v]
    /\ generation[v] = 0
    /\ generation' = [generation EXCEPT ![v] = 1]
    /\ UNCHANGED <<cutcertSeen, manifestAdopted, fenceDurable,
                   fenceShared, targetActive, sourceAuthorized>>

Next ==
    \E v \in Validators :
        \/ DeliverCutCert(v)
        \/ AdoptManifest(v)
        \/ InstallFence(v)
        \/ ShareFence(v)
        \/ ByzShareFence(v)
        \/ ActivateTarget(v)
        \/ AuthorizeSource(v)
        \/ Restart(v)
        \/ RenewGeneration(v)

Spec == Init /\ [][Next]_vars /\ WF_vars(Next)

(***************************************************************************)
(* Invariants                                                              *)
(***************************************************************************)

\* THE property the review asked for: once target authority is exposed
\* anywhere, no residual source quorum can exist. This is a direct
\* cross-engine predicate, not an assumption named FenceInstalled.
NoResidualSourceAuthority ==
    (\E v \in Validators : targetActive[v]) => ~SourceQuorumFormed

\* A correct validator never releases a share before its durable write.
FenceShareOrdering ==
    \A v \in CorrectVals : fenceShared[v] => fenceDurable[v]

\* Target authority is never exposed without the local durable fence.
TargetActivationGated ==
    \A v \in Validators : targetActive[v] => fenceDurable[v]

\* A fenced generation is never resumed: no correct validator with a durable
\* fence authorizes a source action in the original generation.
NoFencedGenerationResumption ==
    \A v \in CorrectVals : (fenceDurable[v] /\ generation[v] = 0) => ~sourceAuthorized[v]

=============================================================================
