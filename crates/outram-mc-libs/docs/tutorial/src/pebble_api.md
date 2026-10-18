# Side page: one HTR-10 pebble in the beginner API (tutorial 5)

> **Research, education and V&V only.** Not for reactor operation, licensing,
> safety decisions or emergency response
> ([`RESPONSIBLE_USE.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/RESPONSIBLE_USE.md)).
> **Review status: first draft, 2026-10-18, AI-assisted, not yet reviewed by a
> human** ([#943](https://github.com/theodoreOnzGit/outram-park-backend/issues/943),
> [#926](https://github.com/theodoreOnzGit/outram-park-backend/issues/926)).
> Built from
> [`@@COMMIT_SHORT@@`](https://github.com/theodoreOnzGit/outram-park-backend/commit/@@COMMIT@@)
> on @@BUILD_DATE@@; every code link points at that commit.

This page belongs to [rung 5](triso.md). That rung explains *why* a pebble is
doubly heterogeneous and what each tracking method costs. This page shows how
to **build** one HTR-10 fuel pebble yourself, in the beginner model API, in
about thirty lines, and what the code tells you back about what it built.

**The question of this page:** *if I write "8335 TRISO particles in a 2.5 cm
sphere", how do I know the geometry really holds 8335 whole particles, at the
packing fraction I meant?*

The whole program is
[`examples/tutorial_htr10_pebble.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/tutorial_htr10_pebble.rs).

---

## 1. What do I write?

**Answer.** The particle, kernel outwards, exactly as in tutorial 4. Then the
fuel zone, built as its own object: a **`DhRegion`**, a sphere of matrix
graphite with particles packed in it. Then the pebble around it.

```rust
let fuel_zone = DhRegion::new(Sphere::radius(cm(2.5)), matrix_graphite)
    .in_component(matrix_c)
    .with_particles(particle, Packing::Random { fraction: pf, seed })
    .delta_tracking();                       // required: no default (#943)
let pebble = Sphere::radius(cm(3.0))
    .filled_with(shell_graphite)
    .containing(fuel_zone)
    .boundary(Boundary::Reflective);         // the bare pebble's k-infinity
model.add(pebble);
```

The regular arrangement changes one line:

```rust
    .with_particles(particle, Packing::LatticeOfCount { particles: 8335, offset: [0.13, 0.37, 0.71] })
```

`pf` is not a number you look up. It is derived from the benchmark's count,
`8335 × (0.0455 / 2.5)³ = 5.0248 %`, as in tutorial 4.

## 2. Why is the double heterogeneity said out loud?

**Answer.** Because the code should never guess it. A `DhRegion` is a
decision the user makes, and it carries its tracking with it:
`.delta_tracking()` is required, and a region without it is refused when the
model is assembled. Delta tracking is unbiased under any valid majorant, so
the choice changes the cost of the run, never the answer.

## 3. What does "whole-particle rejection" mean?

**Answer.** A particle that would poke out of the fuel zone is never placed,
and never cut. The random packing (the workspace's port of OpenMC's
`pack_spheres`) is generated in the cube around the zone and only the
particles lying wholly inside are kept, then the request is rescaled until
the fraction **inside the zone** is within 0.2 % of the one asked for. The
lattice keeps a tile's particle only where it fits, the rule the HTR-10 core
model has always used.

**Predict first.** Will the random packing hold exactly 8335 particles?

**Measure.** No: it holds **8345**, a packing fraction of 5.03084 % against
5.02481 % asked, +0.12 %, inside the packer's 0.2 % tolerance. The lattice
holds exactly **8335**, because its pitch (0.1951235 cm) was searched for that
count. A symmetric offset such as `[0.5; 3]` cannot reach 8335 at all: the
count moves in whole symmetry shells, and the API refuses it and names the
nearest count.

## 4. How do I know what the geometry holds?

**Answer.** Ask the assembled model, not your constants:

```rust
let census = assembled.census()?;
println!("{census}");
```

The census walks the geometry the solver will transport and counts each
particle **once per place in space**. That matters for the random packing: it
is placed in a tracking lattice (OpenMC's `create_triso_lattice`), where a
particle straddling a tile face is drawn in each tile it touches. In the
test pebble, 272 particles appear as 836 tile copies, and the census still
counts 272. The HTR-10 model once held 8240 particles while reporting 8340
(gh:#316, +353 pcm); counting from the geometry is the guard against that.

## 5. Look before you run

The example draws, from the assembled geometry, into
`target/tutorials/htr10_pebble/`:

| picture | what to check |
|---|---|
| `random_pebble_xy.png`, `regular_pebble_xy.png` | particles fill the zone and stop at 2.5 cm; the shell is fuel-free |
| `random_pebble_xz.png`, `regular_pebble_xz.png` | the same in the other plane |
| `*_zone_edge_xy.png` | no particle is cut by the zone boundary |
| `*_particle_xy.png` | five layers, kernel to OPyC; the random one straddles a tracking-tile face and is whole |

The slices are taken at `z = -0.29 × pitch`, not through the centre: the
lattice's offset puts no particle centre within a particle radius of `z = 0`,
so a slice through the pebble's centre would show no lattice particle at all.

## 6. Run it

```text
cargo run --release -p outram-mc-libs --example tutorial_htr10_pebble            # Smoke
cargo run --release -p outram-mc-libs --example tutorial_htr10_pebble -- --dh    # + DhUniverse, same particles
```

**Predictions, posted before the runs** (#943): both arrangements within 2σ
of the crate's nearest record (nee_soon's `DhUniverse` HTR-10 pebble,
1.68517 ± 0.00266, 300.15 K); random against regular not resolved; the model
against `DhUniverse` on the *same* particles consistent with zero.

**Measured** (2026-10-18, `MCSettings::Smoke`, a smoke run, not a result to
quote; Intel i9-13900K, one thread for transport (before gh:#955 made the run multi-threaded), shared machine):

| arm | k-infinity | against 1.68517 |
|---|---|---|
| random | 1.68283 ± 0.00515 | −234 pcm (−0.40σ) |
| regular | 1.69048 ± 0.00515 | +531 pcm (+0.92σ) |
| `DhUniverse`, same 8345 particles | 1.68884 ± 0.00596 | model − DhUniverse −600 pcm (−0.76σ) |

All three predictions held. Regular − random is +765 ± 728 pcm: Smoke cannot
see a difference of that size, and nee_soon's larger run did not either
(+102 ± 347 pcm).

**Modify.** Change the seed. Predict first: how far can `k` move, at
Smoke's σ, when only the particle positions change?

## What this page does not show

| not shown | why | consequence |
|---|---|---|
| diagnostics (spectra, six factors) | they are scored under surface tracking only; a model with a delta-tracked region refuses them rather than miss the region | `k` only on this page |
| chord-length tracking, `ring_rpt()`, `naive_homogenisation()` | decided on #943, not built in the beginner API yet | use `dh_universe` directly |
| a whole bed | tutorials 7 to 9 | one bare pebble, reflective at its surface |
