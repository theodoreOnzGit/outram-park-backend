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
| oblique + friction | **tangential shear-history spring**, Coulomb slip, contact torque / spin-up | 251 | `0` | `1.11e-16` | `5.68e-14` |

**Three of the four are bit-identical to upstream over the whole trajectory.**

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

### 4.2 `contact.rs` diverges from upstream in three ways

Documented rather than silently changed, because that module has its own tests
and callers; [`granular.rs`](../src/granular.rs) implements upstream's version.

1. **No tangential history.** `ξ_t = 0` is hard-coded, so there is no shear
   *spring* — only a Coulomb-capped dashpot. A static assembly built on it
   cannot carry shear, so a heap has **zero angle of repose** and a pebble bed
   will not stand up. This is the single reason `granular.rs` exists.
2. **Lever arm.** Uses `r_i` where upstream uses the contact radius
   `c_r = r_i − δ_n/2`, biasing both the slip velocity and the spin-up torque
   by `O(δ_n)` (2 % at `δ_n = 2e-4 m` on `r = 5e-3 m`).
3. **Damping branch.** Adds tangential damping unconditionally and then caps
   the sum; upstream adds damping **only while sticking**, and on slip rescales
   the elastic force alone *and writes the rescaled displacement back* to the
   history.

---

## 5. What is still NOT validated

Unchanged from the 2026-09-06 declaration, and not weakened by anything above:

- **No experimental comparison.** Nothing here is compared against a measured
  pebble bed, an angle-of-repose experiment, or a published granular benchmark.
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
- **Cohesion and rolling models are not translated.** Upstream's
  `cohesion_model_sjkr*`, `rolling_model_epsd*` and the `limitForce` /
  `viscous` / `heating` switches are not ported (see `granular.rs`
  "Honest scope").
- **Mixed materials are not supported.** A single shared material is assumed;
  upstream carries per-type-pair property matrices.
- **No human V&V.** Everything here is AI-generated draft material under
  `RESPONSIBLE_USE.md` until the maintainer reviews it. The `README.md`
  bookkeeping axes remain **❌ Not yet manually checked**, and nothing in this
  document flips them.

---

## 6. Reproducing

```bash
# unit tests (100)
cargo test --release -p outram-park-fork-liggghts --lib

# cross-code tests against the committed LIGGGHTS reference data (4)
cargo test --release -p outram-park-fork-liggghts --test liggghts_cross_code

# bulk pebble-bed settling (long: ~210 s)
cargo test --release -p outram-park-fork-liggghts --test pebble_bed_bulk -- --ignored
```

Regenerating the reference data from upstream is described in
`reference-data/liggghts/README.md`.
