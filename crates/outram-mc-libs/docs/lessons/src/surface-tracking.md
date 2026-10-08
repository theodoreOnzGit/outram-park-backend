# Surface tracking and the leak that was not physics

## The problem

A neutron sits at a point, heading in a direction. Where does it go next? Surface
tracking answers with two distances and takes the smaller:

1. **distance to collision**, sampled as `d_col = −ln ξ / Σ_t(E)` from the
   local material's total cross section;
2. **distance to boundary**, the nearest surface or lattice-tile edge along the
   ray, over every level of nested geometry.

If the collision comes first, the neutron collides. Otherwise it is moved onto the
boundary, the boundary condition is applied, and the loop starts again in the new
cell. The module doc of
[`transport_csg.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L12-L31)
spells the loop out step by step, ported in structure from OpenMC's
`transport_history_based` [(Romano et al., 2015)](#ref-romano2015openmc).

The surface branch of that choice is short:

```rust,ignore
{{#include ../../../src/physics/transport_csg.rs:1487:1495}}
```

## Standing on a surface is the hard part

Step 2 sounds harmless. It is where the crate's most instructive tracking bug
lived.

After a crossing the particle sits, to within round-off, **exactly on** the
surface. Which cell is it in? The obvious answer is to evaluate the surface
equation at the new point and look at the sign. On the surface that sign is
decided by **rounding error**, not geometry, and it is worst at grazing incidence
on a curved surface.

If the sign comes out wrong, `locate` puts the particle back in the cell it was
leaving. The next `distance_to_boundary` then ignores the surface the particle
is standing on (it is "coincident"), finds no other surface ahead, returns
infinity, and the neutron streams out of the problem. **A tracking error reads
exactly like leakage.**

## Case study: GitHub #168

On a concentric-shell pebble with a reflective outer boundary (so physical leakage
is zero) this lost **85 % of source neutrons**. The fix has two independent parts,
both in
[`geometry/crossing/mod.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/geometry/crossing/mod.rs#L324-L369):

1. **Record the side, do not re-derive it.** At the moment of crossing the
   outgoing side is known exactly: it is the sign of `u_out · n`. The crossing
   returns it as a signed surface token, and `locate` trusts it. This is what
   OpenMC does with its signed surface index (`src/particle.cpp:344`).

2. **Nudge along the normal.** As a second guard, which OpenMC does *not* have,
   the particle is moved `1e-9` cm across the surface **along its normal**, so
   the surface equation changes sign however tangent the direction is
   ([`nudge_across`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/geometry/crossing/mod.rs#L704-L724)).

The first fix is small enough to show whole:

```rust,ignore
{{#include ../../../src/geometry/crossing/mod.rs:234:243}}
```

### What was measured

The regression test
[`reflective_concentric_shells_do_not_leak`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/geometry/crossing/tests.rs#L264-L297)
runs a full power iteration on an all-reflective shell geometry. Its doc comment
records (measured 2026-09-10): with both fixes disabled the geometry leaks
**0.0860 per source neutron** (k = 2.03432 ± 0.01644); with both in place it leaks
**exactly 0.0** (k = 2.22599 ± 0.01072). The pass criterion is leakage
< 1e-3 per source neutron. It is a conservation check on the tracker, not a
physics validation of `k`.

Note what the bug did to `k`: it moved it by about 9 % and produced a perfectly
plausible-looking number. Nothing crashed.

## A sibling bug: the right normal in the wrong frame

Surfaces inside a nested universe are defined about **that universe's origin**.
Evaluating a sphere's normal at a *global* position gives a normal about the wrong
centre, and the nudge then pushes the particle the wrong way. For planes the
normal is constant, so root-level geometries never showed it. The doc comment of
[`cross_surface_in_frame`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/geometry/crossing/mod.rs#L370-L419)
records the measurement that found it: on a 3×3×3 TRISO lattice (2026-09-14),
**37.4 %** of kernel-sphere crossings mis-assigned the next flight segment, against
**0.0 %** of root-universe plane crossings. That clean separation by frame is what
identified the cause.

## Boundary conditions, and refusing what is not implemented

[`cross_surface`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/geometry/crossing/mod.rs#L471-L561)
dispatches on the surface's boundary condition:

- **Vacuum** — the particle leaks;
- **Reflective** — specular reflection, composed over every reflective surface
  meeting at a corner;
- **White** — cosine-law diffuse re-emission (since GitHub #259);
- **Transmissive** — pass through;
- **Periodic** — *panics*.

Until 2026-09-22 `White` and `Periodic` silently fell through to specular
reflection "as a documented gap". A periodic lattice without mirror symmetry has
a genuinely different answer, and the run returned a plausible `k` either way.
Periodic is now refused loudly rather than approximated quietly;
[`validate_boundary_conditions`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/geometry/crossing/mod.rs#L294-L323)
reports it before a run if the caller asks.

**The lesson the whole chapter teaches:** in transport, a geometry defect does not
announce itself. It shows up as a believable change in leakage or `k`. The only
defence is a test whose correct answer is known exactly (here: zero leakage), not
a comparison with a benchmark that might absorb the error.

<!-- references:begin -->
## References

<p class="csl-entry" id="ref-romano2015openmc" style="padding-left: 2em; text-indent: -2em;">Romano, P. K., Horelik, N. E., Herman, B. R., Nelson, A. G., Forget, B., &#38; Smith, K. (2015). OpenMC: A state-of-the-art Monte Carlo code for research and development. <i>Annals of Nuclear Energy</i>, <i>82</i>, 90–97.</p>

<!-- references:end -->
