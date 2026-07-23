# API token usage per commit

> **Auto-generated — do not hand-edit.** Regenerated on every commit by `scripts/token_accounting.py` (via the `post-commit` hook) from the `API-Usage-Since-Last-Commit` trailer that the `prepare-commit-msg` hook stamps into each commit message. Rebuild manually with `python3 scripts/token_accounting.py report`.

## Methodology & caveats

- **Source.** Token counts come from the Claude Code session transcripts (`~/.claude/projects/<slug>/*.jsonl`) — the same data `ccusage` reads. Nothing is estimated or invented.
- **Attribution is temporal**, not per-diff: each row is the token usage recorded *between the previous commit and this one*. A commit that bundles a lot of exploration carries those tokens.
- **`total` = `in` + `out` + `cache_read` + `cache_write`.** Cache-read (prompt-cache re-reads of the growing context) usually dominates; it is shown separately so you can weigh it.
- Commits authored outside a Claude session legitimately show `0` (`source=none`) and are omitted below.
- Sub-agent / parallel-session usage in the same project dir is included.

## Per-commit ledger

| Date | Commit | Subject | Total | in | out | cache_read | cache_write |
|---|---|---|--:|--:|--:|--:|--:|
| 2026-07-23 | `a12db7b` | feat(tooling): per-commit API-token accounting hooks + do... | 4,240,534 | 15 | 6,973 | 4,220,802 | 12,744 |
| 2026-07-23 | `e3ef6f5` | fix(tooling): token ledger report — use git %x1f/%x1e for... | 6,222,248 | 26 | 25,521 | 6,177,397 | 19,304 |
| **TOTAL** | | **2 commits** | **10,462,782** | **41** | **32,494** | **10,398,199** | **32,048** |
