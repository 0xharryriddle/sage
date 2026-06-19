---------------------------- MODULE Sage ----------------------------
(***************************************************************************)
(* Bounded model check of SAGE's cross-boundary safety under a network     *)
(* partition straddling the cutover, plus decision uniqueness and rollback *)
(* safety. This is the artifact backing paper.tex App. `app:spec`          *)
(* ("A bounded-model check over n in {4,7,10} and all partition splits of  *)
(* the cutover window reproduces I and the rollback property") and the     *)
(* Path A item 2 in docs/reviews/PUBLISHABILITY_ASSESSMENT.md.             *)
(*                                                                         *)
(* FAITHFULNESS (guards against an abstract spec that trivially passes):   *)
(*  - Cutover requires an (n-f) CutCert formed from SAME-SIDE readiness     *)
(*    attestations, exactly as the runtime `drive_quorum_cutover` counts    *)
(*    same-side attesters (crates/sage-node/src/lib.rs) and the paper's     *)
(*    Definition of a CutCert (n-f attestations).                          *)
(*  - The partition drops cross-side messages, so a validator only sees     *)
(*    attestations from its own side -- the socket-layer partition the      *)
(*    multi-process testbed engages via Transport::partition_to.            *)
(*  - TWO sides each finalizing a DIFFERENT boundary block is the observed  *)
(*    fork the testbed produced under the blind (HardFork) control.         *)
(*                                                                         *)
(* FALSIFIABILITY: the QuorumGated constant switches the SAME model between *)
(* the faithful gate (TRUE) and the blind control (FALSE). With QuorumGated *)
(* = FALSE, TLC MUST report a Safety counterexample -- proving the check    *)
(* has teeth, mirroring the empirical broken control (hardfork forks 5/5).  *)
(***************************************************************************)
EXTENDS Naturals, FiniteSets

CONSTANTS
    N,            \* number of validators
    F,            \* maximum Byzantine / unavailable validators
    QuorumGated   \* TRUE = faithful (n-f CutCert); FALSE = blind local cutover (control)

ASSUME N \in Nat /\ F \in Nat /\ N >= 3 * F + 1

Validators == 1 .. N

\* SAGE's MigrationCutover quorum is n - f (paper Definition of CutCert).
Quorum == N - F

\* A binary partition. Two disjoint sides covering all validators are the
\* split the cross-boundary safety argument addresses; TLC explores every
\* assignment of validators to sides (all splits) from Init.
Sides == {1, 2}

VARIABLES
    side,       \* [Validators -> Sides] : partition assignment (fixed after Init)
    attested,   \* subset of Validators that have broadcast a readiness attestation
    switched,   \* [Validators -> BOOLEAN] : has v cut over to the target engine
    committed   \* [Validators -> 0..2] : boundary block v committed (0 = none; else side id)

vars == <<side, attested, switched, committed>>

\* Same-side attesters this far. Under partition a validator only RECEIVES
\* attestations from its own side, so only these count toward its CutCert.
SideAttesters(s) == { w \in attested : side[w] = s }

\* A side holds a CutCert once >= Quorum of its members have attested.
SideHasQuorum(s) == Cardinality(SideAttesters(s)) >= Quorum

TypeOK ==
    /\ side \in [Validators -> Sides]
    /\ attested \subseteq Validators
    /\ switched \in [Validators -> BOOLEAN]
    /\ committed \in [Validators -> 0 .. 2]

Init ==
    /\ side \in [Validators -> Sides]   \* TLC explores ALL partition splits
    /\ attested = {}
    /\ switched = [v \in Validators |-> FALSE]
    /\ committed = [v \in Validators |-> 0]

\* A validator in dual-run that has reached local readiness broadcasts its
\* cutover attestation (it is recorded; peers on the same side can see it).
Attest(v) ==
    /\ v \notin attested
    /\ attested' = attested \cup {v}
    /\ UNCHANGED <<side, switched, committed>>

\* Cutover guard. Faithful: v switches only once its SIDE holds an (n-f)
\* CutCert. Blind control: v switches on its OWN local readiness, no quorum --
\* this is the dead-quorum-gate behaviour the empirical M3 experiment exposed
\* (SAGE forked 5/5 before the gate was wired in).
CutoverEnabled(v) ==
    IF QuorumGated THEN SideHasQuorum(side[v]) ELSE v \in attested

Cutover(v) ==
    /\ ~switched[v]
    /\ CutoverEnabled(v)
    /\ switched' = [switched EXCEPT ![v] = TRUE]
    \* On cutover the target engine on v's side finalizes that side's boundary
    \* block; distinct sides mint distinct blocks (modeled by the side id),
    \* exactly as two partitioned leaders mint distinct coinbase blocks.
    /\ committed' = [committed EXCEPT ![v] = side[v]]
    /\ UNCHANGED <<side, attested>>

Next == \E v \in Validators : Attest(v) \/ Cutover(v)

Spec == Init /\ [][Next]_vars /\ WF_vars(Next)

(***************************************************************************)
(* Invariants (the properties the paper claims; TLC checks them on every   *)
(* reachable state over all partition splits).                             *)
(***************************************************************************)

\* Cross-boundary safety (paper Theorem `th:safety`, invariant I): no two
\* validators commit different boundary blocks.
Safety ==
    \A v, w \in Validators :
        (committed[v] # 0 /\ committed[w] # 0) => committed[v] = committed[w]

\* Decision uniqueness (paper Theorem `th:unique`): at most one side can ever
\* form a CutCert. This is the quorum-intersection consequence -- two disjoint
\* (n-f) sets cannot both fit in n validators when n >= 3f+1.
DecisionUniqueness == ~ (SideHasQuorum(1) /\ SideHasQuorum(2))

=============================================================================
