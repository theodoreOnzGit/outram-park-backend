# outram-park-fork-dwsim-libs

> **This is OUTRAM PARK's independent Rust translation of selected DWSIM
> algorithms.** It is not the official DWSIM software and is not affiliated
> with, endorsed by, or sanctioned by DWSIM Inc. or its maintainers. See
> [`TRADEMARKS.md`](./TRADEMARKS.md) for the full attribution and
> non-affiliation notice. Translated from
> [`DanWBR/dwsim`](https://github.com/DanWBR/dwsim), `dwsim8`/`master` branch
> (confirm the current default branch when cloning) — no commit is pinned
> (no persistent local clone is currently maintained); see
> `upstream_source/README.md` for the full provenance record.

Pure-Rust port of DWSIM's chemical-process modelling kernels — thermal-
hydraulics and thermodynamics (flash algorithms, property packages/EOS,
equipment models).

**Status: early-stage.** This crate is currently a scaffold (`src/lib.rs` is
a stub); see `docs/port-scope.md` for the prioritised porting scope (which
DWSIM C# modules to port, tier by tier, with source paths and LOC) and the
bottom-up porting order, and `bd show op-qo2` for the current backlog status.

## License

GPL-3.0-only (see the workspace root `LICENSE`), matching DWSIM's own
upstream license directly — no relicensing step is needed. See
`TRADEMARKS.md` for the full non-affiliation notice.
