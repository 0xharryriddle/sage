# SAGE Multi-Host Benchmark — VPS / Virtual-Hardware Setup Guide

End-to-end, copy-pasteable guide to provision real machines, run the SAGE
multi-host benchmark across them over a real network, and pull the resulting
figures/CSVs back to this local machine.

This removes the single hard constraint of the dev box: it tops out at **n≈22**
real validator processes on one kernel. Independent machines give independent
clocks, schedulers, disks, and a real network — the evidence a Q1 reviewer
expects.

> Honesty rule carried from the project: numbers produced here are REAL only
> because they come from real hosts. Never edit a CSV by hand. The harness
> (`scripts/deploy_multihost.sh`) refuses to run without a real `config/hosts.txt`.

---

## 0. The one networking fact that makes or breaks this

`validator_proc` binds its listener to **its own entry in the `--peers` list**
(`TcpTransport::bind(id, peers)` binds `peers[id]`). The `--bind` argument the
deploy script passes is **ignored** by the binary. Consequences:

- Every host must **own** the IP that sits at its index in the peer list, and
  be able to `bind()` it directly.
- Public/elastic IPs behind NAT will **not** bind (the OS never sees the public
  IP on an interface). A naive "use public IPs" setup fails at startup.

**Robust fix used throughout this guide: a flat WireGuard/Tailscale overlay.**
Every VM gets a stable overlay IP (e.g. `100.x.y.z`) that it genuinely owns and
can bind, and that every other node can dial — across regions, across clouds,
through NAT. This sidesteps the bind problem entirely and gives you real
cross-region latency on top.

(If all your machines are on the same LAN/VPC subnet with private IPs they each
own, you can skip the overlay and use those private IPs directly.)

---

## 1. Choose your hardware tier

| Tier | What | Cost | Use when |
|---|---|---|---|
| 1 | CloudLab / Chameleon (NSF), Grid'5000 (EU) | FREE (academic) | You have/can get an academic account. Best for the paper. |
| 2 | Fly.io / Oracle Always-Free / Hetzner / GCP credit | $0–200 | No academic account; want it now under your control. |
| 3 | 4–6 mini-PCs on a switch | ~$600–1200 | You want a permanent private lab (not required). |

Minimum for an honest "full benchmark": **≥5 independent machines, ≥2 vCPU /
4 GB each.** LAN/same-region is enough for the safety claim; **3 regions** make
the cost/latency claim a real WAN result.

Recommended concrete starting point (cheapest real WAN): **5× Oracle
Always-Free Arm VMs** or **5× Hetzner CPX11** spread over 3 regions.

---

## 1b. Oracle Cloud Always-Free quickstart ($0, ARM)

If you registered the Oracle free tier, this is the zero-cost path. Read the
three gotchas first — each one silently breaks a first run.

**Gotcha 1 — ARM architecture.** Ampere A1 VMs are `aarch64`. An x86_64 binary
built on your dev box gives `exec format error`. You MUST build on an Oracle VM
(Section 6, Variant B) or cross-compile for `aarch64-unknown-linux-gnu`.

**Gotcha 2 — post-June-2026 free limits.** Free tier is now **2 OCPU / 12 GB ARM
total** (was 4/24) + 2 AMD micro VMs (1/8 OCPU, 1 GB each). Layout for a real
cross-machine n=4 BFT test:
- **2× ARM A1 VMs in ONE VCN, 2 validators per VM** → n=4, f=1.
- Partition 2/2 = one VM per side → a genuine cross-kernel fork differential.
- (The 2 AMD micros are too small to co-host a validator comfortably; use them
  only as extra single-validator nodes if you want n up to ~4 one-per-host.)

**Gotcha 3 — bind to the PRIVATE IP.** Same-VCN VMs each own their private VCN
IP and can bind it directly, so **no Tailscale/overlay is needed** (skip
Section 3). Column 4 of `hosts.txt` MUST be the private IP (e.g. `10.0.0.x`),
never the public/NAT IP — the OS never sees the public IP on an interface.

**Capacity note.** ARM A1 provisioning often returns "Out of capacity". Pick a
less-crowded home region, or retry provisioning on a loop; it usually clears.

**Oracle-specific hosts.txt (2 VMs × 2 validators, same VCN, no overlay):**

```
# config/hosts.txt — Oracle free tier, n=4, f=1, 2 validators per VM
# <id> <ssh_user@public_ip_for_ssh>  <region_label>  <private_vcn_ip:port>
0 ubuntu@<vm0_public_ip>   oci-ad1   10.0.0.10:7000
1 ubuntu@<vm0_public_ip>   oci-ad1   10.0.0.10:7001
2 ubuntu@<vm1_public_ip>   oci-ad1   10.0.0.11:7000
3 ubuntu@<vm1_public_ip>   oci-ad1   10.0.0.11:7001
```

Note ids 0,1 share vm0 (different ports) and 2,3 share vm1 — the partition
`--partition 2` then splits {0,1} on vm0 vs {2,3} on vm1, one kernel per side.

**Open the ports inside the VCN:** add an ingress rule to the VCN security list
allowing TCP 7000–7001 from the VCN CIDR (e.g. `10.0.0.0/16`). Oracle's default
security list blocks inter-VM ports until you do this — a common stall cause.

**Build on the ARM VM (Variant B), then run from your local box:**
```bash
# on one Oracle VM (the "build VM"):
ssh ubuntu@<vm0_public_ip> 'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y'
# clone the repo on the VM, then:
ssh ubuntu@<vm0_public_ip> 'cd sage && cargo build --release -p sage-node --bin validator_proc --features real-crypto'
# copy that aarch64 binary to the other VM and set REMOTE_BIN before distribute:
scp ubuntu@<vm0_public_ip>:sage/target/release/validator_proc /tmp/validator_proc_arm
scp /tmp/validator_proc_arm ubuntu@<vm1_public_ip>:/tmp/sage_validator_proc
ssh ubuntu@<vm0_public_ip> 'cp sage/target/release/validator_proc /tmp/sage_validator_proc'
# then from local, skip `build`/`distribute` (binary already staged) and just:
bash scripts/deploy_multihost.sh run sage 8
bash scripts/deploy_multihost.sh run hardfork 8
```

Because Oracle free VMs cost nothing, you can leave them up — no destroy-timer
discipline needed here (that's for the paid Tier-2 providers).

---

## 2. Provision the VMs (Tier 2 example — adapt for Tier 1)

Provision **n** Ubuntu 22.04/24.04 VMs (start with n=5). For each VM, in the
provider console:

1. Image: Ubuntu 22.04 LTS, ≥2 vCPU / 4 GB / 20 GB disk.
2. Open the SSH port (22) to your IP.
3. Add your SSH public key.
4. Note each VM's public IP for the SSH/overlay bootstrap.

Spread them across regions if you want WAN latency (e.g. us-east, eu-central,
ap-southeast).

Put the public IPs in a scratch file locally:

```bash
# ~/sage_vms.txt  (local scratch, public IPs for ssh bootstrap)
ubuntu@<vm0_public_ip>
ubuntu@<vm1_public_ip>
ubuntu@<vm2_public_ip>
ubuntu@<vm3_public_ip>
ubuntu@<vm4_public_ip>
```

Verify SSH to each:

```bash
while read -r h; do echo "== $h =="; ssh -o StrictHostKeyChecking=accept-new "$h" 'hostname; nproc'; done < ~/sage_vms.txt
```

---

## 3. Build the overlay network (Tailscale — easiest)

Tailscale gives each VM a stable `100.x.y.z` IP it owns. On **every** VM:

```bash
# run on each VM (via ssh)
curl -fsSL https://tailscale.com/install.sh | sh
sudo tailscale up --authkey <YOUR_TAILSCALE_AUTHKEY>   # one reusable key for all
tailscale ip -4                                         # prints this VM's 100.x.y.z
```

Scripted for all VMs from local:

```bash
AUTHKEY=tskey-auth-xxxxx
while read -r h; do
  ssh "$h" "curl -fsSL https://tailscale.com/install.sh | sh && sudo tailscale up --authkey $AUTHKEY --ssh"
  echo "$h overlay ip: $(ssh "$h" 'tailscale ip -4')"
done < ~/sage_vms.txt
```

Record each VM's overlay IP, in validator-id order (0..n-1). These overlay IPs
are what go in `config/hosts.txt`.

WireGuard is a fine alternative if you prefer no third party; the only
requirement is "every node owns a stable IP every other node can reach".

---

## 4. Install the runtime dependency on each VM

`validator_proc` is a static-ish Rust binary, but ship it the matching glibc by
building on the same Ubuntu release, or install the toolchain remotely. Simplest
is to **build locally and scp** (Section 6). The VMs need only:

```bash
# on each VM — nothing but a recent glibc; Ubuntu 22.04+ already has it.
# If you build WITH --features real-crypto (default here), no extra runtime deps.
ssh "$h" 'ldd --version | head -1'   # sanity check glibc >= 2.35
```

If your local glibc is newer than the VM's, build on a VM instead (Section 6,
variant B) or use a 22.04 build container.

---

## 5. Write `config/hosts.txt` (the harness contract)

On the **local machine**, one line per validator, **sorted by id**:

```
# config/hosts.txt
# <validator_id> <ssh_user@host>          <region_label>  <bind_ip:port>
0 ubuntu@100.64.0.10   us-east       100.64.0.10:7000
1 ubuntu@100.64.0.11   eu-central    100.64.0.11:7000
2 ubuntu@100.64.0.12   ap-southeast  100.64.0.12:7000
3 ubuntu@100.64.0.13   us-east       100.64.0.13:7000
4 ubuntu@100.64.0.14   eu-central    100.64.0.14:7000
```

Rules that matter:
- Column 2 (`ssh_user@host`) may be the public IP or overlay IP — it is only
  used for ssh.
- Column 4 (`bind_ip:port`) **must be the overlay IP the VM owns** — this is
  what ends up in the peer list and what the node binds. Use the same port on
  every host (e.g. 7000).
- Open the chosen port between nodes. On the overlay this is automatic; on raw
  VPC, allow TCP 7000 in the security group.

A template is provided at `config/hosts.txt.example`.

---

## 6. Build and distribute the binary

**Variant A — build locally, scp to VMs (default):**

```bash
# local
bash scripts/deploy_multihost.sh build        # cargo build --release ... --features real-crypto
bash scripts/deploy_multihost.sh distribute    # scp target/release/validator_proc -> each host:/tmp/sage_validator_proc
```

**Variant B — build on a VM (if glibc mismatch):**

```bash
ssh "$BUILDVM" 'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y'
# scp the repo or git clone it on the VM, then:
ssh "$BUILDVM" 'cd sage && cargo build --release -p sage-node --bin validator_proc --features real-crypto'
# then scp that binary to the other VMs and set REMOTE_BIN accordingly.
```

---

## 7. Run the benchmark sweep

The harness launches one `validator_proc` per host, waits, aggregates, and
detects cross-host forks.

```bash
# local — single run (n = number of lines in hosts.txt)
bash scripts/deploy_multihost.sh run sage 8        # strategy=sage, max_height=8

# control arm (must fork under partition): re-run with hardfork
bash scripts/deploy_multihost.sh run hardfork 8
```

Sweep validator counts and schedules by editing `config/hosts.txt` (n = line
count) and the schedule env vars:

```bash
# longer run, later cutover, bigger safety valve for WAN latency
H_D=4 H_C=10 H_R=20 MAX_SECS=120 \
  bash scripts/deploy_multihost.sh run sage 24
```

Recommended sweep for the paper (re-point hosts.txt at 4, 7, 10, then 16 VMs):
- n ∈ {4, 7, 10, 16} × strategy ∈ {sage, hardfork} × ≥5 seeds.
- For WAN: keep the 3-region layout; for LAN baseline: all VMs one region.

Each run writes `results/raw/multihost_<strategy>.csv` and keeps per-host JSON
in a tmpdir (path printed at the end).

---

## 8. Pull results back to THIS local machine

The CSVs are written **locally** already (the harness runs from your machine and
aggregates over ssh), so they land in `results/raw/` on this box directly. To
also archive the raw per-host JSON the run printed:

```bash
# the run prints: "[run] raw per-host JSON kept in /tmp/tmp.XXXX"
RUN_TMP=/tmp/tmp.XXXX
mkdir -p results/raw/multihost_json/$(date +%Y%m%d)
cp "$RUN_TMP"/result_*.json "$RUN_TMP"/err_*.log results/raw/multihost_json/$(date +%Y%m%d)/
```

If you instead ran the harness ON a VM, pull everything back:

```bash
scp -r "$BUILDVM:sage/results/raw/multihost_*.csv" results/raw/
```

Then regenerate figures locally from the real CSVs (no fabrication — these are
your measured numbers):

```bash
cargo run -p sage-experiments --bin generate_figures   # if wired for multihost
# or hand the CSV to the plotting path used for the other figures
```

---

## 9. Verify the result is real and consistent

```bash
# fork differential sanity: sage must be all-false, hardfork must show a fork
column -t -s, results/raw/multihost_sage.csv      | head
column -t -s, results/raw/multihost_hardfork.csv  | head
# the observed_fork column: sage -> false everywhere; hardfork -> true at the split
```

Acceptance (mirrors the single-box result, now on real hosts):
- `sage`: `observed_fork=false` on every row, all validators reach `max_height`.
- `hardfork` under a partition: `observed_fork=true`.
- Wall-clock columns are non-zero and reflect real inter-region RTT.

---

## 10. Cost control / teardown

```bash
# stop tailscale + power off (provider-specific); destroy VMs when done
while read -r h; do ssh "$h" 'sudo tailscale down' || true; done < ~/sage_vms.txt
# then delete the instances in the provider console to stop billing
```

Oracle Always-Free and CloudLab/Chameleon cost nothing to leave up; commercial
VMs should be destroyed after the sweep.

---

## 11. Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `failed to bind transport` on a node | `bind_ip:port` in hosts.txt is not an IP that VM owns | Use that VM's own overlay/private IP in column 4 |
| Nodes never finalize, time out at `max_secs` | port blocked between hosts | open TCP port (overlay handles this; raw VPC needs a security-group rule) |
| `scp ... failed` in distribute | ssh target wrong / key missing | fix column 2 ssh target; `ssh-copy-id` your key |
| All nodes stall before cutover | partition armed from start, or quorum unreachable | only partition AT `h_c`; ensure n ≥ 3f+1 |
| glibc / `version GLIBC_x` error | local build newer than VM | build on a VM (Section 6B) or a 22.04 container |
| sage shows a fork | misconfig (gate off) or real bug | confirm strategy=sage; check `--cutover-quorum` path; capture JSON and investigate |

---

## 12. What this unlocks in the paper

Once Section 7 runs on ≥5 real hosts:
- Simulator-time cost numbers become **real wall-clock TPS + p50/p95/p99
  finality latency**.
- CutCert gather latency becomes a **measured WAN round**, not a model.
- The fork differential moves from single-box loopback to **real independent
  machines** — the headline safety evidence a reviewer can't wave away.
- The evidence-strength table's LAN/WAN rows flip from "not yet run" to measured.

This is the one step that converts SAGE from a strong simulator+formal artifact
into a defensible real-distributed-systems evaluation.
