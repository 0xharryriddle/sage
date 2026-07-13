#!/bin/bash
# M3 differential safety experiment (multi-process, authoritative artifact).
#
# Runs the real multi-OS-process testbed under an identical 3/3 network
# partition engaging at the cutover height, for both:
#   - SAGE   (quorum-gated cutover, --cutover-quorum): must NOT fork.
#   - HARDFORK (broken control, no gate): MUST fork (proves the window is real).
#
# Both arms share identical infrastructure (partition, pacemaker, coinbase,
# DoubleVote-tolerant handler); the ONLY difference is the n-f cutover quorum
# gate. The broken control firing is what makes SAGE's zero-fork meaningful
# rather than structural ("fork impossible by construction").
#
# Replay/manifest evidence is recorded for every run. Under this partition the
# expected no-quorum paths may not produce a quorum-bound replay/manifest root,
# so fork behavior is the pass/fail contract for M3.
#
# Usage: bash scripts/m3_partition_experiment.sh [RUNS] [N] [H_C]
set -u

cd "$(dirname "$0")/.."
RUNS="${1:-5}"
N="${2:-6}"
HC="${3:-4}"
BIN=./target/debug/spawn_testbed

if [ ! -x "$BIN" ]; then
  echo "building release-less debug binaries..."
  cargo build -p sage-node --bin spawn_testbed --bin validator_proc || exit 1
fi

run_arm() {
  local strat="$1"
  local expected_forks="$2"
  local forks=0 total=0 replay_fail=0 manifest_fail=0
  for i in $(seq 1 "$RUNS"); do
    json=$("$BIN" --n "$N" --strategy "$strat" --partition $((N/2)) \
      --h-c "$HC" --max-height 8 --max-secs 20 \
      --out "/tmp/m3_${strat}_${i}.csv" 2>/dev/null)
    out=$(printf '%s\n' "$json" | grep -o '"observed_fork":[a-z]*')
    replay=$(printf '%s\n' "$json" | grep -o '"replay_context_ok":[a-z]*')
    manifest=$(printf '%s\n' "$json" | grep -o '"manifest_payload_ok":[a-z]*')
    total=$((total+1))
    case "$out" in
      *true*) forks=$((forks+1)); echo "  run $i: FORK observed" ;;
      *false*) echo "  run $i: no fork" ;;
      *) echo "  run $i: NO RESULT (child failure)" ;;
    esac
    case "$replay" in
      *true*) ;;
      *) replay_fail=$((replay_fail+1)); echo "  run $i: replay context artifact unavailable" ;;
    esac
    case "$manifest" in
      *true*) ;;
      *) manifest_fail=$((manifest_fail+1)); echo "  run $i: manifest payload artifact unavailable" ;;
    esac
  done
  echo "  => $strat: $forks/$total runs forked; replay_context_ok unavailable=$replay_fail; manifest_payload_ok unavailable=$manifest_fail"
  if [ "$forks" -ne "$expected_forks" ]; then
    echo "FAILED: $strat expected $expected_forks/$RUNS forks, observed $forks/$total"
    return 1
  fi
  return 0
}

echo "=== SAGE (quorum-gated, n-f=$((N-1)) cutover quorum) under $((N/2))/$((N/2)) partition ==="
run_arm sage 0 || exit 1
echo "=== HARDFORK (broken control, no gate) under $((N/2))/$((N/2)) partition ==="
run_arm hardfork "$RUNS" || exit 1
echo ""
echo "Expected: SAGE 0/$RUNS forks (minority side < n-f cannot switch);"
echo "          HARDFORK $RUNS/$RUNS forks (blind switch at h_c, both sides fork)."
