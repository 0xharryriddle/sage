# cert_timing.csv — provenance and canonical-source note

## This file is NOT the source of the paper's certificate-timing numbers

Two retained artifacts measure the same quantity (`verify_all_p50_us` for `n-f`
real Ed25519 shares) and they DISAGREE, because they are different executions on
different dates:

| Artifact | n=200 verify p50 | Date | Status |
|---|---|---|---|
| `results/figures/finality_certificate_timing.csv` | 3.910 ms | 2026-07-17 | **CANONICAL** — plotted in paper Fig. `fig:certscale`, cited as "about 3.9 ms" |
| `results/raw/local_refresh/scientific_refresh_20260714T170701Z/attempts/remaining_release/cert_timing.csv` | 3910 us | 2026-07-15 | canonical raw source, sealed campaign |
| `results/raw/cert_timing.csv` (this file) | 4482 us | 2026-07-07 | **SUPERSEDED** — earlier run, not cited |

The paper's `3.9 ms` claim and the plot coordinates at `docs/paper/paper.tex:1219`
both trace to the sealed `scientific_refresh_20260714T170701Z` campaign, whose
registration binds the regenerated families to a specific `HEAD`
(`results/manifests/scientific_refresh_20260714T170701Z.json`).

## Why this note exists

Reading `results/raw/cert_timing.csv` alone and comparing it to the paper produces
a false "paper overclaims" finding: 4.482 ms in the CSV against 3.9 ms in the text.
The paper is correct; this CSV is simply an older execution that was never removed.
A reviewer or agent auditing claim-to-artifact traceability must use the figures
CSV or the sealed campaign directory, not this file.

Do not delete this file: it is retained evidence of an earlier run. Do not cite it.

## Note on run-to-run variance

The two executions differ by roughly 15% at n=200 on the same machine class. These
are single-process CPU microbenchmarks with 1,000 verify-all iterations aggregated
to p50/p95; individual samples were not persisted, and the sealed record does not
retain the CPU model. Cross-run and cross-machine timing comparison is therefore
unsupported, which the paper states explicitly. The measurement establishes the
ORDER of the O(n) cost, not a production latency figure.
