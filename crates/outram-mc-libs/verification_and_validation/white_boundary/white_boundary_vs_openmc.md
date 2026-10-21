# White boundary condition vs OpenMC — and the specular alias it replaced

GitHub issue **#259**, under epic #257. Recorded 2026-09-22.

## The defect

`src/geometry/geometry.rs:475` sent three boundary types down one arm:

```rust
BoundaryType::Reflective | BoundaryType::White | BoundaryType::Periodic => {
    // White/Periodic are approximated as reflective (documented gap).
```

`BoundaryType` declared five variants; only three distinct behaviours existed.
A run asking for a white or periodic boundary **completed and returned a
plausible `k`** for a different problem. Nothing announced the substitution.

This is the failure shape of GitHub #187 — a specularly reflective sphere
conserving impact parameter — which this repository already recorded as a
reactor-physics lesson. It is seen here from the other side: #187 was specular
used where white was meant, in a hand-built harness; this was specular used
where white was *asked for*, in the geometry kernel.

## Which upstream

The issue cites OpenMC `608a1c33`. **That commit is not available here.** The
checkout at `/opt/src/openmc` is `afa7a14ac5cb8630f642a77229ca64dc3eaeef81`
(2026-09-19, *"Enable unused variable warnings (#4142)"*), `608a1c33` is not an
object in it, and `git fetch origin 608a1c33` fails with `couldn't find remote
ref`. Every upstream line reference and every reference number below is against
**`afa7a14`**, stated rather than quietly substituted.

`afa7a14` is also the commit the `reference-data/ace` submodule already records
for the OpenMC that ran ICSBEP HEU-MET-FAST-001, so it is the version this
workspace is otherwise comparing against.

## What was ported

`Surface::diffuse_reflect` (`src/surface.cpp:144`), in full:

```cpp
Direction n = this->normal(r);  n /= n.norm();
const double projection = n.dot(u);
const double mu = (projection >= 0.0) ? -std::sqrt(prn(seed)) : std::sqrt(prn(seed));
u = rotate_angle(n, mu, nullptr, seed);
return u / u.norm();
```

The cosine (Lambert) law has `p(mu) = 2 mu`, so `F(mu) = mu^2` and the inverse
transform is `mu = sqrt(xi)`. The sign flips on the projection so the particle
re-enters the cell it came from.

Two deliberate differences from the surrounding code, both recorded because
neither is obvious:

1. **White does not go through `compose_corner_reflection`.** That routine is
   this crate's own fix for *specular* corners (GitHub #168), where composing
   the mirror off each coincident wall is what stops a corner leaking. A cosine
   re-emission has no such composition, and re-diffusing off a second wall
   samples the wrong distribution. The corner-composition filter was narrowed
   from `Reflective | White | Periodic` to `Reflective` in the same change.
2. **`Periodic` is refused, not implemented.** It needs a partner surface and
   the transform between the pair (`TranslationalPeriodicBC`,
   `RotationalPeriodicBC`), and `BoundaryType` carries no partner.
   `Geometry::validate_boundary_conditions` rejects it at construction, and
   `cross_surface` panics as a backstop for a `Geometry` assembled by hand from
   its public fields.

## This is NOT the Wigner-Seitz white boundary

`examples/lump_self_shielding_scan.rs` already had a `CellBoundary::White`. It
is a **different condition** and was deliberately not unified with this one:

| | this port (`WhiteBC`) | the lump scan (Wigner-Seitz) |
|---|---|---|
| position | crossing point kept | uniformly random on the sphere |
| direction | cosine law about the normal | cosine law inward |

Randomising the position is what destroys the impact-parameter memory that made
specular wrong by +39–50 % in that study. They share a name, not a definition.
Checked and rejected for reuse.

## Methodology

A 1-D slab, `x` in `[0, T]`, `y` and `z` in `[-1, 1]` reflective so the slab is
transverse-infinite. The **left** face carries the boundary under test; the
**right** face is vacuum.

The vacuum side is the whole design: it drives an anisotropic angular flux at
the left face. **With a symmetric problem the two conditions agree and the
comparison measures nothing** — the first attempt used a 12 cm slab and got a
3.5 sigma difference, which is why the thickness is scanned rather than chosen.

One energy group, macroscopic and dimensionless, identical on both codes:

| `total` | `absorption` | `scatter` | `fission` | `nu_fission` | `chi` |
|---|---|---|---|---|---|
| 1.0 | 0.30 | 0.70 | 0.20 | 0.50 | 1.0 |

OpenMC `afa7a14`, multi-group mode, 220 batches / 20 inactive / 40 000
particles / seed 1. Deck in `openmc_inputs/` (`mk.py` writes `mgxs.h5`,
`run.py <bc> <T>` runs one case). This port: 20 000 particles / 30 inactive /
200 active / seed `0x5EED_0259`.

Multi-group mode was chosen because it needs **no nuclear data files**, and
none are installed here (`OPENMC_CROSS_SECTIONS` is unset and no
`cross_sections.xml` exists on this machine). A continuous-energy comparison is
blocked until the HDF5 write path of issue #270 can produce a library OpenMC
reads.

## Results (2026-09-22)

OpenMC `afa7a14`:

| T [cm] | reflective | white | white − reflective | sigma |
|---|---|---|---|---|
| 0.5 | 0.546271 ± 0.000155 | 0.524397 ± 0.000137 | **−0.021874 ± 0.000207** | 106 |
| 1.0 | 0.865993 ± 0.000186 | 0.854308 ± 0.000185 | −0.011685 ± 0.000262 | 45 |
| 3.0 | 1.395552 ± 0.000213 | 1.394392 ± 0.000201 | −0.001160 ± 0.000293 | 4.0 |
| 12.0 | 1.638574 ± 0.000089 | 1.638996 ± 0.000084 | +0.000422 ± 0.000122 | 3.5 |

This port:

| T [cm] | reflective | white | white − reflective |
|---|---|---|---|
| 0.5 | 0.546426 ± 0.000535 | 0.524448 ± 0.000476 | **−0.021978** |
| 1.0 | 0.866237 ± 0.000620 | 0.852828 ± 0.000596 | −0.013409 |
| 3.0 | 1.395147 ± 0.000628 | 1.395002 ± 0.000547 | −0.000145 |
| 12.0 | 1.639237 ± 0.000589 | 1.638881 ± 0.000599 | −0.000356 |

Absolute agreement, against a combined 4-sigma budget built from each run's own
reported stderr:

| T [cm] | reflective \|d\| | white \|d\| | budget |
|---|---|---|---|
| 0.5 | 0.000155 | **0.000051** | 0.0023 |
| 1.0 | 0.000244 | 0.001480 | 0.0025 |
| 3.0 | 0.000405 | 0.000610 | 0.0024 |
| 12.0 | 0.000663 | 0.000115 | 0.0025 |

## Interpretation

**The load-bearing number is −0.0220 against OpenMC's −0.0219 at T = 0.5.** That
difference was **identically zero** before this change, because both arms ran
the same code. A gate on it could not have been written.

Every absolute comparison sits inside its combined 4-sigma budget. The largest
single deviation is `T = 1.0` white at 2.3 sigma — not a defect, but the one to
watch if this gate starts drifting, and recorded rather than averaged away.

At `T = 3.0` and `T = 12.0` this port's difference does not reproduce OpenMC's
sign. Both are inside the two runs' combined noise (OpenMC itself is only at 4.0
and 3.5 sigma there), so **no claim is made either way** and the gate does not
assert on them. Asserting a 3.5 sigma sign would be asserting noise.

## A prediction that was wrong

Before measuring, the predicted sign was **white above reflective**, reasoning
that cosine re-emission favours grazing directions which dwell longer in the
slab.

That is backwards, and the measurement says so at 106 sigma. `p(mu) = 2 mu`
**peaks at normal incidence**, so a white face fires neutrons more nearly
straight across the slab and out of the vacuum side: leakage rises and `k`
falls. The specular face instead preserves `|mu_x|`, so a neutron arriving
grazing leaves grazing and keeps dwelling.

Recorded because it was wrong. The prediction was stated before the run, the run
contradicted it, and the physics reading was corrected rather than the
comparison.

## Audit of existing results (issue step 4)

**Nothing in the workspace constructed a `White` or `Periodic` surface.**
`grep -rn "BoundaryType::Periodic\|BoundaryType::White" --include=*.rs crates/`
returned exactly two hits, both the aliasing sites in `geometry.rs` itself.

So no existing V&V number rested on the approximation and **nothing had to be
re-measured**. That is the audit result, not an absence of one — and it is why
this change could be made without re-running the ICSBEP or ring-RPT sets.

## Still open on this issue

`TranslationalPeriodicBC` and `RotationalPeriodicBC` are not ported. Until they
are, periodic is refused rather than approximated, which is the minimum the
issue asks for. The acceptance item *"a non-mirror-symmetric lattice gives
different `k` under periodic than under reflective"* is therefore **not met**
and is not claimed.
