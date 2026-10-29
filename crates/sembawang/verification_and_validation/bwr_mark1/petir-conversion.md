# `bwr_mark1`: the PETIR conversion, measured (2026-10-29, gh:#1033)

> Research, education and V&V only. AI draft, not reviewed. A code-to-code
> check of one change (platform `libm` to PETIR), not a validation.

## Methodology

**What changed.** Every `exp`, `ln`, `powf`, `powi` and `cos` call in
`src/bwr_mark1/` was renamed to PETIR's `r_exp`, `r_ln`, `r_powf`, `r_powi`
and `r_cos` (`petir::mathf::RealMath`). That is 33 call sites in `core.rs`,
`mod.rs`, `params.rs` and `steam.rs`. `sqrt` was left alone, because
IEEE-754 requires it to be correctly rounded. Nothing else changed.

**Prediction, written before the run.** Every quantity moves by less than
1e-8 relative. The one exception would be an event within one step of its
threshold, which could move by a whole `DT_S` step.

**Instrument.** The committed fixture, `headless_30h.csv`, prints rounded
decimals, so it cannot see a last-bit change. A temporary probe test (not
committed) therefore wrote:

- `compare::measure` for all four ablation runs, with `{:?}` (shortest
  round-trip, so exact);
- the full `Frame` with `{:?}` every 600 steps over the 30 h sequence
  (180 frames).

This was done on the code before and after the rename, and the two dumps
were compared number by number. To find the cause, each of the five renames
was reverted on its own and the probe run again.

Platform: x86_64-unknown-linux-gnu, Rust 1.99.1, `--release`.

## Results

| Check | Result |
|---|---|
| Summary metrics, all 4 ablation runs | **bit-identical** (lower head 5.409166666666667 h; H2 at 25 h 124.14795190310869 kg; drywell peak 0.47614387493462323 MPa; vent and explosion none) |
| Frame numbers over 30 h | **9,377 of 9,379 bit-identical** |
| The 2 that differ | both in the frame at 3.0 h: `clad_k` 635.820477021502 -> 635.8204770215016 K (7.2e-16 relative), `q_fluid_w` 6709360.285386335 -> 6709360.285386334 W (1.4e-16 relative) |
| Propagation | none: every later frame is bit-identical |
| Committed fixture | byte-identical, so no re-bless was needed |
| Cause, by bisection | `exp` alone. Reverting `r_exp` gives the old bits exactly; reverting any one of the other four changes nothing. |

**Interpretation.** On this platform PETIR agrees with the platform `libm`
everywhere but one `exp` evaluation, and there by about 1 ulp. That is inside
the root `CLAUDE.md` round-off allowance (below 1e-8 relative) and is flagged
here for the maintainer.

That PETIR's `exp` (a port of ARM optimized-routines) and glibc's `exp`
(derived from the same routines) differ at all is **not explained here**. A
different FMA use in glibc's x86_64 build is a plausible hypothesis, but it
is **not checked**. Native and wasm now take one route: the purpose of
gh:#985.
