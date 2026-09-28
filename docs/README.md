# SAGE Documentation

The canonical manuscript is **SAGE: A Certified Cross-Engine Boundary for Fixed-Committee Consensus Migration in Permissioned Blockchains**, at `paper/SAGE_ThaiCong_IEEE/main.tex`. It uses Elsevier's `elsarticle` class for *Blockchain: Research and Applications*; `_IEEE` in the directory name is historical. The retained `paper.tex` is a legacy draft, not the current manuscript.

The paper presents a conditional locked-readiness protocol argument, not a fully qualified networked implementation or a submission-ready claim. Its `rebuild.py` consumes eight SHA-256-pinned CSVs and two pinned interpretation notes. Three paper-specific CSVs reside under `../results/raw/paper/SAGE_ThaiCong_IEEE/` because the legacy CSVs at `../results/raw/` contain different historical bytes and remain unchanged. The generator derives presentation products; it does not rerun experiments or repair missing trial ledgers.

Run `python3 -B docs/paper/SAGE_ThaiCong_IEEE/rebuild.py --check-data` to validate retained input identities. `--verify-rebuild --work-root <scratch>` compares 22 products across two isolated builds and runs the four staged manuscript gates; it does not confer runtime authority or submission readiness. The detailed operator review ledger under gitignored `docs/reviews/` is not available in a public clone. `RESPONSE_TO_REVIEWS.md` is a historical response with a current-scope warning, not that ledger.

- `ARTIFACT_EVALUATION.md` — claim-to-evidence tiers and their limits.
- `RESPONSE_TO_REVIEWS.md` — response history and current unresolved scope.
- `../formal/README.md` — bounded model checks, not implementation qualification.
- `../README.md` — workspace and source-artifact scope.
