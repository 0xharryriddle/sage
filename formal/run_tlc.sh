#!/bin/bash
# Bounded model check of SAGE cross-boundary safety (Path A item 2).
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

TLC="java -cp $JAR tlc2.TLC -deadlock -workers 4 -cleanup"
fail=0

# Remove TLC's per-run scratch state directory on exit so the artifact stays tidy.
trap 'rm -rf states' EXIT

# Faithful configs MUST pass ("No error has been found").
for cfg in Sage_n4 Sage_n7 Sage_n10; do
  echo "=== FAITHFUL $cfg (must HOLD) ==="
  out=$($TLC -config "$cfg.cfg" Sage.tla 2>&1)
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
  out=$($TLC -config "$cfg.cfg" Sage.tla 2>&1)
  if echo "$out" | grep -q "Invariant Safety is violated"; then
    echo "  [OK] $cfg: Safety counterexample found (gate has teeth)"
  else
    echo "  [FAIL] $cfg: expected a Safety violation but none was reported"
    fail=1
  fi
done

# Rollback module: faithful MUST hold; broken control MUST fail (no-absolute-reversion).
echo "=== FAITHFUL SageRollback (must HOLD) ==="
out=$($TLC -config "SageRollback.cfg" SageRollback.tla 2>&1)
if echo "$out" | grep -q "No error has been found"; then
  echo "  [OK] SageRollback: NoAbsoluteReversion + ProvisionalBounded hold"
else
  echo "  [FAIL] SageRollback: expected no error, got:"
  echo "$out" | grep -E "Error|violated" | head -3
  fail=1
fi

echo "=== BROKEN CONTROL SageRollbackBroken (must FIND counterexample) ==="
out=$($TLC -config "SageRollbackBroken.cfg" SageRollback.tla 2>&1)
if echo "$out" | grep -q "Invariant NoAbsoluteReversion is violated"; then
  echo "  [OK] SageRollbackBroken: reversion counterexample found (gate has teeth)"
else
  echo "  [FAIL] SageRollbackBroken: expected a NoAbsoluteReversion violation but none was reported"
  fail=1
fi

echo "=== BROKEN CONTROL SageRollbackBadCtx (must FIND counterexample) ==="
out=$($TLC -config "SageRollbackBadCtx.cfg" SageRollback.tla 2>&1)
if echo "$out" | grep -q "Invariant ReplayContextFailClosed is violated"; then
  echo "  [OK] SageRollbackBadCtx: bad-replay-context counterexample found (fail-closed gate has teeth)"
else
  echo "  [FAIL] SageRollbackBadCtx: expected a ReplayContextFailClosed violation but none was reported"
  fail=1
fi

echo ""
if [ "$fail" -eq 0 ]; then
  echo "ALL TLC CHECKS PASSED: safety holds n in {4,7,10} + rollback holds; both broken controls falsified."
else
  echo "TLC CHECKS FAILED -- see above."
fi
exit $fail
