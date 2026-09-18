# Verification & validation — `outram-park-fork-liggghts`

**Status as of 2026-09-15.** Methodology *and* measured results, per the
workspace `CLAUDE.md` rule that a V&V document must state both.

Summary of what changed on this date: the crate gained a **cross-code
verification leg against upstream LIGGGHTS-PUBLIC**, which its 2026-09-06
maturity declaration explicitly recorded as missing. **Four of six**
deterministic cases are **bit-identical** to upstream; the two oblique cases
agree to floating-point round-off (1-3 ulp). A bulk pebble-bed settling case
agrees on packing fraction to **0.2 %**. **Three** defects in the pre-existing
code were found and are documented below.

~~Three of four deterministic cases ... Two defects~~ **CORRECTED 2026-09-17** —
this paragraph contradicted section 2's own table, which lists six cases and
four bit-identical, and section 4, which documents three defects
(`integrator.rs`, `contact.rs`, `rolling.rs`). The counts grew as cases and
defects were added and this summary was not updated with them.

**A consolidated single-table view of every case** -- deterministic and bulk,
with the upstream file each is judged against -- is
[`cross-code-summary.md`](./cross-code-summary.md). This document holds the
narrative and the defect histories.

**This is verification, not validation.** It shows this crate reproduces
LIGGGHTS. It does not show that LIGGGHTS' granular physics is right for an
HTR-10 pebble bed. See § 5.

---

## 1. Reference: upstream LIGGGHTS-PUBLIC, built and run

| | |
|---|---|
| Source | `github.com/CFDEMproject/LIGGGHTS-PUBLIC` |
| Commit | `3d5c00f20519e6bb6eb6756f51f1ad36564e649d` (2024-06-07) |
| Build | `make stubs` + `make serial`, g++ 15.2.1, `-O2 -fPIC` |
| Data | `reference-data/liggghts/` (inputs + trajectories + provenance README) |
| Tests | `crates/outram-park-fork-liggghts/tests/liggghts_cross_code.rs` |

Upstream was **compiled and executed**, not quoted from documentation — the
same standard `petir` is held to against GSL 2.8. A two-line patch to
`dump_custom.cpp` raises the printed precision from `%g` (6 significant figures)
to `%.17g`; it changes printed digits only and is documented in the
reference-data README. Without it the comparison saturates at ~`1e-6` relative
and cannot distinguish the port from upstream's `printf`.

The deterministic cases in § 2 use monodisperse spheres `d = 10 mm`,
`ρ = 2500 kg/m³`, `E = 10 MPa`, `ν = 0.3`. The bulk cases do **not** — the
HTR-10 case (§ 4.7) runs the published design point, `d = 60 mm`,
`ρ = 1730 kg/m³`, `E = 5e8 Pa`, `ν = 0.2`; each input deck states its own.
Common to all: SI units, `fix nve/sphere`, `pair_style gran`.

~~All cases: monodisperse spheres `d = 10 mm` ...~~ **CORRECTED 2026-09-17** —
false since the HTR-10 case landed. The same wording was corrected in the
reference-data README in the same change.

---

## 2. Cross-code verification — results

Every **sampled frame** is compared, not just the endpoint.

| Case | Physics exercised | Frames | `max|Δx|` [m] | `max|Δv|` [m/s] | `max|Δω|` [rad/s] |
|---|---|---|---|---|---|
| head-on, Hertz | normal force, viscoelastic damping, restitution | 251 | `0` | `0` | `0` |
| head-on, Hooke | linearised normal model, `v_char = 2 m/s` | 251 | `0` | `0` | `0` |
| wall bounce + gravity | primitive wall branch (`R* = r`, `m* = m`), contact make/break over 400 000 steps | 2001 | `0` | `0` | — |
| rolling, counter-spinning pair | **CDT rolling resistance** (`µ_r = 0.1`) | 201 | `0` | `0` | `0` |
| oblique + friction | **tangential shear-history spring**, Coulomb slip, contact torque / spin-up | 251 | `0` | `1.11e-16` | `5.68e-14` |
| oblique, **no-history** (stateless path) | `contact` + `simulation` vs `tangential no_history` | 251 | `0` | `1.11e-16` | `1.42e-14` |

**Four of the six are bit-identical to upstream over the whole trajectory**;
the two oblique cases agree to round-off (1–3 ulp). The last row verifies the
*other* engine — the stateless `contact` + `simulation` path — against the
upstream model it actually implements; see § 4.2.

The oblique case — the one that exercises everything the stateless
[`contact`](../src/contact.rs) module cannot do — agrees to round-off:
`1.11e-16 m/s` is one ulp at `|v| ≈ 1 m/s`, and `5.68e-14 rad/s` is ≈3 ulp at
the final `ω_z = −83.1815 rad/s`. That residual is the expected consequence of
summing identical terms in a different association order.

Endpoint values, both codes agreeing to every printed digit:

| Case | Final state |
|---|---|
| head-on Hertz | `v_x = −0.900007 m/s` (requested `e = 0.9`) |
| head-on Hooke | `v_x = −0.899972 m/s` |
| oblique | `v = (−0.751987, 0.577628, 0) m/s`, `ω_z = −83.1815 rad/s` |
| wall bounce | `z = 0.013708858 m` |
| rolling | `ω_y = ±13.617325830096817 rad/s` (from `±20`), `v_x = ∓0.238460 m/s` |

Note the Hertz and Hooke cases land on *different* realised restitutions
(`0.900007` vs `0.899972`) and each code reproduces its own model's value. The
agreement is therefore not an artefact of both codes converging on the
requested `0.9`.

---

## 3. Bulk pebble-bed settling

The deterministic cases above verify a *contact*. A pebble bed is a
many-contact, chaotic, frictional assembly, so it is checked separately and
**statistically**.

**Methodology.** Cylindrical container `R = 30 mm` (`D/d = 6`), 354 pebbles
`d = 10 mm`, `ρ = 2500 kg/m³`, `E = 5 MPa`, `ν = 0.3`, `e = 0.5`, `µ = 0.3`,
settled under gravity for 400 000 steps at `dt = 5 µs` (2.0 s simulated). The
Rust run starts from **LIGGGHTS' own post-insertion configuration**, so both
codes integrate the identical initial state. Solid fraction is measured over a
bulk slab excluding 4 particle radii at the bottom wall and at the free
surface, by exact sphere-cap integration.

**Results (2026-09-15).**

| Quantity | Rust | LIGGGHTS | difference |
|---|---|---|---|
| solid fraction `φ` | 0.5571 | 0.5582 | **0.20 % relative** |
| voidage `ε` | 0.4429 | 0.4418 | 0.25 % relative |
| bed top `z_top` [m] | 0.12015 | 0.12244 | 1.9 % (≈ ¼ diameter) |
| coordination number `Z` | 4.475 | — | — |
| residual `KE` [J] | `4.5e-8` | `2.3e-11` | see below |

**On the residual kinetic energy.** The Rust run plateaus around `1e-7 J`
where LIGGGHTS reaches `2.3e-11 J`. This was diagnosed rather than waved
through: **100 % of the residual energy sits in 3 of the 354 particles**, and
the contact count is steady at 964–965 from `t = 0.75 s` onward. The packed bed
itself is static; what remains is a couple of unconstrained pebbles rattling on
the free surface at `≤ 1.6e-2 m/s`. That is also where `z_top` differs. A
chaotic settling run diverging between two codes on its loosest surface
particles, while agreeing on bulk packing to 0.2 %, is the expected outcome —
but the plateau is recorded here as an open observation, not claimed as
understood in full.

---

## 3.1 Radial and axial porosity map — the structure the bulk number hides

Section 3 checks **one** number, the bulk solid fraction. That number is an
average, and for reactor work the average is the least interesting thing about
a packed bed. A bed of equal spheres is not homogeneous near a wall: no sphere
centre can approach closer than one radius, so the centres order into layers
and the local void fraction **oscillates** — `ε = 1` at the wall, a minimum
about half a diameter in, a maximum about a diameter in, damping to the bulk
value over several diameters.

This matters because coolant follows the path of least resistance. The
high-porosity annulus at the wall carries disproportionate flow — **wall
channelling** — in a core whose power is generated in the interior. A bed model
carrying only a bulk porosity cannot represent it.

**Methodology.** The map is taken from **LIGGGHTS' own settled state**
(`reference-data/liggghts/pebble_bed_settled.csv`), not from a Rust run, so it
is anchored to the cross-code reference and a regression here is a regression
in the analysis rather than a re-test of the solver. Porosity is estimated by
deterministic point sampling on a stratified cylindrical grid — no RNG, so the
map is bit-reproducible and usable as a fixture. Bins are `d/10`. The radial
profile samples only the axial bulk window (`z_min + 2d` to `z_max − 2d`) so
the radial structure is not contaminated by the axial one.

**Results (2026-09-16).** Radial, distance `y` from the wall:

| `y/d` | `ε` | |
|---|---|---|
| 0.05 | 0.8753 | wall bin, heading for 1 |
| **0.55** | **0.2631** | **first minimum** — first layer of equators |
| **1.05** | **0.5533** | **first maximum** — gap between layers |
| 1.45 | 0.3209 | second minimum |
| 1.85 | 0.4728 | second maximum |
| 2.35 | 0.3213 | third minimum |

Oscillation period `0.90 d`, i.e. one pebble diameter to within the `0.1 d` bin
width. Peak-to-trough amplitude damps from `0.2902` to `0.1519`.

Axial, height `z` above the floor:

| `z/d` | `ε` | |
|---|---|---|
| 0.05 | 0.8724 | floor bin |
| 0.55 | 0.3592 | first minimum |
| 0.95 | 0.5892 | first maximum |
| 4.0–11.0 | **0.440089** mean (0.367–0.500) | interior |
| 12.75 | 0.9991 | free surface |

**Internal consistency, and it is the reason to trust the map.** The
area-weighted radial mean is `0.442993` and the axial interior mean
`0.440089`. Section 3's bulk voidage for this same bed — computed by an
entirely different route, exact sphere-cap integration with no sampling — is
`0.4429`. Three independent estimators agreeing to within 0.3 % is what
promotes this from a plot to a fixture.

**Regression.** All 30 radial and 128 axial bins are committed in
`tests/pebble_bed_porosity.rs` and compared bin by bin at `2e-3` absolute.
Measured worst deviation on re-run: `4.4e-7` radial, `5.0e-7` axial — the
half-ulp of the fixtures' 6-decimal rounding. The tolerance is not absorbing
drift.

### This contradicts `tampines`' near-wall model, and the shape is why

`tampines::pebble_bed::zbs::ZbsBed::wall_region_porosity` models the near-wall
region as `ε(y) = ε_bulk · (1 + 1.36 exp(−5y/d))`. Its own doc comment already
flags the coefficients as provisional and "not for quantitative wall-region
V&V". The measurement shows the problem is worse than provisional coefficients:

| `y/d` | measured | ZBS placeholder |
|---|---|---|
| 0.05 | 0.8753 | 0.9120 |
| 0.55 | **0.2631** | 0.4814 |
| 1.05 | 0.5533 | 0.4461 |
| 1.45 | 0.3209 | 0.4433 |

The **sign of the error flips** between `y/d = 0.55` and `1.05` — too high at
the minimum, too low at the maximum. A positive decaying exponential times
`ε_bulk` can never dip below `ε_bulk`, and the measured profile reaches 0.263.
**The disagreement is the shape, not the coefficients, so it cannot be fixed by
refitting.** A DEM bed is the natural source of the right profile. Recorded
here; changing `tampines` is a separate change with its own V&V and is not made
on the strength of this.

### Corroboration at HTR-10 geometry (`D/d = 30`) — shape only

The `D/d = 6` reference bed is three pebble diameters in radius, so it cannot
show the oscillation damping out. A second bed was settled at **HTR-10
geometry** to check that it does: 27 000 pebbles of `d = 60 mm`, `ρ = 1760
kg/m³` (A3-3 graphite), `R = 0.9 m`, softened `E = 10 MPa`, `dt = 200 µs`,
settled to `KE = 0.61 J` over 7 000 steps in **892 s** on one core. Settled bed
height 2.01 m against HTR-10's published 1.97 m.

| `y/d` | `ε` | |
|---|---|---|
| 0.05 | 0.8612 | wall bin |
| 0.65 | 0.3013 | first minimum |
| 0.95 | 0.4890 | first maximum |
| 1.35 | 0.2814 | second minimum |
| 1.85 | 0.4043 | second maximum |
| 2.25 | 0.3347 | third minimum |
| 2.75 | 0.3858 | third maximum |

Peak-to-trough amplitude `0.188 → 0.123 → 0.051`: **the oscillation damps into
the bulk within about three diameters**, which is the behaviour `D/d = 6` was
too narrow to exhibit. That is what this run was for.

**Its bulk porosity is NOT quotable, and the reason is worth recording.** The
clean radial window `4 < y/d < 10` gives `ε = 0.3747` and the axial interior
`5 < z/d < 28` gives `0.3691` — mutually consistent, but `ε ≈ 0.37` is
`φ ≈ 0.63`, essentially the random-close-packing limit and denser than this
crate's own `D/d = 6` result (0.4429) or HTR-10's design voidage. The residual
radial swing of **0.090** at four-to-ten diameters from the wall is the tell: a
genuinely random packing is flat there. **The initial condition was an ordered
lattice, and the bed has retained part of that order rather than randomising.**

So this run corroborates the near-wall *shape* and nothing else. A quotable
bulk porosity needs a randomised initial condition — poured insertion, as
`in.pebble_bed` does for the `D/d = 6` case — and that has not been run at this
scale. The map is therefore **not** committed as a fixture; only the `D/d = 6`
map is.

### What this does NOT establish

- **No published radial-voidage correlation is in `crates/kovan-literature`.**
  Mueller (1992), de Klerk (2003), Benenati and Brosilow (1962) and the rest
  are the obvious quantitative gate and none of them is catalogued, so the test
  asserts structural properties and a self-regression only. Quoting
  coefficients from memory would be fabrication. **Cataloguing one of those
  papers and adding a quantitative gate is the single highest-value follow-up
  to this section.**
- **`D/d = 6`.** The bed radius spans three pebble diameters, enough to resolve
  the wall peak and two oscillations, **not** enough to watch the profile damp
  to bulk. HTR-10 is `D/d = 30`.
- The rise in the innermost two bins (`y/d = 2.85, 2.95`) is the cylinder axis,
  a special site at this `D/d` with a tiny sampling annulus. Not asserted, not
  physics to quote.

---

## 3.2 Radial distribution function — random packing, or a crystal?

Section 3.1 maps *where* the voids are. This maps **how the pebbles sit
relative to each other**, which is a different question and the one that
decides whether a densified bed is still a random packing.

Two beds can share a solid fraction and be structurally different. `φ` cannot
distinguish a genuinely random packing from one that has begun to crystallise
into close-packed layers, and for reactor work the distinction matters: ordered
regions open straight coolant paths and straight neutron streaming paths that a
random bed does not have. This is the standard instrument for telling them
apart.

**Methodology.** `g(r)` is the density of other pebble centres at separation
`r`, normalised by a uniform arrangement of the same mean density. Implemented
in `src/rdf.rs`, exercised by `tests/htr10_rdf.rs`, out to `r_max = 5 d` in
250 bins of `d / 50 = 1.2 mm`.

The normalisation is the whole difficulty, and it is handled by **erosion
rather than correction**: only pebbles at least `r_max` from every boundary
serve as shell centres, so every shell lies wholly inside the bed and the
`4πr²dr` normalisation is exact with no boundary term. All pebbles remain
available as *neighbours*; the number density is measured over the same eroded
region. The domain is taken as the cylindrical core only (`z ≥ 0`), since the
discharge conus is not a cylinder and treating it as one would break exactly
the guarantee the estimator depends on.

**Results (measured 2026-09-17, settled flat-floor bed, 27 554 pebbles,
9 147 shell centres).**

| quantity | ours | LIGGGHTS |
|---|---|---|
| contact-peak `g` | **17.635** | **17.588** |
| contact-peak position | `0.990 d` | `0.990 d` |
| contact coordination number | **8.16** | **8.16** |
| closest approach | `0.980 d` | `0.980 d` |
| `g` at large `r` (estimator check) | **0.9985** | **0.9985** |

Three things are worth separating here.

**The estimator is verified, not assumed.** `g → 0.9985` at large `r` is a
property of the *method*, not of the bed: it is what the normalisation must
return, and getting it to within 0.15 % without any boundary correction is the
evidence that the erosion approach works. A naive un-eroded estimator sags well
below 1 for purely geometric reasons.

**The two codes agree on structure, not merely on density.** Identical
coordination number to three significant figures and identical peak position;
the peak heights differ by 0.27 %. This is a stronger statement than the `φ`
agreement in § 4.7, because `φ` is one scalar and this is the whole
pair-correlation curve.

**These are SOFT spheres, and `g(r)` measures it.** The first run of this suite
asserted `g(r) = 0` for all `r < d` on hard-sphere reasoning and **failed**:
`g(0.0594 m) = 17.59`, i.e. pairs 0.6 mm closer than touching. That is not a
defect, it is the model — the standard DEM softening to `E = 5e8 Pa` from
graphite's ~9 GPa, whose maximum contact overlap § 4.7 independently measures
at **1.71 % of the radius**, giving a closest approach of `0.9914 d`. The
measured closest approach, `0.980 d` at the 1.2 mm bin resolution, is
consistent with that independently derived bound.

That makes the near-contact part of `g(r)` a **second, independent check on the
stiffness simplification**: § 4.7 guards it with a single extreme value, while
this is the whole distribution of overlaps across the bed.

**Random packing, or a crystal? Measured: random, in every bed.** This is the
question `g(r)` was added to answer, and the diagnostic is the **second
shell**. A random close packing shows a *split* second peak at `√3 d ≈ 1.732 d`
and `2 d`; an FCC or HCP crystal instead puts a peak at `√2 d ≈ 1.414 d`.
Measured over the committed dataset:

| bed | `g` at `√2 d` (FCC) | `g` at `√3 d` | `g` at `2 d` |
|---|---|---|---|
| LIGGGHTS settled, flat floor | **0.690** | 1.213 | 1.515 |
| ours settled, flat floor | **0.691** | 1.214 | 1.518 |
| LIGGGHTS conus | **0.662** | 1.211 | 1.542 |
| ours conus, `µ = 0.4, µ_r = 0.1` | **0.644** | 1.201 | 1.546 |
| ours conus, `µ = 0.1, µ_r = 0` (`φ ≈ 0.61`) | **0.639** | **1.352** | **1.660** |

`g(√2 d) ≈ 0.64–0.69` is a **trough**, not a peak — there is no FCC signature
anywhere — while `√3 d` and `2 d` both stand above 1. Every bed here is a
genuine random packing.

**The densest bed is the most strongly ordered *and* the least crystalline**,
which is the interesting part. The `µ = 0.1, µ_r = 0` bed that reaches the
published 0.61 has the **highest** second-shell peaks of the set (1.352 and
1.660 against ~1.21 and ~1.54) and simultaneously the **lowest** `√2 d` value.
It densifies by packing more tightly into the same random local geometry, not
by crystallising. That matters for the reactor question: a bed that reached
0.61 by ordering would open straight coolant and neutron-streaming paths, and
this one does not.

**A caveat on the coordination number, which is the weakest figure quoted
here.** `Rdf::contact_coordination` integrates `g` out to the **first local
minimum after the contact peak**, which is the conventional definition but is
sensitive to where that minimum is detected: the minimum is shallow, so bin
noise can move it by a bin or two and the cumulative count with it. The two
flat-floor beds agree exactly (8.16 vs 8.16) because they are nearly the same
configuration, but across *different* beds a difference of a few tenths should
not be read as structural. The conus figures — 10.09 for LIGGGHTS against 9.20
for this port at the same friction — are additionally **not taken at the same
state**: LIGGGHTS ran a fixed 20 000 steps while this port pre-settled
adaptively to a kinetic-energy threshold, so the two beds are at different
points on the same relaxation curve. That difference is not a cross-code
disagreement and must not be quoted as one.

The **peak height and peak position**, and the `√2 d` / `√3 d` / `2 d` contrast
above, are read straight off the histogram and carry no such estimator
sensitivity. Prefer them.

**The dataset.** `reference-data/liggghts/htr10_rdf.csv`, one tidy row per
(case, bin): `case, mu, mu_r, stage, n_pebbles, n_centres, r_m, r_over_d, g,
coordination, counts`. Every bed the HTR-10 study produces is written with
identical settings, so the settings are directly comparable and plottable
against each other.

**What is NOT claimed.** No published `g(r)` for a pebble bed is catalogued in
`crates/kovan-literature`, so this is a cross-code and self-consistency result
only — it is **not** validated against a measured pair-correlation function.
The split second peak at `√3 d` / `2 d` versus a peak at `√2 d` is the
signature that separates random from crystalline packing, and the committed
dataset resolves it (16 bins between the sub-peaks), but no assertion encodes a
verdict: which bed is which is a measurement this dataset exists to report, not
a property asserted in advance.

## 3.3 Compute backends and run-to-run reproducibility

**A defect found while parallelising, and it invalidates a reproducibility
claim this document used to rest on.**

`GranularSystem::compute_forces` enumerates candidate pairs from a spatial cell
grid. That grid was a `std::collections::HashMap`, iterated directly — and
`HashMap` seeds its hasher **randomly per process**. Since `forces[i] += …` is
a floating-point accumulation and addition is not associative, the pair order
changed on every run and so did the last bits of every force.

**Measured**, three identical 200-step runs of the settled HTR-10 bed:

| run | kinetic energy `[J]` |
|---|---|
| 1 | `2.81519841188424304e-2` |
| 2 | `2.81519841188418857e-2` |
| 3 | `2.81519841188432977e-2` |

Differing in the 13th significant figure after 200 steps, in a system that is
chaotic over 50 000. The committed `htr10_settled_ours.csv` could therefore
**not** be regenerated exactly, and the single 2.91e-2 m outlier in the
per-particle cross-code comparison (§ 4.7, 49 % of a pebble diameter against a
median of 61 µm) is consistent with this mechanism. Bead `op-t3l.9`.

The grid is now a flat, counting-sorted cell list visited in ascending linear
index, which is both deterministic and faster. After the change, repeated runs
give an identical bitwise checksum.

**Backends.** `ComputeType` (`src/compute.rs`) selects `CpuSingleThread`,
`CpuMultiThread(ThreadCount)` or `Gpu`, mirroring
`outram_mc_libs::physics::compute::ComputeType` so the two pillars of the
pebble-bed workflow present the same selector.

**One semantic is deliberately stricter here than in the Monte Carlo pillar.**
There, the multi-thread backend agrees with the reference only within
statistical uncertainty. Here it must be **bit-identical**, because this
crate's entire verification claim is bit-identical agreement with LIGGGHTS; a
backend that perturbed the last bits would silently convert an exact claim into
a tolerance. The force loop therefore evaluates contacts concurrently but
accumulates them **in the serial contact order**.

Measured on the settled HTR-10 bed (27 554 pebbles, 12-core host, 200 steps):

| backend | ms/step | checksum |
|---|---|---|
| before this work | 55.95 | *(varied run to run)* |
| `CpuSingleThread` | **18.34** | `45913cc9e4c58660` |
| `CpuMultiThread(12)` | **12.85** | `45913cc9e4c58660` |

**The bit-identity claim is tested, not merely measured once.**
`tests/compute_backend_equivalence.rs` asserts it on a settling ensemble by
XOR-folded IEEE-754 checksum — so a single-ulp difference fails — across both
tangential models (`History` and the stateless `NoHistory`, which take
different paths through the shear store), thread counts **2, 7 and 12** (7
deliberately does not divide the work evenly, so a chunk-boundary ordering bug
cannot hide), and both sides of `BRUTE_FORCE_THRESHOLD`. All pass.

**The parallel speedup is modest, and the reason is structural rather than an
implementation shortfall.** Bit-identity requires an ordered accumulation, and
that phase cannot be parallelised. Measured breakdown at 12 threads: contact
arithmetic 3.7 ms (parallel), ordered accumulation ~3 ms, cell build 4.6 ms,
walls 1.3 ms, begin/end-step 0.6 ms — so roughly 9 ms of every step is
inherently serial and Amdahl's law caps the gain. Most of the 3.05x from the
single-thread column came from two defects rather than from threads: the
per-step `HashMap` rebuild above, and a `ShearHistory` split across two
SipHash-ed maps that cost two lookups and two inserts per contact per step
(15.2 ms of a 33.6 ms step, merged into one map with an FxHash-style integer
hasher).

**The DEM timestep has no GPU path and is not expected to gain one.** The
expensive inner loop carries a persistent per-contact shear history with
contacts born and dying every step — a stateful gather/scatter structure that
is the worst shape for a shader — while the Hertz arithmetic that *would* port
well is only 3.7 ms of an 18 ms step. Selecting `ComputeType::Gpu` for a
timestep is not an error; it runs the CPU path.

**The RDF histogram does have a real GPU kernel**, because it is the opposite
shape: 3.8e8 independent, stateless distance evaluations over a fixed point
set. WGSL has no `f64`, so that kernel is `f32` and is **not** bit-identical to
the CPU reference. Measured on the settled bed: 5 331 843 binned pairs (CPU)
against 5 331 845 (GPU), with **180 bin-assignment differences, 3.4e-5 of all
pairs**. Both effects are `f32` rounding — 180 pairs sitting within `f32`
resolution of an internal bin edge, and 2 within it of the `r_max` cutoff.
**Every number quoted in this document comes from the CPU path.**

## 4. Defects found and corrected

### 4.1 `Particle::integrate` is not symplectic (its docs said it was)

The method applies the **same** acceleration `a(t)` to the position and the
velocity update. For a linear restoring force `a = −ω²x` the one-step Jacobian is

```
M = [ 1 − ω²dt²/2    dt ]        det M = 1 + ω²dt²/2  >  1
    [ −ω²dt           1 ]
```

so phase-space volume, and the energy of a conservative oscillator, grows
geometrically — regardless of how well resolved the step is. A DEM contact
spring is exactly such an oscillator. The doc comment claimed the method was
"velocity-Verlet", "symplectic", and that it "does not secularly drift the
energy". All three were wrong.

**Measured.**

| Test | Result |
|---|---|
| unit oscillator, `dt = 0.1/ω` (63 steps/period), 20 000 steps | `E/E₀ = 2.13e+43` (velocity-Verlet: `0.99969`) |
| Jacobian determinant, finite-differenced | `1.005000` vs predicted `1 + ω²dt²/2 = 1.005` |

**Practical consequence** — realised restitution of a *perfectly elastic*
(`e = 1`) Hertz collision, where any energy change must come from the integrator:

| `dt` [s] | `e` (velocity-Verlet) | `e` (single-shot) | excess |
|---|---|---|---|
| `1e-6` | 1.000000 | 1.003142 | +0.314 % |
| `2e-6` | 1.000000 | 1.006297 | +0.630 % |
| `5e-6` | 1.000002 | 1.015830 | +1.583 % |
| `1e-5` | 1.000012 | 1.031967 | +3.197 % |
| `2e-5` | 1.000038 | 1.065095 | +6.509 % |

Restitution above 1 means the collision *manufactures* energy.

**Why it survived.** The scheme is **exact** for a constant force, and every
test it had — free flight under gravity, constant-torque spin-up — was a
constant-force test.

**Where it is nonetheless adequate, stated honestly.** With dissipative contacts
(`e < 1`) at a well-resolved step, physical damping dominates the injection: a
3-sphere column at `e = 0.9`, `dt = 1 µs` settles under *both* schemes, its
kinetic energy decaying geometrically. The defect bites as `e → 1`, as `dt`
grows, and wherever a long-lived assembly must conserve energy.

**Resolution.** `Particle::integrate` is kept (it is right for a constant-force
kick) with a corrected doc comment. Contact dynamics now use
[`integrator::VelocityVerlet`](../src/integrator.rs), a translation of
LIGGGHTS' `fix_nve_sphere.cpp` kick–drift–kick, for which `det M = 1` to
`1e-10`.

### 4.2 `contact.rs`: one real divergence, fixed — and one claim of mine that was wrong

**Status: fixed and verified 2026-09-16.**

The stateless path is now checked against the upstream model it actually
implements — `pair_style gran model hertz **tangential no_history**` — in
`tests/legacy_path_cross_code.rs`. Measured over 251 frames: positions exact,
`max|Δv| = 1.11e-16 m/s` (1 ulp), `max|Δω| = 1.42e-14 rad/s` (≈3 ulp at
`ω_z ≈ 41.6`). Round-off agreement.

What was actually wrong, and what was not:

1. **Contact-radius lever arm — a real defect, fixed.** `contact.rs` used the
   particle radii `r_i`, `r_j` for the surface-velocity moment and the torque
   arm where upstream uses the contact radii `c_r = r − δ_n/2`
   (`surface_model_default.h`). It now uses `c_r`. The error was `O(δ_n)`, 2 %
   at `δ_n = 2e-4 m` on `r = 5e-3 m`.
2. **Integrator — a real defect, fixed.** [`DemSimulation`] now runs
   `integrator::VelocityVerlet` instead of the non-symplectic single-shot
   update. See § 4.1.
3. **"Damping branch" — NOT a defect. This was my error.** The 2026-09-15
   version of this document claimed `contact.rs` "adds tangential damping
   unconditionally and then caps the sum, where upstream adds it only while
   sticking". That was written by comparing `contact.rs` against upstream's
   **history** model, which is a different model. Against
   `tangential_model_no_history.h` — the one it implements — upstream computes
   `γ = min(γ_t, µ|F_n| / v_rel)` and applies `F_t = −γ v_tr`, i.e. it caps the
   damping at the Coulomb limit, which is exactly what `contact.rs` does. There
   was never a divergence here, and nothing was changed.

**The remaining, deliberate difference** is that `contact.rs` has no shear
history at all (`ξ_t ≡ 0`), so it is upstream's *no-history* model and not its
*history* model. That is a scope choice, not a defect — a stateless
force-from-a-snapshot API has nowhere to keep `ξ_t`. Use
[`crate::granular`] when history is needed, which for a packed bed is always.

### 4.3 `rolling.rs`'s CDT diverged from upstream in three ways — fixed

**Status: fixed and verified 2026-09-16.**

1. **Normal force.** Scaled the torque by the **total** `|F_n|`, including the
   viscous damping term; upstream CDT uses the **elastic** part only,
   `k_n·δ_n`. Measured 21 % apart in the equivalence-test configuration.
2. **Torsion.** Did not remove the component of the resisting torque along the
   contact normal. Upstream removes it unless `torsionTorque` is explicitly
   enabled, and it defaults **off**.
3. **Wall branch.** The caller passed `ω_i − ω_j`; upstream uses the
   contact-point rolling velocity `w_r = c_r ω_i / r` for a wall.

All three are now upstream's. `RollingModel::rolling_torque` takes the elastic
normal force and the contact normal, and carries upstream's `torsion_torque`
switch (default off).

**Verified by transitivity.** `granular::RollingModel::Cdt` is bit-identical to
upstream over a 201-frame trajectory, so `rolling.rs`'s CDT is checked against
*that* rather than against a second reference dataset
(`rolling.rs::cdt_agrees_with_the_cross_code_verified_granular_implementation`,
agreement `< 1e-18 N·m` componentwise).

`RollingModel::ViscousRolling` is **not an upstream model** — LIGGGHTS ships
`cdt`, `epsd`, `epsd2`, `epsd3` and `luding`, and a pure linear rolling dashpot
is not among them. It is a clean-room addition and is labelled as such; it is
**not** covered by any cross-code verification.

### 4.4 The `Particle::integrate` call site is gone

`Particle::integrate` itself is kept — it is exact for a constant force and
that is a legitimate thing to want — but **nothing in the crate integrates
contacts with it any more**. Both engines (`DemSimulation` and
`GranularSystem`) run `integrator::VelocityVerlet`. Its doc comment carries the
measured evidence so the next reader cannot mistake it for velocity-Verlet.

## 4.5 Angle of repose — three invalid attempts, then a faithful one

**Superseded 2026-09-16 by § 4.6.** This section is kept because the three
failures below are the useful part: each is a way of getting a plausible-looking
number out of a badly-posed experiment.

The canonical granular validation case, and the one most directly relevant to a
pebble bed: a heap of frictional spheres stands at a finite angle only because
of the tangential shear history and rolling resistance together.

Three LIGGGHTS setups were run on 2026-09-15 (upstream only — none reached a
port comparison, so **none of it is evidence about this crate's code**):

| Setup | Result | Why it failed |
|---|---|---|
| Pour 600 pebbles from a narrow region onto an open floor | 64 inserted, spread to a flat monolayer | insertion region too small; no confinement during the pour |
| Lifting cylinder, `lattice sc` column, `H/D = 4` | **did not move at all** (`z_max` 0.2721 → 0.2723 m over 3 s after the wall was removed) | a perfectly symmetric lattice column has no lateral force, so a deterministic run sits in its unstable equilibrium forever. A real pour has symmetry-breaking that a lattice does not. |
| Lifting cylinder, random packing, `H/D ≈ 4` then `≈ 1` | surface slope **2.70 deg** then **9.88 deg**; material spread to `r = 0.39 m` and `0.33 m`, some leaving the domain | removing a primitive wall *instantaneously* lets the outer particles leave ballistically. This measures a collapse/splash, not repose. |

The diagnosis that led to § 4.6: the cylinder must be **lifted slowly**, and in
LIGGGHTS that requires a **moving mesh**, because a LIGGGHTS *primitive* wall
cannot move at all — `fix_wall_gran`'s `shear` imposes a tangential surface
velocity without translating the geometry. Every attempt above removed the wall
instantaneously, which is a collapse experiment.

**None of the three numbers above should be quoted.** They are properties of a
badly-posed numerical experiment, not of the contact model.

## 4.6 Angle of repose — done faithfully, and verified

**Status: the case now runs in both codes and a heap forms in both.**
`tests/angle_of_repose.rs`. ~~`#[ignore]`d (~25 min).~~ **CORRECTED 2026-09-18**
— it is not unconditionally `#[ignore]`d: it is
`#[cfg_attr(not(feature = "long-tests"), ignore = ...)]`, so a plain
`cargo test` **runs** it and only `--no-default-features` skips it. Measured
runtime **2313 s (38.5 min)** on 2026-09-16, not ~25 min.

### Faithful to upstream's own mechanism

A LIGGGHTS **primitive** wall cannot move: `fix_wall_gran`'s `shear` imposes a
tangential surface velocity without translating the geometry. So a lifting
cylinder must be a **mesh**, driven by `fix move/mesh`, exactly as in
`examples/LIGGGHTS/Tutorials_public/movingMeshGran`. Both codes read the *same*
geometry file, `reference-data/liggghts/lift_cylinder.stl` (`R = 0.050 m`, 1280
facets, inward normals) — LIGGGHTS via `fix mesh/surface file`, this crate via
`MeshWall::from_ascii_stl`.

Cylinder filled by repeated insertion (upstream's own `insert_every` pattern),
656 pebbles `d = 10 mm`, `E = 5 MPa`, `ν = 0.3`, `e = 0.5`, `µ = 0.5`,
`µ_r = 0.1` (CDT), `dt = 5 µs`. Both codes start from LIGGGHTS' settled state so
they integrate the identical configuration; the cylinder is then raised at
**0.02 m/s** for 4.0 s and the heap rests for 1.5 s.

### Results (2026-09-16, ~~superseded~~) and 2026-09-18 (current)

| Quantity | this crate | LIGGGHTS | difference |
|---|---|---|---|
| angle of repose | ~~12.78 deg~~ **14.09 deg** | **15.43 deg** | ~~2.65 deg~~ **1.34 deg** |
| heap apex | ~~0.0347 m~~ **0.0390 m** | 0.0370 m | **5.4 %** (ours now ABOVE) |
| residual `KE` | ~~2.8e-10 J~~ **6.412e-11 J** (settled) | `9.0e-11 J` (settled) | — |
| particles | 656 (none lost) | 656 (none lost) | — |

**CORRECTED 2026-09-18 — `12.78 deg` came from the NONDETERMINISTIC BUILD and
is not reproducible.** The neighbour grid was a `std::collections::HashMap`
until `7fb86bf58f` (bead `op-t3l.9`); its hasher is seeded randomly per
process, so force-accumulation order — and the last bits — differed on every
run, in a system integrated over 1 100 000 steps. Re-running the unchanged
committed test on the fixed build gives **14.09 deg**, measured twice
independently (548 s and 1209 s runs, identical to 2 d.p.).

Two hypotheses for the shift were tested and eliminated before settling on the
build: the fit filter (`min_count` 3 vs 8 gives 14.092 vs 14.09 — no material
difference), and the mesh itself (see the resolution sweep below).

**So the gap HALVES, and the previous record UNDERSTATED the port's
agreement.** The test's `3 deg` bound was described as one the measurement
"only just clears"; at 1.34 deg it now clears with better than 2x margin.

### Determinism verified, and what it implies for every PRE-FIX number

**The current build is reproducible; the one that produced `12.78` was not.**
Two independent processes, same reference STL, same settings:

| run | angle | apex | flank |
|---|---|---|---|
| process 1 | 14.092 deg | 0.03904 m | 0.10500 m |
| process 2 | 14.092 deg | 0.03904 m | 0.10500 m |

Identical to the printed precision, where the pre-`op-t3l.9` build could not
regenerate its own committed bed.

> **Provenance note, 2026-09-18.** The two rows above were read from a pair of
> concurrently-launched processes whose output files were later found to be
> ambiguously written (one empty, one holding two lines), so a clean re-run was
> commissioned to confirm them. Treat the table as **provisional** until that
> re-run is recorded here. The independent evidence for determinism that does
> NOT depend on it: the committed test was run twice at different times, on
> different core sets, and returned 14.09 deg both times (548 s and 1209 s),
> and `7fb86bf58f` itself reports byte-identical results across processes
> through a 14 000-step slump in all four settings.

**That makes the fix commit a dividing line for the whole cross-code table.**
`7fb86bf58f` (2026-09-18 00:28) replaced the randomly-seeded `HashMap`
neighbour grid with a counting-sorted flat cell list. Every number recorded
before it was produced by a build whose force-accumulation order varied per
process. Recording commits, all confirmed ancestors of the fix:

| number | recorded in | exposed? |
|---|---|---|
| angle of repose `12.78` | `ad8e99cf4b` | **yes — confirmed wrong, now 14.09** |
| bulk packing `0.5571` | `5bbe5fd7bf` | yes — **re-measured `0.5570`, survives** |
| HTR-10 settled `0.5732` | `bf64272dc0` | yes — **re-measured `0.5732`, IDENTICAL** |
| HTR-10 per-particle `61 um` median | `1db4e166b5` | yes — **not re-measured, see below** |
| stiffness sweep `0.5811/0.5754/0.5732` | `f21b0dbfb8` | yes |
| porosity profile `0.8753 / 0.2631 / 0.5533` | `36ce18e37a` | **no** |
| deterministic contact cases (bit-identical, 1-3 ulp) | `5bbe5fd7bf` | **no** |
| `0.6047` friction, RDF `8.16`, conus `11 um` | `7fb86bf58f` | post-fix |

**"Pre-fix" is not the same as "exposed."** The defect needs three or more
simultaneous contacts, so that summation order matters, AND enough steps for
chaos to amplify. The porosity profile reads
`reference-data/liggghts/pebble_bed_settled.csv` — **LIGGGHTS' own bed** — with
a deterministic estimator, so our nondeterminism never enters it. The
deterministic contact cases are two particles and one pair, where there is no
summation ambiguity at all; their measured bit-identity is itself proof of
determinism for that path.

**The principle that falls out, and it is the useful part: BULK,
SELF-AVERAGING quantities survive this defect; SHAPE and
individual-configuration quantities do not.** A packing fraction averages
hundreds to tens of thousands of pebbles and absorbs last-bit perturbations —
the bulk bed moved `0.5571 -> 0.5570`, i.e. 0.0001. An angle of repose is
fitted to the free surface of a heap flank and follows individual pebbles — it
moved 1.31 deg. That predicts which remaining numbers need re-measuring without
re-running them all, and it is why the HTR-10 `0.5732` (bulk) is expected to
hold while the `61 um` per-particle median (individual configuration) is not.
The crate already half-admits the latter, calling the single 2.91e-2 m outlier
"consistent with this mechanism".

**Both halves of that prediction were then tested, and the first one held
exactly.** Re-running `htr10_pebble_bed.rs` on the fixed build (2026-09-18,
491.85 s, 27 554 pebbles) gives `phi = 0.5732` against LIGGGHTS' `0.5732` and a
bed top of 2.1686 m against 2.1687 m — **identical to the recorded values**. So
the headline HTR-10 row, the one this crate's maturity roster calls "the one
that matters for the pebble-bed work", is **unaffected** by the determinism
defect, as a bulk quantity should be.

**The `61 um` per-particle median is NOT re-measured and should be treated as
provisional.** It is the one number in the table whose exposure the principle
predicts, and re-running the test **regenerated
`reference-data/liggghts/htr10_settled_ours.csv`** — so a per-particle
comparison now runs against a different, and for the first time
*reproducible*, bed. Re-measuring it is follow-up work; see the bead.

### The stated mechanism is now UNSUPPORTED (2026-09-18)

The `2.65 deg` gap was attributed — before the run, which is what made it a
hypothesis rather than a rationalisation — to LIGGGHTS' `TriMesh` resolving a
particle touching several facets at a shared edge where this crate's
`MeshWall` takes only the **single nearest facet**. On the reference cylinder a
pebble spans ~2.55 facets, so nearly every wall contact straddles an edge.

That mechanism is a **real code difference** and is not in dispute. What is now
unsupported is that it **explains the residual gap**. Three independent lines
were run and none supports it.

**1. A mesh-resolution sweep** (`examples/repose_mesh_convergence.rs`),
cylinders regenerated at 40/80/160 segments with identical material, lift and
rest — only the mesh differs. The prediction was recorded in that example's
docs *before* measuring: coarsening toward facet-width ~ pebble-diameter should
RAISE the angle toward 15.43.

| mesh | segments | facet width | pebble spans | angle |
|---|---|---|---|---|
| `cyl_40` | 40 | 7.854 mm | 1.27 | 12.952 deg |
| `cyl_80` | 80 | 3.927 mm | 2.55 | 14.358 deg |
| reference STL | 80 | 3.927 mm | 2.55 | **14.092 deg** |
| `cyl_160` | 160 | 1.963 mm | 5.09 | 14.413 deg |

**The control is the useful row.** Two nominally equivalent 80-segment meshes —
the committed reference and a regenerated one differing only in rotational
phase — give 14.358 against 14.092. So this case's **realisation scatter for
equivalent geometry is 0.266 deg**, an error bar the sweep previously lacked.

Against it: refining 80 -> 160 (facet width halved, spans 2.55 -> 5.09) moves
the angle **+0.055 deg = 0.21x scatter, i.e. FLAT**. Coarsening 80 -> 40 moves
it **-1.406 deg = 5.3x scatter** — real, but in the **wrong direction** for the
mechanism. **The prediction is refuted.** The 40-segment drop is consistent
with the shape confound flagged in advance: a 40-gon has flat walls and 9 deg
corners that seat pebbles differently, which is geometry, not facet ratio.

**2. The conus comparison does not corroborate it either, because it is
confounded by run length.** The argument was that the conus/discharge case has
facets 69.94 mm (pebble spans 0.14) and agrees to 11 um median over 27 554
pebbles, while this cylinder has 5.90 mm facets (spans 1.69) and is the one
discordant row — same code path, ratio an order of magnitude apart, agreement
tracking it. But **the conus 11 um is measured at 2 000 steps and this case
runs 1 100 000** — 550x longer — and § 4.8's own table shows the conus
degrading to 1.10 mm by 6 000 steps and 3.55 mm by 12 000. Run length is a far
more parsimonious explanation than facet ratio, so that comparison shows only
that a discrepancy has not amplified in 2 000 steps, not that none exists.

**Next discriminating measurement**, since nothing run so far separates the
two: re-run the conus out to a step count comparable with this case, or re-run
this case truncated to a few thousand steps. Porting upstream's multi-facet
resolution would settle it directly, but that is real work and should not be
started on the strength of a mechanism with no supporting measurement. Beads
filed 2026-09-18.

### The optimisation defect this case exposed

The first run of this case produced a **collapsed monolayer** — apex `0.0100 m`
against LIGGGHTS' `0.0370 m`, `KE = 5.6e-2 J` and still rising after the rest
phase. The cause was not physics: `MeshWall`'s bounding-sphere pruning caches
facet centroids and a whole-mesh hull, and `WallGeometry::translate` moved the
facets **without moving the caches**. A stale pruning cache is *not*
conservative — it prunes against the old positions and silently drops facets
the particle is genuinely touching — so the lifting cylinder stopped confining
anything.

The pruning had been checked against an unpruned scan and found bit-identical,
but only on a **static** mesh, which is why the defect survived. It is now
fixed (caches translate with the geometry, and are rebuilt after a rotation)
and covered by `pruning_stays_exact_after_the_mesh_moves`, which moves the wall
200 steps plus a rotation and requires exact agreement with an unpruned scan
over the *current* facet positions. That regression test was confirmed to fail
without the fix before being trusted.

## 4.7 HTR-10 full core — the geometry that actually matters

**The verification case for the pebble-bed work**, because it is the geometry
every downstream consumer uses: 27 000 pebbles at `D/d = 30`, not 354 at
`D/d = 6`. `tests/htr10_pebble_bed.rs`, gated behind `long-tests`.

### Geometry — the published design point

From IAEA-TECDOC-1382, as transcribed in
`Htr10DesignPoint::iaea_benchmark()` and used by `htgr_sim_v1`: core diameter
180 cm (`R = 0.90 m`), pebble diameter 6 cm, graphite `ρ = 1730 kg/m³`,
27 000 fuel elements, published filling fraction `f = 0.61`, bed height 197 cm.

The constants are **restated** in the test rather than read from
`outram-park-digital-twin-engine`, because depending on that crate would pull
`egui` into this crate's *test* build and tests are not exempt from the Android
rule (maintainer confirmed: egui must not enter `liggghts`). The drift a second
copy invites is caught by `htr10_geometry_matches_the_published_design_point`,
which closes 27 000 pebbles at `f = 0.61` against the published bed height:
implied `1.9672 m` vs `1.97 m`, **−0.14 %**.

### The stiffness simplification was set by measurement, not by estimate

This is the part worth reading. The standard pebble-bed softening reduces
graphite's `E ≈ 9 GPa` so the timestep (`∝ √(m/k)`) stays affordable, on the
argument that quasi-static packing is set by geometry and friction rather than
stiffness. The test does **not** take that on trust: it measures the maximum
contact overlap and asserts it against 2 % of the pebble radius.

That guard failed twice, and each failure moved the modulus:

| `E` | softening | measured max overlap | verdict |
|---|---|---|---|
| `1e8 Pa` | 90x | **3.80 %** of `r` | fails — hand estimate had said 1.07 % |
| `3e8 Pa` | 30x | **2.13 %** of `r` | fails, marginally |
| `5e8 Pa` | 18x | **1.71 %** of `r` | **passes** |

The opening estimate was out by **3.5x** because it used a single pebble's
weight where the real load is the ~2 m column above it. The lesson is the one
this document keeps relearning: an assumption that is only argued is not
checked.

**Crucially, the cross-code verification was unaffected throughout** — LIGGGHTS
runs the same soft material, and the two codes agreed to 0.02 % at `1e8` and to
four decimal places at `3e8`. What the too-soft modulus threatened was never
the port's correctness, only whether the bed is a fair stand-in for a real one.

One genuine physics note from the sweep: **packing fraction is not quite
stiffness-independent**. `φ` moved `0.5811 → 0.5754` between `1e8` and `3e8`,
about 1 %, because softer pebbles interpenetrate and read as denser. Small, but
it contradicts the usual justification's strict form and was only visible
because both stiffnesses were actually run.

One note on how that 1.71 % was reached, because the prediction was not a hit.
Before running it I predicted **1.51 %**, by scaling the `3e8` result with the
Hertz relation `δ ∝ (F/E)^(2/3)` — a factor 0.711. The measured factor was
**0.803**, so the prediction was 13 % low. The scaling assumes a fixed load,
but maximum overlap is an **extreme-value statistic over ~66 000 contacts**,
not a mean-field quantity: the single most-loaded contact in the bed is not
governed by the average. The estimate was good enough to decide how far to
raise `E`, and is not a relation to quote.

### Results at `E = 5e8` (measured 2026-09-17)

| quantity | ours | LIGGGHTS | published |
|---|---|---|---|
| bulk solid fraction `φ` | **0.5732** | **0.5732** | 0.61 (whole-core design closure) |
| bed top | 2.1686 m | 2.1687 m | 1.97 m (mean bed height) |
| max contact overlap | 1.71 % of `r` | — | — |
| final kinetic energy | 8.320e-4 J | 9.848e-5 J | — |
| wall-clock | 2714 s (45.2 min) | 816 s (400 + 416) | — |

`N = 27 554` pebbles, from LIGGGHTS' `fix insert/pack`, against the published
design count of 27 000 — the pack routine overshoots and always has.

Our final kinetic energy is ~8x LIGGGHTS'. In absolute terms both beds are at
rest: over 5391 kg of pebbles that is an RMS velocity of **0.56 mm/s** for this
port against **0.19 mm/s** for LIGGGHTS. Ours is marginally the less settled of
the two; nothing asserts on it.

**The per-particle agreement is the strongest result here**, and it is recorded
in [`cross-code-summary.md`](./cross-code-summary.md): the median pebble ends
**61 µm** from where LIGGGHTS puts it, and 99.93 % of the 27 554 land within
1 mm, after 50 000 independently integrated steps each.

**Runtime decides the gate, and it was measured, not inherited.** 2714 s puts
this test in the workspace's middle tier (5 minutes to about an hour), so
`long-tests` — default-on, skipped only under `--no-default-features` — is the
correct gate and is what it carries.

### The bed is looser than the benchmark's nominal figure

At `E = 5e8`, both codes settle to `φ = 0.5732` against the published `0.61`,
and the bed stands `2.169 m` tall against the published `1.97 m`. **This is not
a code disagreement** — the two codes agree with each other to four decimals.

Two things are being compared that are not the same quantity. The published
`0.61` is a *design closure*, 27 000 pebbles divided by a nominal 5.0 m³ core
volume, not a measured packing fraction; and the slab measurement here is
deliberately bulk-only. Beyond that, a DEM random packing at `µ = 0.4`,
`µ_r = 0.1` simply packs looser than 0.61 — reproducing the benchmark density
would need those friction parameters calibrated down. **Nothing here validates
either number**; it says what this contact model, at these parameters, produces.

> **Followed up and resolved in § 4.9 (2026-09-17).** The sentence above about
> friction was a conjecture when written; it has since been tested by a 2x2
> friction ablation and is **correct**. At `mu = 0.1, mu_r = 0` — the
> *literature* value for graphite-on-graphite, graphite being a solid lubricant
> — the whole-core closure reaches **0.6047 against 0.61, a gap of -0.9 %**.
> `mu` is the dominant control, worth `+0.0196` in `phi`, with `mu_r` worth a
> further `+0.007` to `+0.012`.
>
> This also **supersedes the "different quantities" framing** in the paragraph
> above: it is the *like-for-like* whole-core comparison that closes, so the gap
> was never mainly a measurement-definition problem. It was a parameter problem.
> The earlier retraction of that framing stands.

## 4.8 HTR-10 conus slump — cross-code on the DISCHARGE geometry

**Why this case was added.** Every HTR-10 result before it ran on the core as a
**flat-floored cylinder** — analytic primitive walls only. The recirculation
study replaces that floor with the published bottom conus and fuel discharge
tube, read from a **triangulated mesh**, plus a valve plane. That is new
geometry *and* a different contact path.

It also runs straight at this crate's weakest link. The angle-of-repose case
(§ 4.6) is the **only** other mesh-wall comparison here and is the worst row in
the whole cross-code table — ~~12.78° against 15.43°, a 2.65° gap~~
**14.09° against 15.43°, a 1.34° gap** (corrected 2026-09-18) attributed to
mesh-contact differences. If the mesh-wall path carries a real discrepancy, the
conus is exactly where it would contaminate the HTR-10 result.

**That reassurance is weaker than it reads, and the step counts are why.** The
11 um figure below is at 2 000 steps; § 4.6 runs 1 100 000. This case's own
table degrades to 3.55 mm by 12 000 steps. So a clean conus result bounds how
fast a mesh-contact discrepancy AMPLIFIES; it does not show there is none.
Recorded 2026-09-18. Concluding
anything from untested geometry would have been the mistake these rules exist
to prevent.

**Methodology.** Both codes start from the **identical configuration** —
LIGGGHTS' own settled flat-floor bed, `htr10_settled.csv`, 27 554 pebbles —
handed to LIGGGHTS through the new `csv2data.sh` converter. The flat floor is
removed; the conus + tube mesh (`htr10_discharge.stl`, 480 facets, **read by
both codes**) and the valve plane at `z = −0.61946 m` are attached; each code
integrates 20 000 steps of `dt = 3.5e-5 s` independently. No insertion, so
nothing depends on reproducing an RNG stream. Upstream deck: `in.htr10_conus`;
test: `tests/htr10_conus_cross_code.rs`.

**Results (measured 2026-09-17).**

| step | `φ` ours | `φ` LIGGGHTS | rel | Δ surface height | per-particle median | within 1 mm |
|---|---|---|---|---|---|---|
| 2 000 | 0.5646 | 0.5646 | **−0.00 %** | **+0.0 mm** | **1.10e-5 m** | **27 554 / 27 554** |
| 6 000 | 0.5759 | 0.5759 | −0.00 % | +0.1 mm | 1.10e-3 m | 12 755 / 27 554 |
| 12 000 | 0.5768 | 0.5768 | +0.00 % | +2.7 mm | 3.55e-3 m | 739 / 27 554 |
| 20 000 | 0.5769 | 0.5769 | **−0.01 %** | **+2.8 mm** | 3.82e-3 m | 592 / 27 554 |

**The mesh-wall contact path is verified.** At 2 000 steps — before divergence
has had time to act — the median pebble is **11 µm** from where LIGGGHTS puts
it and **every one of 27 554** is within 1 mm. Whatever ails the
angle-of-repose case, it is not a systematic error in mesh-wall contact
resolution at this scale.

**The later growth is Lyapunov divergence, not disagreement, and the
distinction is the point.** This case drains a 2.17 m column 17 cm down into a
funnel — a large rearrangement in which pebbles change neighbours. Dense
granular flow is chaotic, so two codes differing by one ulp at step 1 *must*
separate exponentially. What survives is the bulk statistics, and those agree
to four decimal places at every checkpoint, ending at **−0.01 %**.

This is a genuinely different situation from § 4.7, where the flat-floor bed
settles from an already-settled state — a small perturbation — and the median
pebble stays within 61 µm over *50 000* steps. The comparison instrument has to
match the physics: per-particle for a small perturbation, bulk for a large
rearrangement. The test asserts accordingly (tightly at the first checkpoint,
loosely at the last) and says so in its own docs, rather than quietly relaxing
a bound until it passed.

**Two of this document's own criteria were wrong and were corrected by
measurement**, both recorded in the test:

- *Per-particle median under 1 mm at the final state* — written by analogy with
  § 4.7, and wrong for a chaotic rearrangement, as above.
- *Bed top within 5 mm*, using **`max z`**. The two beds' single highest
  pebbles ended 5.0 mm apart against a 5.0 mm bar: a marginal failure driven by
  one pebble out of 27 554, while every bulk quantity agreed to four decimals.
  `max z` is a single-pebble statistic and after a chaotic rearrangement the
  highest pebble need not even be the same pebble. Replaced by the **99th
  percentile** of centre height, which 275 pebbles stand above and no single
  placement can move. The same defect was found independently in the
  recirculation driver, where re-inserting one pebble onto the current
  high point moved `max z` by a full diameter and was misread as pebbles
  stacking into towers.

## 4.9 The 0.61 question — a friction ablation, and what slow recirculation does

### The question this section closes

§ 4.7 leaves the central open problem of the HTR-10 work: both codes settle to
`φ = 0.5732` against the published filling fraction of **0.61**, and the
whole-core closure — the like-for-like comparison — is **worse**, not better.
That sign killed the "different quantities" explanation, which was retracted
there. Two hypotheses remained:

1. a single pour with friction lands at **random loose packing**, while a real
   core is continuously **recirculated**; or
2. the **friction parameters themselves** — `µ = 0.4` and CDT `µ_r = 0.1` —
   simply put random loose packing lower than 0.61.

These make opposite predictions and are separable by experiment, so they were
separated by experiment.

### Methodology — an ablation, deliberately not a calibration

A full **2×2 factorial** over the two friction knobs, each run from the
identical starting configuration (LIGGGHTS' settled bed) through the identical
protocol:

| | `µ_r = 0.0` | `µ_r = 0.1` |
|---|---|---|
| **`µ = 0.1`** | `mu10_mur00` | `mu10_mur10` |
| **`µ = 0.4`** | `mu40_mur00` | `mu40_mur10` |

`µ = 0.4` is the value this crate had been using. `µ = 0.1` is not chosen to
hit a target: **nuclear graphite is a solid lubricant**, and graphite-on-
graphite sliding friction is reported in the range 0.1–0.2, so 0.4 was always
the questionable end of the range rather than 0.1 being a tuned one. The
factorial covers both knobs at both ends and reports where each lands.

This distinction is the workspace's own rule (`CLAUDE.md`, "Model hierarchy"):
*never calibrate a free parameter until a comparison passes*, and any
calibration must be accompanied by ablation. Nothing here is fitted to 0.61 —
each cell is run and reported, including the cells that miss.

Driver: `examples/htr10_recirculation_sweep.rs`. Each run
(a) attaches the conus + tube + valve, (b) **pre-settles adaptively** until
kinetic energy per pebble falls below `1e-3` of a one-pebble-radius drop, then
(c) recirculates in batches of 50 — extract the lowest pebbles resting on the
valve, re-insert the same number at the top, settle, repeat — conserving pebble
count exactly.

**Two `φ` series are recorded, at two resolutions, because they answer
different questions.**

| file | resolution | question |
|---|---|---|
| `htr10_phi_vs_step_<label>.csv` | every **500 steps** | how does the bed evolve? |
| `htr10_recirc_<label>.csv` | every **batch** (50 pebbles) | what did each batch do? |

The step-resolved series exists because **most of the movement in `φ` happens
before the first batch**. Draining the flat-floor bed into the conus carries
`φ` from 0.5646 to its plateau over 14 000–16 000 steps, and a per-batch series
compresses that entire transient into a single number at `batch 0` — so the
shape of the slump, and whether each cell has actually reached a plateau rather
than still creeping, would be invisible. `--trace-only` runs just that phase.

Both files are flushed as they go (per chunk and per batch respectively) rather
than written on completion, so a run that dies does not discard an hour of
measurement.

### Two defects in the recirculation harness, found by running it

Both were recorded as blockers in GitHub issue #216 and are fixed here.

**Re-insertion launched pebbles.** Inserting at `top + 0.02` with 0.06 m
pebbles starts a pebble **overlapped by 0.04 m**, two-thirds of a diameter.
Hertz at `E = 5e8` gives `F ≈ 3.4e5 N` on a 0.196 kg pebble — `Δv ≈ 61 m/s` in
one 35 µs step. Observed: bed top climbing 1.99 → 3.14 m, `φ` collapsing
0.5764 → 0.3670. Fixed by **placing** pebbles on the local bed surface
(`GranularSystem::surface_height_at`, now a library method), which gives
exactly zero initial overlap by construction.

**Every batch reused the same 50 insertion positions.** The golden-angle spiral
was indexed within a batch, so batch *n+1* placed its pebbles directly on top
of batch *n*'s, growing 50 towers: bed top **+45 mm per batch** where pebble
conservation allows ~4 mm, driving `KE/E_drop` to 2.86e-2. Fixed by indexing
the sequence **globally** and drawing the radius from a base-2 radical inverse
(a plain modular ramp would put all 50 pebbles of a batch on one ring). After
the fix the bed top moves **+3.7 to +5.1 mm per batch**, as conservation
requires.

A third defect was found in the *measurement* rather than the harness: bed
height was reported as `max z`, a single-pebble statistic that jumps a full
diameter whenever a pebble is placed on the current high point. It is now the
**99th percentile** of centre height. The same defect existed in the cross-code
test (§ 4.8).

### Results — the settled baseline, before any recirculation

Every cell pre-settled into the conus to `KE/E_drop < 1e-3` (reached at
14 000–16 000 steps; the final values are 1.25e-4 to 1.60e-4, so these are
settled beds by measurement, not by a step count). Measured 2026-09-17,
27 554 pebbles, `E = 5e8 Pa`:

| `µ` | `µ_r` | `φ` bulk slab | **`φ` whole-core** | surface height |
|---|---|---|---|---|
| **0.1** | **0.0** | **0.6083** | **0.6047** | 1.845 m |
| 0.1 | 0.1 | 0.6016 | 0.5980 | 1.869 m |
| 0.4 | 0.0 | 0.5887 | 0.5848 | 1.917 m |
| 0.4 | 0.1 | 0.5770 | 0.5729 | 1.965 m |
| — | — | *0.5732* | — | *2.169 m* | (§ 4.7, flat floor, for reference) |

**The published 0.61 is reached.** At `µ = 0.1, µ_r = 0` the whole-core
closure — the *like-for-like* comparison with the benchmark's own figure — is
**0.6047 against 0.61, a gap of −0.9 %**, where § 4.7 recorded −8.7 % and
could not explain it. The bed stands 1.845 m against the published mean height
of 1.97 m.

**Hypothesis 2 is supported and hypothesis 1 is not needed.** The factorial is
clean and close to additive:

| effect | size in `φ` |
|---|---|
| `µ`: 0.4 → 0.1 | **+0.0196** at `µ_r = 0.1`, **+0.0196** at `µ_r = 0` |
| `µ_r`: 0.1 → 0.0 | +0.0117 at `µ = 0.4`, +0.0067 at `µ = 0.1` |

Sliding friction carries roughly twice the effect of rolling friction, and the
`µ` main effect is identical to four decimal places at both `µ_r` levels. The
gap to 0.61 was a **parameter** question, not a protocol question.

**Why `µ = 0.1` is a correction and not a fit.** Nuclear graphite is a solid
lubricant; graphite-on-graphite sliding friction is reported at 0.1–0.2. The
`µ = 0.4` this crate had been carrying was always the questionable end, and
0.61 is reached at the *literature* value rather than at a value chosen to
reach it. Nothing was tuned: all four cells were run and all four are reported,
including the two that miss by 3–6 %.

**A smaller effect worth recording:** the conus slump itself densifies the bed.
At identical parameters (`µ = 0.4, µ_r = 0.1`) the flat-floor bed of § 4.7
settles to `φ = 0.5732` with its top at 2.169 m, and the same bed drained into
the conus reaches **0.5770 at 1.965 m**. Draining into the funnel is itself a
rearrangement, and it compacts.

### Results — what slow recirculation actually does

Each cell recirculated **2 000 pebbles, 7.3 % of the bed**, in 40 batches of 50,
from its own settled baseline. Measured 2026-09-17:

| `µ` | `µ_r` | `φ` settled | `φ` after | **Δφ** | `φ` whole-core after |
|---|---|---|---|---|---|
| **0.1** | **0.0** | 0.6083 | **0.6108** | **+0.0025** | 0.6034 |
| 0.1 | 0.1 | 0.6016 | 0.6022 | +0.0006 | 0.5960 |
| 0.4 | 0.0 | 0.5887 | 0.5866 | −0.0021 | 0.5801 |
| 0.4 | 0.1 | 0.5770 | 0.5697 | **−0.0073** | 0.5633 |

**The answer is not "yes" or "no" — it is "it depends on friction, and the sign
flips".** Slow recirculation **densifies** a low-friction bed and **dilates** a
high-friction one, monotonically in total friction. This was not the expected
result: the hypothesis on record (GitHub issue #216) was that recirculation
densifies, full stop, and that it would explain the gap to 0.61.

The two knobs **compound rather than add**, unlike the settled baselines which
were near-additive: `µ`'s effect on `Δφ` is −0.0046 at `µ_r = 0` but −0.0079 at
`µ_r = 0.1`.

A physical reading, offered as interpretation and not as something measured
here: each batch disturbs the bed, and shearing granular material dilates
(Reynolds). Whether the bed then *recovers* that dilation depends on whether
pebbles can rearrange back into contact — which friction is precisely what
prevents. At `µ = 0.1` recovery wins; at `µ = 0.4` it does not.

**This does not rescue the 0.61 gap, and was never going to.** The effect is an
order of magnitude too small: recirculation moves `φ` by at most 0.007, while
the friction ablation moves it by 0.031. **Friction is the explanation; the
recirculation protocol is not.** That closes the question § 4.7 left open, in
the opposite direction from the leading hypothesis.

The densest bed does, however, end **above** the published figure on the bulk
measure: `φ = 0.6108` at `µ = 0.1, µ_r = 0`, against a quoted 0.61.

### Was it actually quasi-static? Measured, and the first answer was no

The whole case rests on the flow being **quasi-static**, and the test asserts
it rather than assuming it: mean kinetic energy per pebble must stay below
`1e-2` of a one-pebble-radius drop. **At the original 2 000-step settle window
it did not.**

| measure | worst value, 2 000-step settle |
|---|---|
| whole system | **3.78e-2** |
| **core only** (`z > 0`, excluding the discharge stream) | **1.57e-2** |

Two separate things were wrong, and both were found by measurement rather than
argument.

**First, the instrument.** The original ratio averaged over *every* pebble,
including those inside the conus and discharge tube. Those are being extracted
— they are *meant* to be moving — so they dominated an average that was
supposed to describe the bed. Excluding them cuts the figure by 2.4x.

This is not a way to hide motion, and that is checkable rather than asserted:
during the initial slump, when the whole bed genuinely is moving, the two
measures agree to 3 % (core 7.03e-1 against whole-system 7.24e-1 at step
2 000). They only separate once the core settles and the residual motion is
confined to the discharge region (core 2.36e-2 against 9.23e-2 at step 8 000).

**Second, and decisively, the protocol.** Even on the core-only measure,
1.57e-2 is **over the bound**. The bed was not fully relaxing between batches.
The fix is the protocol, not the threshold — raising the settle window from
2 000 to 8 000 steps gives:

| settle window | worst core KE/E_drop |
|---|---|
| 2 000 | 1.57e-2 — **fails** |
| **8 000** | **~2e-4** — passes by ~50x |

`SETTLE_STEPS` in `tests/htr10_recirculation.rs` is now 8 000. **Relaxing the
threshold to make the test pass was considered and rejected**: the bound is the
entire reason the case can claim to measure quasi-static creep rather than
avalanching, and a criterion moved to fit the result measures nothing.

### The rate-independence control — and what it revises

The 40-batch runs used the 2 000-step window, so they sat outside the regime
they claim. Quasi-static flow is **rate-independent** — packing is set by the
*sequence* of rearrangements, not their speed — so a genuine result must
survive a longer settle window. That is a falsifiable prediction, and it was
tested rather than assumed: two cells re-run at **8 000 steps** from the
**identical** settled beds, so the settle window is the only variable.

Both control runs are genuinely quasi-static (worst core `KE/E_drop` of
**2.23e-4** and **4.82e-4**, roughly two orders of magnitude inside the bound),
so these are the trustworthy numbers. Compared at the **same 500 pebbles**
recirculated:

| cell | `Δφ` @ 2 000-step | `Δφ` @ **8 000-step** | `Δφ` whole-core @ 2 000 | @ **8 000** |
|---|---|---|---|---|
| `µ = 0.1, µ_r = 0.0` | +0.00089 | **+0.0015** | −0.00245 | **+0.0001** |
| `µ = 0.4, µ_r = 0.1` | −0.00306 | **−0.0016** | −0.00491 | **−0.0027** |

**The physics survives; two of the numbers do not.**

- **The friction-dependent sign flip is REAL.** Under proper relaxation the
  low-friction bed still densifies (`+0.0015`) and the high-friction bed still
  dilates (`−0.0016`). This was the substantive claim and it holds.
- **The magnitudes were inflated.** The 2 000-step window roughly **doubled**
  the apparent dilation at `µ = 0.4`.
- **One 2 000-step number was an outright artefact.** The whole-core loss of
  `−0.00245` at `µ = 0.1` becomes `+0.0001` — essentially flat — once the bed
  is allowed to relax. It was measuring unrecovered disturbance, not packing.

**The 40-batch `Δφ` table above therefore overstates the effect**, and is kept
because it is what a 7.3 % turnover produced at that window, not because its
magnitudes should be quoted. **Quote the control.** The qualitative conclusion —
recirculation is an order of magnitude too weak to explain the gap to 0.61, and
friction is the explanation — is unchanged and is if anything strengthened, since
the quasi-static effect is *smaller* still.

**The 0.61 result survives quasi-static recirculation**: `µ = 0.1, µ_r = 0`
ends at a whole-core closure of **0.6048** against the published 0.61.

### Bed structure after recirculation — still random, and more so

`g(r)` for all **11** beds is committed in `htr10_rdf.csv`. The crystallinity
diagnostic (§ 3.2) on the full set:

| bed | `g(√2 d)` — FCC | `g(√3 d)` | `g(2 d)` |
|---|---|---|---|
| LIGGGHTS settled, flat floor | 0.690 | 1.213 | 1.515 |
| ours settled, flat floor | 0.691 | 1.214 | 1.518 |
| LIGGGHTS conus | 0.662 | 1.211 | 1.542 |
| ours conus `µ=0.4, µ_r=0.1` | 0.644 | 1.201 | 1.546 |
| ours conus `µ=0.4, µ_r=0.0` | 0.632 | 1.243 | 1.584 |
| ours conus `µ=0.1, µ_r=0.1` | 0.637 | 1.269 | 1.600 |
| ours conus `µ=0.1, µ_r=0.0` | 0.639 | 1.352 | 1.660 |
| ours **recirculated** `µ=0.4, µ_r=0.1` | 0.648 | 1.185 | 1.550 |
| ours **recirculated** `µ=0.4, µ_r=0.0` | 0.629 | 1.249 | 1.573 |
| ours **recirculated** `µ=0.1, µ_r=0.1` | 0.623 | 1.329 | 1.634 |
| ours **recirculated** `µ=0.1, µ_r=0.0` | **0.617** | **1.381** | **1.667** |

**No bed shows an FCC signature.** `g(√2 d)` is a trough everywhere,
0.617–0.691, while `√3 d` and `2 d` both stand above 1 — the split second peak
of a random packing, in all 11.

**The densest bed is simultaneously the most random-close-packed and the least
crystalline.** The `µ = 0.1, µ_r = 0` recirculated bed has the **highest**
`√3 d` and `2 d` peaks of the set and the **lowest** `√2 d` value. It reaches
0.61 by packing more tightly into random local geometry, not by ordering —
which is the physically important distinction for a reactor, since ordered
regions would open straight coolant and neutron-streaming paths.

### Reproducibility, verified at full scale

The `--trace-only` runs re-derive each settled bed from the flat-floor start,
in a fresh process, and the result was compared byte-for-byte against the bed
the sweep had cached:

```text
DETERMINISM mu40_mur10: byte-identical to the cached bed
DETERMINISM mu40_mur00: byte-identical to the cached bed
DETERMINISM mu10_mur10: byte-identical to the cached bed
DETERMINISM mu10_mur00: byte-identical to the cached bed
```

**27 554 pebbles, through a 14 000–18 000 step slump, byte-identical across
processes, in all four settings.** This is a far stronger check than the
200-step checksum of § 3.3, and it is the property that did **not** hold before
the neighbour-grid fix — the committed reference beds can now be regenerated
exactly. The regression test independently reproduced `φ 0.5766 → 0.5732` to
four decimals on two separate runs.

## 5. What is still NOT validated

Unchanged from the 2026-09-06 declaration, and not weakened by anything above.
Two items are **qualified** by the 2026-09-17 work and say so inline; nothing is
removed.

- **No experimental comparison.** Nothing here is compared against a measured
  pebble bed or a published granular benchmark. The angle-of-repose case
  (§ 4.6) is a **cross-code** comparison against LIGGGHTS, not a validation:
  both codes produce `13-15 deg`, which is well below the `25-35 deg` typical
  of real granular materials. That is a known consequence of perfectly
  spherical DEM particles with modest rolling friction, not evidence that
  either code is wrong — but it does mean **no repose angle from this work
  should be quoted as a validated material property**.
- **Mesh-wall contact is not upstream's.** `MeshWall` resolves a particle
  against the single nearest facet; LIGGGHTS' `TriMesh` resolves multi-facet
  edge contacts. ~~This is why § 4.6 agrees to `2.65 deg` rather than to
  round-off.~~ **CORRECTED 2026-09-18** — § 4.6 agrees to **1.34 deg**, and the
  causal claim is now UNSUPPORTED: a mesh-resolution sweep is flat within a
  measured 0.266 deg realisation scatter, and coarsening moves the angle the
  WRONG WAY. The code difference is real; that it explains the residual is not
  established. Porting upstream's resolution is outstanding work, but should
  not be started on the strength of a mechanism with no supporting
  measurement.

  **Qualified 2026-09-17 by § 4.8, in the direction of *less* concern.** On the
  HTR-10 conus — a mesh wall of 480 facets carrying the weight of 27 554
  pebbles — the two codes agree to **11 µm median with every pebble inside
  1 mm** at 2 000 steps. So whatever drives the repose gap, it is **not** a
  systematic error in mesh-wall contact resolution at this scale. The likelier
  reading is that the repose case is dominated by the *lifting* geometry and by
  edge contacts on a small cylinder, where multi-facet resolution matters and
  a conus wall's shallow 29.6 deg facets do not. This does not close the item —
  upstream's resolution is still not ported — but it bounds where it bites.
- **Bulk packing is verified against LIGGGHTS, not against reality.** For
  reference, the settled voidage both codes produce (`ε ≈ 0.442`) sits above the
  Dixon (1988) correlation value for `D/d = 6` (`ε = 0.4198`, i.e. `φ = 0.5802`)
  — a 2.2 percentage-point difference. That is a plausible gap for a
  friction-dominated DEM pour versus a poured/settled experimental correlation,
  but it is **an observation, not a validation**.

  ~~and no attempt has been made here to calibrate `µ`, `e` or `E` against bed
  data~~ **CORRECTED 2026-09-17** — § 4.9 now runs a **friction ablation**, and
  the direction of that 2.2-point gap is consistent with what it found: `µ` is
  the dominant control on packing, and the `µ = 0.4` these numbers were taken
  at is above the 0.1–0.2 reported for graphite-on-graphite. **This is still
  not a calibration** — no parameter was fitted to any measurement, all four
  cells of the factorial are reported, and the `D/d = 6` case has not been
  re-run at the lower friction. But the earlier sentence implied the question
  had not been looked at, and it now has.

- **Reaching the published 0.61 is not validation either, and must not be read
  as such.** § 4.9 reaches a whole-core closure of 0.6047 against the published
  0.61 at `µ = 0.1, µ_r = 0`. Three reasons that is weaker than it looks: the
  benchmark's 0.61 is recorded in `docs/reactor-scoping/htr10-plant-data.md` as
  **"Quoted"** from a specification table, not as a measurement; `27 000` pebbles
  in a `1.97 m` core *is* 0.609 by arithmetic, so the figure may be a design
  closure rather than an observation; and agreement of one scalar is weak
  evidence in any case. The ablation's value is that it **explains** the gap and
  makes it a parameter question, not that it validates the model.
- **No thermal DEM cross-check.** `thermal`, `thermal_radiation`, `bonded`,
  `rolling`, `mesh_wall` and `coupling` have unit tests only; none is compared
  against upstream.
- **Cohesion, and the EPSD rolling family, are not translated.** Upstream's
  `cohesion_model_sjkr*`, `rolling_model_epsd*`/`luding`, and the `limitForce` /
  `viscous` / `heating` switches are not ported (see `granular.rs`
  "Honest scope"). The CDT rolling model **is** ported and verified.
- **Mixed materials are not supported.** A single shared material is assumed;
  upstream carries per-type-pair property matrices.
- **No human V&V.** Everything here is AI-generated draft material under
  `RESPONSIBLE_USE.md` until the maintainer reviews it. The `README.md`
  bookkeeping axes remain **❌ Not yet manually checked**, and nothing in this
  document flips them.

---

## 6. Reproducing

```bash
# unit tests (117)
cargo test --release -p outram-park-fork-liggghts --lib

# cross-code tests against the committed LIGGGHTS reference data (5 + 1 legacy path)
cargo test --release -p outram-park-fork-liggghts --test liggghts_cross_code
cargo test --release -p outram-park-fork-liggghts --test legacy_path_cross_code

# structure: g(r) for every committed HTR-10 bed, plus backend agreement.
# Fast (~2 s) and writes reference-data/liggghts/htr10_rdf.csv.
cargo test --release -p outram-park-fork-liggghts --test htr10_rdf

# the parallel backend is BIT-IDENTICAL to the scalar one (both tangential
# models, thread counts 2/7/12, both sides of the neighbour-search threshold).
# Fast (<1 s); this is what holds the claim in 3.3 to account.
cargo test --release -p outram-park-fork-liggghts --test compute_backend_equivalence

# the long cases. All are gated behind the crate's `long-tests` feature,
# which is ON by default -- so they run in an ordinary `cargo test` and are
# skipped only under `--no-default-features` (or `cargo quick-test`). They do
# NOT need `-- --ignored`.
cargo test --release -p outram-park-fork-liggghts --test pebble_bed_bulk          # 317 s
cargo test --release -p outram-park-fork-liggghts --test angle_of_repose          # 2313 s
cargo test --release -p outram-park-fork-liggghts --test htr10_pebble_bed         # see 4.7
cargo test --release -p outram-park-fork-liggghts --test htr10_conus_cross_code   # see 4.8
cargo test --release -p outram-park-fork-liggghts --test htr10_recirculation      # see 4.9
```

The friction ablation of § 4.9 is a **sweep**, so it is a driver rather than a
test — one run per cell, each writing its own per-batch history and final bed:

```bash
cargo run --release -p outram-park-fork-liggghts --example htr10_recirculation_sweep -- \
    --label mu10_mur00 --mu 0.1 --mu-r 0.0 --batches 40 --threads 12
```

The pre-settled bed is cached per label, so re-running one cell skips ~10
minutes of settling and every repeat starts from the identical state.

### The upstream side

The LIGGGHTS conus deck is `reference-data/liggghts/in.htr10_conus`. It needs
the settled bed as a `read_data` file, which the committed converter produces:

```bash
cd reference-data/liggghts
./csv2data.sh htr10_settled.csv htr10_settled.data 0.06 1730 -0.70 6.00
<path-to>/lmp_serial -in in.htr10_conus
./dumpframe.sh dump.htr10_conus 2000 htr10_conus_t2000_liggghts.csv     # etc.
```

Building upstream (including the mandatory output-precision patch) is described
in `reference-data/liggghts/README.md`.

### Plotting

Geometry, every position file, and ready-to-run `matplotlib` scripts for the
3-D bed, the recirculation history and `g(r)` are in
[`htr10-bed-geometry-and-positions.md`](./htr10-bed-geometry-and-positions.md).
The scripts there were executed before being committed.

### A note on quoted runtimes

Runtimes in this document were measured on **two different hosts** and are
dated where quoted:

- the 2026-09-15/16 figures (317 s, 2313 s, 2714 s) on a 4-core Intel Xeon
  @ 2.80 GHz;
- the 2026-09-17 figures on a 12-core host, and **after** the performance work
  of § 3.3 — which cut the HTR-10 timestep from 55.95 ms to 12.85 ms, i.e. a
  runtime quoted before that date is not comparable to one quoted after it.

Re-measure rather than trusting either. This is the same trap the workspace
`CLAUDE.md` records under the "Any test over 5 minutes" rule, where documented
runtimes were found wrong by a factor of four.

Regenerating the reference data from upstream is described in
`reference-data/liggghts/README.md`.
