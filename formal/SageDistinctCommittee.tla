------------------- MODULE SageDistinctCommittee -------------------
(***************************************************************************)
(* Bounded cutover-decision model for a migration committee V_M and a      *)
(* potentially different target committee V_T. Readiness attesters belong  *)
(* to V_M; target validators in V_T install the boundary block.            *)
(*                                                                         *)
(* The fixed committee layout represents the configured overlap size; TLC  *)
(* explores all binary side assignments for that layout. This model does  *)
(* not implement a distinct-committee protocol or establish a general     *)
(* quorum theorem. It omits Byzantine equivocation, the source committee, *)
(* engine consensus, certificate dissemination, fence/activation, and     *)
(* rollback.                                                               *)
(***************************************************************************)
EXTENDS Naturals, FiniteSets

CONSTANTS
    NM,          \* size of migration committee V_M
    NT,          \* size of target committee V_T
    FM,          \* fault bound attributed to migration committee
    FT,          \* fault bound attributed to target committee
    Overlap,     \* number of validators shared by V_M and V_T
    JointGated   \* TRUE = maximum threshold; FALSE = target-only control

ASSUME NM \in Nat /\ NT \in Nat /\ FM \in Nat /\ FT \in Nat
ASSUME Overlap \in Nat /\ Overlap <= NM /\ Overlap <= NT
ASSUME NM >= 3 * FM + 1 /\ NT >= 3 * FT + 1

\* A canonical layout for the configured committee sizes and overlap.
MigrationOnly == (Overlap + 1) .. NM
Shared        == 1 .. Overlap
TargetOnly    == (NM + 1) .. (NM + NT - Overlap)

VM == Shared \cup MigrationOnly
VT == Shared \cup TargetOnly
AllValidators == VM \cup VT

Sides == {1, 2}

MigrationQuorum == NM - FM
TargetQuorum    == NT - FT

JointQuorum ==
    IF MigrationQuorum > TargetQuorum THEN MigrationQuorum ELSE TargetQuorum

\* Attestations come only from VM. The control mistakenly sizes that gate
\* against VT alone; if TargetQuorum <= NM / 2, two disjoint migration-side
\* attester sets can each satisfy it.
RequiredQuorum == IF JointGated THEN JointQuorum ELSE TargetQuorum

VARIABLES
    side,       \* [AllValidators -> Sides], fixed binary partition
    attested,   \* subset of VM that has broadcast a readiness attestation
    committed   \* [VT -> 0..2], installed boundary block on target validators

vars == <<side, attested, committed>>

SideAttesters(s) == { w \in VM : w \in attested /\ side[w] = s }
SideHasQuorum(s) == Cardinality(SideAttesters(s)) >= RequiredQuorum

TypeOK ==
    /\ side \in [AllValidators -> Sides]
    /\ attested \subseteq VM
    /\ committed \in [VT -> 0 .. 2]

Init ==
    /\ side \in [AllValidators -> Sides]
    /\ attested = {}
    /\ committed = [v \in VT |-> 0]

Attest(v) ==
    /\ v \in VM
    /\ v \notin attested
    /\ attested' = attested \cup {v}
    /\ UNCHANGED <<side, committed>>

\* The two partition sides have distinct representative boundary blocks.
Cutover(v) ==
    /\ v \in VT
    /\ committed[v] = 0
    /\ SideHasQuorum(side[v])
    /\ committed' = [committed EXCEPT ![v] = side[v]]
    /\ UNCHANGED <<side, attested>>

Next == \E v \in AllValidators : Attest(v) \/ Cutover(v)

Spec == Init /\ [][Next]_vars /\ WF_vars(Next)

Safety ==
    \A v, w \in VT :
        (committed[v] # 0 /\ committed[w] # 0) => committed[v] = committed[w]

DecisionUniqueness == ~ (SideHasQuorum(1) /\ SideHasQuorum(2))

=============================================================================
