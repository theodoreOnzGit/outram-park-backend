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
| 2026-07-24 | `b195d34` | docs(readme): add a (clearly-in-jest) Phua Chu Kang tagline | 6,734,604 | 22 | 11,910 | 4,983,785 | 1,738,887 |
| 2026-07-24 | `7693155` | outram-mc: port remaining CSG quadric surfaces — Plane, X... | 51,484,074 | 185 | 117,074 | 49,259,689 | 2,107,126 |
| 2026-07-24 | `fc7c55f` | chore(docs): refresh token-usage ledger | 3,281,946 | 12 | 9,564 | 3,263,979 | 8,391 |
| 2026-07-24 | `cb3d3e4` | Merge outram-foam branch into develop: dwsim Tier-1 therm... | 9,929,504 | 29 | 15,312 | 9,895,505 | 18,658 |
| 2026-07-24 | `10e52fe` | Merge outram-blender branch into develop: outram-park-for... | 16,208,481 | 49 | 24,226 | 16,154,607 | 29,599 |
| 2026-07-24 | `81c7a61` | Merge outram-mc branch into develop: port remaining CSG q... | 16,208,481 | 49 | 24,226 | 16,154,607 | 29,599 |
| 2026-07-24 | `6b67471` | Merge outram-blender branch into develop: cfMesh boundary... | 25,763,962 | 76 | 35,336 | 25,678,518 | 50,032 |
| 2026-07-24 | `1a14bf9` | Merge outram-foam branch into develop: multiphase drift-f... | 25,763,962 | 76 | 35,336 | 25,678,518 | 50,032 |
| 2026-07-24 | `a143f0e` | outram-mc: fix nested-lattice surface-tracking under-coun... | 35,914,153 | 113 | 146,389 | 34,020,064 | 1,747,587 |
| 2026-07-24 | `a0ae11b` | Merge outram-blender into develop: cfMesh quality checks,... | 36,127,440 | 102 | 46,361 | 36,013,530 | 67,447 |
| 2026-07-24 | `1ccf235` | Merge outram-mc into develop: fix nested-lattice surface-... | 36,127,440 | 102 | 46,361 | 36,013,530 | 67,447 |
| 2026-07-24 | `724ea2e` | Merge pflotran into develop: outram-park-mpi communicator... | 36,127,440 | 102 | 46,361 | 36,013,530 | 67,447 |
| 2026-07-24 | `b2a5bac` | Merge remote-tracking branch 'origin/develop' into claude... | 157,164 | 4 | 290 | 155,614 | 1,256 |
| 2026-07-24 | `0ab7d2b` | chore: pin transitive kstring to 2.0.2 for rustc 1.94 MSRV | 2,508,572 | 55 | 15,210 | 2,467,629 | 25,678 |
| 2026-07-24 | `41052a6` | docs: regenerate api.md mirrors for dwsim-libs, multiphas... | 1,165,500 | 23 | 6,110 | 1,150,818 | 8,549 |
| 2026-07-24 | `fed0bf3` | chore(docs): refresh token-usage ledger | 2,321,347 | 37 | 21,061 | 2,214,978 | 85,271 |
| **TOTAL** | | **26 commits** | **482,022,475** | **1,678** | **932,153** | **472,653,816** | **8,434,828** |
