# API token usage per commit

> **Auto-generated — do not hand-edit.** Regenerated on every commit by `docs/historian/token_usage.py` (via the `post-commit` hook) from the `API-Usage-Since-Last-Commit` commit trailers. Rebuild with `python3 docs/historian/token_usage.py report`; query a period with `python3 docs/historian/token_usage.py query --from DDMMYY --to DDMMYY`.

## Methodology & caveats

- **Source.** Counts come from the Claude Code session transcripts (`~/.claude/projects/<slug>/*.jsonl`, same data `ccusage` reads). Nothing is invented.
- **Attribution is temporal**, not per-diff: each row is the usage recorded *between the previous commit and this one*.
- **`total` = `in` + `out` + `cache_read` + `cache_write`.** Cache-read dominates; shown separately.
- Commits authored outside a Claude session show `0` (`source=none`) and are omitted below.

## Per-commit ledger

| Date | Commit | Subject | Total | in | out | cache_read | cache_write |
|---|---|---|--:|--:|--:|--:|--:|
| 2026-07-23 | `a12db7b` | feat(tooling): per-commit API-token accounting hooks + do... | 4,240,534 | 15 | 6,973 | 4,220,802 | 12,744 |
| 2026-07-23 | `e3ef6f5` | fix(tooling): token ledger report — use git %x1f/%x1e for... | 6,222,248 | 26 | 25,521 | 6,177,397 | 19,304 |
| 2026-07-23 | `e7fb2d3` | chore(docs): refresh token-usage ledger | 1,457,487 | 3 | 5,553 | 1,439,859 | 12,072 |
| 2026-07-23 | `3130b38` | Merge outram-mc branch into develop: njoy MGXS scatter/Ch... | 3,913,769 | 16 | 8,852 | 3,894,474 | 10,427 |
| 2026-07-23 | `d7866cf` | Merge pflotran branch into develop: pflotran upstream-par... | 11,355,332 | 43 | 27,013 | 11,296,360 | 31,916 |
| 2026-07-23 | `f3603e9` | Merge outram-foam branch into develop: outram-foam-multip... | 23,527,461 | 87 | 46,170 | 22,952,274 | 528,930 |
| 2026-07-23 | `7f9e652` | Merge outram-blender branch into develop: mesh-operator s... | 29,195,660 | 106 | 51,904 | 28,605,018 | 538,632 |
| 2026-07-23 | `d58fe9c` | Merge outram-blender branch into develop: revolve/spin op... | 40,774,298 | 148 | 69,573 | 40,136,391 | 568,186 |
| 2026-07-23 | `db91d17` | docs(singlish): add 'bang gang' — knock off work / finish... | 48,295,948 | 175 | 80,743 | 47,636,531 | 578,499 |
| 2026-07-24 | `6c50cdd` | refactor(tooling): consolidate token accounting into docs... | 7,215,668 | 23 | 8,724 | 7,175,809 | 31,112 |
| 2026-07-24 | `7693155` | outram-mc: port remaining CSG quadric surfaces — Plane, X... | 51,484,074 | 185 | 117,074 | 49,259,689 | 2,107,126 |
| 2026-07-24 | `fc7c55f` | chore(docs): refresh token-usage ledger | 3,281,946 | 12 | 9,564 | 3,263,979 | 8,391 |
| **TOTAL** | | **12 commits** | **230,964,425** | **839** | **457,664** | **226,058,583** | **4,447,339** |
