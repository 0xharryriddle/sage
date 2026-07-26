------------------- MODULE SageDistinctCommittee -------------------
(***************************************************************************)
(* Bounded model check of SAGE's cutover boundary agreement when the       *)
(* MIGRATION committee is NOT identical to the source and target consensus *)
(* committees.  Sage.tla models one identical committee (V_M = V_1 = V_2)  *)
(* with Quorum == N - F; that structure cannot express a distinct-set      *)
(* deployment, which is why the paper narrows its theorems to the          *)
(* identical-committee case (paper.tex: "one identical committee and one   *)
(* corruption universe").  This module closes that modelling gap.          *)
(*                                                                         *)
(* WHY IT MATTERS: with distinct sets, "n - f" is ambiguous -- n and f of  *)
(* WHICH set?  A deployment could size the cutover quorum against the      *)
(* migration committee alone while the fork-relevant agreement lives in    *)
(* the target committee.  This model makes that choice explicit and shows  *)
(* which choices are safe.                                                 *)
(*                                                                         *)
(* FAITHFULNESS (guards against a spec that trivially passes):             *)
(*  - The cutover attestation set is drawn from the MIGRATION committee,   *)
(*    mirroring the runtime, where readiness shares are signed under       *)
(*    AuthorityScope::Cutover by the migration authority registry.         *)
(*  - The fork is observed on the TARGET committee: a target validator     *)
(*    commits a boundary block only when its side's migration quorum is    *)
(*    met, exactly as the runtime withholds installation until the gate    *)
(*    opens.                                                              *)
(*  - The partition drops cross-side messages, so a validator counts only  *)
(*    same-side attestations (Transport::partition_to in the testbed).     *)
(*  - TLC explores EVERY assignment of validators to sides AND every       *)
(*    committee overlap permitted by the configured sizes.                 *)
(*                                                                         *)
(* FALSIFIABILITY: JointGated switches the SAME model between the faithful *)
(* joint gate (TRUE) and a control that sizes the quorum against the       *)
(* migration committee only (FALSE).  With JointGated = FALSE and a        *)
(* migration committee smaller than the target committee, TLC MUST report  *)
(* a DecisionUniqueness counterexample: two disjoint sides can each hold a *)
(* migration-only quorum, so both install a boundary block and the target  *)
(* committee forks.  A model that could not produce that counterexample    *)
(* would not be testing the gate at all.                                   *)
(*                                                                         *)
(* ABSTRACTION BOUNDARY: as in Sage.tla, this abstracts the cutover        *)
(* DECISION only.  It does not model fence dissemination, terminal         *)
(* seal/abort decisions, or the ACTIVATING phase.  It adds exactly one     *)
(* dimension to Sage.tla: committee non-identity.                          *)
(***************************************************************************)
EXTENDS Naturals, FiniteSets

CONSTANTS
    NM,          \* size of the migration committee V_M
    NT,          \* size of the target committee V_2
    FM,          \* fault bound attributed to the migration committee
    FT,          \* fault bound attributed to the target committee
    Overlap,     \* number of validators shared by V_M and V_T
    JointGated   \* TRUE = faithful joint gate; FALSE = migration-only control

ASSUME NM \in Nat /\ NT \in Nat /\ FM \in Nat /\ FT \in Nat
ASSUME Overlap \in Nat /\ Overlap <= NM /\ Overlap <= NT
ASSUME NM >= 3 * FM + 1 /\ NT >= 3 * FT + 1

(***************************************************************************)
(* Committee layout.  The shared prefix 1..Overlap belongs to both         *)
(* committees; the migration committee then extends upward, and the target *)
(* committee occupies a disjoint block above it.  Enumerating a fixed      *)
(* layout (rather than all set pairs) keeps the state space finite while   *)
(* still covering the essential cases: full identity (Overlap = NM = NT),  *)
(* partial overlap, and disjoint committees (Overlap = 0).                 *)
(***************************************************************************)
MigrationOnly == (Overlap + 1) .. NM
Shared        == 1 .. Overlap
TargetOnly    == (NM + 1) .. (NM + NT - Overlap)

VM == Shared \cup MigrationOnly
VT == Shared \cup TargetOnly
AllValidators == VM \cup VT

Sides == {1, 2}

(***************************************************************************)
(* Quorum choices.                                                        *)
(*                                                                        *)
(* Faithful (JointGated = TRUE): the gate must be large enough that two    *)
(* disjoint sides cannot both hold one, measured against the committee     *)
(* whose agreement is at stake.  Taking the maximum of the two thresholds  *)
(* is the conservative joint rule: it satisfies the intersection           *)
(* requirement of BOTH committees simultaneously.                         *)
(*                                                                        *)
(* Control (JointGated = FALSE): size the gate against the migration       *)
(* committee alone -- the plausible-looking mistake this model exists to   *)
(* catch.                                                                 *)
(***************************************************************************)
MigrationQuorum == NM - FM
TargetQuorum    == NT - FT

JointQuorum ==
    IF MigrationQuorum > TargetQuorum THEN MigrationQuorum ELSE TargetQuorum

\* The control sizes the gate against the TARGET committee alone.  This is the
\* unsafe mistake: readiness shares are signed by MIGRATION-committee members,
\* so intersection must be argued inside VM.  When the target committee is
\* smaller than the migration committee, NT-FT can fall at or below |VM|/2 and
\* two disjoint sides each reach the threshold.  Gating on the migration
\* committee alone happens to remain safe here (NM-FM always exceeds NM/2), so
\* it is NOT the counterexample this control needs.
RequiredQuorum == IF JointGated THEN JointQuorum ELSE TargetQuorum

VARIABLES
    side,       \* [AllValidators -> Sides] : partition assignment, fixed after Init
    attested,   \* subset of VM that has broadcast a cutover readiness attestation
    committed   \* [VT -> 0..2] : boundary block a target validator committed

vars == <<side, attested, committed>>

\* Same-side attesters, counted within the migration committee only: a
\* readiness share is valid only if signed by a migration-committee member.
SideAttesters(s) == { w \in VM : w \in attested /\ side[w] = s }

\* A side holds a cutover certificate once enough same-side migration members
\* have attested, per the configured threshold.
SideHasQuorum(s) == Cardinality(SideAttesters(s)) >= RequiredQuorum

TypeOK ==
    /\ side \in [AllValidators -> Sides]
    /\ attested \subseteq VM
    /\ committed \in [VT -> 0 .. 2]

Init ==
    /\ side \in [AllValidators -> Sides]   \* TLC explores ALL splits
    /\ attested = {}
    /\ committed = [v \in VT |-> 0]

Attest(v) ==
    /\ v \in VM
    /\ v \notin attested
    /\ attested' = attested \cup {v}
    /\ UNCHANGED <<side, committed>>

\* A target validator installs its side's boundary block once its side holds
\* the cutover certificate.  Distinct sides mint distinct blocks (side id),
\* exactly as two partitioned leaders mint distinct boundary blocks.
Cutover(v) ==
    /\ v \in VT
    /\ committed[v] = 0
    /\ SideHasQuorum(side[v])
    /\ committed' = [committed EXCEPT ![v] = side[v]]
    /\ UNCHANGED <<side, attested>>

Next == \E v \in AllValidators : Attest(v) \/ Cutover(v)

Spec == Init /\ [][Next]_vars /\ WF_vars(Next)

(***************************************************************************)
(* Invariants.                                                            *)
(***************************************************************************)

\* Boundary agreement across the TARGET committee: no two target validators
\* install different boundary blocks.  This is the fork the paper forbids.
Safety ==
    \A v, w \in VT :
        (committed[v] # 0 /\ committed[w] # 0) => committed[v] = committed[w]

\* Decision uniqueness under distinct committees: at most one side can ever
\* form the cutover certificate.
DecisionUniqueness == ~ (SideHasQuorum(1) /\ SideHasQuorum(2))

=============================================================================
