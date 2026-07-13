#!/bin/bash
# M8 faithful-Cox differential (loopback, real Ed25519, 20 trials/arm).
#
# Upgrades the earlier 5-trial faithful-Cox result to 20 trials/arm so the
# claim-scope "faithful-Cox head-to-head" row moves from FUTURE to a cited,
# Wilson-bounded result. Both arms run the IDENTICAL n=6, f=1, 3/3-partition-at-h_c
# testbed over real loopback TCP with real Ed25519 signatures; the ONLY difference
# is the cutover gate:
#   - coxfaithful: Cox's own 2f+1 (=3) StableCheckpoint quorum  -> MUST fork
#     (each side of 3 reaches 2f+1 and switches; 2(2f+1) <= n at n=6,f=1).
#   - sage:        SAGE's n-f (=5) cutover quorum               -> MUST NOT fork
#     (a side of 3 < 5 cannot form the quorum; no side switches).
#
# HONESTY: loopback is the correct substrate for a SAFETY differential — the fork
# is produced by concurrent two-leader execution across a partition, not by
# geography, so co-located processes over real TCP reproduce it faithfully. The
# faithful-Cox arm omits Cox's epoch-mismatch catch-up (a post-switch LIVENESS
# feature, orthogonal to boundary safety), which is why its minority side cannot
# make progress on independent hosts; loopback avoids that confound.
#
# Usage: bash scripts/m8_coxfaithful_differential.sh [RUNS]
set -u
cd "$(dirname "$0")/.."
RUNS="${1:-20}"
BIN=./target/release/spawn_testbed
OUT=results/raw/coxfaithful_differential_20.csv

if [ ! -x "$BIN" ]; then
  echo "release spawn_testbed missing; build it first" >&2
  exit 1
fi

mkdir -p results/raw
echo "experiment,strategy,gate_threshold,trials,forked,fork_rate,wilson95_lo,wilson95_hi" > "$OUT"

run_arm() {
  local strat="$1" gate="$2" expect="$3"
  local forks=0 total=0
  for i in $(seq 1 "$RUNS"); do
    local json forked
    json=$("$BIN" --n 6 --f 1 --strategy "$strat" --partition 3 \
      --h-c 4 --max-height 8 --max-secs 20 \
      --out "/tmp/m8_${strat}_${i}.csv" 2>/dev/null)
    forked=$(printf '%s\n' "$json" | grep -o '"observed_fork":[a-z]*' | grep -o '[a-z]*$')
    total=$((total+1))
    if [ "$forked" = "true" ]; then
      forks=$((forks+1)); echo "  [$strat $i] FORK"
    else
      echo "  [$strat $i] no fork"
    fi
    rm -f "/tmp/m8_${strat}_${i}.csv"
  done
  # Wilson 95% score interval computed in python (stdlib only).
  python3 - "$forks" "$total" "$strat" "$gate" <<'PY' >> "$OUT"
import sys, math
k=int(sys.argv[1]); n=int(sys.argv[2]); strat=sys.argv[3]; gate=sys.argv[4]
z=1.959963984540054
p=k/n if n else 0.0
den=1+z*z/n
center=(p+z*z/(2*n))/den
half=(z*math.sqrt(p*(1-p)/n+z*z/(4*n*n)))/den
lo=max(0.0,center-half); hi=min(1.0,center+half)
label={"coxfaithful":"CoxFaithful","sage":"SAGE"}.get(strat,strat)
print(f"coxfaithful_partition,{label},{gate},{n},{k},{p:.2f},{lo:.3f},{hi:.3f}")
PY
  echo "=> $strat: $forks/$total forked (expected $expect)"
}

echo "=== M8 faithful-Cox differential (n=6, f=1, 3/3 partition at h_c, $RUNS trials/arm) ==="
run_arm coxfaithful "2f+1 (=3)" "fork"
run_arm sage        "n-f (=5)"  "nofork"
echo
echo "wrote $OUT"
cat "$OUT"
