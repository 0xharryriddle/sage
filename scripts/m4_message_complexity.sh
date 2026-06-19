#!/bin/bash
# M4 message-complexity sweep (multi-process, empirical O(n^2) evidence).
#
# Runs the real multi-OS-process testbed (spawn_testbed) across a grid of
# validator counts under the honest no-partition SAGE path, and records the
# total real wire messages each run emitted (summed across all validators,
# excluding partition-blocked and impairment-dropped sends). For an all-to-all
# BFT broadcast pattern the per-round message count scales as O(n^2), so the
# total over a fixed number of heights should fit a quadratic in n.
#
# Output: results/raw/m4_message_complexity.csv with columns
#   n,f,max_height,migration_success,observed_fork,total_messages,per_round
# where per_round = total_messages / max_height (rounds ~ heights finalized).
#
# This is descriptive evidence (one trial per n by default); pass RUNS>1 to
# average across trials for a smoother curve. Pure ASCII output, raw lines
# exposed, no privileged netem required.
#
# Usage: bash scripts/m4_message_complexity.sh [RUNS] "[N_LIST]" [MAX_HEIGHT]
set -u

cd "$(dirname "$0")/.."

RUNS="${1:-1}"
N_LIST="${2:-4 7 10 13 16 19 22}"
MAX_HEIGHT="${3:-8}"
BIN=./target/debug/spawn_testbed
OUT=results/raw/m4_message_complexity.csv

if [ ! -x "$BIN" ]; then
  echo "building debug binaries..."
  cargo build -p sage-node --bin spawn_testbed --bin validator_proc || exit 1
fi

mkdir -p results/raw
echo "n,f,max_height,trial,migration_success,observed_fork,total_messages,per_round" > "$OUT"

echo "=== M4 message-complexity sweep (SAGE, no partition) ==="
echo "    n in [$N_LIST], max_height=$MAX_HEIGHT, runs=$RUNS per n"
echo ""

for N in $N_LIST; do
  # f scales so the BFT quorum 2f+1 stays a strict majority; keep f=1 for the
  # smallest n and grow it ~ (n-1)/3 to keep the fault model meaningful.
  F=$(( (N - 1) / 3 ))
  [ "$F" -lt 1 ] && F=1
  for i in $(seq 1 "$RUNS"); do
    line=$("$BIN" --n "$N" --f "$F" --strategy sage \
      --h-d 2 --h-c 4 --h-r 8 --max-height "$MAX_HEIGHT" --max-secs 30 \
      --out "/tmp/m4_n${N}_${i}.csv" 2>/dev/null \
      | grep -o '{.*}')
    # Parse the JSON summary line with a tiny sed/grep extraction (no jq dep).
    tm=$(echo "$line"   | grep -o '"total_messages":[0-9]*'      | grep -o '[0-9]*')
    ms=$(echo "$line"   | grep -o '"migration_success":[0-9]*'   | grep -o '[0-9]*')
    of=$(echo "$line"   | grep -o '"observed_fork":[a-z]*'       | grep -o '[a-z]*$')
    mh=$(echo "$line"   | grep -o '"max_finalized_height":[0-9]*'| grep -o '[0-9]*')
    [ -z "$tm" ] && tm=0
    if [ -z "$mh" ] || [ "$mh" -eq 0 ]; then
      per=0
    else
      per=$(( tm / mh ))
    fi
    echo "n=$N f=$F trial=$i  total_messages=$tm  per_round=$per  success=${ms:-?}/$N  fork=${of:-?}"
    echo "$N,$F,$MAX_HEIGHT,$i,${ms:-0},${of:-unknown},$tm,$per" >> "$OUT"
  done
done

echo ""
echo "wrote $OUT"
echo "--- raw CSV ---"
cat "$OUT"
