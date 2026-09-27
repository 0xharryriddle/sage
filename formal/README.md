# SAGE Bounded Model Check (TLA+ / TLC)

This directory contains finite decision and authority *design abstractions*, not
an end-to-end runtime proof. `Sage.tla` checks target-side boundary agreement
under an **assumed** old-generation fence. Separate `SageFence.tla`,
`ManifestAgreement.tla`, `SageRollback.tla`, and `SageReadinessLock.tla`
explore narrower obligations; combining their verdicts is not a refinement
proof or authority to execute a migration, activation, or Candidate Retry.

## Files

| File | Purpose |
|---|---|
| `Sage.tla`, `Sage_n{4,7,10}.cfg`, `SageBlind_n{4,7}.cfg` | Binary-partition decision abstraction under an assumed legacy fence; faithful HOLD, blind `Safety` counterexample. |
| `SageDistinctCommittee.tla`, `SageDistinctCommittee_{partial,disjoint}.cfg`, `SageDistinctCommitteeBroken.cfg` | Fixed partial/disjoint migration and target committee layouts. Joint-threshold HOLD; target-only threshold counterexample. Decision abstraction only. |
| `ManifestAgreement.tla`, `ManifestAgreement.cfg`, `ManifestAgreementBroken.cfg` | One n=4/f=1 epoch, full-payload n−f manifest share agreement HOLD; Byzantine single-producer authentication control violates `CompleteManifestAgreement`. |
| `SageFence.tla`, `SageFence_n{4,5}.cfg`, `SageFenceSplit_n4.cfg`, `SageFenceShareEarly.cfg`, `SageFenceUngated.cfg` | Partial-delivery cross-engine authority design model; n=4/5 admitted HOLD; inadmissible source quorum, premature share, CutCert-only activation controls violate named invariants. |
| `SageRollback.tla`, `SageRollback.cfg`, `SageRollbackBroken.cfg`, `SageRollbackBadCtx.cfg`, `SageRollbackUncertifiedSeal.cfg` | Certificate-gated abort/seal and local replay restoration; faithful HOLD, three separate broken controls. |
| `SageReadinessLock.tla`, its five configs, `check_readiness_lock.sh` | Independent n=4/f=1 Rule 1 design check: faithful HOLD; early-share and restart-bypass controls; two positive reachability witnesses. Not a retry model. |
| `run_tlc.sh` | Historical legacy decision/rollback runner only. Not modified here; it removes `formal/states/` on exit. Do **not** use it in a checkout that retains that directory. |
| `tla2tools.jar` | Vendored TLC 2.19. |

## Verification (from the repository root)

Requires Java and the vendored TLC jar. Run directly from the repository root,
with fresh isolated metadata outside `formal/states/`, and inspect each TLC
exit code and named invariant. For example, the complete-manifest HOLD and
single-producer control are:

```bash
java -Xmx2g -cp formal/tla2tools.jar tlc2.TLC -workers 4 -deadlock \
  -metadir "$(mktemp -d)" -config formal/ManifestAgreement.cfg formal/ManifestAgreement.tla
java -Xmx2g -cp formal/tla2tools.jar tlc2.TLC -workers 4 -deadlock \
  -metadir "$(mktemp -d)" -config formal/ManifestAgreementBroken.cfg formal/ManifestAgreement.tla
```

The faithful config must print `No error has been found`; the broken config
must report `Invariant CompleteManifestAgreement is violated`. The temporary
directories are not in the repository; remove only directories you created
after inspecting the verdict. The same invocation pattern, with the indicated
model and config, applies to:

| Model | HOLD configs | Expected counterexample config → invariant |
|---|---|---|
| `Sage.tla` | `Sage_n4.cfg`, `Sage_n7.cfg`, `Sage_n10.cfg` | `SageBlind_n4.cfg`, `SageBlind_n7.cfg` → `Safety` |
| `SageDistinctCommittee.tla` | `SageDistinctCommittee_partial.cfg`, `SageDistinctCommittee_disjoint.cfg` | `SageDistinctCommitteeBroken.cfg` → `DecisionUniqueness` or `Safety` |
| `SageFence.tla` | `SageFence_n4.cfg`, `SageFence_n5.cfg` | `SageFenceSplit_n4.cfg` → `NoResidualSourceAuthority`; `SageFenceShareEarly.cfg` → `FenceShareOrdering`; `SageFenceUngated.cfg` → `TargetActivationGated` |
| `SageRollback.tla` | `SageRollback.cfg` | `SageRollbackBroken.cfg` → `NoAbsoluteReversion`; `SageRollbackBadCtx.cfg` → `ReplayContextFailClosed`; `SageRollbackUncertifiedSeal.cfg` → `NoUncertifiedAbsolutePromotion` |

The separate Rule 1 runner checks its faithful HOLD, two named safety-control
counterexamples, and two *intentional reachability* counterexamples:

```bash
bash formal/check_readiness_lock.sh
```

`NoCutCertAfterLockedRestart` and `NoSourceVoteBeforeLock` are intentionally
false reachability probes; they are **not** safety failures. The runner uses
temporary TLC metadata and fails on timeout, deadlock, parser error, or the
wrong named verdict. The historical `run_tlc.sh` neither covers these new
models nor safely preserves `formal/states/`; no new model is registered in
that witnessed runner. None of these checks was executed merely by copying
its source: report HOLD only after a completed direct TLC run.

## Scope and limitations

- `Sage.tla` explores binary partitions for n ∈ {4,7,10} and assumes the old
  generation cannot commit at the fixed boundary. `Safety` means target-side
  boundary agreement only; it does not prove cross-engine safety by itself.
- `SageDistinctCommittee.tla` explores binary assignments for *fixed* committee
  sizes/overlaps and a single attestation per migration member. It does not
  model Byzantine double-signing, availability, or runtime support for distinct
  committees. The joint threshold is sufficient only in this abstraction.
- `SageFence.tla` models fence knowledge and authority decisions under partial
  delivery and crash/restart, not engine consensus or certificate dissemination.
  The retained faithful configs are n=4/5; no n=7 faithful HOLD is claimed.
- `ManifestAgreement.tla` models complete-payload shares and one-body adoption
  in one n=4/f=1 epoch, not gossip or live convergence.
- `SageRollback.tla` abstracts one agreed manifest identity, durable terminal
  votes, authority rollback and local replay-context restoration. It assumes
  certificates and fences as modeled; it does not implement their networking.
- `SageReadinessLock.tla` models two complete candidate identities, one old
  source block at the boundary+1, an atomic durable slot/stop, one crash and
  reload, and Byzantine dual-domain actions. Its source quorum is *not* pooled
  across blocks; no candidate unlock, timeout, retry, target activation, or
  model-to-Rust refinement is represented.

All models are bounded safety/decision checks with deliberate falsifying
controls, not a general theorem, execution clearance, or evidence of any
Candidate Retry stage. No generated `formal/states/` dumps or test results are
part of this source-only set.
