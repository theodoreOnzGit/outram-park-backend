# The problem: a particle that must hold its fission products

A high-temperature gas-cooled reactor does not rely on a metal cladding to keep
fission products in. Its fuel is thousands of **TRISO particles**, each under
a millimetre across, embedded in graphite. Every particle is its own
containment. So the questions this crate answers are, per particle:

1. **Does it stay intact?** Fission gas and CO build up inside it, and the
   ceramic shell carries the pressure. Sometimes it does not hold.
2. **If it is intact, what still gets out?** Some species diffuse through
   intact coatings, especially at high temperature.
3. **How much reaches the coolant**, and from there, the offsite chain?

## The layers

A TRISO particle is a set of concentric spheres:

| Layer | Job |
|---|---|
| **Kernel** (UO₂ here) | the fuel; fission products are born in it |
| **Buffer** (porous carbon) | void space for fission gas; absorbs recoil |
| **IPyC** (dense pyrolytic carbon) | seals the kernel, supports the SiC |
| **SiC** (silicon carbide) | the pressure vessel and the main diffusion barrier |
| **OPyC** (dense pyrolytic carbon) | protects the SiC, bonds to the matrix |

## What the code represents

The crate has **two** geometric descriptions, because it has two kinds of
model.

**For the random-walk (Lagrangian) model,** the particle is five concentric
CSG spheres, a
[`TrisoCell`](../../api/boon_lay/lagrangian_decay_simulator/lagrangian_diffusion/single_particle_simulator/constructive_solid_geometry/struct.TrisoCell.html).
Its standard instance is the IAEA CRP-6 geometry, built in
[`new_crp6_geometry`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/lagrangian_decay_simulator/lagrangian_diffusion/single_particle_simulator/constructive_solid_geometry/mod.rs#L137-L157):

```rust,ignore
{{#include ../../../src/lagrangian_decay_simulator/lagrangian_diffusion/single_particle_simulator/constructive_solid_geometry/mod.rs:137:157}}
```

That is a 425 µm kernel, then 100 / 40 / 35 / 40 µm coatings, outer radius
427.5 µm. Each region also carries a temperature (1600 °C by default, matching case
3a of Hales et al. 2021) and a fast-neutron fluence (zero by default), which the diffusion
coefficients depend on
([constructor](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/lagrangian_decay_simulator/lagrangian_diffusion/single_particle_simulator/constructive_solid_geometry/mod.rs#L84-L130)).

**For the fuel-failure model,** only the SiC shell matters mechanically. It is
a
[`SicLayer`](../../api/boon_lay/fuel_failure/geometry/struct.SicLayer.html)
with an inner and outer radius, and the model is **thin-shell throughout**:
the PANAMA-I report gives no thick-wall (Lamé) form, so none is offered
([`stress.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/stress.rs#L44-L45)).

**For the TRISO-ATOPS model,** the particle is not resolved at all. Each
chemical group of fission products is treated as diffusing out of one
*equivalent sphere* (the Booth idealisation). The next chapters show why
that is a reasonable thing to do, and where it stops being one.

## A real reactor's particle: HTR-10

The HTR-10 particle used by the crate's HTR-10 application is not typed in
again here. It is taken from
[`tampines::pebble_bed::triso::TrisoParticle::htr10`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/tampines/src/pebble_bed/triso.rs#L340-L373),
transcribed from IAEA-TECDOC-1382 part 2, Table 4-17: kernel radius 250 µm,
and cumulative outer radii 340, 380, 415 and 455 µm. Reusing one source for
the geometry means the neutronics, thermal and fuel-failure sides cannot
quietly disagree about the particle.

> **Check yourself.** The CRP-6 SiC layer and the HTR-10 SiC layer are both
> 35 µm thick, but at different radii (352.5–387.5 µm against 380–415 µm).
> Which of the two carries the larger hoop stress at the same internal
> pressure? (The thin-shell stress scales as `r·p / (2·d)`.)
