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
| 2026-07-24 | `46b4ccf` | outram-mc: port Torus{X,Y,Z} CSG surfaces — completes the... | 40,092,252 | 115 | 71,203 | 39,909,015 | 111,919 |
| 2026-07-24 | `f10a000` | outram-mc: HexLattice 3-D axial rings + X-orientation rou... | 8,028,654 | 21 | 12,114 | 8,002,461 | 14,058 |
| 2026-07-24 | `9f67449` | outram-mc: add the lattice/ dir files (complete f10a000) | 6,064,023 | 18 | 11,721 | 6,039,420 | 12,864 |
| 2026-07-24 | `b2a5bac` | Merge remote-tracking branch 'origin/develop' into claude... | 157,164 | 4 | 290 | 155,614 | 1,256 |
| 2026-07-24 | `f136ea0` | Merge outram-blender into develop: cfMesh polyMesh disk w... | 43,981,600 | 126 | 53,883 | 43,848,765 | 78,826 |
| 2026-07-24 | `f7b506e` | Merge outram-foam into develop: multiphase two-fluid Eule... | 43,981,600 | 126 | 53,883 | 43,848,765 | 78,826 |
| 2026-07-24 | `998fbf5` | Merge outram-mc into develop: Torus{X,Y,Z} CSG surfaces (... | 43,981,600 | 126 | 53,883 | 43,848,765 | 78,826 |
| 2026-07-24 | `85c74a6` | Merge pflotran into develop: HDF5 snapshot I/O via pure-R... | 51,903,622 | 148 | 61,930 | 51,754,543 | 87,001 |
| 2026-07-24 | `0ab7d2b` | chore: pin transitive kstring to 2.0.2 for rustc 1.94 MSRV | 2,508,572 | 55 | 15,210 | 2,467,629 | 25,678 |
| 2026-07-24 | `41052a6` | docs: regenerate api.md mirrors for dwsim-libs, multiphas... | 1,165,500 | 23 | 6,110 | 1,150,818 | 8,549 |
| 2026-07-24 | `3e1d2d0` | Merge outram-blender into develop: cfMesh octree near-wal... | 56,555,651 | 160 | 66,281 | 56,396,808 | 92,402 |
| 2026-07-24 | `7ca58b5` | Merge outram-foam into develop: pin transitive kstring 2.... | 56,555,651 | 160 | 66,281 | 56,396,808 | 92,402 |
| 2026-07-24 | `fed0bf3` | chore(docs): refresh token-usage ledger | 2,321,347 | 37 | 21,061 | 2,214,978 | 85,271 |
| 2026-07-24 | `05d284b` | Merge remote-tracking branch 'origin/develop' into claude... | 1,494,863 | 24 | 7,295 | 1,479,955 | 7,589 |
| 2026-07-24 | `8e43d71` | Merge outram-blender into develop: cfMesh multi-level oct... | 65,934,291 | 187 | 77,208 | 65,751,893 | 105,003 |
| 2026-07-24 | `367e0d0` | Merge pflotran into develop: outram-park-mpi groups + Car... | 65,934,291 | 187 | 77,208 | 65,751,893 | 105,003 |
| 2026-07-24 | `afc4906` | Merge remote-tracking branch 'origin/develop' into claude... | 2,640,058 | 40 | 10,855 | 2,603,033 | 26,130 |
| 2026-07-24 | `3dc85bc` | cfmesh: add prism boundary layers (add_boundary_layers) | 189,149 | 4 | 932 | 187,255 | 958 |
| 2026-07-24 | `0050b92` | Merge remote-tracking branch 'origin/develop' into claude... | 1,455,130 | 27 | 6,591 | 1,439,646 | 8,866 |
| 2026-07-24 | `5b21186` | Merge claude/outram-blender-mggk7u into develop | 72,716,199 | 205 | 86,371 | 72,513,566 | 116,057 |
| 2026-07-24 | `fe1ca23` | Merge claude/pflortran-merge-develop-8k3wji into develop | 72,716,199 | 205 | 86,371 | 72,513,566 | 116,057 |
| 2026-07-24 | `0cc63a4` | Merge claude/outram-foam-8ookor into develop | 72,716,199 | 205 | 86,371 | 72,513,566 | 116,057 |
| 2026-07-24 | `efb8acd` | cfmesh: add polyhedral (median) dual — one cell per verte... | 5,250,656 | 87 | 93,137 | 5,095,585 | 61,847 |
| 2026-07-24 | `e579dd7` | cfmesh: V&V — polyhedral dual bridges to a solvable foam ... | 2,974,882 | 39 | 10,976 | 2,937,002 | 26,865 |
| 2026-07-24 | `5d58952` | Merge pflotran into develop: generic distributed CG + rea... | 82,330,543 | 231 | 98,230 | 82,098,368 | 133,714 |
| 2026-07-24 | `3f5b374` | docs: refresh generated token-usage ledger | 891,335 | 10 | 3,133 | 884,930 | 3,262 |
| 2026-07-24 | `56eba24` | docs: refresh generated token-usage ledger (lag row) | 754,285 | 10 | 5,805 | 743,477 | 4,993 |
| 2026-07-24 | `2500a77` | Merge remote-tracking branch 'origin/develop' into claude... | 2,961,391 | 33 | 11,240 | 2,933,602 | 16,516 |
| **TOTAL** | | **50 commits** | **1,284,126,599** | **4,172** | **2,045,055** | **1,272,146,503** | **9,930,869** |
