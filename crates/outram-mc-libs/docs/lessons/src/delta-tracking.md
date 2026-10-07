# Delta tracking: work OpenMC does not have

## The problem

A pebble-bed core holds tens of thousands of fuel pebbles, each holding thousands
of TRISO particles, each a stack of five concentric shells. Surface tracking (the
previous chapter) must find the **nearest** of all those surfaces at every flight.
Along one neutron path there can be an enormous number of them, and the neutron
stops at every one.

## The idea

Delta (Woodcock) tracking [(Leppänen, 2010)](#ref-leppanen2010delta) stops asking where the surfaces are. Pick a
**majorant** `Σ_maj(E) ≥ Σ_t(E)` that bounds the total cross section of every
material the neutron could be in. Then:

1. sample the flight on the majorant, `s = −ln ξ / Σ_maj`;
2. at the landing point, look up the **local** material's true `Σ_t`;
3. accept a **real** collision with probability `Σ_t / Σ_maj`; otherwise it is a
   **virtual** collision, nothing happens, and the flight continues.

The neutron only ever asks "what material am I in *here*?". The two primitives
are a few lines each in
[`physics/delta_tracking/flight.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/delta_tracking/flight.rs#L37-L64):

```rust,ignore
{{#include ../../../src/physics/delta_tracking/flight.rs:37:42}}

{{#include ../../../src/physics/delta_tracking/flight.rs:54:64}}
```

The method is **unbiased for any valid majorant**. A loose majorant costs time (more
virtual collisions), never accuracy. A majorant that is too *small* is the
dangerous direction, which is why
[`Majorant::bounding`](../../api/outram_mc_libs/physics/delta_tracking/majorant/struct.Majorant.html)
exists: on reconstructed resonance data a peak between two grid points would
otherwise slip under the bound. ~~the bin-maximum constructor~~ **CORRECTED
2026-10-05 (GitHub #585):** sampling each bin was not enough. On ENDF/B-VIII.0
it left the HTR-10 kernel 18 % above the majorant at 1.689 MeV. `bounding` now
tabulates on every nuclide's own breakpoints, where linear-linear data peaks.

## This is NEW WORK, not a port

OpenMC is pure surface tracking. The crate states this on the type that selects
the method,
[`TrackingMethod`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/cell.rs#L143-L187), and on
2026-10-03 the module doc of `delta_tracking.rs` was corrected to stop listing
OpenMC among the codes that have it: a search of the OpenMC source tree for "delta
tracking", "woodcock" and "majorant" finds nothing
([module doc](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/delta_tracking/mod.rs#L1-L12); the module moved to `physics::delta_tracking` on 2026-10-06, gh:#718).
The references are Woodcock et al., ANL-7050 (1965), and the codes that do use it,
Serpent and RMC.

Because there is no upstream to read, this code is verified differently: against
surface tracking on the same problem.

## Per region, not per run

Why let each region choose its tracking method instead of choosing once for the
whole run? Because one strong absorber anywhere raises the majorant
**everywhere**. The doc comment on `TrackingMethod` records the measurement
(2026-09-17, `examples/majorant_absorber_price.rs`): adding one illustrative B₄C
control rod to the bounded material set costs **26.3× in tracking steps at the
thermal peak**, and it costs that just as much in reflector graphite far from the
rod. Scoping the majorant to the region that benefits (the bed) recovers it.

## Where it plugs into the history loop

In the CSG history loop, surface and delta tracking differ in **one place**: how
far to the next real collision, and in which material.
[`transport_csg.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1474-L1562)
matches on the tracking method. The delta arm asks for the distance out of **the
region that chose delta tracking**, not the nearest surface:

```rust,ignore
{{#include ../../../src/physics/transport_csg.rs:1496:1507}}
```

That distance is a new query,
[`distance_out_of_level`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/geometry/crossing/mod.rs#L250-L280),
also new work. The ordinary `distance_to_boundary` takes the minimum over every
nesting level, so inside a bed it returns the next TRISO surface, which is exactly
what a delta tracker exists to ignore.

## Case study: the right estimator for the method

Everything after the flight is shared between the two methods, and that is where
a defect hid. Tallies under surface tracking use the **track-length** estimator:
each segment of length `d` deposits `w·d`, scored against the one material the
segment crossed. Under delta tracking a flight **crosses materials virtually**,
so there is no single material for the segment. Scoring it against the material
at the start of the flight attributes the whole path to the wrong cross sections.

The comment at
[`transport_csg.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1595-L1622)
records what that did on the HTR-10 bed with 8 groups: the tallied `k_inf` came
out **4175 pcm** below what the run's own `k_eff/(1−L)` balance implies, and
**below `k_eff`**, which is impossible for a leaking system. Surface-tracking the
same geometry closed that balance to −133 pcm. The fix is the **collision
estimator** in delta regions, scoring `w/Σ_t` at the resolved collision site,
where the material is known. That is what OpenMC and Serpent use in such regions (for Serpent, see [Leppänen, 2017](#ref-leppanen2017delta)).

Note how the defect was caught: not against a benchmark, but by a **balance the
run must satisfy with itself**.

## Verification: the two methods must agree

[`examples/godiva_delta_vs_surface_tracking.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/godiva_delta_vs_surface_tracking.rs#L55-L76)
runs Godiva both ways over 8 seeds per arm, with a 3σ pass gate (three, not one,
"a V&V gate that fires on ordinary statistical fluctuation trains people to ignore
it"). Recorded 2026-09-15, ENDF/B-VIII.0:

| arm | k_mean | sd (pcm) | sem (pcm) |
|---|---|---|---|
| surface | 1.00273 | 143 | 50 |
| delta | 1.00283 | 148 | 52 |

**surface − delta = −10 ± 73 pcm (0.1σ).** The example draws a further
conclusion: the crate sits about +250 pcm above OpenMC on identical data, and
since both tracking methods share that offset, the cause is not boundary crossing,
flight sampling or the majorant. It is in the physics the two share. These numbers
predate later changes to fission-bank resampling (GitHub #460, 2026-09-30) and
have not been re-recorded since.

<!-- references:begin -->
## References

<p class="csl-entry" id="ref-leppanen2010delta" style="padding-left: 2em; text-indent: -2em;">Leppänen, J. (2010). Performance of Woodcock Delta-Tracking in Lattice Physics Applications Using the Serpent Monte Carlo Reactor Physics Burnup Calculation Code. <span style="font-style: italic;">Annals of Nuclear Energy</span>, <span style="font-style: italic;">37</span>(5), 715–722. <a href="https://doi.org/10.1016/j.anucene.2010.01.011">https://doi.org/10.1016/j.anucene.2010.01.011</a></p>

<p class="csl-entry" id="ref-leppanen2017delta" style="padding-left: 2em; text-indent: -2em;">Leppänen, J. (2017). On the Use of Delta-Tracking and the Collision Flux Estimator in the Serpent 2 Monte Carlo Particle Transport Code. <span style="font-style: italic;">Annals of Nuclear Energy</span>, <span style="font-style: italic;">105</span>, 161–167. <a href="https://doi.org/10.1016/j.anucene.2017.03.006">https://doi.org/10.1016/j.anucene.2017.03.006</a></p>

<!-- references:end -->
