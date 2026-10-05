# TRISO: lumps inside lumps, ending in the HTR-10 pebble bed

> **Research, education and V&V only.** Not for reactor operation, licensing,
> safety decisions or emergency response
> ([`RESPONSIBLE_USE.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/RESPONSIBLE_USE.md)).
> **Review status: first draft, 2026-10-05, AI-assisted, not yet reviewed by a
> human.** Built from
> [`@@COMMIT_SHORT@@`](https://github.com/theodoreOnzGit/outram-park-backend/commit/@@COMMIT@@)
> on @@BUILD_DATE@@; every code link points at that commit.

<div class="mcw-demo" data-mc-widget="demo" data-src="../../demos/monte-carlo/?rung=triso&amp;mode=watch" data-label="▶ Start the TRISO pebble demo here (Watch mode)"></div>

*This is the demo the tutorial opened with. It processes eleven ENDF/B-VIII.0
tapes in your browser, then follows one neutron at a time through a slice of
an HTR-10 fuel pebble in its reflective cell. Each track is a real history.
Zoom in on one of the specks: it is a TRISO particle, and the orange dot at its
centre is the fuel.*

## The problem

On the first page you watched neutrons fly through this pebble without being
able to say what they were doing. Four rungs later you can. A neutron is born
fast in a fuel grain ([rung 1](godiva.md)), slows down in graphite
([rung 2](ugraphite.md)), and the fuel is gathered into lumps so the U-238
resonances shield themselves ([rung 3](lumped.md)), arranged in a repeated
structure ([rung 4](lct008.md)).

What is new is that **the lumps are themselves full of smaller lumps**. A
pebble is a ball of graphite holding thousands of fuel grains, and a core is a
heap of tens of thousands of pebbles. That is called *double heterogeneity*,
and it makes the way rungs 1 to 4 followed a neutron, surface by surface,
very slow.

**The question of this lesson:** *how do you follow a neutron through
hundreds of millions of tiny spheres without spending all your time finding
the next one, and what does each shortcut cost?*

---

## 1. What is a TRISO particle?

**Answer.** A grain of uranium oxide about half a millimetre across, wrapped in
four coatings. The coatings exist to keep the fission products in, at high
temperature, for the life of the fuel. The [TRISO-ATOPS track, rung 1](../triso-atops/triso.html#the-five-layers)
explains each layer in detail. In one line each:

| layer | HTR-10 thickness | what it is for |
|---|---|---|
| UO₂ kernel | 250 µm radius | the fuel |
| buffer (porous carbon) | 90 µm | room for fission gas and recoiling fragments |
| inner pyrolytic carbon (IPyC) | 40 µm | a dense seal, and the surface the SiC is laid on |
| silicon carbide (SiC) | 35 µm | the pressure vessel and the main fission-product barrier |
| outer pyrolytic carbon (OPyC) | 40 µm | protects the SiC and bonds the particle to the matrix |

The dimensions are Li, Yu & Wei (2014) Table 2, as typed into
[`TrisoSpec::HTR10_LI2014`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/fhr_pebble.rs#@@L:crates/outram-mc-libs/src/pebble_beds/fhr_pebble.rs:const=HTR10_LI2014@@).
An HTR-10 fuel pebble is a 6 cm graphite ball. Its inner 5 cm are the fuel
zone, holding **8335** of these particles in graphite matrix; the outer 0.5 cm
is a fuel-free shell.

To a neutron, only the kernel holds anything interesting at resonance
energies. The coatings are carbon and silicon: more moderator.

<div class="predict">

**Predict.** The kernels take up less than 1 % of the fuel zone's volume.
Rung 3 found that lumping natural uranium in graphite raised $k_\infty$ a
great deal. Will resolving the kernels matter here, where the uranium is
already 17 % enriched and the "lumps" are only 0.5 mm across?

</div>

## 2. Lumps inside lumps

**Answer.** Yes, a lot. There are two levels of lumping:

1. **grains in a pebble**: within the fuel zone, the uranium sits in kernels,
   not spread through the graphite;
2. **pebbles in a core**: the fuel zones sit in balls separated by fuel-free
   shells, dummy (all-graphite) balls and coolant.

Each level shields the U-238 resonances the way rung 3's lump did. The
first level is the one that is specific to TRISO fuel, and it is what this
rung measures.

**The measurement.** Take the fuel zone of an HTR-10 pebble and make it an
infinite medium: a 2 cm cube of it with mirror walls, so nothing leaks. Fill
it twice with exactly the same atoms:

- **heterogeneous**: 1018 UO₂ kernels of radius 0.025 cm, packed at random
  (step 7), in graphite matrix;
- **homogenised**: the same atoms smeared evenly through the cube.

The atom densities are IAEA-TECDOC-1382 Table 4-38. The difference between
the two $k_\infty$ is the worth of the first level of lumping.

The example is
[`examples/htr10_fuel_zone_kinf.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs).
Its two materials are built once and shared by both cases, so they cannot
drift apart:

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs::main to=crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs::build_materials depth=2 -->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Call chain from `htr10_fuel_zone_kinf.rs::main` to `htr10_fuel_zone_kinf.rs::build_materials`: 1 hop, 1 shortest chain. Each step shows its code; the name links to it on GitHub.

**1.** [`htr10_fuel_zone_kinf.rs::main`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L396)

<!-- snippet-check: crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:396 fn main -->
<!-- snippet-check: crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:425 build_materials -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:396:396}}
    // …
{{#include ../../../../../crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:423:426}}
    // … (the rest of the function: follow the link above)
```

**2.** → [`htr10_fuel_zone_kinf.rs::build_materials`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L283) · called at [L425](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L425) — Kernel, matrix and the inventory-matched homogenised fuel zone, for either route.

<!-- snippet-check: crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:283 fn build_materials -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:283:322}}
    // … (the rest of the function: follow the link above)
```

Unresolved calls inside the functions on this chain:

- in [`htr10_fuel_zone_kinf.rs::main`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L396):
  - UNRESOLVED(closure): `arg` at [L466](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L466) (→ [`crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:454`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L454)) — call through a closure or fn-typed binding `arg`
  - UNRESOLVED(closure): `arg` at [L467](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L467) (→ [`crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:454`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L454)) — call through a closure or fn-typed binding `arg`
  - UNRESOLVED(closure): `arg` at [L468](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L468) (→ [`crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:454`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L454)) — call through a closure or fn-typed binding `arg`
  - UNRESOLVED(closure): `majorant_for` at [L495](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L495) (→ [`crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:485`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L485)) — call through a closure or fn-typed binding `majorant_for`
  - UNRESOLVED(closure): `majorant_for` at [L522](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L522) (→ [`crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:485`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L485)) — call through a closure or fn-typed binding `majorant_for`
<!-- /code-walk -->

</div>

**The prediction, written before the run** (2026-10-05, commit `61adc0a4a`,
[`verification_and_validation/tutorial_rung5/README.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/verification_and_validation/tutorial_rung5/README.md)):
homogenised below heterogeneous (firm), by **10 000–20 000 pcm**, central
guess −16 000 pcm.

**What the code gives** (2026-10-05, ENDF/B-VIII.0 read directly, URR and
DBRC on, bound-graphite S(α,β) on the matrix carbon, 10 000 neutrons ×
[50 + 200] generations per case, one core of a 2.1 GHz Xeon; recorded in the
example's doc comment and the rung-5 record):

| case | $k_\infty$ |
|---|---|
| heterogeneous (kernels explicit) | 1.57136 ± 0.00088 |
| homogenised (same atoms) | 1.44684 ± 0.00090 |
| **homogenised − heterogeneous** | **−12 452 ± 126 pcm** |

- **Resolving 0.5 mm grains is worth twelve thousand pcm.** The sign is the
  one rung 3 taught: a kernel is black at a U-238 resonance peak whatever its
  size, so its interior is shielded and more neutrons escape to thermal
  energies. Smear the same atoms through the graphite and that shielding is
  gone.
- **Against the prediction:** the sign held, and the size is inside the
  predicted range. But I expected it slightly *larger* than the old
  LOW-tier number and it came out *smaller* (−12 452 against −14 643 ±
  1167); the change, +2191 ± 1174 pcm, is 1.9σ and not resolved.
- **Neither $k_\infty$ is an HTR-10 result.** This is a fuel-zone infinite
  medium with no shell, coolant, reflector or leakage, and nothing published
  corresponds to it. Only the difference is a measurement, and it is a
  comparison of the code with itself: **verification**.

<div class="history">

**History of this number.** The example's first recorded result (2026-09-11)
was on the LOW data tier (an embedded windowed-multipole library with a
10-group fast fallback) with **free-gas** graphite: heterogeneous
1.59671 ± 0.00897, homogenised 1.45028 ± 0.00747, difference
~~−14 643 ± 1167 pcm~~. It predates the correct-physics defaults
(2026-09-20) and used neither the graphite S(α,β) law nor ENDF data, so it
was re-measured for this page. It is kept in the example's doc comment as
the record.

</div>

<div class="predict">

**Predict.** Rungs 1 to 4 followed a neutron from surface to surface. How
many surfaces does one fuel pebble have?

</div>

## 3. Why surface tracking struggles

**Answer.** Every TRISO particle is five nested spheres, so one pebble has
8335 × 5 ≈ **42 000 surfaces**. A full HTR-10 core holds about 27 000 balls
(the IAEA-TECDOC-1382 design point, as transcribed in
`outram-park-fork-liggghts`'s V&V record); if even a third were fuel pebbles,
that is over 10⁸ surfaces. Surface tracking must, at every step, find the distance to
the *nearest* of them along the flight, stop there, work out which cell the
neutron is now in, and start again.

The surfaces are also close together. A coating is 35–90 µm thick, so a
neutron passing through a particle stops at up to ten surfaces (five in, five
out) within a millimetre,
and at each stop it does no physics at all. The geometry work, not the
physics, sets the cost.

**Formula.** In a random packing of spheres of radius $r$ at volume fraction
$f$, the mean distance a straight line travels through the matrix between two
spheres is

$$\bar\ell_\text{matrix} = \frac{4}{3} r \frac{1 - f}{f}.$$

For HTR-10's particles ($r = 0.0455$ cm to the outside of the OPyC, $f =
0.0502$ of the fuel zone) that is about 1.2 cm, against a thermal transport mean
free path in pebble graphite of about 2.6 cm (`DhTreatment::Scls`'s doc
comment gives its window, mean free path plus particle radius, as 2.68 cm on
the FHR pebble). So a typical flight in the fuel zone crosses one or more
particles.

**A measured check** (2026-09-14, recorded in
[`examples/dh_tracking_speedup.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/dh_tracking_speedup.rs)):
a one-speed walk through 51 193 randomly packed particles, the same histories
tracked two exact ways on the same 2.1 GHz Xeon. Surface tracking took
**4.17–5.06 µs per history**; delta tracking, the method of the next step,
took **0.52–0.75 µs**, 6.3–8.1 times faster, and the two agreed on the
absorption probability to −1.0σ (0.8047 ± 0.0009 against 0.8034 ± 0.0009).

<div class="predict">

**Predict.** What if we never looked for surfaces at all, and only asked
"what material is at this point?"

</div>

## 4. Delta (Woodcock) tracking

**Answer.** Pretend the whole problem is filled with a material more opaque
than anything really in it, with total cross section $\Sigma_\text{maj}$ (the
*majorant*). Sample flights in that fictitious material, which needs no
surfaces because it is the same everywhere. At the end of each flight, look
up the material really there, and keep the collision with probability

$$P_\text{real} = \frac{\Sigma_t(\mathbf r, E)}{\Sigma_\text{maj}(E)} .$$

Otherwise the collision is *virtual*: nothing happens, and the neutron flies
on with the same energy and direction.

**Why this is exact.** Along a straight line, collisions in the fictitious
material occur at rate $\Sigma_\text{maj}$ per unit length. Keeping each one
with probability $\Sigma_t/\Sigma_\text{maj}$ thins that stream to rate
$\Sigma_t(\mathbf r)$ at every point, which is exactly the rate of real
collisions. So the first real collision lands where it would have landed with
surface tracking, with the same probability density

$$p(s) = \Sigma_t(s) \exp\left(-\int_0^s \Sigma_t(s') ds'\right).$$

Nothing is approximated: the geometry is still every kernel, exactly where it
is.

**When it is not exact.** The thinning argument needs
$\Sigma_t \le \Sigma_\text{maj}$ *everywhere*. Where the majorant is too low,
a stop inside the dense material is always accepted, but there are too few
stops: collisions are lost exactly where the cross section is largest, which
preferentially removes absorption. Nothing crashes and nothing warns. The
cost of a majorant that is too *high* is only time, spent on virtual
collisions.

<div class="mcw" data-mc-widget="woodcock" data-ratio="1.5"></div>

*Illustration (one energy, a 10 cm line with six 2 mm kernels, JavaScript's
random numbers). Send 10 000 neutrons: the sampled first-collision density
follows the exact one. Then drag the majorant below the kernels' cross
section and send them again: the kernels are under-sampled, and nothing tells
you.*

**The code walk.** The fuel-zone example calls the delta-tracked power
iteration; each history flies to its next real collision in `delta_flight`,
which samples the distance at the majorant and accepts or rejects in
`classify_collision`:

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs::main to=crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs::classify_collision depth=8 -->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Call chain from `htr10_fuel_zone_kinf.rs::main` to `delta_tracking.rs::classify_collision`: 6 hops, 2 shortest chains. Each step shows its code; the name links to it on GitHub.

**1.** [`htr10_fuel_zone_kinf.rs::main`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L396)

<!-- snippet-check: crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:396 fn main -->
<!-- snippet-check: crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:505 run_keff_delta -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:396:396}}
    // …
{{#include ../../../../../crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:503:506}}
    // … (the rest of the function: follow the link above)
```

**2.** → [`keff_delta.rs::run_keff_delta`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L599) · called at [L505](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L505) — Run fission-source power iteration over a **reflective cube** filled with a two-(or-more-)material dispersion medium, transporting each history by delta (Woodcock) tracking.

<!-- snippet-check: crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:599 fn run_keff_delta -->
<!-- snippet-check: crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:610 run_keff_delta_in -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:599:611}}
    // … (the rest of the function: follow the link above)
```

**3.** → [`keff_delta.rs::run_keff_delta_in`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L620) · called at [L610](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L610)

<!-- snippet-check: crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:620 fn run_keff_delta_in -->
<!-- snippet-check: crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:633 run_keff_delta_seq_in -->
<!-- snippet-check: crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:635 run_keff_delta_par_in -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:620:636}}
    // … (the rest of the function: follow the link above)
```

**4.** → [`keff_delta.rs::run_keff_delta_seq_in`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L672) · called at [L633](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L633) — Scalar, single-thread delta-tracked power iteration — the **trusted, deterministic, bit-reproducible reference** backend (`ComputeType::CpuSingleThread`).

<!-- snippet-check: crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:672 fn run_keff_delta_seq_in -->
<!-- snippet-check: crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:719 transport_history -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:672:679}}
    // …
{{#include ../../../../../crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:717:720}}
    // … (the rest of the function: follow the link above)
```

**9.** → [`keff_delta.rs::transport_history`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L946) · called at [L719](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L719) — Transport one source neutron (plus its same-generation `(n,2n)` secondaries) to death by delta tracking, banking fission neutrons.

<!-- snippet-check: crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:946 fn transport_history -->
<!-- snippet-check: crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:991 delta_flight -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:946:953}}
    // …
{{#include ../../../../../crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:989:992}}
    // … (the rest of the function: follow the link above)
```

**10.** → [`keff_delta.rs::delta_flight`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L510) · called at [L991](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L991) — Fly a neutron to its next **real** collision inside the reflective cube by delta tracking, returning the collision position, the material index there, and the direction it arrived along (for post-collision scattering).

<!-- snippet-check: crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:510 fn delta_flight -->
<!-- snippet-check: crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:541 classify_collision -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:510:517}}
    // …
{{#include ../../../../../crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:539:542}}
    // … (the rest of the function: follow the link above)
```

**11.** → [`delta_tracking.rs::classify_collision`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L580) · called at [L541](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L541) — Decide whether a delta-tracking collision is real or virtual by rejection on the ratio `Σ_t(local)/Σ_maj`.

<!-- snippet-check: crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs:580 fn classify_collision -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs:580:590}}
```

**8.** (from step 3) → [`keff_delta.rs::run_keff_delta_par_in`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L794) · called at [L635](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L635) — Rayon-parallel delta-tracked power iteration (`ComputeType::CpuMultiThread`).

<!-- snippet-check: crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:794 fn run_keff_delta_par_in -->
<!-- snippet-check: crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:875 transport_history -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:794:801}}
    // …
{{#include ../../../../../crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:873:876}}
    // … (the rest of the function: follow the link above)
```

**9.** → [`keff_delta.rs::transport_history`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L946) · called at [L875](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L875) — Transport one source neutron (plus its same-generation `(n,2n)` secondaries) to death by delta tracking, banking fission neutrons.

<!-- snippet-check: crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:946 fn transport_history -->
<!-- snippet-check: crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:991 delta_flight -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:946:953}}
    // …
{{#include ../../../../../crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:989:992}}
    // … (the rest of the function: follow the link above)
```

**10.** → [`keff_delta.rs::delta_flight`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L510) · called at [L991](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L991) — Fly a neutron to its next **real** collision inside the reflective cube by delta tracking, returning the collision position, the material index there, and the direction it arrived along (for post-collision scattering).

<!-- snippet-check: crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:510 fn delta_flight -->
<!-- snippet-check: crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:541 classify_collision -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:510:517}}
    // …
{{#include ../../../../../crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:539:542}}
    // … (the rest of the function: follow the link above)
```

**11.** → [`delta_tracking.rs::classify_collision`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L580) · called at [L541](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L541) — Decide whether a delta-tracking collision is real or virtual by rejection on the ratio `Σ_t(local)/Σ_maj`.

<!-- snippet-check: crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs:580 fn classify_collision -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs:580:590}}
```

Unresolved calls inside the functions on this chain:

- in [`htr10_fuel_zone_kinf.rs::main`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L396):
  - UNRESOLVED(closure): `arg` at [L466](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L466) (→ [`crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:454`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L454)) — call through a closure or fn-typed binding `arg`
  - UNRESOLVED(closure): `arg` at [L467](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L467) (→ [`crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:454`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L454)) — call through a closure or fn-typed binding `arg`
  - UNRESOLVED(closure): `arg` at [L468](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L468) (→ [`crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:454`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L454)) — call through a closure or fn-typed binding `arg`
  - UNRESOLVED(closure): `majorant_for` at [L495](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L495) (→ [`crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:485`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L485)) — call through a closure or fn-typed binding `majorant_for`
  - UNRESOLVED(closure): `majorant_for` at [L522](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L522) (→ [`crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:485`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L485)) — call through a closure or fn-typed binding `majorant_for`
- in [`keff_delta.rs::run_keff_delta_seq_in`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L672):
  - UNRESOLVED(trait): `material_at` at [L696](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L696) (→ [`crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:129`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L129)) — trait method `MaterialQuery::material_at`; the implementor is chosen by type, not followed
- in [`keff_delta.rs::run_keff_delta_par_in`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L794):
  - UNRESOLVED(trait): `material_at` at [L839](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L839) (→ [`crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:129`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L129)) — trait method `MaterialQuery::material_at`; the implementor is chosen by type, not followed
- in [`keff_delta.rs::transport_history`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L946):
  - UNRESOLVED(trait): `begin_history` at [L965](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L965) (→ [`crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:135`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L135)) — trait method `MaterialQuery::begin_history`; the implementor is chosen by type, not followed
- in [`keff_delta.rs::delta_flight`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L510):
  - UNRESOLVED(trait): `material_at` at [L537](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L537) (→ [`crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:129`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L129)) — trait method `MaterialQuery::material_at`; the implementor is chosen by type, not followed
<!-- /code-walk -->

</div>

The only geometry question delta tracking asks is "which material is at this
point?". For the packed kernels that is
[`PackedSpheres::is_inside_kernel`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#@@L:crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs:fn=is_inside_kernel@@),
a lookup in a uniform grid that scans only the kernels touching the query
point's grid cell. The example passes it to the kernel as a closure, a hop
the code-walk tool cannot follow, so it is linked here by hand.

After a real collision, the physics is rung 2's, unchanged: the same nuclide
choice, the same S(α,β) / free-gas / target-at-rest fork, the same URR
tables.

### Test the assumption the result rests on: is the majorant really a bound?

The whole method rests on $\Sigma_t \le \Sigma_\text{maj}$. The example built
its majorant with
[`Majorant::bounding`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#@@L:crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs:fn=bounding@@):
4096 logarithmic energy bins, each sampled at 32 points, plus a 10 % margin.
Nothing had ever checked it on ENDF data. So before the re-measurement in
step 2, the example was given an audit that evaluates both materials'
$\Sigma_t$ at 2 million energies plus every point of every nuclide's own
energy grid, and stops if the majorant is ever exceeded.

**It was not a bound.** At **1.689 MeV**, inside the UO₂ kernel, the true
$\Sigma_t$ was **1.184 times** the majorant (2026-10-05, the ablation
`OUTRAM_HTR10_MAJORANT=bounding`, recorded in the rung-5 record). The
sampling step there is about 340 eV, and a resonance narrower than that fell
between two samples, clearing the 10 % margin with room to spare. The
homogenised material was bounded (0.941), because smearing dilutes the
kernel's peaks by a factor of a hundred.

How much that moved $k$ was **not measured**: it is a narrow band at an
energy where few collisions happen, so it may be small, but "may be small" is
exactly the kind of assumption this step is about. The fix was to change the
*protocol*, not to raise the margin until the check passed. Pointwise ENDF
cross sections are linear between their own grid points, so the total of any
mixture is largest at one of those points. The example now tabulates the
majorant on **every point of every nuclide's own grid** (plus a log grid for
the thermal-scattering and URR parts, which are not pointwise), and the audit
then reads 0.909: bounded everywhere it was checked, by the 10 % margin.
Step 2's numbers were measured with that majorant.

The same construction with a 30 % margin, as `DhUniverse::keff` uses it for
step 5's problem, was audited too, each material against its own majorant:
worst 0.897, bounded.

<div class="predict">

**Predict.** Delta tracking is exact and much faster than surface tracking.
Why would anyone use something cheaper and approximate instead?

</div>

## 5. Shortcuts, and what they cost

**Answer.** Even delta tracking must store every particle's position, and a
core holds hundreds of millions of them. The shortcuts give up some of the
geometry to save memory and time:

| treatment | what it keeps | what it gives up |
|---|---|---|
| **delta tracking** | every particle, exactly | nothing (the reference) |
| **chord-length sampling (CLS)** | nothing stored: particle crossings are sampled from the statistics of chord lengths as the neutron flies | memory of where the particles were; and, as applied here, the kernel is smeared through its whole particle |
| **semi-implicit CLS (SCLS)** | CLS plus the particles met recently, inside a moving window | as CLS, with a bounded memory |
| **naive homogenisation** | nothing: the fuel zone is one smeared material | all grain-level self-shielding |
| **ring-RPT** | the same smeared fuel, but in a spherical shell at a fitted radius | the radial shape of the fuel |

Each is a variant of
[`DhTreatment`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/dh_universe.rs#@@L:crates/outram-mc-libs/src/dh_universe.rs:enum=DhTreatment@@),
and `DhUniverse::keff` runs any of them through the same delta-tracked power
iteration, changing only the answer to "which material is at this point?":

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/examples/dh_keff_vv.rs::main to=crates/outram-mc-libs/src/pebble_beds/keff_delta.rs::run_keff_delta_in depth=3 -->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Call chain from `dh_keff_vv.rs::main` to `keff_delta.rs::run_keff_delta_in`: 2 hops, 1 shortest chain. Each step shows its code; the name links to it on GitHub.

**1.** [`dh_keff_vv.rs::main`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/dh_keff_vv.rs#L538)

<!-- snippet-check: crates/outram-mc-libs/examples/dh_keff_vv.rs:538 fn main -->
<!-- snippet-check: crates/outram-mc-libs/examples/dh_keff_vv.rs:702 keff -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/dh_keff_vv.rs:538:538}}
    // …
{{#include ../../../../../crates/outram-mc-libs/examples/dh_keff_vv.rs:700:703}}
    // … (the rest of the function: follow the link above)
```

**2.** → [`dh_universe.rs::DhUniverse::keff`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/dh_universe.rs#L1477) · called at [L702](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/dh_keff_vv.rs#L702) — Solve the eigenvalue for this universe.

<!-- snippet-check: crates/outram-mc-libs/src/dh_universe.rs:1477 fn keff -->
<!-- snippet-check: crates/outram-mc-libs/src/dh_universe.rs:1488 run_keff_delta_in -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/dh_universe.rs:1477:1489}}
    // … (the rest of the function: follow the link above)
```

**3.** → [`keff_delta.rs::run_keff_delta_in`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L620) · called at [L1488](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/dh_universe.rs#L1488)

<!-- snippet-check: crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:620 fn run_keff_delta_in -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/pebble_beds/keff_delta.rs:620:659}}
    // … (the rest of the function: follow the link above)
```

Unresolved calls inside the functions on this chain:

- in [`dh_keff_vv.rs::main`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/dh_keff_vv.rs#L538):
  - UNRESOLVED(other): `clone` at [L688](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/dh_keff_vv.rs#L688) (→ [`crates/outram-mc-libs/src/physics/keff.rs:123`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/keff.rs#L123)) — resolves to `#[derive(Debug, Clone)]`, not a function body
<!-- /code-walk -->

</div>

**The problem they are judged on** is the FHR reference unit cell of
[`examples/dh_keff_vv.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/dh_keff_vv.rs):
one TRISO pebble (a 1.9 cm fuel zone at a particle packing fraction of 0.30,
in a 2.0 cm pebble), in molten-salt coolant out to a mirror sphere at 3 cm,
600 K, ENDF/B-VIII.0 with the graphite S(α,β) law. Same geometry, same atoms
and same random-number seed in every arm; only the treatment changes.

**Recorded 2026-09-14, re-measurement pending
([#582](https://github.com/theodoreOnzGit/outram-park-backend/issues/582)).**
7200 neutrons × [15 + 40] generations per arm, single-threaded, on the same
2.1 GHz Xeon class as this page's other runs; from the doc comment of
`dh_keff_vv.rs`:

| treatment | $k$ | vs delta tracking | σ | speed |
|---|---|---|---|---|
| delta tracking (exact) | 1.38647 ± 0.00211 | — | — | 1.00× |
| chord-length sampling | 1.35380 ± 0.00267 | −3267 pcm | 9.6, resolved | 2.16× |
| semi-implicit CLS | 1.35282 ± 0.00228 | −3365 pcm | 10.8, resolved | 2.09× |
| naive homogenisation | 1.34345 ± 0.00234 | −4302 pcm | 13.7, resolved | 1.75× |
| ring-RPT | 1.38684 ± 0.00252 | +37 pcm | 0.1, not resolved | 2.13× |

**Why these are not the current numbers.** They predate the
correct-physics defaults (URR and DBRC on, 2026-09-20), and SCLS's row may
predate a fix to its wiring (2026-09-14) whose defects had made it, in the
crate's own words, "CLS in all but name" (`DhTreatment::Scls`'s doc comment;
which side of the fix that row was measured on is not recorded). Two later partial records exist:

- 2026-10-02 (i9-13900K, delta, naive and ring-RPT only, recorded in
  `verification_and_validation/ring_rpt/ring_rpt_vs_openmc.md`): delta
  1.38155 ± 0.00222, naive **−4136 pcm** (11.6σ), ring-RPT **+239 pcm**
  (0.8σ, not resolved).
- 2026-10-05 (this rung's record, stopped at the hand-over to #582): delta
  1.38155 ± 0.00222 again (the same seed gives the same answer), and CLS
  1.34536 ± 0.00247, **−3619 ± 332 pcm** (10.9σ). SCLS was not re-run.

What holds across every record so far:

- **Naive homogenisation is resolved and expensive in reactivity**, about
  −4100 to −4300 pcm: it throws away the grain-level shielding of step 2.
- **Ring-RPT is not resolved from exact** in any record: concentrating the
  smeared fuel in the right shell recovers what uniform smearing loses. The
  "right" radius is fitted, to an OpenMC calculation of this pebble, which is
  its catch.
- **CLS as applied here (the kernel smeared through its whole particle)
  carries a resolved bias of −3300 to −3600 pcm.** Most of it is the
  smearing, not the chord sampling: applied to the kernel itself, CLS was
  +669 pcm, not resolved (2026-09-18, 800 neutrons, `docs/cls-scls-vv.md`).
- **SCLS is the wrong method for a graphite pebble**: its memory window is
  about a transport mean free path, 2.7 cm, larger than the 1.9 cm fuel zone
  it is meant to be a local view of. On 2026-09-18 it measured −4251 pcm and
  ran 14.5 times *slower* than exact delta tracking (`DhTreatment::Scls`'s
  doc comment).

A bias that is "not resolved" means the run could not measure it, not that
it is zero. Which shortcut is acceptable depends on the tolerance of the
question being asked.

<div class="predict">

**Predict.** Delta tracking needs every particle's position. Where do those
positions come from, and what happens to a particle that straddles the edge
of the fuel zone?

</div>

## 6. Random packing and cut particles

**Answer.** A real fuel zone's particles sit at random. This code places them
by **random sequential addition** (RSA), a port of OpenMC's
`_random_sequential_pack`: draw a centre uniformly in the box, reject it if
the new sphere would overlap one already placed, repeat until the requested
number is in.
[`pack_spheres`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#@@L:crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs:fn=pack_spheres@@)
does it with a grid, so each overlap test only looks at nearby spheres. RSA
jams at a volume fraction of about 0.38, far above a fuel zone's 0.05–0.30.

**The cut-particle pitfall.** A box packing has to be trimmed to the
spherical fuel zone, and some particles straddle its surface. There are two
honest choices, and one dishonest one:

- **keep the cut pieces**, so the geometry contains part-particles (the fuel
  inventory is right, the particles at the edge are not real particles);
- **keep only whole particles**, and correct the count so the inventory is
  still the one asked for;
- keep only whole particles and *say nothing*: the zone then holds less fuel
  than specified, and every comparison with a smeared treatment (which uses
  the requested fraction) compares two different fuel loadings.

This code did the third, and found it. On the FHR pebble a request of 0.30
realised **0.2894** once the cut particles were dropped, 3.7 % less heavy
metal than the smeared arms carried. It now re-packs until the whole
particles inside the ball hit the requested fraction to 0.2 %, in
[`pack_in_ball`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/dh_universe.rs#@@L:crates/outram-mc-libs/src/dh_universe.rs:fn=pack_in_ball@@):

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/src/dh_universe.rs::DhUniverse::pebble to=crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs::pack_spheres depth=5 -->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Call chain from `dh_universe.rs::DhUniverse::pebble` to `sphere_packing.rs::pack_spheres`: 3 hops, 1 shortest chain. Each step shows its code; the name links to it on GitHub.

**1.** [`dh_universe.rs::DhUniverse::pebble`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/dh_universe.rs#L898) — Build a TRISO fuel pebble under the chosen treatment.

<!-- snippet-check: crates/outram-mc-libs/src/dh_universe.rs:898 fn pebble -->
<!-- snippet-check: crates/outram-mc-libs/src/dh_universe.rs:961 pack_in_ball -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/dh_universe.rs:898:898}}
    // …
{{#include ../../../../../crates/outram-mc-libs/src/dh_universe.rs:959:962}}
    // … (the rest of the function: follow the link above)
```

**2.** → [`dh_universe.rs::pack_in_ball`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/dh_universe.rs#L1919) · called at [L961](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/dh_universe.rs#L961) — RSA-pack whole particles into a ball of `radius`, **hitting the requested packing fraction inside that ball** rather than inside the cube it was generated in.

<!-- snippet-check: crates/outram-mc-libs/src/dh_universe.rs:1919 fn pack_in_ball -->
<!-- snippet-check: crates/outram-mc-libs/src/dh_universe.rs:1940 generate -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/dh_universe.rs:1919:1941}}
    // … (the rest of the function: follow the link above)
```

**3.** → [`sphere_packing.rs::PackingConfig::generate`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L154) · called at [L1940](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/dh_universe.rs#L1940) — Generate the packed sphere list for this configuration.

<!-- snippet-check: crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs:154 fn generate -->
<!-- snippet-check: crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs:156 pack_spheres -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs:154:157}}
    // … (the rest of the function: follow the link above)
```

**4.** → [`sphere_packing.rs::pack_spheres`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L207) · called at [L156](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L156) — Random Sequential Addition packing of equal spheres in a cubic domain.

<!-- snippet-check: crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs:207 fn pack_spheres -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs:207:246}}
    // … (the rest of the function: follow the link above)
```

Unresolved calls inside the functions on this chain:

- in [`dh_universe.rs::DhUniverse::pebble`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/dh_universe.rs#L898):
  - UNRESOLVED(other): `clone` at [L1103](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/dh_universe.rs#L1103) (→ [`crates/outram-mc-libs/src/material/material.rs:61`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/material.rs#L61)) — resolves to `#[derive(Debug, Clone)]`, not a function body
- in [`dh_universe.rs::pack_in_ball`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/dh_universe.rs#L1919):
  - UNRESOLVED(closure): `attempt` at [L1959](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/dh_universe.rs#L1959) (→ [`crates/outram-mc-libs/src/dh_universe.rs:1931`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/dh_universe.rs#L1931)) — call through a closure or fn-typed binding `attempt`
  - UNRESOLVED(closure): `attempt` at [L1970](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/dh_universe.rs#L1970) (→ [`crates/outram-mc-libs/src/dh_universe.rs:1931`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/dh_universe.rs#L1931)) — call through a closure or fn-typed binding `attempt`
<!-- /code-walk -->

</div>

The fuel-zone cube of step 2 avoids the problem differently: RSA keeps every
centre at least one radius from the walls, so no kernel is cut, and the
mirror walls make the cube an infinite medium. The price is a kernel-free
skin one kernel radius (0.25 mm) deep at each wall; the requested count of
1018 kernels is still all placed, so the inventory is right.

<div class="predict">

**Predict.** A whole core has tens of thousands of pebbles. Should they, too,
be packed at random?

</div>

## 7. The HTR-10 pebble bed

**Answer.** HTR-10 is a 10 MW pebble-bed test reactor. Its first approach to
criticality, loading a mixture of fuel and all-graphite "dummy" pebbles until
the core went critical, is a published benchmark (IAEA-TECDOC-1382), and
other codes have computed $k_\text{eff}$ against loading height for it.

> *History placeholder: when and where HTR-10 was built and first went
> critical needs a source with a page number, to be supplied by the
> maintainer. Nothing about it is stated as fact here.*

The calculation here builds the whole core: the bed,
the reflector with its control-rod and coolant channels, the cavity above,
and every TRISO particle in every fuel pebble. A pebble in the bed is
delta-tracked; the reflector outside it is surface-tracked.

![HTR-10 model, vertical slice through the axis, N = 12 layers](https://raw.githubusercontent.com/theodoreOnzGit/outram-park-backend/@@COMMIT@@/crates/nee_soon/verification_and_validation/htr10_geometry_images/htr10_rz_full.png)

*The whole model, R–Z slice, drawn from the assembled geometry
([`htr10_geometry_images/`](https://github.com/theodoreOnzGit/outram-park-backend/tree/@@COMMIT@@/crates/nee_soon/verification_and_validation/htr10_geometry_images),
2026-10-01): the bed of pebbles in helium, the conus and discharge tube
below, the reflector around. Zoom in one level:*

![One HTR-10 fuel pebble, x-y slice](https://raw.githubusercontent.com/theodoreOnzGit/outram-park-backend/@@COMMIT@@/crates/nee_soon/verification_and_validation/htr10_geometry_images/htr10_xy_one_pebble.png)

*One fuel pebble: the fuel zone's particles on a cubic lattice, the fuel-free
shell, neighbouring pebbles and helium. And one more:*

![One HTR-10 TRISO particle, x-y slice](https://raw.githubusercontent.com/theodoreOnzGit/outram-park-backend/@@COMMIT@@/crates/nee_soon/verification_and_validation/htr10_geometry_images/htr10_xy_triso.png)

The model is
[`assemble_explicit_triso`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/nee_soon/src/htr10_rmc/core_model.rs#@@L:crates/nee_soon/src/htr10_rmc/core_model.rs:fn=assemble_explicit_triso@@)
in the `nee_soon` crate: three levels of nested universes (the bed's hex
lattice, a tile of pebble pieces, and the TRISO lattice inside each fuel zone),
which rung 4's lattice machinery tracks. The bed cell is marked
`delta_tracked`, so a neutron inside it flies by step 4's method through
every particle of every pebble.

**The result** (recorded 2026-10-01/02, commit `f888cfd98e`,
[`htr10_seker_2026_10_01_10k/`](https://github.com/theodoreOnzGit/outram-park-backend/tree/@@COMMIT@@/crates/nee_soon/verification_and_validation/htr10_seker_2026_10_01_10k)):
$k_\text{eff}$ against loading height, N = 10 to 20 layers of pebbles, on
ENDF/B-VIII.0 and on ENDF/B-VII.0, each point 10 000 neutrons × [5 + 135]
generations (the reference paper's own statistics), one seed per point.

![k_eff against loading height, both libraries, with RMC and MCNP](https://raw.githubusercontent.com/theodoreOnzGit/outram-park-backend/@@COMMIT@@/crates/nee_soon/verification_and_validation/htr10_seker_2026_10_01_10k/keff_vs_height_endf8_endf7.png)

The reference is Li, Yu & Wei (2014)'s RMC calculation (ENDF/B-VII.0). Şeker
& Çolak (2003)'s MCNP values (ENDF/B-VI, on an independent model) are shown
as a gauge only. Neither quotes an uncertainty. Each point is compared at the
height where the reference model holds the same number of balls as ours.

| N | ENDF/B-VIII.0: k ± 1σ | − RMC (pcm) | σ | ENDF/B-VII.0: k ± 1σ | − RMC (pcm) | σ |
|---|---|---|---|---|---|---|
| 10 | 0.92645 ± 0.00108 | −525 ± 108 | **−4.9** | 0.92815 ± 0.00096 | −355 ± 96 | **−3.7** |
| 11 | 0.96620 ± 0.00101 | −107 ± 101 | −1.1 | 0.96256 ± 0.00106 | −471 ± 106 | **−4.4** |
| 12 | 0.99566 ± 0.00114 | −376 ± 114 | **−3.3** | 0.99888 ± 0.00109 | −54 ± 109 | −0.5 |
| 13 | 1.02769 ± 0.00104 | +128 ± 104 | +1.2 | 1.02690 ± 0.00106 | +48 ± 106 | +0.5 |
| 14 | 1.05134 ± 0.00109 | −95 ± 108 | −0.9 | 1.05394 ± 0.00107 | +165 ± 107 | +1.5 |
| 15 | 1.07566 ± 0.00115 | +125 ± 115 | +1.1 | 1.07556 ± 0.00107 | +115 ± 107 | +1.1 |
| 16 | 1.09447 ± 0.00111 | +87 ± 111 | +0.8 | 1.09766 ± 0.00093 | +406 ± 93 | **+4.4** |
| 17 | 1.11352 ± 0.00103 | +242 ± 103 | +2.3 | 1.11736 ± 0.00105 | +626 ± 105 | **+5.9** |
| 18 | 1.13389 ± 0.00117 | +427 ± 117 | **+3.7** | 1.13459 ± 0.00094 | +497 ± 94 | **+5.3** |
| 19 | 1.14787 ± 0.00092 | +352 ± 92 | **+3.8** | 1.14994 ± 0.00101 | +558 ± 101 | **+5.5** |
| 20 | 1.16172 ± 0.00096 | +310 ± 96 | **+3.2** | 1.16495 ± 0.00101 | +634 ± 101 | **+6.3** |

(k rounded to five places from the record's `results_table.md`; the residuals
and σ columns are the record's.)

- **All 22 points are within ±1000 pcm of RMC**, the crate's acceptance
  band, and 18 of 22 within ±500 pcm.
- **The misses are real, and shown.** 5 of 11 VIII.0 points and 7 of 11 VII.0
  points sit 3σ or more from RMC, up to 6.3σ. With σ now about 100 pcm, the
  residual is not noise.
- **There is a drift with height:** +8.3 pcm/cm on VIII.0 and +11.4 pcm/cm
  on VII.0 against RMC, low at the bottom and high at the top
  ([gh:#218](https://github.com/theodoreOnzGit/outram-park-backend/issues/218)).
  Against MCNP's helium column it is −4.7 and −1.6 pcm/cm. Part of it is
  therefore a difference between the two references themselves; which one is
  right is not known.
- **What this is:** a code-to-code comparison of a model against two other
  codes' models of the same benchmark, so **verification**, not validation.
  Here $k$ crosses 1 between N = 12 and 13 on both libraries; this page does
  not compare that with the loading height measured in the experiment.

### The deliberate liberties

Every model simplifies. These are the simplifications in this one, why each
was taken, and what is known about its cost (from the record's `README.md`
and `RUN_PARAMETERS.md`, and `crates/nee_soon/src/htr10_rmc/`):

| liberty | why | what it may cost |
|---|---|---|
| pebbles on **Şeker & Çolak (2003)'s regular 13-ball lattice cell**, not the real random bed | it is the bed both reference models use, so the comparison is like for like with them | like for like with the references, **not with the reactor**; the random bed's effect on k has not been measured (below) |
| **every pebble whole**: balls crossing the wall or the cone are rejected (gh:#472) | a cut pebble is not a pebble; Şeker p.267 does the same | the built bed heights, 9.798 N + 6 cm, differ from Şeker's, so points are compared at **equal ball count**, interpolating the reference between tabulated heights |
| **TRISO particles on a cubic lattice** inside each fuel pebble, not randomly packed | the reference models use a lattice too, and a lattice is one universe repeated | 8335 whole particles per pebble, as specified (+0.060 % in fuel volume against the lattice that missed it, < 5 pcm); the random-versus-lattice effect has not been measured on this core |
| helium at an **assumed** atmospheric pressure, 101.33 kPa, 300.15 K | Şeker & Çolak (2003) p.267 states the helium but not its pressure | helium is nearly transparent; not measured |
| the VII.0 arm takes **helium and the control-rod metals from VIII.0** tapes | no VII.0 tapes for them are in the checkout | a mixed library in that arm; not measured |
| VII.0 has no SiC thermal law, so SiC carbon is free gas there | ENDF/B-VII.0 has no SiC evaluation | a real library difference, part of VII.0 − VIII.0 |
| references quote **no uncertainty**; MCNP is ENDF/B-VI on an independent model | that is what the papers give | "nσ" above counts our statistics only |
| one seed per point | 10 000 × 140 generations already took 40–69 min per point on 5 threads of an i9-13900K | the σ is the within-run estimate; no multi-seed pooling |

### The real bed is random, and has not been run

A real pebble bed is a random heap. This workspace can make one: its granular
solver (`outram-park-fork-liggghts`, a port of the LIGGGHTS discrete-element
code) settles pebbles under gravity into the HTR-10 vessel, and with
graphite-literature friction it reaches a filling fraction within 0.9 % of the
published 0.61 ([gh:#216](https://github.com/theodoreOnzGit/outram-park-backend/issues/216)).
`outram-mc-libs` has the seam to hand such a bed to transport
([`pebble_beds::dem_bed`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/dem_bed.rs)).

**But no $k_\text{eff}$ of the HTR-10 core on a DEM-settled random bed exists
in this repository.** Searched 2026-10-05: `crates/nee_soon`,
`crates/outram-park-fork-liggghts` and `crates/outram-mc-libs`. The DEM seam
returns pebble centres only and builds no geometry, and nothing calls it from
a criticality run. So the effect of the lattice liberty above is **not
known**, and this page does not estimate it.

## Run it yourself

The fuel-zone comparison of step 2 (a few minutes per case at the default
400 neutrons; the recorded run used more):

```text
cargo run --release -p outram-mc-libs --example htr10_fuel_zone_kinf
THREADS=2 cargo run --release -p outram-mc-libs --example htr10_fuel_zone_kinf -- 2000 20 80
```

The shortcuts of step 5 (`OUTRAM_DH_ONLY=delta,naive` runs just those arms):

```text
OUTRAM_DH_VV_HISTORIES=7200 cargo run --release -p outram-mc-libs --example dh_keff_vv
```

**Modify.** Run the fuel-zone example with
`OUTRAM_HTR10_GRAPHITE_TSL=crystalline`. Predict first: does the ideal-crystal
graphite law raise or lower $k_\infty$, and does it change the difference
between the two cases?

**Create.** The fuel-zone example has no shortcuts. Add a ring-RPT-style case:
put the smeared fuel only in a shell of the cube, at the radius you choose,
and see how close to the explicit answer you can get. Then ask whether the
radius you found would carry over to a different packing fraction.

## Next

This is the last rung of the ladder so far. You can now read every part of
the [demo](../../demos/monte-carlo/?rung=triso&mode=watch) the tutorial
opened with: the fast birth in a kernel, the slow-down in graphite, the
virtual collisions that delta tracking throws away, and why the fuel is in
grains at all. A demo rung for the whole HTR-10 core is planned
([#528](https://github.com/theodoreOnzGit/outram-park-backend/issues/528)).

## Deliberate liberties on this page

| liberty | why | cost |
|---|---|---|
| fuel-zone $k_\infty$ is an infinite medium of the fuel zone only, no shell, coolant or leakage | it isolates the grain-level lumping | not an HTR-10 result; only the difference between the two cases is |
| TRISO coatings smeared into the matrix in the fuel-zone example | only the kernel matters at resonance energies; the coatings are moderator | not measured here |
| UO₂ in the fuel-zone example is free gas (no U-in-UO₂ / O-in-UO₂ law) | the graphite law dominates the thermal spectrum | not measured here; the HTR-10 core runs carry both laws |
| DH shortcuts judged on the FHR cell, not on HTR-10 | that is the problem the treatments were built and verified on | the biases are for that packing fraction (0.30) and spectrum |
| HTR-10 liberties | see the table in step 7 | see the table in step 7 |

The rung-5 working files are in
[`verification_and_validation/tutorial_rung5/`](https://github.com/theodoreOnzGit/outram-park-backend/tree/@@COMMIT@@/crates/outram-mc-libs/verification_and_validation/tutorial_rung5):
the predictions, the run logs and the results.

## The whole call tree

Everything `htr10_fuel_zone_kinf.rs`'s `main` reaches inside the workspace,
three calls deep. Generated by `kovan-cli code-walk`.

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs::main depth=3 -->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Everything `crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs::main` reaches in the workspace, to 3 hops: 53 functions, 33 unresolved calls. A function is expanded once; later calls to it say *(expanded elsewhere in this walk)*.

- [`htr10_fuel_zone_kinf.rs::main`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L396) `fn main()`
  - [`keff.rs::KeffSettings::default`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/keff.rs#L200) `fn default() -> Self` — A modest run (2000 histories × [30 inactive + 70 active]) with the U-235 thermal Watt spectrum, on the single-thread deterministic reference backend. · called at [L414](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L414)
    - [`variance_reduction.rs::VarianceReduction::default`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/variance_reduction.rs#L73) `fn default() -> Self` — Analog. · called at [L210](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/keff.rs#L210)
  - [`htr10_fuel_zone_kinf.rs::low_tier_nuclides`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L180) `fn low_tier_nuclides() -> (Vec<Nuclide>, Layout, String, String)` — The 2026-09-11 data route: embedded WMP CORE library with its 10-group fast fallback, free-gas carbon. · called at [L420](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L420)
    - [`nuclide.rs::Nuclide::from_core`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L529) `pub fn from_core(name: &str) -> Result<Self, NjoyError>` — **LOW fidelity.** Resolve a nuclide from the embedded CORE nuclear-data libraries. · called at [L182](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L182)
      - [`blob.rs::WmpLibrary::core`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/wmp/blob.rs#L430) `pub fn core() -> &'static WmpLibrary` — The embedded **CORE** nuclide set — 125 reactor-grade + LFTR nuclides (ENDF/B-VII.1 windowed multipole, MIT CRPG; see `docs/wmp-nuclide-manifest.md`), baked into the crate so every build resolves cross sections offline with no HDF5 and no downloads. · called at [L530](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L530) · *(calls below the depth limit not shown)*
      - [`blob.rs::WmpLibrary::get`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/wmp/blob.rs#L464) `pub fn get(&self, name: &str) -> Result<WindowedMultipole, NjoyError>` — Decode one nuclide by name. · called at [L530](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L530) · *(calls below the depth limit not shown)*
      - [`mod.rs::MgxsLibrary::core`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/nuclear_data/mod.rs#L600) `pub fn core() -> &'static MgxsLibrary` — The embedded CORE fast-range MGXS library (ENDF/B-VIII.0 MF=3 background, Watt-collapsed to 10 groups per nuclide from each WMP `e_max` up to 20 MeV). · called at [L533](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L533) · *(calls below the depth limit not shown)*
      - [`mod.rs::MgxsLibrary::get`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/nuclear_data/mod.rs#L589) `pub fn get(&self, name: &str) -> Option<&Mgxs>` — The fast-MGXS set for `name`, if present. · called at [L533](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L533) · *(calls below the depth limit not shown)*
      - [`nuclide.rs::nubar_for`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L5188) `fn nubar_for(name: &str, fissionable: bool) -> NuBar` — A constant-in-energy ν̄ table for one nuclide — the stopgap secondary-data source until ACER 4b (ENDF MF=1/452) is wired through njoy. · called at [L534](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L534) · *(calls below the depth limit not shown)*
      - [`secondary.rs::FissionSpectrum::default`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/nuclear_data/secondary.rs#L361) `fn default() -> Self` — A representative fast-fission Watt spectrum (U-235 thermal-fission params). · called at [L539](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L539) · *(calls below the depth limit not shown)*
      - UNRESOLVED(other): `default` at [L548](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L548) (→ [`crates/outram-mc-libs/src/material/nuclide.rs:492`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L492)) — resolves to `#[derive(Debug, Clone, Default)]`, not a function body
      - UNRESOLVED(other): `default` at [L563](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L563) (→ [`crates/outram-mc-libs/src/material/speed.rs:33`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/speed.rs#L33)) — resolves to `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]`, not a function body
  - [`htr10_fuel_zone_kinf.rs::endf_nuclides`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L216) `fn endf_nuclides(temp_k: f64) -> (Vec<Nuclide>, Layout, String, String)` — The default route: ENDF/B-VIII.0 read directly from `reference-data/endf/` through this workspace's NJOY port (RECONR + BROADR at `temp_k`, tolerance 1e-3), with the constructor's correct-physics defaults (URR probability tables and DBRC, `Nuclide::from_tape`), C-12 and C-13 at natural abundance, and bound-graphite S(alpha,beta) on both carbon isotopes. · called at [L422](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L422)
    - [`reference_data.rs::reference_endf`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/reference_data.rs#L91) `pub fn reference_endf(file: &str) -> Option<PathBuf>` — Absolute path of reference tape `file` (e.g. · called at [L218](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L218)
      - [`reference_data.rs::reference_endf_dir`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/reference_data.rs#L52) `pub fn reference_endf_dir() -> PathBuf` — The directory reference tapes are read from, whether or not it exists. · called at [L94](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/reference_data.rs#L94) · *(calls below the depth limit not shown)*
    - [`nuclide.rs::Nuclide::from_endf_file`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L1642) `pub fn from_endf_file(path: &std::path::Path, name: &str, temp_k: f64, tolerance: f64) -> Result<Self, NjoyError>` — Build a nuclide from an ENDF file **on disk** — the ordinary case. · called at [L222](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L222)
      - [`tape.rs::Tape::read_file`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/endf/tape.rs#L98) `pub fn read_file(path: &std::path::Path) -> Result<Self, NjoyError>` — Parse an ENDF ASCII tape from a file on disk. · called at [L1648](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L1648) · *(calls below the depth limit not shown)*
      - [`tape.rs::Tape::materials`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/endf/tape.rs#L225) `pub fn materials(&self) -> Vec<i32>` — Every ENDF material number on this tape, ascending and deduplicated. · called at [L1652](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L1652) · *(calls below the depth limit not shown)*
      - [`nuclide.rs::Nuclide::from_tape`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L2360) `pub fn from_tape(tape: &njoy_outram_park_fork::endf::tape::Tape, mat: i32, name: &str, temp_k: f64, tolerance: f64) -> Result<Self, NjoyError>` — Build a nuclide from an ENDF tape **already in hand** — no network, no feature gate. · called at [L1655](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L1655) · *(calls below the depth limit not shown)*
    - [`nuclide.rs::Nuclide::urr_range_ev`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L1162) `pub fn urr_range_ev(&self) -> Option<(f64, f64)>` — The unresolved range these tables cover, as `(e_low, e_high)` \[eV\], or `None` when the nuclide has no tables. · called at [L227](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L227)
    - [`nuclide.rs::Nuclide::has_dbrc`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L1310) `pub fn has_dbrc(&self) -> bool` — Whether the DBRC correction is enabled on this nuclide. · called at [L228](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L228)
    - [`thermal.rs::ThermalScattering::from_endf_file`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/thermal.rs#L639) `pub fn from_endf_file(path: &str, mat: i32, temperature_k: f64, name: &str) -> Result<Self, NjoyError>` — Build the pre-tabulated bound-atom thermal treatment — inelastic **and** elastic — from an ENDF `tsl-*` thermal evaluation file. · called at [L240](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L240)
      - [`tape.rs::Tape::read`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/endf/tape.rs#L113) `pub fn read<R: Read>(reader: R) -> Result<Self, NjoyError>` — Parse an ENDF ASCII tape from any `Read` source. · called at [L647](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/thermal.rs#L647) · *(calls below the depth limit not shown)*
      - [`thermal.rs::ThermalScattering::from_tape`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/thermal.rs#L818) `pub fn from_tape(tape: &njoy_outram_park_fork::endf::tape::Tape, mat: i32, temperature_k: f64, name: &str) -> Result<Self, NjoyError>` · called at [L648](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/thermal.rs#L648) · *(calls below the depth limit not shown)*
    - [`thermal.rs::ThermalScattering::selected_temperature_k`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/thermal.rs#L1010) `pub fn selected_temperature_k(&self) -> f64` — The temperature \[K\] the S(α,β) tables actually represent — a tabulated grid point when the request matched one within NJOY's `T/1000 + 5` K tolerance, otherwise the requested temperature itself (the tables were interpolated to it). · called at [L250](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L250)
    - UNRESOLVED(closure): `load` at [L254](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L254) (→ [`crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:217`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L217)) — call through a closure or fn-typed binding `load`
    - UNRESOLVED(closure): `load` at [L255](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L255) (→ [`crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:217`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L217)) — call through a closure or fn-typed binding `load`
    - UNRESOLVED(closure): `load` at [L256](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L256) (→ [`crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:217`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L217)) — call through a closure or fn-typed binding `load`
    - [`nuclide.rs::Nuclide::with_thermal_scattering`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L605) `pub fn with_thermal_scattering(mut self, thermal: ThermalScattering) -> Self` — Attach a bound-atom S(α,β) `ThermalScattering` treatment to this nuclide (builder style, consumes and returns `self`). · called at [L257](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L257)
    - UNRESOLVED(closure): `load` at [L257](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L257) (→ [`crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:217`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L217)) — call through a closure or fn-typed binding `load`
    - UNRESOLVED(other): `clone` at [L257](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L257) (→ [`crates/outram-mc-libs/src/material/thermal.rs:542`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/thermal.rs#L542)) — resolves to `#[derive(Debug, Clone)]`, not a function body
    - UNRESOLVED(closure): `load` at [L258](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L258) (→ [`crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:217`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L217)) — call through a closure or fn-typed binding `load`
    - UNRESOLVED(closure): `load` at [L259](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L259) (→ [`crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:217`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L217)) — call through a closure or fn-typed binding `load`
    - UNRESOLVED(closure): `load` at [L260](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L260) (→ [`crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:217`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L217)) — call through a closure or fn-typed binding `load`
  - [`htr10_fuel_zone_kinf.rs::build_materials`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L283) `fn build_materials(l: &Layout, temperature: f64, f: f64) -> (Material, Material, Material)` — Kernel, matrix and the inventory-matched homogenised fuel zone, for either route. · called at [L425](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L425)
    - UNRESOLVED(closure): `mk` at [L296](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L296) (→ [`crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:284`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L284)) — call through a closure or fn-typed binding `mk`
    - UNRESOLVED(closure): `mk` at [L313](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L313) (→ [`crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:284`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L284)) — call through a closure or fn-typed binding `mk`
    - UNRESOLVED(closure): `mk` at [L327](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L327) (→ [`crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:284`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L284)) — call through a closure or fn-typed binding `mk`
  - [`sphere_packing.rs::PackedSpheres::pack`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L340) `pub fn pack(radius: f64, half_width: f64, packing_fraction: f64, seed: u64) -> Result<Self, PackingError>` — Pack a cubic domain by RSA and build the membership grid in one step. · called at [L427](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L427)
    - [`sphere_packing.rs::pack_spheres`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L207) `pub fn pack_spheres(radius: f64, half_width: f64, packing_fraction: f64, seed: u64) -> Result<Vec<Sphere>, PackingError>` — Random Sequential Addition packing of equal spheres in a cubic domain. · called at [L346](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L346)
      - [`sphere_packing.rs::sphere_count`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L172) `fn sphere_count(radius: f64, half_width: f64, packing_fraction: f64) -> usize` — Number of equal-radius spheres a target packing fraction implies in a cube. · called at [L223](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L223) · *(calls below the depth limit not shown)*
      - UNRESOLVED(closure): `cell_of` at [L245](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L245) (→ [`crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs:242`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L242)) — call through a closure or fn-typed binding `cell_of`
      - UNRESOLVED(closure): `cell_of` at [L246](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L246) (→ [`crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs:242`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L242)) — call through a closure or fn-typed binding `cell_of`
      - UNRESOLVED(closure): `cell_of` at [L247](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L247) (→ [`crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs:242`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L242)) — call through a closure or fn-typed binding `cell_of`
      - [`lcg.rs::prn`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/petir/src/rng/lcg.rs#L127) `pub fn prn(seed: &mut u64) -> f64` — Advance the seed one step and return a uniform sample in [0, 1). · called at [L275](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L275) · *(calls below the depth limit not shown)*
      - UNRESOLVED(closure): `cell_of` at [L278](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L278) (→ [`crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs:242`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L242)) — call through a closure or fn-typed binding `cell_of`
      - [`position.rs::Position::new`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/position.rs#L64) `pub fn new(x: f64, y: f64, z: f64) -> Self` · called at [L296](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L296) · *(calls below the depth limit not shown)*
      - UNRESOLVED(closure): `nearby` at [L297](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L297) (→ [`crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs:244`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L244)) — call through a closure or fn-typed binding `nearby`
      - UNRESOLVED(closure): `nearby` at [L298](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L298) (→ [`crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs:244`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L244)) — call through a closure or fn-typed binding `nearby`
      - UNRESOLVED(closure): `nearby` at [L299](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L299) (→ [`crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs:244`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L244)) — call through a closure or fn-typed binding `nearby`
    - [`sphere_packing.rs::PackedSpheres::from_spheres`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L355) `pub fn from_spheres(spheres: Vec<Sphere>, half_width: f64, radius: f64) -> Self` — Build a membership grid over an already-generated packing. · called at [L347](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L347)
      - UNRESOLVED(closure): `cell_of` at [L362](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L362) (→ [`crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs:358`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L358)) — call through a closure or fn-typed binding `cell_of`
      - UNRESOLVED(closure): `cell_of` at [L363](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L363) (→ [`crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs:358`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L358)) — call through a closure or fn-typed binding `cell_of`
      - UNRESOLVED(closure): `cell_of` at [L364](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L364) (→ [`crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs:358`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L358)) — call through a closure or fn-typed binding `cell_of`
  - [`sphere_packing.rs::PackedSpheres::len`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L484) `pub fn len(&self) -> usize` — Number of packed kernels. · called at [L444](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L444)
  - [`sphere_packing.rs::PackedSpheres::packing_fraction`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L508) `pub fn packing_fraction(&self) -> f64` — Realized volumetric packing fraction `N · V_sphere / V_cube`. · called at [L447](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L447)
  - UNRESOLVED(closure): `arg` at [L466](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L466) (→ [`crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:454`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L454)) — call through a closure or fn-typed binding `arg`
  - UNRESOLVED(closure): `arg` at [L467](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L467) (→ [`crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:454`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L454)) — call through a closure or fn-typed binding `arg`
  - UNRESOLVED(closure): `arg` at [L468](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L468) (→ [`crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:454`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L454)) — call through a closure or fn-typed binding `arg`
  - [`delta_tracking.rs::Majorant::bounding_without_breakpoints`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L286) `pub fn bounding_without_breakpoints(materials: &[Material], nuclides: &[Nuclide], e_min: f64, e_max: f64, n_bins: usize, subsamples: usize, margin: f64) -> Self` — **ABLATION — not a bound on pointwise data.** The pre-#585 `Self::bounding`: `Σ_t` sampled at `subsamples` points in each of `n_bins` log bins, the bin maximum written to both bin edges, times `1 + margin`. · called at [L487](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L487)
    - [`delta_tracking.rs::sampled_envelope`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L510) `fn sampled_envelope(materials: &[Material], nuclides: &[Nuclide], e_min: f64, e_max: f64, n_bins: usize, subsamples: usize) -> (Majorant, Vec<f64>)` — The sampled bin envelope shared by `Majorant::bounding` (as its floor) and `Majorant::bounding_without_breakpoints`: `n_bins` log bins over `[e_min, e_max]`, `Σ_t` sampled at `subsamples` points per bin, and the bin maximum written to both bin edges. · called at [L296](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L296)
      - [`mathf.rs::f64::r_ln`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/mathf.rs#L110) `fn r_ln(self) -> f64` · called at [L520](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L520) · *(calls below the depth limit not shown)*
      - [`mathf.rs::f64::r_exp`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/mathf.rs#L116) `fn r_exp(self) -> f64` · called at [L522](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L522) · *(calls below the depth limit not shown)*
      - [`material.rs::Material::macro_xs_total_upper_bound`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/material.rs#L174) `pub fn macro_xs_total_upper_bound(&self, e: f64, nuclides: &[Nuclide]) -> f64` — An upper bound on Σ_t(E) \[cm⁻¹\] over every URR band, for delta-tracking majorants (GitHub #407). · called at [L526](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L526) · *(calls below the depth limit not shown)*
      - UNRESOLVED(closure): `sigma_t_max` at [L537](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L537) (→ [`crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs:523`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L523)) — call through a closure or fn-typed binding `sigma_t_max`
  - [`htr10_fuel_zone_kinf.rs::build_majorant`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L358) `fn build_majorant(materials: &[Material], nuclides: &[Nuclide]) -> Majorant` — The delta-tracking majorant: `Majorant::bounding` over `[1e-5, 2e7]` eV, 4096 x 32, margin 0.1. · called at [L489](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L489)
    - [`delta_tracking.rs::Majorant::bounding`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L220) `pub fn bounding(materials: &[Material], nuclides: &[Nuclide], e_min: f64, e_max: f64, n_bins: usize, subsamples: usize, margin: f64) -> Self` — Build a majorant that bounds `Σ_t` **by construction** wherever the data is pointwise, thermal (S(α,β)) or multigroup, and by dense sampling where it is analytic (WMP) or a URR band total. · called at [L359](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L359)
      - `material.rs::Material::macro_xs_total_upper_bound` *(expanded elsewhere in this walk)* · called at [L233](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L233)
      - `delta_tracking.rs::sampled_envelope` *(expanded elsewhere in this walk)* · called at [L240](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L240)
      - [`delta_tracking.rs::used_nuclides`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L495) `fn used_nuclides(materials: &[Material]) -> Vec<usize>` — The distinct nuclide indices `materials` refer to, ascending. · called at [L247](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L247) · *(calls below the depth limit not shown)*
      - [`nuclide.rs::Nuclide::majorant_breakpoints`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L3973) `pub fn majorant_breakpoints(&self, e_min_ev: f64, e_max_ev: f64) -> Vec<f64>` — Every energy \[eV\] in `[e_min_ev, e_max_ev]` at which `Self::total_upper_bound` changes form, unsorted and possibly with duplicates: `Self::native_energy_grid` (the pointwise section grids, or the WMP window edges and group bounds), plus the S(alpha,beta) `ThermalScattering::breakpoints` and the URR table energies and range ends. · called at [L249](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L249) · *(calls below the depth limit not shown)*
      - UNRESOLVED(closure): `sigma_t_max` at [L263](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L263) (→ [`crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs:230`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L230)) — call through a closure or fn-typed binding `sigma_t_max`
      - UNRESOLVED(closure): `sigma_t_max` at [L264](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L264) (→ [`crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs:230`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L230)) — call through a closure or fn-typed binding `sigma_t_max`
      - UNRESOLVED(closure): `sigma_t_max` at [L265](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L265) (→ [`crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs:230`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L230)) — call through a closure or fn-typed binding `sigma_t_max`
      - [`delta_tracking.rs::Majorant::at`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L407) `pub fn at(&self, e: f64) -> f64` — The majorant Σ_maj \[cm⁻¹\] at energy `e` \[eV\] — conservative (takes the larger bracketing grid value so it never under-bounds between points). · called at [L266](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L266) · *(calls below the depth limit not shown)*
  - UNRESOLVED(closure): `majorant_for` at [L495](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L495) (→ [`crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:485`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L485)) — call through a closure or fn-typed binding `majorant_for`
  - [`htr10_fuel_zone_kinf.rs::audit_majorant`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L372) `fn audit_majorant(label: &str, materials: &[Material], nuclides: &[Nuclide], maj: &Majorant, fatal: bool)` — Check the delta-tracking majorant actually bounds `Sigma_t`, with `Majorant::audit`. · called at [L496](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L496)
    - [`delta_tracking.rs::Majorant::audit`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L319) `pub fn audit(&self, materials: &[Material], nuclides: &[Nuclide], e_lo: f64, e_hi: f64, n_log: usize) -> MajorantAudit` — Check this majorant against `Σ_t` of `materials`, at far more energies than it was built on, and return the worst ratio `Σ_t / Σ_maj` found. · called at [L379](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L379)
      - `delta_tracking.rs::used_nuclides` *(expanded elsewhere in this walk)* · called at [L329](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L329)
      - `nuclide.rs::Nuclide::majorant_breakpoints` *(expanded elsewhere in this walk)* · called at [L331](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L331)
      - [`mathf.rs::f64::r_powf`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/mathf.rs#L128) `fn r_powf(self, y: f64) -> f64` · called at [L337](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L337) · *(calls below the depth limit not shown)*
      - `delta_tracking.rs::Majorant::at` *(expanded elsewhere in this walk)* · called at [L352](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L352)
      - `material.rs::Material::macro_xs_total_upper_bound` *(expanded elsewhere in this walk)* · called at [L354](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/delta_tracking.rs#L354)
  - [`sphere_packing.rs::PackedSpheres::is_inside_kernel`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L387) `pub fn is_inside_kernel(&self, p: Position) -> bool` — Is the point `p` \[cm\] inside any packed kernel? · called at [L498](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L498)
    - UNRESOLVED(closure): `cell_of` at [L389](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L389) (→ [`crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs:388`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/sphere_packing.rs#L388)) — call through a closure or fn-typed binding `cell_of`
  - [`keff_delta.rs::run_keff_delta`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L599) `pub fn run_keff_delta<Q>(half_width: f64, materials: &[Material], nuclides: &[Nuclide], majorant: &Majorant, material_at: Q, settings: &KeffSettings) -> KeffResult where Q: MaterialQuery,` — Run fission-source power iteration over a **reflective cube** filled with a two-(or-more-)material dispersion medium, transporting each history by delta (Woodcock) tracking. · called at [L505](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L505)
    - [`keff_delta.rs::run_keff_delta_in`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L620) `pub fn run_keff_delta_in<Q>(domain: DeltaDomain, materials: &[Material], nuclides: &[Nuclide], majorant: &Majorant, material_at: Q, settings: &KeffSettings) -> KeffResult where Q: MaterialQuery,` · called at [L610](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L610)
      - [`keff_delta.rs::run_keff_delta_seq_in`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L672) `pub fn run_keff_delta_seq_in<Q>(domain: DeltaDomain, materials: &[Material], nuclides: &[Nuclide], majorant: &Majorant, material_at: Q, settings: &KeffSettings) -> KeffResult where Q: MaterialQuery,` — Scalar, single-thread delta-tracked power iteration — the **trusted, deterministic, bit-reproducible reference** backend (`ComputeType::CpuSingleThread`). · called at [L633](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L633) · *(calls below the depth limit not shown)*
      - [`keff_delta.rs::run_keff_delta_par_in`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L794) `pub fn run_keff_delta_par_in<Q>(domain: DeltaDomain, materials: &[Material], nuclides: &[Nuclide], majorant: &Majorant, material_at: Q, settings: &KeffSettings, thread_count: ThreadCount) -> KeffResult where Q: MaterialQuery,` — Rayon-parallel delta-tracked power iteration (`ComputeType::CpuMultiThread`). · called at [L635](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/keff_delta.rs#L635) · *(calls below the depth limit not shown)*
  - UNRESOLVED(closure): `majorant_for` at [L522](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L522) (→ [`crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs:485`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L485)) — call through a closure or fn-typed binding `majorant_for`
  - [`htr10_fuel_zone_kinf.rs::vv_gate`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L674) `fn vv_gate(k_het: f64, s_het: f64, k_hom: f64, s_hom: f64, dk_pcm: f64, dk_sigma_pcm: f64)` — V&V gate: the **one** claim this program is entitled to make. · called at [L571](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/htr10_fuel_zone_kinf.rs#L571)
<!-- /code-walk -->

</div>
