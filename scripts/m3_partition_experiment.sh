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
  local forks=0 total=0
  for i in $(seq 1 "$RUNS"); do
    out=$("$BIN" --n "$N" --strategy "$strat" --partition $((N/2)) \
      --h-c "$HC" --max-height 8 --max-secs 20 \
      --out "/tmp/m3_${strat}_${i}.csv" 2>/dev/null | grep -o '"observed_fork":[a-z]*')
    total=$((total+1))
    case "$out" in
      *true*) forks=$((forks+1)); echo "  run $i: FORK observed" ;;
      *false*) echo "  run $i: no fork" ;;
      *) echo "  run $i: NO RESULT (child failure)" ;;
    esac
  done
  echo "  => $strat: $forks/$total runs forked"
}

echo "=== SAGE (quorum-gated, n-f=$((N-1)) cutover quorum) under $((N/2))/$((N/2)) partition ==="
run_arm sage
echo "=== HARDFORK (broken control, no gate) under $((N/2))/$((N/2)) partition ==="
run_arm hardfork
echo ""
echo "Expected: SAGE 0/$RUNS forks (minority side < n-f cannot switch);"
echo "          HARDFORK $RUNS/$RUNS forks (blind switch at h_c, both sides fork)."
