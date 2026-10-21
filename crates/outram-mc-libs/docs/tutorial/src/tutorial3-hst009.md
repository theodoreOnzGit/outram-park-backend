# Tutorial 3 — HEU-SOL-THERM-009: a thermal system, through the beginner API

> **Research, education and V&V only.** Not for reactor operation, licensing,
> safety decisions or emergency response
> ([`RESPONSIBLE_USE.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/RESPONSIBLE_USE.md)).
> **Review status: first draft, 2026-10-18, AI-assisted, not yet reviewed by a
> human.** Built from
> [`@@COMMIT_SHORT@@`](https://github.com/theodoreOnzGit/outram-park-backend/commit/@@COMMIT@@)
> on @@BUILD_DATE@@; every code link points at that commit.

This page is about the **beginner API** (`outram_mc_libs::model`), not about
transport. Tutorials 1 and 2 ran Godiva, a bare sphere of uranium metal in
which every neutron stays fast
(`examples/endf_to_keff.rs` and `examples/tutorial_material_and_tallies.rs`;
the book has no page for them yet, the Godiva rung is
[Rung 1](godiva.md)). Here the neutrons are slowed down by water, and the
result has to be read differently.

## The question

**How do I describe a sphere inside a sphere inside a sphere, with water in
it, and how do I know the computer built the system I meant?**

The system is ICSBEP **HEU-SOL-THERM-009, case 1**: a sphere of highly
enriched uranium oxyfluoride *solution*, in a thin aluminium tank, reflected by
water. The benchmark value is **k = 0.9990 ± 0.0043** (van der Marck, *Nucl.
Data Sheets* 107 (2006) 3061, an open secondary source; the handbook itself is
licence restricted). The model is the OpenMC one in `mit-crpg/benchmarks`
(Paul Romano, 2013), transcribed once in
[`vv::hst009`](../../code-review/#crates/outram-mc-libs/src/vv.rs), which every
recorded HST-009 number in this crate uses.

| shape | outer radius [cm] | material |
|---|---|---|
| solution | 11.5177 | U-234/235/236/238, F-19, O, H; H in H2O |
| tank | 11.6764 | 1100 aluminium (Al-27, Si, Mn-55) |
| reflector | 35.0, vacuum outside | water; H in H2O |

## What you write

Build the geometry **inside out, one `.containing` per statement**. Each
shape says how big it is, what fills it, which *component* it belongs to (a
name the results are reported under), and what it contains:

```rust,ignore
let solution_ball = Sphere::radius(cm(hst009::R_SOLUTION))
    .filled_with(solution)
    .in_component(solution_c);
let tank_ball = Sphere::radius(cm(hst009::R_TANK))
    .filled_with(tank.clone())
    .in_component(tank_c)
    .containing(solution_ball.clone());
let reflector = Sphere::radius(cm(hst009::R_REFLECTOR))
    .filled_with(water)
    .in_component(water_c)
    .containing(tank_ball);
model.add(reflector);
model.set_temperature(Temperature::Uniform(t_model));
```

The new things over Godiva, each one line of the example:

- **Light water as a moderator.** `.bound_in(Moderator::LightWater)` attaches
  the thermal scattering law of hydrogen in H2O to H-1. Without it a thermal
  neutron would scatter off hydrogen as if it were a free gas. The law is
  made from the ENDF/B-VIII.0 evaluation by the workspace's LEAPR port, not
  read from a table. ([`Material::bound_in`](../../code-review/#crates/outram-mc-libs/src/model/material.rs::Material::bound_in).)
- **An alloy preset with its citation.** `Material::aluminium_1100()` is a
  starting point whose doc comment cites its source; your own alloy goes in
  with `Material::from_fractions(Basis::Weight, ..)`
  ([`Material::aluminium_1100`](../../code-review/#crates/outram-mc-libs/src/model/material.rs::Material::aluminium_1100)).
- **The spec's atom densities**, read from `vv::hst009`, not retyped.
- **Nested shells**: the order of statements is the order of the physics.
  [`Shape::containing`](../../code-review/#crates/outram-mc-libs/src/model/shape.rs::Shape::containing)
  is the only link between a shell and what is inside it.

Run it with the data and the transport in two calls (or one,
`model.keff(..)`):

```rust,ignore
let nuclides = assembled.load_nuclides(model.data())?;
let run = assembled.keff_with(&nuclides, &KeffSettings::new(preset, Some(diagnostics)))?;
```

([`AssembledModel::keff_with`](../../code-review/#crates/outram-mc-libs/src/model/run.rs::AssembledModel::keff_with).)

```text
cargo run --release -p outram-mc-libs --example tutorial_hst009                      # Auto
cargo run --release -p outram-mc-libs --example tutorial_hst009 -- --fast            # smoke test
cargo run --release -p outram-mc-libs --example tutorial_hst009 -- --seeds 8         # + 8 pooled seeds
cargo run --release -p outram-mc-libs --example tutorial_hst009 -- --seeds 8 --water # + steam-table arm
```

**Run sizes are presets**, from `src/model/run.rs`:

| preset | per cycle × active cycles | meaning |
|---|---|---|
| `MCSettings::Smoke` | 1 000 × 50 | does it run at all; not a result |
| `MCSettings::Fast` | 5 000 × 120 | a smoke test; not a result to quote |
| `MCSettings::Auto` | 10 000 × 250 | the default for a quotable run |
| `MCSettings::High` | 50 000 × 1 500 | for a tight σ |

Inactive cycles are chosen by a source-convergence watcher, not by you. Runs
**print OpenMC-style progress by default** (one line per cycle with k and the
running average); `KeffSettings::silent()` turns it off. The Auto watcher's
thresholds are not yet justified (the module docs of `run.rs` say so), so the
convergence verdict is information, not proof.

<div class="predict">

**Predict.** Godiva was a metal sphere with no moderator. In this solution,
hydrogen outnumbers uranium-235 about 36 to 1. Will most of the flux *in the
solution* be thermal (below 0.625 eV)? And in the water outside?

</div>

## Look at it before you run it

The book's rule: **pictures first, k second**. The example draws the assembled
geometry (what the transport will see, not what you typed) into
`target/tutorials/hst009/` (gitignored, so run the example to get them):

- `hst009_xy.png` and `hst009_xz.png`: slices through the centre. Three
  concentric discs, solution, tank, water.
- `hst009_tank_wall.png`: a 1 cm window on the tank wall. The wall is 1.587 mm
  thick (11.6764 − 11.5177 cm), invisible at full scale, so it gets its own
  zoom.

If the tank is missing or the water is inside the solution, nothing downstream
is worth reading.

The example then **checks that your model is the V&V model**: surfaces, cell
regions and fills identical to `vv::hst009::geometry()`, and every atom
density equal to `vv::hst009::materials()`. Result
(`tests/model_hst009_tutorial.rs`, 2026-10-18): 3 surfaces and 3 cells
identical; of 14 (nuclide, density) pairs, **11 bit-identical and 3 one ulp
off** (U-236 and F-19 in the solution, Mn-55 in the tank). The reason:
`AtomDensity` stores atoms per m³, and the conversion from per barn·cm is not
one-to-one on a 64-bit float. So the model is the same to 1 part in 10¹⁶,
**not bit for bit**; a V&V case moved onto the API cannot promise bit-identical
k as the types stand.

## What you get

All results below were measured **2026-10-18** on ENDF/B-VIII.0 tapes from
`reference-data/endf/`, run with `--seeds 16 --water --workers 4`, on one
Intel i9-13900K shared with other jobs (CPU only), and recorded in the doc
comment of
[`examples/tutorial_hst009.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/tutorial_hst009.rs).

> **Which data these numbers were measured on.** Since they were taken, **route 4
> changed** (gh:#931: the data now come through `process` at its defaults, PURR
> 20 bands / 64 ladders / 10 000 samples, HEATR first). The numbers below are
> from **before** that change, and have **not** been re-run. The #931 re-measure
> moved HST-009 by **−40 ± 21 pcm** (1.9σ, **unresolved**), on a 96-seed record:
> 1.00076 ± 0.00014 at 5 000 × [40 + 120]
> (`verification_and_validation/route4_process_931/README.md`, 2026-10-18). Read
> the +22 pcm below with that in mind; the tutorial's own gate (within 4σ of the
> old record) was not re-run on the new data.

**Which number to quote:** the 16-seed pooled one. The single Auto run is one
draw from a distribution about 45 pcm wide seed to seed (its own quoted σ,
79 pcm, is wider).

| check | result | status |
|---|---|---|
| Auto, seed 1 | k = 1.00154 ± 0.00079 (batch means 0.00067); +254 ± 79 pcm from the benchmark | one draw; +1.00σ from the record |
| **16 seeds, 10 000 × [90 + 250]** | **k = 1.00093 ± 0.00011** (sd per run 45 pcm) | **quotable** (on pre-#931 data) |
| ... against the benchmark | **+193 ± 11 pcm** from 0.9990 ± 0.0043 | 0.45 benchmark σ: the benchmark's own uncertainty is 40 times ours, so it cannot say more |
| ... against the crate's own record | +22 pcm = **+0.92σ** from route 4's 1.00072 ± 0.00021 (32 seeds, 5 000 × [40 + 120], 2026-10-15, U-236 in) | consistent; the record is verification of the code against itself, not an independent check |
| Fast, seed 1 | k = 1.00134 ± 0.00163 | smoke test, not quoted |

(The 4σ gate in the example is a **regression gate** sized by what a run
resolves, not an accuracy claim.)

Two honest remarks on those rows. First, a result of +193 pcm *above* a
benchmark that is itself ±430 pcm is agreement; it is not a measurement of
the code's bias. Second, the 16 seeds are **under-dispersed** against their own
quoted σ (χ² per degree of freedom 0.33, p = 0.993; seed sd 45 pcm against an
internal 80). The steam-water arm on the same seeds was not. This is recorded,
not explained, and the quoted ± are the seed-to-seed ones.

### The six-factor formula

Godiva could not test it; this system can. For a thermal system,

$$k = \eta f p \varepsilon P_{FNL} P_{TNL}$$

where $\eta$ is neutrons produced per thermal absorption in fuel, $f$ the
fraction of thermal absorptions that are in fuel, $p$ the resonance escape
probability, $\varepsilon$ the fast fission factor, and $P_{FNL}$, $P_{TNL}$
the fast and thermal non-leakage probabilities. The run scores each factor,
splitting at 0.625 eV. The product telescopes to $P/(A+L)$ (track-length
production over absorption plus leakage, per source neutron), so it sits
below $k$ by the net (n,xn) gain, and the example **predicted** the gap
before the quoted run: about −14 pcm (Fast measured $A+L-1 = 1.37 \times 10^{-4}$).

| quantity | result (paired per seed, 16 seeds) |
|---|---|
| six-factor product − k | **−7.9 ± 16.0 pcm** (predicted −14: consistent with that and with 0) |
| P (track-length k) − k | +10.6 ± 24.2 pcm (consistent with 0) |

Both are below what 16 seeds resolve, so the claim is only "consistent with".
For Auto seed 1 the factors were η 2.0223, f 0.4523, p 0.8194, ε 1.3718,
P_FNL 0.98557, P_TNL 0.98673 (product 0.99978 ± 0.00087). **f is low because
"thermal absorption" includes the reflector's water**, where about 70 % of that
component's flux is thermal. The factors are system-wide tallies; they are not
the factors of the solution alone.

### Spectra per component

`Diagnostics::default().with_spectrum_per_component(..)` reports one spectrum
per named component (Auto seed 1):

| component | flux below 0.625 eV | note |
|---|---|---|
| solution | 5.7 % | flux per lethargy rises all the way to 1 eV; no Maxwellian maximum. H/U-235 ≈ 36, a strongly absorbing solution |
| tank | 20.3 % | in between |
| reflector | 69.8 % | peak near 0.043 eV |

So the thermal part of this system is the *reflector*, not the solution:
the solution's flux is mostly above 0.625 eV.

## A lesson in two waters

Steam-table water (`Material::water(T, p)`) is **pure** water: natural H and
O at the IF97 density. The spec's water is what the experimenters' water was
*modelled* as. `resolved.compare_number_densities(..)` prints the difference
([`ResolvedMaterial::compare_number_densities`](../../code-review/#crates/outram-mc-libs/src/model/resolve.rs::ResolvedMaterial::compare_number_densities)):

| water | density [g/cm³] | vs spec |
|---|---|---|
| spec (H-1 and O-16 only) | 0.99678 | |
| steam tables, 293.6 K, 1 atm (adds H-2 1.04e-5, O-17 1.26e-5 per b·cm) | 0.99811 | +0.134 % |
| steam tables, 298.15 K | 0.99705 | +0.027 % |

The spec's water is close to 25 °C water. **The spec wins** for the benchmark;
the difference is printed as a lesson, not applied as a correction.

How much does it move k? The example predicted a denser reflector gives
**Δk positive and at most about +20 pcm, not resolvable at 16 seeds**.
Measured, same seeds, paired: **k(steam) − k(spec) = −33 ± 29 pcm (−1.2σ),
not resolved**; its 2σ interval is −90 to +24 pcm. The prediction sits inside
it at the upper end. The honest reading is "no effect resolved", not
"steam-table water is worse".

## Every simplification

| simplification | kind | what it is worth |
|---|---|---|
| O-17 folded into O-16 (the OpenMC model's own approximation, carried unchanged) | open | not estimated here |
| Cu and Zn left out of the tank | open | not estimated here |
| The geometry is three ideal spheres, the OpenMC model's idealisation of the experiment | open | whatever the handbook's detail omits has not been assessed here |
| One temperature, 293.6 K, everywhere (`Temperature::Uniform`) | no data | the spec gives one temperature |
| Spec water, not steam-table water | measured, unresolved | −33 ± 29 pcm (above) |
| Benchmark value from a secondary source | no data | the handbook is licence restricted |
| Numbers measured before #931 | measured, unresolved | −40 ± 21 pcm on the route-4 record |
| 16 seeds | too expensive | σ 11 pcm; the quoted ± are seed to seed |

## What we assumed

- The OpenMC transcription of the specification is the specification
  (checked: the tutorial lowers to it; not checked: the transcription against
  the handbook).
- The spec's water, tank and solution compositions are correct as given.
- The H in H2O law is the right law for the hydrogen in the *solution* as
  well as in the water; a solution of salts is not pure water.
- The nuclear data are ENDF/B-VIII.0 as processed by the workspace's NJOY
  port, at the example's data route (route 4), unchanged for this lesson.
- Nothing was tuned: run sizes are presets or the seed count on the command
  line; no input was changed after seeing k.

## Use, modify, create

**Use.** Run the example with `--fast` and look at the three pictures before
the number. **Modify.** Change `R_SOLUTION` by 1 % in your own copy and watch
the six factors: which one moves? **Create.** Describe a different tank, two
shells and a reflector, with your own alloy.

## Next question

HST-009 has *three shapes and three materials*. A fuel particle in a
high-temperature reactor has *five shells* inside a *cube of graphite*, and
one particle stands for billions.
[Tutorial 4](tutorial4-triso-particle.md) builds one HTR-10 TRISO particle with
the same statements, adds a mirror, and asks what k means for an infinite
array.

## Sources

- `examples/tutorial_hst009.rs` (doc comments: case, methodology, predictions,
  results); `tests/model_hst009_tutorial.rs` (lowering).
- gh:[#943](https://github.com/theodoreOnzGit/outram-park-backend/issues/943)
  (tutorial decisions and progress comments);
  gh:[#931](https://github.com/theodoreOnzGit/outram-park-backend/issues/931)
  (route 4 moved onto `process`, re-measure).
- `verification_and_validation/route4_process_931/README.md`.
- `src/model/run.rs` (presets, progress).
