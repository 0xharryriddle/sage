#!/bin/bash
# Kernel network-namespace testbed (on-box "bridge tier" above loopback).
#
# HONESTY CONTRACT:
#   Each validator runs in its OWN Linux network namespace with its OWN network
#   stack, connected via veth pairs to a bridge, with REAL kernel `tc netem`
#   impairment per link (not the in-process software model used elsewhere). This
#   is a genuine fidelity upgrade over loopback: real separate network stacks and
#   real kernel-level loss/delay/jitter. It still runs on ONE kernel and ONE
#   physical clock, so it does NOT provide independent clock drift, independent
#   scheduler/disk failure domains, or real WAN latency. For those, use
#   scripts/provision_local_vms.sh (multi-VM) or a real CloudLab/Fly.io/Hetzner
#   deployment via scripts/deploy_multihost.sh. This tier sits ABOVE loopback and
#   BELOW multi-VM / LAN / WAN in the evidence-strength table.
#
# Requires: root (ip netns / tc), the validator_proc + spawn-style fork check.
# Usage: sudo bash scripts/netns_testbed.sh [N] [DELAY_MS] [JITTER_MS] [LOSS_PCT]
set -u
cd "$(dirname "$0")/.."

N="${1:-4}"
DELAY_MS="${2:-50}"
JITTER_MS="${3:-10}"
LOSS_PCT="${4:-0.1}"
BASE_PORT="${BASE_PORT:-7100}"
SUBNET="10.99.0"
BR="sagebr0"
BIN="$(pwd)/target/release/validator_proc"
OUTDIR="$(mktemp -d)"

if [ "$(id -u)" -ne 0 ]; then
  echo "[refuse] needs root for ip netns / tc netem. Re-run with sudo." >&2
  exit 3
fi
if [ ! -x "$BIN" ]; then
  echo "[build] cargo build --release -p sage-node --bin validator_proc"
  cargo build --release -p sage-node --bin validator_proc --features real-crypto || exit 1
fi

cleanup() {
  echo "[cleanup] tearing down namespaces + bridge"
  for i in $(seq 0 $((N-1))); do
    ip netns del "sage$i" 2>/dev/null || true
    ip link del "veth$i" 2>/dev/null || true
  done
  ip link del "$BR" 2>/dev/null || true
}
trap cleanup EXIT

echo "[setup] bridge $BR + $N namespaces on $SUBNET.0/24"
ip link add "$BR" type bridge
ip addr add "$SUBNET.1/24" dev "$BR"
ip link set "$BR" up

# Build the peer list (index = validator id) up front.
peers=""
for i in $(seq 0 $((N-1))); do
  peers="${peers}${peers:+,}$SUBNET.$((10+i)):$BASE_PORT"
done
echo "[setup] peers: $peers"

for i in $(seq 0 $((N-1))); do
  ns="sage$i"; veth="veth$i"; vpeer="vpeer$i"; ip="$SUBNET.$((10+i))"
  ip netns add "$ns"
  ip link add "$veth" type veth peer name "$vpeer"
  ip link set "$veth" master "$BR"; ip link set "$veth" up
  ip link set "$vpeer" netns "$ns"
  ip netns exec "$ns" ip addr add "$ip/24" dev "$vpeer"
  ip netns exec "$ns" ip link set "$vpeer" up
  ip netns exec "$ns" ip link set lo up
  # REAL kernel netem on the namespace side of the link.
  ip netns exec "$ns" tc qdisc add dev "$vpeer" root netem \
    delay "${DELAY_MS}ms" "${JITTER_MS}ms" loss "${LOSS_PCT}%" 2>/dev/null || \
    echo "  [warn] netem add failed on $ns (sch_netem module?)"
  echo "[launch] $ns @ $ip:$BASE_PORT"
  ip netns exec "$ns" "$BIN" --id "$i" --n "$N" --f 1 \
    --peers "$peers" --bind "0.0.0.0:$BASE_PORT" \
    --strategy sage --max-height 8 --h-d 2 --h-c 4 --h-r 8 --max-secs 40 \
    > "$OUTDIR/result_$i.json" 2>"$OUTDIR/err_$i.log" &
done

echo "[run] waiting for $N namespaced validators..."
wait

echo "experiment,n,validator_id,delay_ms,jitter_ms,loss_pct,committed_height" \
  > results/raw/netns_testbed.csv
forkset=""
for i in $(seq 0 $((N-1))); do
  h=$(grep -o '"committed_height":[0-9]*' "$OUTDIR/result_$i.json" 2>/dev/null | grep -o '[0-9]*' | head -1)
  echo "netns,$N,$i,$DELAY_MS,$JITTER_MS,$LOSS_PCT,${h:-0}" >> results/raw/netns_testbed.csv
  hashes=$(grep -o '"committed_hashes":{[^}]*}' "$OUTDIR/result_$i.json" 2>/dev/null)
  forkset="$forkset$hashes"
done
echo "[done] wrote results/raw/netns_testbed.csv ; raw JSON in $OUTDIR"
echo "NOTE: real kernel netem + separate net stacks; ONE kernel/clock (not WAN/VM)."
