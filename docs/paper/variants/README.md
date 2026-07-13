# Paper Variants

This directory preserves non-canonical paper sources and backups used during the paper consolidation/refactor.

## Files

- `paper_modified_by_mentor.tex` — mentor-edited variant. Build metadata from the missing `paper_new.tex` records MD5 `52e9e707606c94d000b5fb59b2ae4350`, matching this file, so it is treated as the recovered `paper_new` content.
- `paper.pre_merge_2026-06-19.tex` — backup of `docs/paper.tex` created before the latest consolidation pass that merged the current paper with the mentor/new variant.

## Canonical Source

The canonical full source is `docs/paper/paper.tex`. The root `docs/paper.tex` file is only a lightweight compatibility entrypoint that inputs the canonical source, so edits should be made in `docs/paper/paper.tex`.

## Policy

Keep variants immutable unless explicitly correcting provenance metadata. New paper backups should be dated and documented here.
