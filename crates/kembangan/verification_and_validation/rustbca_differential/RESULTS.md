# Results: boon-lay's RustBCA copy against upstream RustBCA v3.0.0 (gh:#1426, 2026-11-13)

Predictions: `PREDICTIONS.md` (committed as `dc752e335`, before any run). Harness: `shared.rs`
(one file, compiled against both codes). Upstream: tag v3.0.0 = `673bd3f`, unmodified, built outside
the workspace (`rustc 1.99.1`, glibc 2.39, `rand 0.10.3`). The copy: `libm` 0.2.16 via
`petir::real::Real`. Pinned by `tests/rustbca_upstream_differential.rs`.

## The headline

**The copy reproduces upstream bit for bit when its `libm` calls are replaced by the platform libm,
and differs from upstream only at libm level (1 ulp per call) as shipped.**

| Build of the copy | Kernel values bit-identical | Function values | Cascade values | Ions identical in every line |
|---|---:|---:|---:|---:|
| **all transcendentals via glibc** (control, scratch edit, reverted) | **24 624 / 24 624** | **2 116 / 2 116** | **39 848 / 39 848** | **720 / 720** (all 691 277 lines of the full dump `cmp`-identical) |
| as shipped (`libm` crate, musl-derived) | 21 088 | 1 918 | 26 625 | 9 |
| counterfactual: `exp`/`ln`/`powf` through PETIR's ARM routines (scratch, reverted) | 22 092 | 2 112 | 29 254 | 9 |

So there is **no port defect in the arithmetic, the data tables, the RNG stream or the control flow**:
the seeded ChaCha8 stream (`petir::rng`, gh:#995) is draw-for-draw identical to `rand` 0.10.3's, and
`Real::powi` equals what `f64::powi` lowers to on x86-64 here. The control run is the localisation: with the
libm calls replaced, there is no first divergent operation because there is no divergence.

## What was compared

* Kernel: 4 320 rows, 4 104 numeric (24 624 values) and 216 `PANIC` (MAGIC with Lenz-Jensen is
  `unimplemented` upstream, in both codes); status of every row identical.
* Functions: 418 rows, 2 116 values.
* Cascades: 90 cases x 8 ions = 720 ions, 275 974 output particles, 125 100 incident-ion collisions
  (233 ions reflected, 487 implanted). Every ion: identical particle count, identical flags, identical
  collision count and trajectory length (0 structural differences of 720).

## Shipped build, as pinned

| Layer | Values | Bit-identical | 1 ulp | 2-4 ulp | > 4 ulp | Worst relative | Where |
|---|---:|---:|---:|---:|---:|---:|---|
| Kernel (`K`) | 24 624 | 21 088 (85.6 %) | 1 910 | 499 | 1 127 | 3.06e-11 | `H W LENZ_JENSEN GM10 100 keV p=1.5 A` |
| Functions (`F`) | 2 116 | 1 918 (90.6 %) | 139 | 41 | 18 | 1.84e-15 | `SCREEN MOLIERE Z=1,18` |
| Cascades (`P`,`T`) | 39 848 | 26 625 (66.8 %) | 3 345 | 4 460 | 5 418 | 1.08e-11 | incident trajectory, case 12 ion 5, collision 115 |

Per kernel output (any difference): `theta` 11.0 % of rows, `psi` 28.2 %, `psi_recoil` 22.1 %, `recoil_energy`
10.7 %, `asymptotic_deflection` 11.1 %, `x0` 3.1 %. The ">4 ulp" values are near-grazing rows
(theta ~ 1e-8 radian) where a 1-ulp change in an integrand is amplified by cancellation; worst relative
3.06e-11 is on such a row. All kernel/cascade differences are accounted for below; none is unexplained.

Cascades: 668 of 711 non-identical ions have a divergent incident-trajectory entry, the first at
collision index 2 to 560 (median 7), and its size is 1 ulp for 478 of them, 2 ulp for 108, larger for the
rest. Incident-ion final energy relative difference: median 0, p90 3.4e-15, p99 4.0e-14, max 1.2e-13.
Final position and direction: median 4.2e-16, p99 4.7e-14, max 1.4e-13. The all-particle digest (recoil
cascade included) is identical in 10 ions only: a cascade holds thousands of libm calls, so
"bit-identical" is the wrong yardstick for it (see P3 below); the right one is structure (identical in 720/720)
and magnitude (all <= 1.2e-13 for the incident ion).

## Which libm call does it: per-operation evidence

Probe (`driver/libm_probe_main.rs`, 2e6 random arguments per row over RustBCA's argument ranges; platform
= glibc 2.39). "libm" is what `Real::f(x)` calls; "arm" is PETIR's `petir::real::{exp, ln, powf}` free
functions (ARM optimized-routines, the algorithm glibc >= 2.28 uses for these three):

| op | libm vs glibc: calls differing | max | arm vs glibc |
|---|---:|---:|---:|
| `exp(-x)`, x in [0,40] | 9.83 % | 1 ulp | 0.070 % |
| `exp(-ln u)` | 9.39 % | 1 ulp | 0.064 % |
| `ln u` | 7.24 % | 1 ulp | 0.011 % |
| `ln` over 1e-6..1e6 | 0.95 % | 1 ulp | 0.0006 % |
| `sin` / `cos` | 3.11 % / 3.09 % | 1 ulp | n/a |
| `sin_cos` (both outputs) | 3.09 % / 3.12 % | 1 ulp | n/a |
| `atan2` | 23.06 % | 1 ulp | n/a |
| `acos` | 7.63 % | 1 ulp | n/a |
| `cbrt` | 0 % | 0 | n/a |
| `powf(x, -1/3)`, general `powf` | 7.40 %, 9.61 % | 1 ulp | 0.074 %, 0.071 % |

Ablation on the copy (start from the all-glibc control; route ONE op back to `libm`; rest glibc):

| only this op via `libm` | kernel bit-identical (of 24 624) | function (of 2 116) | cascade values (of 39 848) | ions identical (of 720) |
|---|---:|---:|---:|---:|
| `exp` | 23 465 | 1 986 | 32 084 | 108 |
| `ln` | 24 624 | 2 114 | 39 848 | 720 |
| `powf` | 24 587 | 2 044 | 39 848 | 720 |
| `sin_cos` | 24 504 | 2 116 | 36 695 | 102 |
| `sin` | 24 496 | 2 116 | 37 619 | 113 |
| `cos` | 23 562 | 2 116 | 39 848 | 720 |
| `atan2` | 23 360 | 2 116 | 31 455 | 16 |
| `acos` | 24 554 | 2 116 | 39 848 | 720 |
| `cbrt` | 24 624 | 2 116 | 39 848 | 720 |

Reading: the cascades are moved by `exp`, `sin_cos`, `sin` and above all `atan2` (the lab-frame
deflection `psi = atan2(sin theta, Ma/Mb + cos theta)`, evaluated on every collision, 23 % of calls differ).
`powf`, `cos`, `acos`, `cbrt`, `ln` change kernel or function values but never a default-path cascade here.
The worst kernel row (3.06e-11) is localised to `cos`: with only `cos` routed to `libm` it is the same row,
the same value (the Gauss-Mehler integrand). `x0` (the Newton root finder for the distance of closest
approach) differs on 3.1 % of rows with the `libm` crate and on 0 % with the ARM routes: that is `exp`/`powf`.

## Predictions scored (PREDICTIONS.md)

| | Predicted | Measured | |
|---|---|---|---|
| P1 | functions: >= 95 % bit-identical; worst <= 1e-14 | 90.6 %; 1.84e-15 | **bit-identical share MISSED** (underestimated the 7-10 % mismatch of `exp`/`powf`); bound met |
| P2 | kernel: >= 70 % bit-identical; theta/psi/recoil worst <= 1e-12; status identical | 85.6 %; worst 3.06e-11 (theta 1.5e-11, recoil_E 3.1e-11) on near-grazing rows | share met; **1e-12 bound MISSED by 30x on the near-grazing rows**, inside the 1e-9 allowance I stated for them; status 4 320/4 320 identical |
| P3 | cascades: >= 50 % ions bit-identical; N equal >= 90 %; collision count differs <= 15 %; final E <= 1e-10 | **9 of 720 (1.3 %)**; N equal 100 %; collision count differs 0 %; final E max 1.2e-13 | **bit-identical share badly MISSED** (I had not counted the thousands of libm calls per cascade); the structural and magnitude predictions met |
| P4 | every mismatch's first divergent op is a transcendental call | confirmed by the control (full dump identical with libm calls swapped) | met |

## Not covered (stated, not hidden)

* `INTERPOLATEDPLUS`, GAUSS_MEHLER with other `n_points`, Morse / Lennard-Jones (root-finder features not carried), 2-D
  geometries, layered `Mesh1D`, compounds (several species per cell), `weak_collision_order`, the
  `GASEOUS` / `THRESHOLD` mean-free-path models, `Normal`/`Uniform` input-distribution sampling in
  `process_input_file`. Cases were chosen to straddle the three default code paths (Mesh0D, one species,
  LIQUID, KR_C + Mendenhall-Weller for cascades; four screened potentials x four integrals for the kernel).
* Statistical observables with each code's own RNG (case 2 of the issue: sputtering yield, reflection
  coefficient, depth histogram, 10^5 ions) are not run: with the RNG stream identical the seeded comparison above
  is strictly stronger (the same histories, not the same distribution). Not re-checked on a different platform's libm.
* Bit-identity is with glibc 2.39 x86-64; the copy itself is platform-independent (pure Rust `libm`), so
  the pin test is too.

## Deferred for the maintainer

1. **Route `exp`/`ln`/`powf` in `src/rustbca/` through PETIR's ARM routines** (`petir::real::{exp, ln, powf}`)
   instead of `Real::exp(x)` (= `libm` crate). Counterfactual measured above: functions 90.6 -> 99.8 % bit-identical,
   `x0` 3.1 -> 0 %, worst cascade relative 1.1e-11 -> 7.1e-13; cascades stay at 9/720 identical because
   `atan2`/`sin_cos` remain. It would move the seeded-cascade digests pinned in
   `local_change_tests::seeded_cascades_are_pinned_across_the_rng_swap` (#995) and is a design choice
   ("one fixed implementation on every platform" holds for both), so it is not made here.
2. Routing `atan2`/`sin`/`cos` to a glibc-identical implementation does not exist in PETIR; making cascades
   bit-identical to glibc would need CORE-MATH-style correctly rounded versions. Not pursued.
