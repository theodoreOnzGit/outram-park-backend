# SAMM `bigeta` against compiled NJOY2016 (unit level), gh:#1366

Research, education and V&V only; not for facility operation or safety decisions.

## Methodology

- Upstream: NJOY2016, <https://github.com/njoy/NJOY2016> (modified BSD-3, LANL; GPL/AGPL-compatible,
  see `../../../LICENSE.njoy`). Versions compared: **2016.80 `ab27c6414a0f25b173f2b637e6dc7e428fdaff00`**
  (2026-10-08) and **2016.79 `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`** (the port's baseline). Accessed 2026-11-10.
  Clones are gitignored under `../../../upstream_source/` (`NJOY2016/` sparse, `src` only; `NJOY2016-2016.79/src`
  from `git archive`). Provenance: `../../../upstream_source/README.md`.
- `extract.sh 80|79 <dir>` checks the SHA-256 of `samm.f90` and extracts `getfg`, `bigeta`, `getps` verbatim,
  plus `kseries` (`bigeta` cut after the I0/K0/I1/K1 series, one line added to save the four values; see script).
- `driver.f90` + `run.sh`: gfortran 13.3.0 `-O2` (no `-march`, so no FMA) over a grid of
  eta in {5.5, 6, 8, 12, 20, 35, 50, 100} x rho in {0.001, 0.01, 0.05, 0.1, 0.3, 0.5, 0.55, 1, 3},
  keeping the cases `coulx` sends to `bigeta` (eta > 10 rho and eta > 5) with 2 rho eta <= 80,
  x lll in {0, 1, 3} x flags (jdopha,jdoder,ishift) in {(0,0,-1), (1,1,1)}. Prints hex bits of
  I0, K0, I1, K1 (line `K`) and of p, s, dp, sinphi, cosphi, dphi and F, G, F', G' at L=lll (line `B`).
  Initial outputs match `pspcou` (cosphi = 1, the rest 0).
- Port: `src/samm/coulomb/dispatch.rs::bigeta`. The port does not expose K0/K1, so the test also carries a
  test-only mirror of the four series loops, once with f64 division (the port's line) and once with
  single-precision division (2016.80's line), to attribute the difference.
- Tests: `tests/samm_bigeta_vs_njoy2016_units.rs`.

## Prediction (written before the first run, from the issue and the source diff)

1. Upstream 2016.80 vs 2016.79 differ only in `b=b+1./(k+1)` (float32 division, rounded to f64) against
   `b=b+1/(k+1)` (integer 0 for k >= 1, so b stays 1). 2016.79 drops the harmonic sum of K0 and K1.
2. Port (f64 `1.0/(k+1)`) vs **2016.80**: K0, K1 differ by about 1e-8 relative or less (float32 relative error
   <= 6e-8 per `1/(k+1)`, weighted by series terms smaller than the leading log term), so **not bit-identical**
   whenever the loop runs past k = 1 with a visible term; bit-identical only for very small q = 2 rho eta where the
   second term is below the rounding of the sum. The differences carry through G, G', P, S, phi at the same
   order (<= ~1e-7).
3. Port vs **2016.79**: differs by the whole harmonic term, far above 1e-8 (grows with q; O(1e-3..1) at q ~ 1..10),
   except small q.
4. I0 and I1 (no changed line) are **bit-identical** port vs both versions; the port's `r_powi(2)` vs Fortran `(k+1)**2`
   integer square is exact. Likewise `getfg`/`getps` post-processing is bit-identical given identical K inputs, which this
   test cannot isolate beyond the above.

## Results (2026-11-10; gfortran 13.3.0 -O2; rustc 1.99.1 release)

**The prediction was wrong on size.** The issue and this README predicted ~1e-8. Measured, the f64-division
mirror (the port's old line) against 2016.80 K0 matches in 0/56 and K1 in 3/56, with worst relative
difference 1.0, because K0 and K1 are cancelling sums: at large q = 2 rho eta the series terms exceed the
result by ~exp(2z), so the float32 rounding (6e-8) of `1/(k+1)` is amplified to O(1). A float32-division mirror
reproduces 2016.80's I0, K0, I1, K1 in **56/56** bits (so the cause is exactly the `1.` literal), and an
integer-division mirror reproduces 2016.79's in 56/56. I0 and I1 are bit-identical everywhere. 2016.80 vs 2016.79
themselves: K0 0/56 and K1 0/56 identical (worst 1.83 and 1.0); p 0/336 identical.

Port `bigeta` vs 2016.80 over the 336 calls (bit-identical counts):

| quantity | before any change | after harmonic-step fix only | after both fixes |
|---|---|---|---|
| p | 12 | 224 | **336** |
| s | 175 | 280 | **336** |
| dp | 263 | 280 | **336** |
| sinphi | 173 | 254 | **336** |
| dphi | 316 | 329 | **336** |
| cosphi | 207 | 207 | 316 (336 when jdopha = 1) |

Two port defects, both fixed in `src/samm/coulomb/dispatch.rs` (against 2016.80, gh:#1368 sync item; the sync
issue is the maintainer's, this is its `bigeta` part):

1. `b += 1.0/(k+1) as f64` (f64) in the K0 and K1 loops; 2016.80 is `b=b+1./(k+1)` (float32 quotient). Now
   `harmonic_step`: `f64::from(1.0_f32 / (k+1) as f32)`. (Not 2016.79's: integer division, 0 for k >= 1.)
2. Found only because (1) alone left lll >= 1 mismatching at 1-2 ulp: the port called `getfg(eta, rho, 0, lll, ..)`
   where upstream passes `llmax`, which `pspcou` sets to `lll+2`; `getfg` derives `limit = max(3, llmax+1)` from it,
   so the recursions started from a different limit. Now `lll + 2`. All `lll = 0` cases were already identical.

Not changed, reported: `coulx`'s finite-eta branch (`dispatch.rs`, `getfg(eta, rho, lll, lll, ..)`) has the same
`llmax` pattern (upstream: `llmax`, i.e. `lll+2`); it is outside `bigeta` and has no comparison here yet.

`cosphi` when `jdopha = 0`: upstream's `getps` leaves it untouched (the caller's `pspcou` initial value 1); the port
returns 0. It is not a requested output; the test compares `cosphi` only when `jdopha = 1`.

Reference elementary functions: `exp(-pi*eta)` through petir `r_exp` is 56/56 bit-identical to glibc here, `sqrt` identical.

Reproduce: `./run.sh` (regenerates `upstream_output_v79.txt`, `upstream_output_v80.txt`), then
`cargo test --release -p njoy-outram-park-fork --test samm_bigeta_vs_njoy2016_units`.
