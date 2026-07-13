#!/bin/bash
# M7 local-scale multi-process tier (system-upgrade P1.1).
#
# Runs the REAL multi-OS-process testbed (spawn_testbed) at larger validator
# counts than the 6-host cloud tier, under two software WAN-like impairment
# profiles, on a single machine. This stresses the pacemaker, view-change
# cascades, CutCert gather, and per-validator signature verification at scale
# that the n=6 real-host tier cannot reach.
#
# HONESTY: this is LOCAL real-process evidence (N separate OS processes, real
# loopback TCP, software netem-style per-send delay/jitter), NOT independent-host
# and NOT physical geo. The single-box liveness valve (--max-secs) can cut off
# the largest n before every validator finishes; any such run is recorded with
# its actual success count rather than silently dropped (see M4, which documented
# n=22 finishing ~15/22). A cutoff is a liveness-ceiling artifact of one box, NOT
# a safety failure: observed_fork must remain false in every recorded run.
#
# Output: results/raw/m7_scale_local.csv
#   profile,n,f,trial,migration_success,reached_height,target_height,observed_fork,total_messages
#
# Usage: bash scripts/m7_scale_local.sh [RUNS] "[N_LIST]" [MAX_HEIGHT] [MAX_SECS]
set -u
cd "$(dirname "$0")/.."

RUNS="${1:-3}"
N_LIST="${2:-10 16 22 31}"
MAX_HEIGHT="${3:-8}"
MAX_SECS="${4:-45}"
BIN=./target/debug/spawn_testbed
OUT=results/raw/m7_scale_local.csv

if [ ! -x "$BIN" ]; then
  echo "building debug binaries..."
  cargo build -p sage-node --bin spawn_testbed --bin validator_proc || exit 1
fi

mkdir -p results/raw
echo "profile,n,f,trial,migration_success,reached_height,target_height,observed_fork,total_messages" > "$OUT"

# Two impairment profiles: metro (10ms/2ms) and WAN-like (50ms/10ms). Both are
# software per-send models forwarded to every child; no sudo/netem required.
run_profile() {
  local profile="$1" delay="$2" jitter="$3"
  echo "########## profile=$profile (delay=${delay}ms jitter=${jitter}ms) ##########"
  for N in $N_LIST; do
    local F=$(( (N - 1) / 3 ))
    [ "$F" -lt 1 ] && F=1
    for i in $(seq 1 "$RUNS"); do
      local line tm ms of mh
      line=$("$BIN" --n "$N" --f "$F" --strategy sage \
        --h-d 2 --h-c 4 --h-r 8 --max-height "$MAX_HEIGHT" --max-secs "$MAX_SECS" \
        --delay-ms "$delay" --jitter-ms "$jitter" \
        --out "/tmp/m7_${profile}_n${N}_${i}.csv" 2>/dev/null \
        | grep -o '{.*}')
      tm=$(echo "$line" | grep -o '"total_messages":[0-9]*'         | grep -o '[0-9]*')
      ms=$(echo "$line" | grep -o '"migration_success":[0-9]*'      | grep -o '[0-9]*')
      of=$(echo "$line" | grep -o '"observed_fork":[a-z]*'          | grep -o '[a-z]*$')
      mh=$(echo "$line" | grep -o '"max_finalized_height":[0-9]*'   | grep -o '[0-9]*')
      [ -z "$tm" ] && tm=0
      [ -z "$mh" ] && mh=0
      # Safety guard: a fork at ANY scale is a real failure, not a liveness valve.
      if [ "${of:-unknown}" = "true" ]; then
        echo "SAFETY FAILURE: observed_fork=true at profile=$profile n=$N trial=$i"
        echo "$profile,$N,$F,$i,${ms:-0},$mh,$MAX_HEIGHT,true,$tm" >> "$OUT"
        exit 1
      fi
      echo "profile=$profile n=$N f=$F trial=$i  success=${ms:-0}/$N  reached_h=$mh/$MAX_HEIGHT  fork=${of:-unknown}  msgs=$tm"
      echo "$profile,$N,$F,$i,${ms:-0},$mh,$MAX_HEIGHT,${of:-unknown},$tm" >> "$OUT"
    done
  done
}

echo "=== M7 local-scale multi-process tier (SAGE, no partition) ==="
echo "    n in [$N_LIST], max_height=$MAX_HEIGHT, max_secs=$MAX_SECS, runs=$RUNS/profile/n"
echo ""
run_profile metro 10 2
run_profile wan   50 10

echo ""
echo "wrote $OUT"
echo "--- raw CSV ---"
cat "$OUT"
