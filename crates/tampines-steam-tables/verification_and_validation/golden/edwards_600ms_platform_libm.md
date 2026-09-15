# Edwards–O'Brien blowdown — platform-libm golden reference

**Generated:** 2026-09-14 (UTC)
**Recorded at commit:** `09787761` — *petir: lift the RealMath trait into the crate*
**Reproduced from:** `892c3898` with `--features platform-libm`

## What this is, and why it exists

This is **not** a V&V result against experiment. It is a **golden reference**: a
recorded trajectory of `edwards_obrien_pipe_blowdown_600ms`, taken on the
transcendental route that was the default before `baf9408b` routed
`exp`/`ln`/`powf` through PETIR's ARM optimized-routines ports.

It exists because that sweep changed the outcome of this case. Before it the
test passed; after it the test panics partway through. The sweep is a ~1 ulp
change per call and is **not** wrong — the ports sit within about 1 ulp of glibc
and are identical on every platform, which the platform route is not. What the
episode exposed is that the Edwards break cell is marginal enough for a last-ulp
perturbation to tip it into the density floor (`bn:op-s2dc`), and that "1009
library tests unchanged, V&V fixtures character-identical" is the wrong
acceptance check for a ~10⁹-evaluation chained transient (`bn:op-ppmk`).

Keeping the pre-sweep trajectory lets the defect be studied against something
concrete instead of remembered.

## Methodology

**Case.** Edwards & O'Brien (1970) pipe blowdown, T&A modified nodalisation: 24
uniform cells, `dt = 30 µs`, `t_end = 0.600 s` (20 000 timesteps), break area
87 % of pipe cross-section at the end of volume 24, adiabatic interior, Hendrie
(1973) axial enthalpy initial condition. `SolverMode::Pimple` — the test's
default; the all-Mach hybrid is opt-in via `EDW_HYBRID=1` and is **not** used
here.

**Command.**

```bash
cargo test --release -p tampines-steam-tables --features platform-libm \
    --test edwards_blowdown edwards_obrien_pipe_blowdown_600ms -- --nocapture
```

**Acceptance criterion.** The three CSVs in this directory must reproduce
**byte-for-byte**. They are written at full precision by the test itself
(`%.6f` time, `%.4f` pressure, `%.5f` void fraction), so any trajectory change
at all shows up as a diff. This is a *reproduction* gate, not a tolerance band:
the reference's whole purpose is to be exactly re-derivable.

**Machine.** A platform-libm result is platform-dependent by construction, so
the host is part of the provenance and not an aside: Intel Xeon @ 2.10 GHz,
4 cores, glibc 2.39, rustc 1.98.1, Linux 6.18.44, release profile.

## Results

Recorded at `09787761` (test passed, **346.17 s**):

| quantity | value |
|---|---|
| break flow at `t = 0` | 0.00 kg/s |
| break flow peak | 127.6 lbm/s |
| GS-1 flashing plateau (mean 0.02–0.06 s) | 392.7 psia |
| GS-1 pressure RMSE vs Edwards data (16 pts, 0–0.30 s) | 58.6 psia |
| GS-1 pressure at `t_end` | 26.4 psia |
| min T past 0.42 s | GS-1 390.4 K, GS-4 396.6 K, GS-7 398.1 K |
| GS-1 tail pressure rebound | +0.0 psia |
| artificial-cooling verdict | no cold tail (artefact absent) |

Gauge pressures (psia) at checkpoints, from `sim_pressure_gs.csv`:

| t (s) | GS-1 | GS-2 | GS-3 | GS-4 | GS-5 | GS-6 | GS-7 |
|---|---|---|---|---|---|---|---|
| 0.000 | 1015.3 | 1015.3 | 1015.3 | 1015.3 | 1015.3 | 1015.3 | 1015.3 |
| 0.010 | 386.6 | 435.8 | 423.5 | 434.3 | 433.1 | 426.4 | 419.9 |
| 0.100 | 410.0 | 431.0 | 303.9 | 385.8 | 410.0 | 408.4 | 409.2 |
| 0.300 | 289.8 | 290.6 | 309.5 | 318.0 | 319.9 | 323.4 | 330.4 |
| 0.600 | 26.4 | 28.3 | 29.9 | 32.1 | 32.9 | 33.3 | 33.7 |

Void fraction at GS-5: 0.000 at `t = 0`, 0.729 at 0.300 s, 0.994 at 0.600 s.

**Reproduction check, run 2026-09-14 at `892c3898` with
`--features platform-libm`:** test passed in 346.06 s; every headline metric
above identical; and all three CSVs **bit-identical** to the committed
reference (`diff` clean on 607 data rows each of pressure, void fraction and
break flow).

That result carries a second, unplanned finding. Between `09787761` and
`892c3898` this crate also gained the Region 5 `(p,h)`/`(p,s)`/`(h,s)` wiring,
the `ThermoClosure`/`KnpFaceClosure`/`PsiRefresh` enums, and the GUI isochore
work. The byte-identical reproduction shows every one of those was
**behaviour-neutral on this case at their default settings** — which is what
they were designed to be, now measured rather than asserted.

## Interpretation, stated plainly

- The reference is **reproducible**, so it is usable as a gate rather than a
  souvenir.
- It says nothing about whether this solver is *right*. GS-1 RMSE of 58.6 psia
  against the Edwards data is the same number it always was; this directory
  does not improve it and does not claim to.
- It is **platform-dependent**. Re-derived on a host with a different libm, the
  byte-for-byte gate would be expected to fail while the physics stayed the
  same. Treat a diff on another machine as information about the libm, not a
  regression, and read `petir::mathf`'s module docs before concluding anything.

## References

Edwards, A. R. & O'Brien, T. P. (1970). Studies of phenomena connected with the
depressurization of water reactors. *Journal of the British Nuclear Energy
Society*, 9(2), 125–135.

Tomlinson, E. T. & Aumiller, D. L. (1999). *An assessment of RELAP5-3D using the
Edwards pipe problem*. B-T-3271.

Hendrie, J. M. (1973). USAEC letter — axial enthalpy initial condition.
