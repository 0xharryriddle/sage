---------------------- MODULE SageReadinessLock ----------------------
(***************************************************************************)
(* Bounded DESIGN check of Rule 1 and source quiescence, not a Rust         *)
(* refinement, a full protocol proof, or an activation/retry model.         *)
(* n=4, f=1, one epoch, one boundary HEIGHT, two distinct complete          *)
(* candidate bodies at that height, and ONE representative source block     *)
(* above it in old generation g1. Each issued source flag signs THAT SAME   *)
(* block at h_b+1, so SourceVoters counts one possible finalization quorum, *)
(* not votes pooled across conflicting blocks/heights. Matching released   *)
(* ready shares form a CutCert. There is no candidate unlock, timeout, retry,*)
(* or new source generation. Prepared/issued evidence survives a crash.     *)
(*                                                                         *)
(* ObserveReady abstracts local finalization, recomputation, and Ready at  *)
(* the candidate; the engine and cryptography are assumed, not checked.    *)
(* LockCandidate atomically persists the one-body slot AND the source stop. *)
(* The volatile signing guard is synchronized by this write and reloaded   *)
(* before any source use after a crash. A correct ready share is externally *)
(* released only after the write. Crash may occur once per trace at any     *)
(* point, including before or after the write; the next use must reload.    *)
(* Byzantine signers may share both candidates and issue source votes.     *)
(* Distinct candidates never combine in a certificate.                    *)
(*                                                                         *)
(* The two switches deliberately remove just one safety obligation each:   *)
(* early share release before the atomic write, or restart without         *)
(* restoring the volatile signing guard. The bounded reachability control  *)
(* intentionally violates NoCutCertAfterLockedRestart to exhibit a valid   *)
(* certificate formed with a correct signer who crashed AFTER locking.     *)
(***************************************************************************)
EXTENDS Naturals, FiniteSets, TLC

CONSTANTS CorrectVals, ByzVals, Candidates, NoCandidate, QSource,
          AllowEarlyShare, AllowRestartBypass

Validators == CorrectVals \cup ByzVals
N == Cardinality(Validators)
F == Cardinality(ByzVals)
QCut == N - F
Perms == Permutations(CorrectVals) \cup Permutations(ByzVals)

ASSUME /\ CorrectVals \cap ByzVals = {}
       /\ Cardinality(CorrectVals) = 3
       /\ F = 1
       /\ Cardinality(Candidates) = 2
       /\ NoCandidate \notin Candidates \cup Validators
       /\ QSource \in Nat /\ QSource > 2 * F /\ QSource <= N
       /\ AllowEarlyShare \in BOOLEAN
       /\ AllowRestartBypass \in BOOLEAN

VARIABLES
    observed,        \* correct local verification of a candidate (volatile)
    slot,            \* durable first candidate body, never changed or cleared
    prohibited,      \* durable old-generation source stop (atomic with slot)
    guard,           \* volatile stop consulted by all correct source paths
    shared,          \* externally released ready shares, indexed by body
    prepared,        \* historical prepared source authorization above boundary
    issued,          \* historical released source authorization above boundary
    crashTarget,     \* at most one correct validator crashes per trace
    crashPhase,      \* "fresh", "down", or "reloaded"
    crashAfterLock,  \* records whether the crash followed a durable lock
    sharedAfterReload \* witness: a ready share was released after reload

vars == <<observed, slot, prohibited, guard, shared, prepared, issued,
          crashTarget, crashPhase, crashAfterLock, sharedAfterReload>>

Up(v) == ~(crashTarget = v /\ crashPhase = "down")
ReadySigners(b) == {v \in Validators : b \in shared[v]}
CutCert(b) == Cardinality(ReadySigners(b)) >= QCut
SourceVoters == {v \in Validators : issued[v]} \* one block at h_b+1, generation g1

TypeOK ==
    /\ observed \in [CorrectVals -> (Candidates \cup {NoCandidate})]
    /\ slot \in [CorrectVals -> (Candidates \cup {NoCandidate})]
    /\ prohibited \in [CorrectVals -> BOOLEAN]
    /\ guard \in [CorrectVals -> BOOLEAN]
    /\ shared \in [Validators -> SUBSET Candidates]
    /\ prepared \in [Validators -> BOOLEAN]
    /\ issued \in [Validators -> BOOLEAN]
    /\ crashTarget \in CorrectVals \cup {NoCandidate}
    /\ crashPhase \in {"fresh", "down", "reloaded"}
    /\ crashAfterLock \in BOOLEAN
    /\ sharedAfterReload \in BOOLEAN

Init ==
    /\ observed = [v \in CorrectVals |-> NoCandidate]
    /\ slot = [v \in CorrectVals |-> NoCandidate]
    /\ prohibited = [v \in CorrectVals |-> FALSE]
    /\ guard = [v \in CorrectVals |-> FALSE]
    /\ shared = [v \in Validators |-> {}]
    /\ prepared = [v \in Validators |-> FALSE]
    /\ issued = [v \in Validators |-> FALSE]
    /\ crashTarget = NoCandidate
    /\ crashPhase = "fresh"
    /\ crashAfterLock = FALSE
    /\ sharedAfterReload = FALSE

\* Local eligibility is abstracted; a correct validator observes one body.
ObserveReady(v, b) ==
    /\ Up(v)
    /\ observed[v] = NoCandidate
    /\ slot[v] = NoCandidate
    /\ shared[v] = {}
    /\ observed' = [observed EXCEPT ![v] = b]
    /\ UNCHANGED <<slot, prohibited, guard, shared, prepared, issued,
                   crashTarget, crashPhase, crashAfterLock, sharedAfterReload>>

\* No prepared OR issued source action is compatible with attesting at hb.
\* Slot and prohibition are one atomic durable transition, not two writes.
LockCandidate(v) ==
    /\ Up(v)
    /\ observed[v] \in Candidates
    /\ slot[v] = NoCandidate
    /\ ~prepared[v] /\ ~issued[v]
    /\ (shared[v] = {} \/ shared[v] = {observed[v]})
    /\ slot' = [slot EXCEPT ![v] = observed[v]]
    /\ prohibited' = [prohibited EXCEPT ![v] = TRUE]
    /\ guard' = [guard EXCEPT ![v] = TRUE]
    /\ UNCHANGED <<observed, shared, prepared, issued,
                   crashTarget, crashPhase, crashAfterLock, sharedAfterReload>>

\* A released share remains visible even if its signer crashes later.
ReleaseReady(v, b) ==
    /\ Up(v)
    /\ shared[v] = {}
    /\ ~prepared[v] /\ ~issued[v]
    /\ IF AllowEarlyShare
          THEN (observed[v] = b \/ slot[v] = b)
          ELSE slot[v] = b /\ prohibited[v] /\ guard[v]
    /\ shared' = [shared EXCEPT ![v] = {b}]
    /\ sharedAfterReload' = (sharedAfterReload \/
           (crashTarget = v /\ crashPhase = "reloaded" /\ crashAfterLock))
    /\ UNCHANGED <<observed, slot, prohibited, guard, prepared, issued,
                   crashTarget, crashPhase, crashAfterLock>>

\* Byzantine validators may sign in both domains, with no lock obligation.
ByzReleaseReady(v, b) ==
    /\ b \notin shared[v]
    /\ shared' = [shared EXCEPT ![v] = @ \cup {b}]
    /\ UNCHANGED <<observed, slot, prohibited, guard, prepared, issued,
                   crashTarget, crashPhase, crashAfterLock, sharedAfterReload>>

\* Source proposal/vote/finalization authorization has a prepare/release gap.
\* Both paths check the same loaded guard; delayed work cannot bypass it.
CorrectPrepareSource(v) ==
    /\ Up(v) /\ ~guard[v]
    /\ ~prepared[v] /\ ~issued[v]
    /\ prepared' = [prepared EXCEPT ![v] = TRUE]
    /\ UNCHANGED <<observed, slot, prohibited, guard, shared, issued,
                   crashTarget, crashPhase, crashAfterLock, sharedAfterReload>>

CorrectReleaseSource(v) ==
    /\ Up(v) /\ ~guard[v]
    /\ prepared[v] /\ ~issued[v]
    /\ issued' = [issued EXCEPT ![v] = TRUE]
    /\ UNCHANGED <<observed, slot, prohibited, guard, shared, prepared,
                   crashTarget, crashPhase, crashAfterLock, sharedAfterReload>>

ByzPrepareSource(v) ==
    /\ ~prepared[v] /\ ~issued[v]
    /\ prepared' = [prepared EXCEPT ![v] = TRUE]
    /\ UNCHANGED <<observed, slot, prohibited, guard, shared, issued,
                   crashTarget, crashPhase, crashAfterLock, sharedAfterReload>>

ByzReleaseSource(v) ==
    /\ prepared[v] /\ ~issued[v]
    /\ issued' = [issued EXCEPT ![v] = TRUE]
    /\ UNCHANGED <<observed, slot, prohibited, guard, shared, prepared,
                   crashTarget, crashPhase, crashAfterLock, sharedAfterReload>>

\* One crash at an arbitrary point. Durable state and external shares survive;
\* volatile eligibility and the signing guard do not. No use while down.
Crash(v) ==
    /\ crashPhase = "fresh"
    /\ crashTarget' = v
    /\ crashPhase' = "down"
    /\ crashAfterLock' = prohibited[v]
    /\ observed' = [observed EXCEPT ![v] = NoCandidate]
    /\ guard' = [guard EXCEPT ![v] = FALSE]
    /\ UNCHANGED <<slot, prohibited, shared, prepared, issued, sharedAfterReload>>

Reload(v) ==
    /\ crashTarget = v /\ crashPhase = "down"
    /\ crashPhase' = "reloaded"
    /\ guard' = [guard EXCEPT ![v] =
                    IF AllowRestartBypass THEN FALSE ELSE prohibited[v]]
    /\ UNCHANGED <<observed, slot, prohibited, shared, prepared, issued,
                   crashTarget, crashAfterLock, sharedAfterReload>>

Next ==
    \/ \E v \in CorrectVals, b \in Candidates :
         ObserveReady(v, b) \/ ReleaseReady(v, b)
    \/ \E v \in CorrectVals :
         \/ LockCandidate(v)
         \/ CorrectPrepareSource(v)
         \/ CorrectReleaseSource(v)
         \/ Crash(v)
         \/ Reload(v)
    \/ \E v \in ByzVals, b \in Candidates : ByzReleaseReady(v, b)
    \/ \E v \in ByzVals : ByzPrepareSource(v) \/ ByzReleaseSource(v)
    \/ UNCHANGED vars  \* finite safety prefixes may stutter; deadlock check stays on

\* Safety only: finite prefixes and stuttering; no unconditional liveness.
Spec == Init /\ [][Next]_vars

\* The durable slot and prohibition cannot be written separately.
AtomicSlotAndStop ==
    \A v \in CorrectVals : (slot[v] # NoCandidate) <=> prohibited[v]

\* Shares are bound to exactly the persistently selected complete body.
CorrectReadyShareDurable ==
    \A v \in CorrectVals :
        shared[v] # {} =>
            (slot[v] \in Candidates /\ shared[v] = {slot[v]} /\ prohibited[v])

\* No correct signer prepared or issued a source action above this boundary.
NoPreparedOrIssuedAfterLock ==
    \A v \in CorrectVals : prohibited[v] => ~(prepared[v] \/ issued[v])

\* Before any post-crash signing/consensus use, the durable stop is reloaded.
RestartReloadBeforeUse ==
    \A v \in CorrectVals : (Up(v) /\ prohibited[v]) => guard[v]

\* At n=4 a quorum of three matching bodies cannot coexist with a distinct
\* quorum: their intersection includes a correct one-body signer.
UniqueCutCertBody ==
    ~\E b1, b2 \in Candidates : b1 # b2 /\ CutCert(b1) /\ CutCert(b2)

\* The central Rule 1 lemma, including source votes issued BEFORE the CutCert.
\* A matching certificate has QCut signers; at most f can be Byzantine and
\* every correct signer is stopped both before and after the attestation.
NoSourceQuorumAfterCutCert ==
    \A b \in Candidates : CutCert(b) => Cardinality(SourceVoters) < QSource

\* Intentionally FALSE in the reachability config: a correct signer locks,
\* crashes, reloads its durable stop, THEN releases its matching ready share;
\* the other signers form a CutCert containing this post-restart share.
NoCutCertAfterLockedRestart ==
    ~(\E b \in Candidates :
        /\ CutCert(b)
        /\ crashPhase = "reloaded"
        /\ crashAfterLock /\ sharedAfterReload
        /\ \E v \in CorrectVals :
             /\ crashTarget = v
             /\ slot[v] = b /\ prohibited[v] /\ guard[v]
             /\ b \in shared[v])

\* Intentionally FALSE in another faithful reachability config: old-source
\* authorization for the representative block is permitted before any lock.
NoSourceVoteBeforeLock ==
    ~\E v \in CorrectVals : issued[v] /\ slot[v] = NoCandidate

=============================================================================
