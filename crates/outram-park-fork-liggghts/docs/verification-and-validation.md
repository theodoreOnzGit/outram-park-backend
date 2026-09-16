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
