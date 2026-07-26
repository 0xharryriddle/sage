#!/bin/bash
# Bounded checks of target-side boundary agreement under an assumed legacy
# fence, decision uniqueness, and rollback invariants (Path A item 2).
#
# Runs TLC over the faithful quorum-gated controller for n in {4,7,10} (must
# HOLD: Safety + DecisionUniqueness across all partition splits), and over the
# blind control (no quorum gate) for n in {4,7} (must FAIL with a Safety
# counterexample). The blind control FAILING is what proves the check has teeth
# rather than passing vacuously -- it mirrors the empirical M3 result where the
# HardFork broken control forks 5/5 while quorum-gated SAGE forks 0/5.
#
# Backs the paper claim (paper.tex App. app:spec): "A bounded-model check over
# n in {4,7,10} and all partition splits of the cutover window reproduces I and
# the rollback property."
#
# Usage: bash formal/run_tlc.sh
set -u
cd "$(dirname "$0")"

JAR=tla2tools.jar
if [ ! -f "$JAR" ]; then
  echo "FATAL: $JAR not found in formal/" >&2
  exit 2
fi

# Keep TLC's metadata out of formal/states/, which contains tracked conformance
# inputs. Do not pass TLC's broad `-cleanup` flag: each invocation gets an
# isolated temporary metadata directory and the shell removes only that tree.
TLC_META=$(mktemp -d)
fail=0
trap 'rm -rf "$TLC_META"' EXIT

tlc_run() {
  local metadir
  metadir=$(mktemp -d "$TLC_META/run.XXXXXX")
  java -Xmx3g -cp "$JAR" tlc2.TLC -deadlock -workers 4 -metadir "$metadir" "$@"
}

# Faithful configs MUST pass ("No error has been found").
for cfg in Sage_n4 Sage_n7 Sage_n10; do
  echo "=== FAITHFUL $cfg (must HOLD) ==="
  out=$(tlc_run -config "$cfg.cfg" Sage.tla 2>&1)
  if echo "$out" | grep -q "No error has been found"; then
    echo "  [OK] $cfg: Safety + DecisionUniqueness hold across all splits"
  else
    echo "  [FAIL] $cfg: expected no error, got:"
    echo "$out" | grep -E "Error|violated" | head -3
    fail=1
  fi
done

# Blind control configs MUST fail with a Safety violation (falsifiability gate).
for cfg in SageBlind_n4 SageBlind_n7; do
  echo "=== BLIND CONTROL $cfg (must FIND counterexample) ==="
  out=$(tlc_run -config "$cfg.cfg" Sage.tla 2>&1)
  if echo "$out" | grep -q "Invariant Safety is violated"; then
    echo "  [OK] $cfg: Safety counterexample found (gate has teeth)"
  else
    echo "  [FAIL] $cfg: expected a Safety violation but none was reported"
    fail=1
  fi
done

# Distinct-committee (MC04) joint-quorum boundary agreement. The paper's main
# theorems are stated for one identical committee (V_M = V_1 = V_2); this module
# relaxes that to a migration committee and a target committee that may only
# partially overlap, or be disjoint. Faithful configs use the conservative joint
# threshold; the broken control sizes the gate against the target committee
# alone, which is unsafe when the target committee is the smaller one.
for cfg in SageDistinctCommittee_partial SageDistinctCommittee_disjoint; do
  echo "=== FAITHFUL $cfg (must HOLD) ==="
  out=$(tlc_run -config "$cfg.cfg" SageDistinctCommittee.tla 2>&1)
  if echo "$out" | grep -q "No error has been found"; then
    echo "  [OK] $cfg: Safety + DecisionUniqueness hold under a non-identical committee"
  else
    echo "  [FAIL] $cfg: expected no error, got:"
    echo "$out" | grep -E "Error|violated" | head -3
    fail=1
  fi
done

echo "=== BROKEN CONTROL SageDistinctCommitteeBroken (must FIND counterexample) ==="
out=$(tlc_run -config "SageDistinctCommitteeBroken.cfg" SageDistinctCommittee.tla 2>&1)
if echo "$out" | grep -qE "Invariant (DecisionUniqueness|Safety) is violated"; then
  echo "  [OK] SageDistinctCommitteeBroken: counterexample found (joint gate has teeth)"
else
  echo "  [FAIL] SageDistinctCommitteeBroken: expected a Safety/DecisionUniqueness violation but none was reported"
  fail=1
fi

# Complete-manifest agreement: the faithful multi-share ManifestCertificate
# MUST hold; the legacy single-producer path MUST fail (MC02 falsifiability).
echo "=== FAITHFUL ManifestAgreement (must HOLD) ==="
out=$(tlc_run -config "ManifestAgreement.cfg" ManifestAgreement.tla 2>&1)
if echo "$out" | grep -q "No error has been found"; then
  echo "  [TLC] $(echo "$out" | grep "No error has been found" | tail -1)"
  echo "  [OK] ManifestAgreement: No error found; CompleteManifestAgreement holds"
else
  echo "  [FAIL] ManifestAgreement: expected no error, got:"
  echo "$out" | grep -E "Error|violated" | head -3
  fail=1
fi

echo "=== BROKEN CONTROL ManifestAgreementBroken (must FIND counterexample) ==="
out=$(tlc_run -config "ManifestAgreementBroken.cfg" ManifestAgreement.tla 2>&1)
if echo "$out" | grep -q "Invariant CompleteManifestAgreement is violated"; then
  echo "  [TLC] $(echo "$out" | grep "Invariant CompleteManifestAgreement is violated" | head -1)"
  echo "  [TRACE] $(echo "$out" | grep -F '/\ adopted =' | tail -1)"
  echo "  [OK] ManifestAgreementBroken: Invariant CompleteManifestAgreement is violated; single-signer counterexample found (gate has teeth)"
else
  echo "  [FAIL] ManifestAgreementBroken: expected a CompleteManifestAgreement violation but none was reported"
  fail=1
fi

# Rollback module: faithful MUST hold; broken control MUST fail (no-absolute-reversion).
echo "=== FAITHFUL SageRollback (must HOLD) ==="
out=$(tlc_run -config "SageRollback.cfg" SageRollback.tla 2>&1)
if echo "$out" | grep -q "No error has been found"; then
  echo "  [OK] SageRollback: NoAbsoluteReversion + ProvisionalBounded hold"
else
  echo "  [FAIL] SageRollback: expected no error, got:"
  echo "$out" | grep -E "Error|violated" | head -3
  fail=1
fi

echo "=== BROKEN CONTROL SageRollbackBroken (must FIND counterexample) ==="
out=$(tlc_run -config "SageRollbackBroken.cfg" SageRollback.tla 2>&1)
if echo "$out" | grep -q "Invariant NoAbsoluteReversion is violated"; then
  echo "  [OK] SageRollbackBroken: reversion counterexample found (gate has teeth)"
else
  echo "  [FAIL] SageRollbackBroken: expected a NoAbsoluteReversion violation but none was reported"
  fail=1
fi

echo "=== BROKEN CONTROL SageRollbackBadCtx (must FIND counterexample) ==="
out=$(tlc_run -config "SageRollbackBadCtx.cfg" SageRollback.tla 2>&1)
if echo "$out" | grep -q "Invariant ReplayContextFailClosed is violated"; then
  echo "  [OK] SageRollbackBadCtx: bad-replay-context counterexample found (fail-closed gate has teeth)"
else
  echo "  [FAIL] SageRollbackBadCtx: expected a ReplayContextFailClosed violation but none was reported"
  fail=1
fi

# MC03 terminal-seal gate: the certificate-gated StepSeal MUST hold (checked in
# the faithful SageRollback.cfg via NoUncertifiedAbsolutePromotion); the
# AllowUncertifiedSeal broken control (legacy deadline auto-seal, no n-f seal
# quorum) MUST falsify it -- proving the terminal-certificate gate has teeth.
echo "=== BROKEN CONTROL SageRollbackUncertifiedSeal (must FIND counterexample) ==="
out=$(tlc_run -config "SageRollbackUncertifiedSeal.cfg" SageRollback.tla 2>&1)
if echo "$out" | grep -q "Invariant NoUncertifiedAbsolutePromotion is violated"; then
  echo "  [OK] SageRollbackUncertifiedSeal: uncertified-seal counterexample found (terminal-cert gate has teeth)"
else
  echo "  [FAIL] SageRollbackUncertifiedSeal: expected a NoUncertifiedAbsolutePromotion violation but none was reported"
  fail=1
fi

echo ""
if [ "$fail" -eq 0 ]; then
  echo "ALL TLC CHECKS PASSED: target-boundary and complete-manifest agreement hold in their bounded models; rollback holds; broken controls falsified."
else
  echo "TLC CHECKS FAILED -- see above."
fi
exit $fail
