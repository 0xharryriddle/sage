#!/bin/bash
# Track G: coherent adversarial negative-controls campaign.
#
# Each fault is INJECTED on the real multi-process testbed and the observed SAGE
# response is recorded as the pass contract. Where a fault has a meaningful
# broken control, we run BOTH arms so the guard is proven to have teeth (the bad
# outcome IS observed when the guard is disabled), not vacuously passing.
#
# Faults the multi-process testbed can inject directly (run here):
#   1. partition_at_cutover  -- faithful SAGE must NOT fork; hardfork control MUST fork.
#   2. byzantine_equivocation -- equivocator within f: SAGE must NOT fork (quorum
#      intersection); detector liveness is proven by row 1's control firing.
#
# Faults covered at the model-check / unit-test tier (NOT re-run here; cited):
#   3. corrupt/missing replay context -> rollback REFUSED:
#        formal/SageRollback.tla ReplayContextFailClosed + SageRollbackBadCtx.cfg
#        (TLC catches the broken control) AND
#        sage-controller rollback.rs tests (missing / wrong-height / hash-mismatch).
#   4. quorum threshold short by one -> cutover does NOT fire:
#        sage-controller cutover_quorum_gate_enforced (unit + property tests);
#        sage-manifest certificate rejects_insufficient_quorum.
#   5. progress timeout / abort -> legacy stays live, bounded rollback:
#        sage-controller progress_timeout_triggers_rollback, abort_cert_triggers_rollback;
#        formal/SageRollback.tla NoAbsoluteReversion + SageRollbackBroken.cfg control.
#
# This split is deliberate and honest: the testbed exercises faults that need
# concurrent real processes (forks); the deterministic guards are exhaustively
# checked where a single-box test or TLC is the stronger evidence.
#
# Usage: bash scripts/negative_controls.sh [RUNS] [N] [H_C]
set -uo pipefail
cd "$(dirname "$0")/.."

RUNS="${1:-5}"
N="${2:-6}"
HC="${3:-4}"
BIN=./target/debug/spawn_testbed
OUT=results/raw/negative_controls.csv

if [ ! -x "$BIN" ]; then
  echo "[build] cargo build -p sage-node --bin spawn_testbed --bin validator_proc"
  cargo build -p sage-node --bin spawn_testbed --bin validator_proc || exit 1
fi

mkdir -p results/raw
echo "fault,arm,expected,n,planned_runs,completed_runs,error_runs,forked_runs,observed,pass" > "$OUT"
fail=0

# Run one arm: $1=fault label, $2=arm label, $3=expected(fork|nofork), rest=BIN args
run_arm() {
  local fault="$1" arm="$2" expected="$3"; shift 3
  local forks=0 completed=0 errors=0
  for i in $(seq 1 "$RUNS"); do
    if ! json=$("$BIN" "$@" --max-height 8 --max-secs 20 \
      --out "/tmp/ng_${fault}_${arm}_${i}.csv" 2>/dev/null); then
      errors=$((errors+1)); continue
    fi
    verdict=$(printf '%s\n' "$json" | python3 -c '
import json, sys
objects = []
for line in sys.stdin:
    try:
        value = json.loads(line)
    except json.JSONDecodeError:
        continue
    if isinstance(value, dict) and type(value.get("observed_fork")) is bool:
        objects.append(value["observed_fork"])
if len(objects) != 1:
    raise SystemExit(2)
print("true" if objects[0] else "false")
') || { errors=$((errors+1)); continue; }
    completed=$((completed+1))
    [ "$verdict" = true ] && forks=$((forks+1))
  done
  local observed pass
  if [ "$errors" -gt 0 ] || [ "$completed" -ne "$RUNS" ]; then
    observed=inconclusive; pass=FAIL; fail=1
  elif [ "$forks" -gt 0 ]; then observed=fork
  else observed=nofork
  fi
  if [ "$observed" != inconclusive ]; then
    if [ "$observed" = "$expected" ]; then pass=PASS; else pass=FAIL; fail=1; fi
  fi
  echo "  $fault/$arm: $forks/$completed completed forked, errors=$errors -> observed=$observed expected=$expected [$pass]"
  echo "$fault,$arm,$expected,$N,$RUNS,$completed,$errors,$forks,$observed,$pass" >> "$OUT"
}

echo "=== Fault 1: partition straddling cutover height ==="
run_arm partition_at_cutover faithful_sage nofork \
  --n "$N" --strategy sage --partition $((N/2)) --h-c "$HC"
run_arm partition_at_cutover broken_hardfork fork \
  --n "$N" --strategy hardfork --partition $((N/2)) --h-c "$HC"

echo "=== Fault 2: Byzantine equivocation at cutover (equivocator within f) ==="
run_arm byzantine_equivocation faithful_sage nofork \
  --n "$N" --strategy sage --byzantine 0 --equivocate-at "$HC" --h-c "$HC"

echo ""
echo "[done] wrote $OUT"
echo "Faults 3-5 (replay-context fail-closed, quorum gate, abort/rollback) are"
echo "covered at the TLC + Rust-test tier; see header for exact artifacts."
if [ "$fail" -eq 0 ]; then
  echo "ALL NEGATIVE-CONTROL ROWS PASSED (faithful no-fork; broken control forks)."
else
  echo "NEGATIVE-CONTROL CAMPAIGN FAILED -- see rows above."
fi
exit $fail
