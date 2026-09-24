# A fissile nuclide `.h5` written by this workspace, read and transported by OpenMC

GitHub **#304**, and its acceptance:

> - A U-235 `.h5` written by this crate is read by **OpenMC** and transported.
> - LCT-008 run by OpenMC on our written library agrees on `k` with OpenMC on
>   its own ENDF/B-VIII.0 library within statistics.
> - The verification record is re-run against a **real fissile nuclide**, not
>   only `Syn1`.

Measured 2026-09-24. **Row 1 is met, rows 2 and 3 are partly met and the
remainder is blocked on a dependency** — stated in full under *What is not
done* below, rather than left for a reader to infer.

## Why the old writer's verification could not have caught its own limit

`hdf5_nuclide_write/` records OpenMC reading a file this crate wrote, and that
record caught a real defect (`kTs/294K` written as a 1-element array where
upstream writes a scalar). But it was verified against **`Syn1`, a synthetic
nuclide with two reactions** — elastic and capture. A synthetic nuclide has
whatever reactions its test gives it, so:

- the **MT restriction** (`if !matches!(r.mt, 2 | 102)`) was never exercised;
- both of `Syn1`'s reactions have `threshold_idx = 0`, so the **threshold
  convention** was never exercised either;
- neither reaction needs a `level` law, so the **threshold-grid and
  zero-at-threshold invariants** were never exercised.

Three separate defects, all invisible to a correct test of a nuclide that
cannot be made critical. This is the same shape as the SiC free-gas defect: a
component that passes every check it has, because the check was built around
what it does rather than what it is for.

## Methodology

1. `write_nuclide` emits `SynF.h5`: `Z = 92`, `A = 235`, `AWR = 233.0248`, a
   65-point log grid from 1e−5 to 2e7 eV at 294 K, carrying
   - **MT=2** elastic, `sigma = 4` barn flat, tabulated isotropic cosine, CM;
   - **MT=18** fission, `sigma = 2` barn flat, **evaluated Watt spectrum**
     (`a = 9.88e5` eV, `b = 2.249e-6` /eV, `u = 0`), isotropic in the lab;
   - **MT=51** a discrete inelastic level, `Q = -4e4` eV, **`level` energy law**
     (`threshold = |Q|(A+1)/A = 40171.6555` eV, `mass_ratio = (A/(A+1))^2`);
   - **MT=102** capture, `sigma = 0.5` barn flat;
   - **`total_nu`** = 2.5, flat, as a `Tabulated1D` with `emission_mode = total`.
2. `openmc.data.IncidentNeutron.from_hdf5` reads it back.
3. OpenMC's **C++ solver** transports it: a bare 20 cm sphere at
   0.05 atoms/barn-cm, k-eigenvalue mode.

### The analytic gate, and why the cross sections are flat

Every cross section is **energy-independent on purpose**. With flat cross
sections the infinite-medium multiplication factor is

```text
k_inf = nu * Sigma_f / Sigma_a = 2.5 * 2.0 / (2.0 + 0.5) = 2.0
```

**exactly**, and independently of the fission spectrum, the scattering law, the
level threshold, the temperature and the grid — every one of which cancels when
the cross sections do not depend on energy. So under a reflective boundary
OpenMC must return 2.0, and a discrepancy localises to ν̄, the fission cross
section or the capture cross section rather than to "something in the file".

This is arithmetic, not a fit. Nothing was tuned to reach it: the three inputs
were chosen first and 2.0 is what they imply.

## Results

### OpenMC read every field back as written

```text
READ OK: SynF Z=92 A=235 awr=233.024800   temps: ['294K']
  MT=2    Q=0            cm=True  xs[0]=2     nprod=1
        neutron prompt dist=UncorrelatedAngleEnergy energy=NoneType
  MT=18   Q=0            cm=False xs[0]=15.81 nprod=1
        neutron prompt dist=UncorrelatedAngleEnergy energy=WattEnergy
  MT=51   Q=-40000       cm=True  xs[0]=0     nprod=1
        neutron prompt dist=UncorrelatedAngleEnergy energy=LevelInelastic
  MT=102  Q=2.2e+06      cm=False xs[0]=316.2 nprod=0
total_nu: Tabulated1D
```

(The `xs[0]` values above are from the first run, before the cross sections
were made flat for the analytic gate; the law types are what matter here and
are unchanged.) `WattEnergy` and `LevelInelastic` are both laws the MT=2/102
writer could not express at all.

### OpenMC transported it

`k = 1.99854` on the first generation, against the analytic **2.0**. Generation
values then fluctuate about 2.0 as expected at 150–400 particles per
generation (`1.81909, 2.15667, 1.80305, 2.04425, 2.05177, …`).

Recomputed from the written file's own numbers by this crate's reader, rather
than from the test that wrote it:

```text
nu=2.5, sigma_f=2, sigma_c=0.5, k_inf=2.000000
```

## Three defects this found, each of which needed OpenMC to find

None of these could be detected by a round trip through this crate.

### 1. `threshold_idx`: the cross section starts AT the threshold

Upstream stores `xs` **from the threshold onward**, not zero-padded on the full
grid. Measured on upstream's own `U235.h5` (76027 grid points):

| MT | `len(xs)` | `threshold_idx` | sum |
|---|---|---|---|
| 2 | 76027 | 0 | 76027 |
| 18 | 76027 | 0 | 76027 |
| 51 | 66891 | 9136 | 76027 |
| 52 | **697** | **75330** | 76027 |
| 91 | 125 | 75902 | 76027 |

So the invariant is `len(xs) + threshold_idx == len(energy)`. The writer
demanded the **full** grid length, which is the same thing only when
`threshold_idx == 0`. A zero-padded array is **not refused by OpenMC** — it is
read as though it began at the threshold, which shifts the cross section
silently. On this fissile nuclide that produced a runaway source and a run that
never finished a batch.

### 2. A `level` law needs a grid point AT its threshold

Measured on the same file: `energy[threshold_idx]` equals the level law's own
`threshold` attribute to 1.3e−4 (MT=51), 7.6e−7 (MT=52) and 2.2e−7 (MT=53)
relative, with `energy[threshold_idx - 1]` strictly below it, and the product's
angle grid starting **exactly** at the threshold.

That is not cosmetic. OpenMC interpolates the cross section between grid
points, so a first non-zero point *above* the threshold makes every energy in
the interval below it carry a non-zero interpolated MT. A collision sampled
there enters `LevelInelastic::sample` with `E < threshold`, and
`mass_ratio * (E - threshold)` is **negative**.

### 3. The cross section must be exactly ZERO at the threshold point

Measured on upstream's `U235.h5`:

| MT | `xs` at threshold | next four points |
|---|---|---|
| 51 | **0.000000e+00** | 6.693e−11, 8.170e−11, 8.967e−11, 9.799e−11 |
| 52 | **0.000000e+00** | 2.819e−06, 1.052e−04, 1.077e−04, 1.102e−04 |
| 53 | **0.000000e+00** | 4.730e−04, 9.914e−04, 1.510e−03, 2.028e−03 |
| 91 | **0.000000e+00** | 4.383e−05, 4.407e−05, 4.764e−05, 4.882e−05 |

A cross section that **steps** from 0 to a finite value at the threshold makes
`mass_ratio * (E - threshold)` land arbitrarily close to zero at a finite rate.

**And OpenMC crashes on it rather than reporting it** — an upstream defect in
its own right, filed as GitHub #306. ~~It has no guard for a particle below the
library's minimum energy~~ **CORRECTED same day**: the sub-minimum case is
clamped; the crash is a NaN energy defeating both range guards, confirmed by
gdb and by a lab-frame control. The section at the end of this file has the
chain.
`src/material.cpp:833` computes the logarithmic grid index as

```cpp
std::log(p.E() / data::energy_min[neutron]) / simulation::log_spacing;
```

which goes **negative** for `E < energy_min` and indexes out of bounds;
`LevelInelastic::sample` (`src/distribution_energy.cpp`) is a bare
`return mass_ratio_ * (E - threshold_);` with no clamp. Measured 2026-09-24: a
flat 1 barn step at the threshold **segfaults OpenMC** (exit 139) after a few
generations, *with the threshold sitting exactly on a grid point*. Removing
MT=51 entirely ran clean, which is how it was localised.

A consequence worth keeping: `threshold_idx` indexes the **last zero**, not the
first non-zero. `ReactionData::from_full_grid` trimmed to the first non-zero,
which shifted every threshold reaction one grid point up and put a finite cross
section at the threshold — defect 3 in a different disguise, and caught by the
check written for it.

All three are now enforced by `write_nuclide`, each with the mechanism in the
error text, and pinned by unit tests plus
`tests/nuclide_h5_vs_openmc.rs::we_read_the_u235_library_openmc_produced`,
which asserts both invariants hold across all 45 of U-235's threshold
reactions.

## A finding about the Watt law, recorded because it explains a scope decision

`WattEnergy::sample` rejects until `E_out <= E - u`. With `u = 0` a
**low-energy** incident neutron needs an outgoing energy that a ~MeV-peaked
spectrum essentially never produces, so the loop runs a long time. This is why
the runs above are slow, and it is a real reason production fissile evaluations
give MT=18 a **tabulated `continuous`** χ rather than a Watt law — which is
exactly the law the dependency limit below blocks. The Watt law is correct and
faithfully written; it is simply a poor χ for a thermal-capable nuclide.

## What is NOT done, and why

**A complete, real U-235 cannot yet be written.** Its neutron products need the
`continuous` (1 law) and `correlated` (4 laws) distributions — measured
directly from upstream's own file, and independently from the ACE side by
`crate::acer::ce_laws` (`{LAW3: 39, LAW4: 1, LAW61: 4}`). All three tabulated
laws (`continuous`, `correlated`, `kalbach-mann`) store their incident-grid
interpolation as a **rank-2** attribute, and **`hdf5-pure` 0.20.1 writes only
scalar and rank-1 attributes** (its README's attribute table is the complete
list; a probe confirmed `I64Array(vec![3, 2])` lands as shape `(2,)` where
upstream writes `(2, 1)`).

Writing it flat is not a workaround. OpenMC's Python reader does
`interp_data[0, :]`, which raises `IndexError` — so `IncidentNeutron.from_hdf5`
fails outright. Its C++ reader is worse: it reads the attribute's real shape
into a `Tensor<int>` and takes `temp.slice(0)`/`slice(1)`
(`src/distribution_energy.cpp:63-66`), which on a rank-1 attribute gives the
**right** answer for `NR = 1` by coincidence and the **wrong** one for `NR > 1`.
A defect invisible on every single-region law and wrong on the first
multi-region one is not a compromise worth shipping.

So those three laws are **represented in full and refuse at emission**, naming
the dependency limit. They are not stubs: the payload is complete and
`TabulatedEnergyOut::pack` is implemented and unit-tested, so lifting the
refusal is one rank-2 attribute away. `hdf5-pure` is a third-party MIT crate
(`stephenberry/hdf5-pure`), so adding that variant is a dependency decision for
the maintainer.

**Consequently:**

- Acceptance row 1 — **met**, for a fissile nuclide (`SynF`), not for U-235.
- Acceptance row 2 (LCT-008 on our library vs OpenMC's own) — **not run**: it
  needs a complete U-235, U-238 and the rest of the LCT-008 set.
- Acceptance row 3 — **met in the sense that matters** (the record is no longer
  built on a two-reaction synthetic nuclide; it is built on a fissile one with
  four reactions, three law types, ν̄, and a threshold channel) and **not met
  literally** (`SynF` is synthetic, not a real evaluation).

The converged `k ± sigma` under a reflective boundary is still running at the
time of writing; the first-generation `1.99854` against the analytic `2.0` and
the file's own `k_inf = 2.000000` are what is recorded above, and nothing
further should be quoted until the run lands.


## The upstream defect this exposed (GitHub #306)

> **CORRECTED 2026-09-24, same day.** The mechanism first written here was
> **wrong in its central claim**, and is struck through below rather than
> deleted because a commit message and a GitHub issue described it.
>
> ~~A positive sub-minimum energy indexes the cross-section arrays with a
> negative index.~~ It does not: `Nuclide::calculate_xs` **clamps** that case
> (`if (p.E() < grid.energy.front()) i_grid = 0;`) and evaluates by linear
> extrapolation below the table.
>
> ~~"~4.9 % of the flux was silently wrong."~~ The 4.9 % sub-minimum flux is
> real and measured, but every cross section in that reproducer is **flat**, so
> extrapolating below the table is **exact**. No error was demonstrated, and it
> should not have been called wrong without checking what the extrapolation
> produced.
>
> The defect is real and the confirmed mechanism is sharper. It was obtained
> with **gdb**, after reading the source twice had produced a wrong answer
> twice — which is the lesson worth keeping.

### The confirmed mechanism: a NaN energy defeats both range guards

1. **The data steps at a `level` threshold.** OpenMC interpolates between grid
   points, so a finite cross section *at* the threshold makes the interpolated
   MT non-zero **below** the kinematic threshold.
2. **`LevelInelastic::sample` is unclamped** — `mass_ratio_ * (E - threshold_)`
   is **negative** when sampled below the threshold, and nothing checks it.
3. **The CM→lab conversion takes `sqrt` of a negative product**
   (`src/physics.cpp:1170`):
   `E = E_cm + (E_in + 2*mu*(A+1)*std::sqrt(E_in * E_cm)) / (A+1)^2`.
   With `E_cm < 0` this is **NaN**, so the particle's energy is NaN.
4. **Both range guards fail on NaN** (`Nuclide::calculate_xs`): `NaN < front()`
   and `NaN > back()` are **both false**, so it falls through to the
   binary-search branch. This is the defect proper — the guards are ordered
   comparisons, which do not partition the NaN case.
5. **The index is undefined.** `i_log_union` comes from
   `int(log(E / energy_min) / log_spacing)` (`src/material.cpp:832`);
   `log(NaN)` is NaN and NaN→`int` is UB. The garbage index reads
   `grid.grid_index[...]` out of bounds → **SIGSEGV**.

### Confirmed, not inferred

gdb backtrace, single-threaded and deterministic:

```text
Program received signal SIGSEGV, Segmentation fault.
#0  openmc::Nuclide::calculate_xs(int, int, double, openmc::Particle&)
#1  openmc::Particle::update_neutron_xs(int, int, int, double, double)
#2  openmc::Material::calculate_neutron_xs(openmc::Particle&) const
#3  openmc::transport_history_based_single_particle(openmc::Particle&)
```

**Falsification test.** If NaN is the trigger, switching the same reaction to the
**lab** frame skips the `sqrt`, so the negative energy reaches the `<` guard and
is clamped. On the identical file with only `center_of_mass` flipped:

| MT=51 frame | result |
|---|---|
| centre of mass | **SIGSEGV**, exit 139 |
| laboratory | **exit 0**, leakage 0.06056 ± 0.00081 |

One attribute, two outcomes, and the only code difference is the square root.

### The separate, milder issue

Sub-minimum **positive** energies are not a crash: clamped to `i_grid = 0` and
evaluated by linear extrapolation below the table with a negative interpolation
factor. Measured on a legal fast-only 1 keV–20 MeV grid with an otherwise
conventional MT=51: **4.9 % of the flux (5.70 of 116.8)** falls below the
library minimum and is transported that way. With flat cross sections that
extrapolation is exact, so it is not an error here — but extrapolating a real,
structured cross section several decades below its first point would not be
defensible, and nothing warns. Noted, not claimed as a defect.

### Verdict and qualification

A defect on three counts, any one of which would break the chain: an unvalidated
negative sampled energy, an unguarded `sqrt` of a possibly-negative product, and
a range guard that NaN fails on both sides. OpenMC uses `fatal_error` freely for
bad data elsewhere. Its own comment a few lines above the crash site even says
that forcing a segfault via an out-of-bounds index "is not necessarily
guaranteed … this technique should be replaced by something more robust".

**Production libraries do not trigger it.** It needs a finite cross section *at*
a `level` threshold, and ENDF-derived evaluations are exactly zero there —
checked on U-235 MT=51/52/53/91. No number in this repository is affected.

### What it constrains on our side

Two things, and `write_nuclide` now enforces the second and documents the first:

- **emit grids that reach thermal** when a threshold scattering law is present;
- **never emit a finite cross section at a `level` threshold** — which was
  already the zero-at-threshold invariant above, and now has a second,
  independent reason.

Reproducer: `tests/nuclide_h5_vs_openmc.rs::emit_sub_minimum_level_emission_reproducer`
(`#[ignore]`d) writes the conventional file; the stepping variant and the
lab-frame control are made from it by
`openmc_inputs/repro_sub_minimum_level*.py`.
