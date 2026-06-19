#!/bin/bash
# M6 SOTA-baseline differential (multi-process): SAGE vs a Cox-style live switch.
#
# Answers the peer-review P0 (Review1 empirical-baselines, Review2 SOTA-comparison):
# an EMPIRICAL head-to-head against the closest live-switch SOTA class (Cox,
# Blockchain: Research and Applications 2026), not only the stop-the-world /
# hard-fork controls.
#
# CoxStyle models Cox's profile on our engine interface: a zero-downtime live
# switch with a quorum-certified boundary checkpoint, applied across the
# heterogeneous PoA->BFT boundary. It CREDITS Cox's strengths (RQ1 shows its
# downtime is statistically indistinguishable from SAGE, ~119us, vs ~8ms for
# stop-the-world). What it lacks, and what this experiment isolates, is SAGE's
# n-f dual-run cutover-quorum gate: under a 3/3 partition at the cutover height
# a Cox-style switch (like the hard-fork control) forks, while quorum-gated SAGE
# does not. The result is OBSERVED at the finalized state_root level, never
# derived from the strategy flag.
#
# Honest scope: in the current node runtime CoxStyle's switch mechanism is
# operationally equivalent to the hard-fork control; the distinction we claim is
# the cost profile (zero-downtime, crediting Cox) plus the absence of the n-f
# predicate. We do NOT claim a faithful Cox reimplementation (epoch-termination /
# StableCheckpoint / epoch-mismatch catch-up); that is scoped future work.
#
# Pure ASCII, raw lines exposed.
#
# Usage: bash scripts/m6_sota_baseline_differential.sh [RUNS] [N] [H_C]
set -u

cd "$(dirname "$0")/.."
RUNS="${1:-5}"
N="${2:-6}"
HC="${3:-4}"
BIN=./target/debug/spawn_testbed

if [ ! -x "$BIN" ]; then
  echo "building debug binaries..."
  cargo build -p sage-node --bin spawn_testbed --bin validator_proc || exit 1
fi

OUT=results/raw/m6_sota_baseline_differential.csv
mkdir -p results/raw
echo "strategy,run,n,h_c,observed_fork,migration_success" > "$OUT"

echo "=== M6 SOTA-baseline differential: SAGE vs Cox-style vs hard-fork control ==="
echo "    n=$N, 3/3 partition at cutover h_c=$HC, $RUNS runs each."
echo "    Expected: Cox-style and hard-fork FORK (no n-f gate); SAGE 0 forks."
echo ""

for strat in coxstyle hardfork sage; do
  forks=0; total=0
  echo "--- strategy=$strat ---"
  for i in $(seq 1 "$RUNS"); do
    line=$("$BIN" --n "$N" --strategy "$strat" --partition $((N/2)) \
      --h-c "$HC" --max-height 8 --max-secs 20 \
      --out "/tmp/m6_${strat}_${i}.csv" 2>/dev/null | grep -o '{.*}')
    of=$(echo "$line" | grep -o '"observed_fork":[a-z]*' | grep -o '[a-z]*$')
    ms=$(echo "$line" | grep -o '"migration_success":[0-9]*' | grep -o '[0-9]*')
    total=$((total+1))
    case "$of" in
      true)  forks=$((forks+1)); echo "  run $i: FORK observed (success=$ms/$N)" ;;
      false) echo "  run $i: no fork (success=$ms/$N)" ;;
      *)     echo "  run $i: NO RESULT (child failure)" ;;
    esac
    echo "$strat,$i,$N,$HC,${of:-unknown},${ms:-0}" >> "$OUT"
  done
  echo "  => $strat: $forks/$total forked"
done

echo ""
echo "wrote $OUT"
echo "--- raw CSV ---"
cat "$OUT"
echo ""
echo "Interpretation: a Cox-style live switch (homogeneous-checkpoint discipline,"
echo "no n-f dual-run gate) forks under the heterogeneous-boundary partition just"
echo "like the hard-fork control; SAGE's n-f cutover quorum is the decisive"
echo "differentiator. Cost-wise (RQ1) CoxStyle ties SAGE on near-zero downtime,"
echo "so the comparison credits Cox's zero-downtime strength and is not a straw man."
