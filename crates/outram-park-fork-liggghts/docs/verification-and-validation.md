# Verification & validation — `outram-park-fork-liggghts`

**Status as of 2026-09-15.** Methodology *and* measured results, per the
workspace `CLAUDE.md` rule that a V&V document must state both.

Summary of what changed on this date: the crate gained a **cross-code
verification leg against upstream LIGGGHTS-PUBLIC**, which its 2026-09-06
maturity declaration explicitly recorded as missing. Three of four deterministic
cases are **bit-identical** to upstream; the fourth agrees to floating-point
round-off. A bulk pebble-bed settling case agrees on packing fraction to
**0.2 %**. Two defects in the pre-existing code were found and are documented
below.

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

All cases: monodisperse spheres `d = 10 mm`, `ρ = 2500 kg/m³`, `E = 10 MPa`,
`ν = 0.3`, SI units, `fix nve/sphere`, `pair_style gran`.

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
`tests/angle_of_repose.rs`, `#[ignore]`d (~25 min).

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

### Results (2026-09-16)

| Quantity | this crate | LIGGGHTS | difference |
|---|---|---|---|
| angle of repose | **12.78 deg** | **15.43 deg** | 2.65 deg |
| heap apex | 0.0347 m | 0.0370 m | 6.2 % |
| residual `KE` | `2.8e-10 J` (settled) | `9.0e-11 J` (settled) | — |
| particles | 656 (none lost) | 656 (none lost) | — |

A heap forms in both codes and both come to rest. The `2.65 deg` gap is larger
than the round-off agreement the primitive-wall cases reach, and the reason was
stated before the run rather than after: LIGGGHTS' `TriMesh` resolves a particle
touching several facets at a shared edge, while this crate's `MeshWall` takes
only the **single nearest facet**. On a tessellated cylinder that differs for
every particle sitting on a vertical edge between adjacent quads — which, for
particles pressed against the wall, is most of them. This is a **statistical**
comparison of the resulting heap, not a trajectory comparison, and the test's
`3 deg` bound is a regression catch that the measurement only just clears.

Closing that gap means porting upstream's multi-facet mesh contact resolution.
That is real work and is **not** done.

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
| `5e8 Pa` | 18x | *(see the test)* | — |

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

### The bed is looser than the benchmark's nominal figure

At `E = 3e8`, both codes settle to `φ ≈ 0.575` against the published `0.61`, and
the bed stands `2.16 m` tall against the published `1.97 m`. **This is not a
code disagreement** — the two codes agree with each other to four decimals.

Two things are being compared that are not the same quantity. The published
`0.61` is a *design closure*, 27 000 pebbles divided by a nominal 5.0 m³ core
volume, not a measured packing fraction; and the slab measurement here is
deliberately bulk-only. Beyond that, a DEM random packing at `µ = 0.4`,
`µ_r = 0.1` simply packs looser than 0.61 — reproducing the benchmark density
would need those friction parameters calibrated down. **Nothing here validates
either number**; it says what this contact model, at these parameters, produces.

## 5. What is still NOT validated

Unchanged from the 2026-09-06 declaration, and not weakened by anything above:

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
  edge contacts. This is why § 4.6 agrees to `2.65 deg` rather than to
  round-off. Porting upstream's resolution is outstanding work.
- **Bulk packing is verified against LIGGGHTS, not against reality.** For
  reference, the settled voidage both codes produce (`ε ≈ 0.442`) sits above the
  Dixon (1988) correlation value for `D/d = 6` (`ε = 0.4198`, i.e. `φ = 0.5802`)
  — a 2.2 percentage-point difference. That is a plausible gap for a
  friction-dominated DEM pour versus a poured/settled experimental correlation,
  but it is **an observation, not a validation**, and no attempt has been made
  here to calibrate `µ`, `e` or `E` against bed data.
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
# unit tests (102)
cargo test --release -p outram-park-fork-liggghts --lib

# cross-code tests against the committed LIGGGHTS reference data (5)
cargo test --release -p outram-park-fork-liggghts --test liggghts_cross_code

# bulk pebble-bed settling (long: ~210 s)
cargo test --release -p outram-park-fork-liggghts --test pebble_bed_bulk -- --ignored
```

Regenerating the reference data from upstream is described in
`reference-data/liggghts/README.md`.
