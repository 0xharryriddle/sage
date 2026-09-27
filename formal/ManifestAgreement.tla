---------------------- MODULE ManifestAgreement ----------------------
(***************************************************************************)
(* Bounded model check of complete-manifest agreement for one migration    *)
(* epoch. Two manifest bodies carry the SAME CutCert-certified boundary    *)
(* but bind DIFFERENT tool hashes and replay-context roots.                 *)
(*                                                                         *)
(* FAITHFUL PATH (SingleProducerAuth = FALSE):                              *)
(*  - each ManifestShare authenticates CompletePayload(body), not merely   *)
(*    the shared boundary/CutCert;                                          *)
(*  - a correct validator emits at most one Manifest share in the epoch;   *)
(*  - adoption requires n-f distinct Manifest shares; and                  *)
(*  - the local one-manifest-per-epoch lock permits one adopted body and    *)
(*    retains it if a conflicting body is later presented.                 *)
(* Quorum intersection therefore prevents two distinct complete bodies     *)
(* from both becoming adoptable.                                           *)
(*                                                                         *)
(* BROKEN CONTROL (SingleProducerAuth = TRUE):                              *)
(* adoption instead accepts one producer signature. The Byzantine producer *)
(* may sign both envelopes, reproducing the pre-MC02 path: two correct      *)
(* validators can hold and adopt different bodies despite the same         *)
(* certified boundary. TLC MUST violate CompleteManifestAgreement.          *)
(*                                                                         *)
(* SCOPE: this is a bounded acceptance/agreement model. It does not model   *)
(* reliable dissemination, network gossip, or live multi-host convergence. *)
(***************************************************************************)
EXTENDS Naturals, FiniteSets

CONSTANTS
    N,                  \* validator count
    F,                  \* maximum Byzantine validators
    SingleProducerAuth  \* FALSE = n-f ManifestCertificate; TRUE = legacy control

ASSUME N \in Nat /\ F \in Nat /\ 0 < F /\ N >= 3 * F + 1

Validators == 1 .. N
Byzantine == 1 .. F
Correct == Validators \ Byzantine
Bodies == {1, 2}
NoBody == 0
Epoch == 1
Quorum == N - F

\* Both bodies reuse one CutCert-certified boundary. Their complete signing
\* payloads nevertheless differ in the tool hash and replay-context root.
CertifiedBoundary(body) == 1
ChainId(body) == 1
ConfigId(body) == 1
CutoverHeight(body) == 10
ParentHash(body) == 9
BoundaryRoot(body) == 8
SourceGeneration(body) == 1
TargetGeneration(body) == 2
ToolHash(body) == IF body = 1 THEN 11 ELSE 22
ReplayContextRoot(body) == IF body = 1 THEN 111 ELSE 222
CutCertPayloadHash(body) == 7
CompletePayload(body) ==
    <<ChainId(body),
      Epoch,
      ConfigId(body),
      CutoverHeight(body),
      ParentHash(body),
      CertifiedBoundary(body),
      BoundaryRoot(body),
      SourceGeneration(body),
      TargetGeneration(body),
      ToolHash(body),
      ReplayContextRoot(body),
      CutCertPayloadHash(body)>>
Payloads == {CompletePayload(body) : body \in Bodies}

VARIABLES
    held,            \* [Validators -> SUBSET Bodies]: locally received bodies
    manifestShares,  \* [Payloads -> SUBSET Validators]: shares keyed by complete payload
    producerSigned,  \* subset of Bodies signed by the Byzantine legacy producer
    adopted          \* [Validators -> Bodies \cup {NoBody}]: durable epoch lock

vars == <<held, manifestShares, producerSigned, adopted>>

TypeOK ==
    /\ held \in [Validators -> SUBSET Bodies]
    /\ manifestShares \in [Payloads -> SUBSET Validators]
    /\ producerSigned \subseteq Bodies
    /\ adopted \in [Validators -> Bodies \cup {NoBody}]

Init ==
    /\ held = [v \in Validators |-> {}]
    /\ manifestShares = [payload \in Payloads |-> {}]
    /\ producerSigned = {}
    /\ adopted = [v \in Validators |-> NoBody]

\* Network delivery is nondeterministic: the two correct validators that later
\* adopt need not receive the same envelope. No reliable-gossip assumption is
\* embedded in the safety check.
Hold(v, body) ==
    /\ body \notin held[v]
    /\ held' = [held EXCEPT ![v] = @ \cup {body}]
    /\ UNCHANGED <<manifestShares, producerSigned, adopted>>

HasCorrectShareInEpoch(v) ==
    \E payload \in Payloads : v \in manifestShares[payload]

\* A share is over CompletePayload(body), represented by indexing the retained
\* share set by that full tuple. Byzantine validators may share both payloads.
\* A correct validator shares at most one complete payload in this epoch.
Share(v, body) ==
    /\ body \in held[v]
    /\ v \notin manifestShares[CompletePayload(body)]
    /\ (v \in Byzantine \/ ~HasCorrectShareInEpoch(v))
    /\ manifestShares' =
          [manifestShares EXCEPT ![CompletePayload(body)] = @ \cup {v}]
    /\ UNCHANGED <<held, producerSigned, adopted>>

\* The pre-MC02 producer is Byzantine and may authenticate both conflicting
\* envelopes with a single producer signature.
ProducerSign(body) ==
    /\ body \notin producerSigned
    /\ producerSigned' = producerSigned \cup {body}
    /\ UNCHANGED <<held, manifestShares, adopted>>

ManifestQuorum(body) ==
    Cardinality(manifestShares[CompletePayload(body)]) >= Quorum

Authenticated(body) ==
    IF SingleProducerAuth
    THEN body \in producerSigned
    ELSE ManifestQuorum(body)

\* Only correct-validator adoption matters to the theorem. adopted[v]=NoBody
\* is the durable one-manifest-per-epoch lock: a conflicting later body cannot
\* replace the retained body at v.
Adopt(v, body) ==
    /\ v \in Correct
    /\ body \in held[v]
    /\ adopted[v] = NoBody
    /\ Authenticated(body)
    /\ adopted' = [adopted EXCEPT ![v] = body]
    /\ UNCHANGED <<held, manifestShares, producerSigned>>

Next ==
    \/ \E v \in Validators, body \in Bodies : Hold(v, body)
    \/ \E v \in Validators, body \in Bodies : Share(v, body)
    \/ \E body \in Bodies : ProducerSign(body)
    \/ \E v \in Correct, body \in Bodies : Adopt(v, body)

Spec == Init /\ [][Next]_vars

(***************************************************************************)
(* Complete-manifest agreement: among correct validators, at most one       *)
(* distinct complete body reaches the adopted state in this epoch.          *)
(***************************************************************************)
CompleteManifestAgreement ==
    \A v, w \in Correct :
        (adopted[v] # NoBody /\ adopted[w] # NoBody) => adopted[v] = adopted[w]

=============================================================================
