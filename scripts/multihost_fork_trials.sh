#!/bin/bash
# Multi-host fork-differential trial loop (real hardware, n=6, 3/3 partition at h_c).
# Runs RUNS trials of each arm (sage vs hardfork), records observed_fork per trial,
# and writes a summary CSV. Mirrors the single-box m3 methodology on real VMs.
#
# Usage: bash scripts/multihost_fork_trials.sh [RUNS]
set -u
cd "$(dirname "$0")/.."
RUNS="${1:-5}"
SUMMARY="results/raw/multihost_fork_differential.csv"
echo "arm,trial,n,forked,side_a_h4_roots,side_b_h4_roots,verdict" > "$SUMMARY"

# Kill any leftover validator_proc on every host and wait until port 7000 is
# released, so a back-to-back trial cannot fail with "Address already in use".
# Without this, the previous trial's sockets linger and the next trial's bind
# fails, producing empty JSON that the aggregator mis-scores as forked=false.
HOSTS_SSH=$(awk '!/^#/ && NF>=4 {print $2}' config/hosts.txt)
cleanup_ports() {
  for h in $HOSTS_SSH; do
    ssh -o ConnectTimeout=5 "$h" 'pkill -x validator_proc 2>/dev/null; exit 0' >/dev/null 2>&1 || true
  done
  # Wait (bounded) for port 7000 to be free on every host.
  local tries=0
  while [ "$tries" -lt 15 ]; do
    local busy=0
    for h in $HOSTS_SSH; do
      if ssh -o ConnectTimeout=5 "$h" 'ss -ltn 2>/dev/null | grep -q ":7000 "' 2>/dev/null; then
        busy=$((busy+1))
      fi
    done
    [ "$busy" -eq 0 ] && break
    sleep 1
    tries=$((tries+1))
  done
}

run_arm() {
  local strat="$1" expect="$2"
  local forks=0
  for i in $(seq 1 "$RUNS"); do
    cleanup_ports
    local tmpout="results/raw/_trial_${strat}_${i}.csv"
    OUT="$tmpout" PARTITION=3 H_C=4 MAX_SECS=90 \
      bash scripts/deploy_multihost.sh run "$strat" 8 >/dev/null 2>&1
    # observed_fork column is identical across rows; take the first data row.
    local forked launched
    forked=$(awk -F, 'NR==2{print $NF}' "$tmpout" 2>/dev/null)
    # Distinguish a genuine no-fork from a failed launch: a trial where no
    # validator finalized ANY height (all committed_height=0) did not actually
    # run and must not be scored as forked=false.
    launched=$(awk -F, 'NR>1 && $6+0>0' "$tmpout" 2>/dev/null | head -1)
    if [ -z "$launched" ]; then
      forked="nolaunch"
    fi
    [ "$forked" = "true" ] && forks=$((forks+1))
    echo "$strat,$i,6,$forked,-,-,expect_${expect}" >> "$SUMMARY"
    echo "[$strat trial $i] forked=$forked (expect $expect)"
    rm -f "$tmpout"
  done
  echo "=> $strat: $forks/$RUNS trials forked (expected: $expect)"
}

echo "########## SAGE arm (n-f gate; expect NO fork) ##########"
run_arm sage nofork
echo "########## HARDFORK arm (no gate; expect fork) ##########"
run_arm hardfork fork
echo
echo "=== SUMMARY ($SUMMARY) ==="
column -t -s, "$SUMMARY"