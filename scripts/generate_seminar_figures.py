from pathlib import Path
import csv
import math

import matplotlib.pyplot as plt
import numpy as np
from matplotlib.patches import Rectangle

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "docs" / "seminar_assets"
OUT.mkdir(parents=True, exist_ok=True)

SAGE = "#087E8B"
SAGE_DARK = "#07545C"
UNSAFE = "#E4572E"
NEUTRAL = "#D9A441"
BLUE = "#3569A8"
INK = "#17252A"
MUTED = "#5D6B70"
GRID = "#DCE5E7"
BG = "#F7FAF9"

plt.rcParams.update({
    "font.family": "DejaVu Sans",
    "font.size": 15,
    "axes.titlesize": 24,
    "axes.labelsize": 16,
    "xtick.labelsize": 13,
    "ytick.labelsize": 13,
    "axes.edgecolor": "#9AA8AC",
    "axes.labelcolor": INK,
    "text.color": INK,
    "xtick.color": INK,
    "ytick.color": INK,
    "figure.facecolor": BG,
    "axes.facecolor": BG,
})


def save(fig, name):
    # Preserve the 16:9 canvas; tight cropping destroys projector-safe margins.
    fig.savefig(OUT / f"{name}.svg", facecolor=BG)
    fig.savefig(OUT / f"{name}.png", dpi=220, facecolor=BG)
    plt.close(fig)


def title(ax, main, sub):
    fig = ax.figure
    fig.suptitle(main, x=.07, y=.95, ha="left", fontweight="bold", fontsize=20)
    fig.text(.07, .895, sub, color=MUTED, fontsize=12)
    fig.subplots_adjust(left=.11, right=.94, top=.79, bottom=.17)


def fig_fork_rate():
    labels = ["SAGE\n$n-f$ gate", "Hard fork\nblind", "Cox-threshold\n$2f+1$"]
    rates = np.array([0.0, 1.0, 1.0])
    lows = np.array([0.0, .839, .839])
    highs = np.array([.161, 1.0, 1.0])
    colors = [SAGE, UNSAFE, NEUTRAL]
    fig, ax = plt.subplots(figsize=(13.333, 7.5))
    x = np.arange(3)
    ax.bar(x, rates * 100, width=.58, color=colors, alpha=.9, zorder=2)
    ax.errorbar(x, rates * 100, yerr=np.vstack(((rates-lows)*100, (highs-rates)*100)),
                fmt="none", ecolor=INK, elinewidth=2.2, capsize=8, zorder=4)
    ax.scatter(x, rates * 100, s=150, color=colors, edgecolor="white", linewidth=2, zorder=5)
    outcomes = ["0/20 forks\n95% CI: 0–16.1%", "20/20 forks\n95% CI: 83.9–100%",
                "20/20 forks\n95% CI: 83.9–100%"]
    for i, (r, txt) in enumerate(zip(rates, outcomes)):
        y = 23 if r == 0 else 71
        ax.text(i, y, txt, ha="center", va="center", color=SAGE_DARK if r == 0 else "white",
                fontsize=14, fontweight="bold")
    ax.set_xticks(x, labels); ax.set_ylim(0, 112)
    ax.set_ylabel("Observed fork rate (%)"); ax.set_yticks([0, 20, 40, 60, 80, 100])
    ax.grid(axis="y", color=GRID, linewidth=1, zorder=0)
    title(ax, "Partition safety: the boundary gate changes the observed outcome",
          "Balanced 3/3 partition at cutover · 20 repeated process runs per arm · Wilson 95% intervals")
    ax.text(.025, .78, "Why 0 forks?\nNeither partition can form $n-f$.",
            transform=ax.transAxes, fontsize=12, color=SAGE_DARK,
            bbox=dict(boxstyle="round,pad=.4", facecolor="#DDF2F0", edgecolor="none"))
    save(fig, "01_partition_safety")


def fig_evidence_ladder():
    tiers = ["Loopback\nprocesses", "Independent\ncloud hosts", "Injected RTT\n~104 ms"]
    sage = np.array([0., 0., 0.]); control = np.array([100., 100., 100.])
    ns = [20, 5, 3]
    sage_hi = np.array([16.1, 43.4, 56.2]); control_lo = np.array([83.9, 56.6, 43.8])
    fig, ax = plt.subplots(figsize=(13.333, 7.5)); y = np.arange(3)
    ax.errorbar(sage, y-.12, xerr=np.vstack((np.zeros(3), sage_hi)), fmt="o", ms=13,
                color=SAGE, ecolor=SAGE, elinewidth=4, capsize=7, label="SAGE", zorder=3)
    ax.errorbar(control, y+.12, xerr=np.vstack((100-control_lo, np.zeros(3))), fmt="X", ms=13,
                color=UNSAFE, ecolor=UNSAFE, elinewidth=4, capsize=7, label="Blind hard fork", zorder=3)
    for i, n in enumerate(ns):
        ax.text(2, i-.38, f"SAGE  0/{n}", color=SAGE_DARK, fontweight="bold", fontsize=12)
        ax.text(98, i+.42, f"Blind hard fork  {n}/{n}", color=UNSAFE,
                ha="right", fontweight="bold", fontsize=12)
        ax.text(sage_hi[i]+1.5, i-.12, f"CI upper {sage_hi[i]:.1f}%", color=SAGE_DARK, va="center", fontsize=10)
        ax.text(control_lo[i]-1.5, i+.12, f"CI lower {control_lo[i]:.1f}%", color=UNSAFE,
                ha="right", va="center", fontsize=10)
    ax.set_yticks(y, tiers); ax.set_xlim(-8, 108); ax.set_ylim(2.6, -.6)
    ax.set_xticks([0, 20, 40, 60, 80, 100]); ax.set_xlabel("Observed fork rate (%)")
    ax.grid(axis="x", color=GRID)

    title(ax, "The observed direction repeats across three evidence tiers",
          "Points are fork rates; whiskers are Wilson 95% intervals · only the n=3 intervals overlap")
    save(fig, "02_evidence_ladder")


def load_tps():
    p = ROOT / "results" / "raw" / "multihost_tps_3arm.csv"
    vals = {}
    with p.open(newline="") as f:
        for row in csv.DictReader(f):
            vals.setdefault(row["strategy"], []).append(float(row["committed_tps"]))
    return {k: (sum(v)/len(v), np.std(v, ddof=1)) for k, v in vals.items()}


def fig_safety_performance():
    tps = load_tps()
    safety = [("SAGE", 0, SAGE), ("Hard fork", 100, UNSAFE),
              ("Cox-threshold", 100, NEUTRAL)]
    throughput = [("sage", "SAGE", SAGE), ("hardfork", "Hard fork", UNSAFE),
                  ("coxstyle", "Cox-style", NEUTRAL)]
    fig, (ax, ax2) = plt.subplots(1, 2, figsize=(13.333, 7.5), gridspec_kw={"width_ratios": [1, 1.25]})
    safety_names = [item[0] for item in safety]
    safety_colors = [item[2] for item in safety]
    forks = [item[1] for item in safety]
    ax.bar(np.arange(3), forks, color=safety_colors, width=.64)
    ax.set_xticks(np.arange(3), safety_names, rotation=12); ax.set_ylim(0, 112)
    ax.set_ylabel("Fork rate (%)"); ax.grid(axis="y", color=GRID)
    for i, value in enumerate(forks): ax.text(i, value+4, f"{value}%", ha="center", fontweight="bold")
    throughput_names = [item[1] for item in throughput]
    throughput_colors = [item[2] for item in throughput]
    means = [tps[item[0]][0] for item in throughput]
    sds = [tps[item[0]][1] for item in throughput]
    ax2.bar(np.arange(3), means, yerr=sds, capsize=6, color=throughput_colors, width=.64)
    ax2.set_xticks(np.arange(3), throughput_names, rotation=12); ax2.set_ylim(0, 900)
    ax2.set_ylabel("Committed throughput (TPS)"); ax2.grid(axis="y", color=GRID)
    for i, value in enumerate(means): ax2.text(i, value+42, f"{value:.0f}", ha="center", fontweight="bold")
    ax.set_title("Partition safety campaign", fontsize=14, fontweight="bold")
    ax2.set_title("No-fault throughput campaign", fontsize=14, fontweight="bold")
    fig.suptitle("Safety improves sharply while measured throughput remains close", x=.07, y=.95,
                 ha="left", fontsize=20, fontweight="bold")
    fig.text(.07, .895, "Different Cox controls are used in the two campaigns; they are not one protocol arm.",
             color=MUTED, fontsize=12)
    fig.text(.5, .075, "Throughput error bars = across-validator SD · observed mean spread = 1.65%",
             ha="center", color=MUTED, fontsize=11)
    fig.subplots_adjust(left=.09, right=.95, top=.78, bottom=.22, wspace=.33)
    save(fig, "03_safety_performance")


def fig_cert_scaling():
    p = ROOT / "results" / "raw" / "cert_timing.csv"
    rows = list(csv.DictReader(p.open(newline="")))
    n = np.array([int(r["n"]) for r in rows])
    signers = np.array([int(r["signers"]) for r in rows])
    p50 = np.array([float(r["verify_all_p50_us"])/1000 for r in rows])
    p95 = np.array([float(r["verify_all_p95_us"])/1000 for r in rows])
    kib = np.array([float(r["cert_bytes"])/1024 for r in rows])
    fig, (ax, ax2) = plt.subplots(1, 2, figsize=(13.333, 7.5))
    ax.plot(n, p50, marker="o", ms=9, lw=3, color=SAGE)
    ax.plot(n, p95, marker="o", ms=6, lw=1.8, ls="--", color=SAGE_DARK, label="p95")
    ax.set_xlabel("Committee size $n$"); ax.set_ylabel("Verify all signatures (ms)")
    ax.set_ylim(0, 5.7); ax.grid(color=GRID)
    ax.legend(["median", "p95"], frameon=False, loc="upper left")
    ax.annotate("4.48 ms median", (200, p50[-1]), xytext=(-112, -28), textcoords="offset points",
                arrowprops=dict(arrowstyle="->", color=SAGE_DARK), color=SAGE_DARK, fontweight="bold")
    ax2.plot(n, kib, marker="s", ms=9, lw=3, color=BLUE)
    ax2.set_xlabel("Committee size $n$"); ax2.set_ylabel("Certificate size (KiB)")
    ax2.set_ylim(0, 11); ax2.grid(color=GRID)
    ax2.annotate("9.42 KiB", (200, kib[-1]), xytext=(-100, -28), textcoords="offset points",
                 arrowprops=dict(arrowstyle="->", color=BLUE), color=BLUE, fontweight="bold")
    fig.suptitle("Measured one-time CutCert cost at larger committees", x=.07, y=.95, ha="left",
                 fontsize=20, fontweight="bold")
    fig.text(.07, .895, "Ed25519 list certificate · n-f signers · at n=200: 134 signers, up to 66 faults",
             color=MUTED, fontsize=12)
    fig.text(.5, .075, "Once per migration boundary — not a per-block cost", ha="center", color=MUTED, fontsize=11)
    fig.subplots_adjust(left=.09, right=.95, top=.78, bottom=.20, wspace=.32)
    save(fig, "04_cutcert_scaling")


def fig_rollback():
    fig, ax = plt.subplots(figsize=(13.333, 7.5))
    labels = ["Abort inside\nrollback window", "Abort after\nseal deadline"]
    allowed = [100, 0]
    refused = [0, 100]
    x = np.arange(2)
    ax.bar(x, allowed, width=.62, color=SAGE, label="Rollback executed")
    ax.bar(x, refused, bottom=allowed, width=.62, color=UNSAFE, label="Absolute reversion refused")
    ax.text(0, 50, "20 / 20\nrollback", ha="center", va="center", color="white", fontsize=18, fontweight="bold")
    ax.text(1, 50, "20 / 20\nrefused", ha="center", va="center", color="white", fontsize=18, fontweight="bold")
    ax.set_xticks(x, labels); ax.set_ylim(0, 112); ax.set_ylabel("Trials (%)")
    ax.grid(axis="y", color=GRID, zorder=0)
    title(ax, "Rollback is permitted only while target history is provisional",
          "Before seal: discard target-only blocks · After seal: reject legacy reversion")
    ax.legend(frameon=False, loc="upper center", ncol=2)
    save(fig, "05_bounded_rollback")


def fig_competitor_heatmap():
    rows = ["Stop/restart", "Hard fork", "Reconfiguration", "Merge / forkless", "Abstract / Aliph",
            "Cox", "Adaptive policy", "SAGE"]
    cols = ["Live", "Engine\nswap", "Cross-fault\nboundary", "Shadow\nstate", "Migration\nquorum", "Bounded\nrollback", "Multi-tier\nevidence"]
    # 0 no/out of scope, .5 partial/specialized, 1 explicit capability.
    data = np.array([
        [0, .5, .5, 0, 0, .5, .5],
        [1, 1, .5, 0, 0, 0, 0],
        [1, 0, 0, 0, .5, .5, 1],
        [1, .5, .5, .5, .5, 0, 1],
        [1, 1, 0, 0, .5, .5, 1],
        [1, 1, 0, .5, .5, .5, 1],
        [.5, .5, .5, .5, 0, 0, .5],
        [1, 1, 1, 1, 1, 1, 1],
    ])
    from matplotlib.colors import ListedColormap
    cmap = ListedColormap(["#E8ECEC", "#F2D59A", SAGE])
    fig, ax = plt.subplots(figsize=(13.333, 7.5))
    ax.imshow(data, cmap=cmap, vmin=0, vmax=1, aspect="auto")
    ax.set_xticks(np.arange(len(cols)), cols)
    ax.set_yticks(np.arange(len(rows)), rows)
    ax.tick_params(axis="x", top=True, labeltop=True, bottom=False, labelbottom=False, pad=8)
    for i in range(data.shape[0]):
        for j in range(data.shape[1]):
            val = data[i,j]
            txt = "YES" if val == 1 else ("PART" if val == .5 else "—")
            ax.text(j, i, txt, ha="center", va="center", fontweight="bold",
                    color="white" if val == 1 else INK, fontsize=12)
    ax.add_patch(Rectangle((-.5, 6.5), len(cols), 1, fill=False, edgecolor=SAGE_DARK, lw=4))
    fig.suptitle("SAGE's novelty is the combination, not live switching alone",
                 x=.07, y=.95, ha="left", fontsize=20, fontweight="bold")
    fig.text(.07, .895, "YES = explicit capability     PART = partial/specialized     — = outside core scope",
             color=MUTED, fontsize=11)
    fig.text(.5, .065, "Capability synthesis — not a performance ranking",
             ha="center", color=MUTED, fontsize=11)
    fig.subplots_adjust(left=.16, right=.95, top=.72, bottom=.15)
    ax.set_xticks(np.arange(-.5, len(cols), 1), minor=True)
    ax.set_yticks(np.arange(-.5, len(rows), 1), minor=True)
    ax.grid(which="minor", color="white", linewidth=2)
    ax.tick_params(which="minor", bottom=False, left=False)
    save(fig, "06_competitor_heatmap")


if __name__ == "__main__":
    fig_fork_rate()
    fig_evidence_ladder()
    fig_safety_performance()
    fig_cert_scaling()
    fig_rollback()
    fig_competitor_heatmap()
    print(f"Generated 6 SVG + 6 PNG figures in {OUT}")
