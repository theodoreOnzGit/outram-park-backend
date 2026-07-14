# outram-park-fork-dwsim-libs

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


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

**Status: equipment-model correlations landed.** Five DWSIM equipment
models are ported with `uom`-typed public APIs: `pipe` (Darcy-Weisbach +
Beggs & Brill + Lockhart-Martinelli two-phase pressure drop), `valve` (IEC
60534 liquid/gas/two-phase Kv sizing), `heat_exchanger` (LMTD, epsilon-NTU
effectiveness, Bowman/Underwood multi-pass F-correction), `expander`
(isentropic + Schultz polytropic-efficiency turbine model), and `pump`
(direct calculation modes + NPSH). This crate has no property-package/flash
of its own by design -- functions that need a flash-dependent quantity (e.g.
an outlet temperature) take it as an input or a caller-supplied closure,
rather than reimplementing DWSIM's thermodynamics stack; `tampines` is the
intended consumer with flash access. See `docs/port-scope.md` for the full
prioritised porting scope and `bd show op-qo2` for the current backlog
status (deferred items: Petalas-Aziz, full Tinker shell-and-tube rating,
Pipe's transient network solver, Floater-Hormann curve interpolation, and
DWSIM's flash-algorithm/property-package/reactor tiers, none of which are
needed for `tampines`'s current equipment-model scope).

## License

GPL-3.0-only (see the workspace root `LICENSE`), matching DWSIM's own
upstream license directly — no relicensing step is needed. See
`TRADEMARKS.md` for the full non-affiliation notice.
