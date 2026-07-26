# `local_client_r2_smoke` — harness smoke probe, NOT evidence

Single-rate probe used only to prove the client-campaign pipeline runs end to end
after the client-finality verifier scheme-drift fix
(`docs/reviews/2026-07-27-client-finality-verifier-scheme-drift.md`).

## Status: NOT PUBLISHABLE

`r2_smoke/campaign_manifest.json` carries `status=invalid`, written by the harness
itself. The campaign-level sealer failed fail-closed with:

```
FAIL: insufficient measured overload points: required 2, found 0
```

That is correct behaviour, not a defect. A single non-overloaded rate cannot
establish a saturation knee, so the sealer refuses to seal the campaign.

## What this probe legitimately demonstrates

One thing only: after the verifier fix, a real multi-process trial passes
independent cryptographic verification.

- `rate=200`, seed 42, n=4, f=1, 2 clients, receipt quorum 3, 512B payloads
- 1000/1000 requests finalized, `success_fraction=1.0`
- `scripts/verify_client_results.py` → `PASS: validated 1400 client request rows`
- Before the fix the same artifact failed at line 1 with
  `invalid Ed25519 finality share from validator 0`

## What it does NOT establish

- No throughput claim. One point is not a curve.
- No latency claim. The host was co-tenanted (see
  `results/raw/local_client_r2/README.md`).
- Nothing about p50/p95/p99, which R2 requires.

## Do not cite

No paper claim may reference this directory. R2 needs a quiet host, a real rate
ladder crossing the knee, and ≥5 seeds per point with trial-level CIs.
