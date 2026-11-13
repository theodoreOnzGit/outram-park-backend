# DWSIM differential beyond reactors and PT flash (gh:#1356) — predictions first

Upstream: DWSIM `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`,
GPL-3.0), shallow-fetched 2026-11-13 from <https://github.com/DanWBR/dwsim>
(sparse checkout of `DWSIM.Thermodynamics`, `DWSIM.UnitOperations`,
`DWSIM.ExtensionMethods`, `DWSIM.Math`). Port: this crate, release build.

## PREDICTIONS (committed BEFORE any driver was run)

Written 2026-11-13 after reading the upstream and port sources, before running
either side on these cases.

### Route

A full .NET/Mono build of upstream was **not attempted**: the recipe in
`docs/upstream-harness/README.md` needs ~128 NuGet packages, Mono, a VB runtime
.deb and a multi-project VB build with a GUI project reference, against 7 GB of
free disk (5 GB floor) and 4 shared cores with `CARGO_BUILD_JOBS=1`. This was a
cost judgement, not a measured failure. The route is therefore the fallback:
**verbatim transcription** of the upstream VB routines into small Python
drivers (IEEE doubles; `math.exp/log/log10/pow` call the same glibc `libm` that
.NET/Mono `Math.*` call on Linux), with `check_verbatim.sh` proving each
transcribed expression is present in the pinned source. What this route cannot
show: anything depending on upstream's property-package plumbing (stateful
`CurrentMaterialStream`, `Assets/*.dat`, flash loops). Those rows stay open in
the #1357 gap list.

### Studies and bounds (derived from operation counts, not from results)

Unit round-off `eps = 2.22e-16`.

**S1 NRTL / UNIQUAC `ln gamma`** (`GAMMA_MR`, `Models/NRTL.vb:221-413`,
`Models/UNIQUAC.vb:245-448`). Port differs from upstream in *source order of
operations*, read before running:
- NRTL: port adds each `j` term into `C/S` one at a time; upstream sums the `j`
  terms first (`SumY`) then adds to `C/S` (`lnVg(i) += ...sum`). Port forms
  `c*t*t`, upstream `C*T^2`.
- UNIQUAC: upstream multiplies by the reciprocal `1/(1.98721*T)` where the port
  divides; upstream groups `(-a)+((b*T)+(c*T^2))`, the port `((-a)+b*t)+c*t*t`;
  upstream forms `phi/theta` as `(x*R*(1/r))/(x*Q*(1/q))`, the port `(R/r)/(Q/q)`;
  `sum1` is `theta*(tau/S)` upstream, `(theta*tau)/S` in the port.
- Transcendentals: port `exp/ln` go through PETIR, upstream through libm.

Predictions: (a) **not** all bit-identical before any fix (the reorderings
above are real); NRTL with `B=C=0` binary should be the most often identical.
(b) Worst relative `gamma` gap **<= 16 eps = 3.6e-15** for `|ln gamma| <= 3`
(each reorder moves `ln gamma` by a few ulps of its size, `gamma = exp(.)`
multiplies that by `|ln gamma|`); a breach means a real defect, not round-off.
(c) After translating upstream's order, the arithmetic gap vanishes and the
residual is PETIR-vs-libm `exp/ln` only: **<= 2 ulp of gamma** per point.
Gate: 1e-13 relative; pin the bit-identical count and worst gap.

**S2 Petroleum correlations** (`PetroleumCharacterization/PropertyMethods.vb`,
30 pure functions: Tc, Pc, omega, MW, viscosity, SG conversions) on a Tb x SG
grid inside each correlation's stated range. Closed forms, port written in the
same order (`powf`/`powi` via PETIR). Prediction: **<= 4 ulp** each, bound
`1e-14` relative; expected exceptions are only places where upstream's `x ^ 2`
(libm `pow`) meets the port's `powi(2)` (`x*x`), and uom unit round trips
(`kg/mol` <-> `g/mol`), each <= 1 ulp. Gate 1e-13.

**S3 Shortcut column (FUG)** (`UnitOperations/ShortcutColumn.vb:149-512`,
`DWSIM.Math/BrentMinimize.vb:176-295`). Upstream's K-values come from its flash
package, which is not reproduced here, so the driver is **seeded with the
port's own `K_i`** (as the column harness was seeded with the port's estimates):
this compares the FUG algorithm, not the K-values. Cases: C3-C6 with light/heavy
keys (C4,C5: single-root Underwood) and (C3,C6: distributed-key mode), Ideal and
Peng-Robinson K, feed q = 0.5 and 0.8 (avoids the enthalpy re-computation of q).
Predictions: `D`, `xd`, `xb`, `Nmin`, `N`, `ofs` within 1e-12 relative (only
`ln/log10/pow` PETIR-vs-libm); `Rmin`: the same Brent path means 1e-12, but a
flipped Brent comparison would move `theta` by up to the Brent tolerance 1e-7
and `Rmin` by `|dRmin/dtheta| * 1e-7`, so the fallback bound is **1e-6
absolute**; if breached the first divergent iteration is reported.

**Not run this session (recorded, not skipped silently):** flash family PH/PS,
bubble/dew, stability TPD, SRK/PR1978/PRSV2/LKP/UNIFAC (all need the stateful
property-package plumbing), unit operations needing a flash (heater, pump,
compressor, valve, pipe, heat exchanger). See the #1357 gap list.

## RESULTS (measured 2026-11-13, release, pinned upstream `1abf72d1`)

Reproduce: `python3 -I <driver>.py <outdir>` (activity, petroleum), `fug_driver.py
fug_inputs.txt out.txt`; `./check_verbatim.sh <dwsim clone>` (output:
`check_verbatim_output.txt`: 21 statements and 194 literals found upstream);
tests `tests/upstream_{activity,petroleum,shortcut}_parity.rs` with frozen
bit-pattern fixtures in `tests/fixtures/upstream_gh1356/`. Inputs are arbitrary
test parameters (no external data; provenance: this folder); alkane constants
are Poling, Prausnitz & O'Connell (2001), Appendix A.

| study | points | before any fix | after | prediction | verdict |
|---|---|---|---|---|---|
| S1 NRTL gamma | 345 pts, 966 gammas | 866 bit-identical, worst 4.0e-16 (2 ulp) | **964**, worst 1.5e-16 (1 ulp) | arithmetic reorders visible; <= 3.6e-15; after fix <= 2 ulp | held (NRTL closer than predicted: 90 % identical before) |
| S1 UNIQUAC gamma | 345 pts, 966 gammas | 313 bit-identical, worst 2.0e-15 (12 ulp) | **966, 0 ulp** | same | held; 12 ulp inside the 16 eps bound |
| S2 petroleum correlations | 1328 | 1280 bit-identical, worst 4.2e-15 | unchanged (no port change) | <= 4 ulp, 1e-14 | relative bound held; **ulp bound failed on one row** (26 ulp, Walther B, logZ1 ~ logZ2, cancellation); the control proves libm-vs-PETIR only |
| S3 FUG shortcut (K seeded from the port) | 8 cases x 14 | 110/112 bit-identical | unchanged | 1e-12; Rmin 1e-6 abs | held (worst non-Rmin 0, abs dRmin 5.6e-17) |

### Mismatches localised to the first divergent operation

- **NRTL (100 gammas before the fix).** `activity_driver.py` evaluates the
  port's own operation order next to upstream's and reports the first
  differing intermediate (`activity_localisation.txt`): `tau`, `G`, `S`, `C`
  agree in every point; the first divergence is `ln gamma` (term-by-term
  accumulation into `C/S` versus `SumY` then add). Fixed (upstream's order).
  The remaining 2 gammas differ by 1 ulp in `exp` (PETIR vs libm).
- **UNIQUAC (653 gammas before).** Read, then confirmed by the fix removing
  every difference: reciprocal multiply for `tau`, `(-a)+((b T)+(c T^2))`
  grouping, `fi/teta` with `x` inside, `theta*(tau/S)`, and the
  `((-ln S)-sum1)+1` order. Not separately bisected per operation.
- **Walther viscosity (6 rows).** The libm control in the test reproduces
  upstream bit for bit: `log10`/`exp` implementation, not arithmetic.
- **FUG `Rmin`, distributed-key branch (2 cases).** `fug_port_lu_control.py`
  reproduces the port's values by substituting the port's `lu_solve` for
  Mapack's LU: the dense solve, not Brent, not the physics. Left as accepted
  round-off (< 1e-8); flagged for the maintainer.

### Port changes

1. NRTL/UNIQUAC `activity_coefficients` re-ordered to upstream's operations
   (`src/thermo/activity.rs`; no kovan review stamp existed on them).
   Deliberate deviation kept: UNIQUAC at `x_i = 0` returns the finite
   infinite-dilution value where upstream returns NaN (0/0 in `fi/teta`).
2. Doc claims contradicted by the new measurements, struck through in place:
   the `columns/shortcut.rs` module banner ("not validated against DWSIM's own
   output") and the T14, T43, S13 rows of `docs/upstream-port-coverage.md`.

### Upstream observation (not a port defect)

In the distributed-key Underwood branch `Rmin` can come out negative (-2.8e-2
Ideal, -6.4e-3 Peng-Robinson at feed q = 0.8); the `Rmin > R` guard
(`ShortcutColumn.vb:405`) cannot catch it. The port reproduces it.
