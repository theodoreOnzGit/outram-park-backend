# FARRER PARK — verification report

**Status: AI-assisted draft. Verification only. NO human V&V sign-off.**
Nothing in this document is validation. Every case here compares the code
against an analytical solution, a manufactured solution, or an independent
numerical derivative of its own formulae. No physical measurement, and no
published benchmark, appears anywhere in it. See `RESPONSIBLE_USE.md` and
`VERIFICATION_AND_VALIDATION.md` at the workspace root for what that
distinction means and why it is enforced.

All numbers below were produced by running the test suite in release mode on
**2026-09-11**:

```bash
cargo test --release -p farrer-park
```

Each case's methodology and results are also recorded in the `///` doc comment
of the test that produces them, per the workspace `CLAUDE.md` rule that a V&V
test must document both. This file is the collected view, not a second source
of truth: if the two ever disagree, the test is right and this file is stale.

---

## Summary

| # | Case | Test | Headline result |
|---|---|---|---|
| 1 | Patch test, all element types | `tests/patch.rs` | machine precision: displacement 9.0e-17, stress 1.3e-14 worst |
| 2 | Manufactured-solution convergence | `tests/mms.rs` | orders 1.998 / 1.000 (linear), 2.978 / 1.989 (quadratic) |
| 3 | Thick-walled cylinder vs Lame | `tests/analytical.rs` | 2.45 % max stress error, first order; 0.021 % displacement, second order |
| 4 | Cantilever vs beam theory | `tests/analytical.rs` | excess over Euler-Bernoulli 3.677 / 0.928 / 0.233 % at L/H = 4 / 8 / 16 |
| 5 | Uniaxial J2 plasticity | `tests/plasticity.rs` | exact to round-off (1.95e-14) on loading, unloading and reverse yield |
| 5b | Consistent tangent | `tests/plasticity.rs` | agrees with a central difference to 1.055e-7 relative |
| 5c | Newton convergence order | `tests/plasticity.rs` | **2.004** measured on a partially plastic step |
| 6a | Volumetric locking, Quad4, `nu = 0.499` | `tests/locking.rs` | full integration order 0.69-1.24; **B-bar 2.004**; 16.96x smaller error |
| 6b | Volumetric locking, Hex8, `nu = 0.499` | `tests/locking.rs` | full integration order **0.634**; **B-bar 2.050**; 6.36x smaller error |
| 7 | Fully plastic limit load vs closed form | `tests/locking.rs` | full integration +23.9 % to +0.42 %; **B-bar +1.06 % to +0.017 %** |
| 8 | Shear locking, Quad4 cantilever | `tests/locking.rs` | 11.25 % too stiff at `ny = 2`, 66.7 % at aspect 4; **B-bar does NOT cure it** |
| 9a | Thin plate, plane stress vs plane strain | `tests/plane_stress.rs` | both exact to round-off; contraction differs by exactly `1 + nu` |
| 9b | Plane-stress / plane-strain equivalence | `tests/plane_stress.rs` | agree to 2.9e-16 under the exact `(E*, nu*)` transformation |
| 10 | Plane-stress J2, uniaxial | `tests/plane_stress.rs` | exact to round-off (3.58e-16); `sigma_zz` identically zero |
| 11 | Plane-stress J2, equibiaxial | `tests/plane_stress.rs` | exact to round-off (6.77e-16); `q = sigma_xx` to 3.5e-16 |

Shared configuration: isotropic linear elasticity or J2 plasticity with linear
isotropic hardening; two-dimensional cases are **plane strain** unless stated
otherwise (plane stress is implemented as of 2026-09-11, cases 9-11); the
element formulation is **full integration** unless stated otherwise (B-bar is
implemented as of 2026-09-11, cases 6-8); linear systems solved by
ILU(0)-preconditioned conjugate gradients through `outram-foam-basic-lib`'s
shared Krylov layer, except the locking cases, which use Jacobi — see case 6.

---

## 1. Patch test

### Methodology

An irregular patch of each element type — unit square in two dimensions, unit
cube in three — with three divisions per side and **every interior node
displaced** by up to 12 % of the element size (25 % for Tri6), deterministically
seeded. A patch test on a uniform grid is much weaker, because a uniform grid
passes for reasons unrelated to the element formulation.

The exact linear displacement field

- 2-D: `u_x = 1e-4 (1 + 2x + 3y)`, `u_y = 1e-4 (-0.5 + 1.5x - 2y)`
- 3-D: `u_x = 1e-4 (1 + 2x + 3y - z)`, `u_y = 1e-4 (-0.5 + 1.5x - 2y + 0.5z)`,
  `u_z = 1e-4 (0.25 - x + 0.75y + 1.25z)`

is imposed on every boundary degree of freedom by strong elimination. Material
`E = 200 GPa`, `nu = 0.3`. Linear solves to `1e-13` relative residual.

**Pass criterion:** the interior must reproduce the field, and every quadrature
point must carry the single constant stress `C_e : eps_exact`, both to better
than `1e-9` relative — several orders looser than what is measured, so the
assertion is a regression guard rather than a precision claim.

Tri6 is built by promoting an **already-jittered** Tri3 mesh, so its midside
nodes land at the midpoints of the distorted edges and the elements stay
straight-sided. Jittering a Tri6 mesh directly would curve the edges, on which a
linear field genuinely is not reproducible.

### Results

| Element | Nodes | Elements | max rel. displacement error | max rel. stress error |
|---|---|---|---|---|
| Tri3 | 16 | 18 | 9.035e-17 | 2.690e-15 |
| Tri6 | 49 | 18 | 1.807e-16 | 1.313e-14 |
| Quad4 | 16 | 9 | 9.035e-17 | 2.475e-15 |
| Tet4 | 64 | 162 | 9.035e-17 | 1.978e-15 |
| Hex8 | 64 | 27 | 9.035e-17 | 1.649e-15 |

Penalty-method variant (Quad4, `beta = 1e10` relative to the largest diagonal):

| Quantity | Value |
|---|---|
| max relative displacement error | 4.005e-12 |
| max relative stress error | 4.794e-11 |

### Interpretation

Every element passes at machine precision. The displacement errors are one to
two units in the last place; the stress errors are a decade higher because
forming a stress multiplies nodal round-off by the elastic modulus and
differentiates the shape functions. Tri6's stress error is the largest of the
five, which is expected rather than suspicious: its gradients vary within the
element, so the strain is a longer floating-point sum.

This verifies the shape functions and their derivatives, the isoparametric
Jacobian on a distorted element, and the assembly and scatter path. It verifies
nothing about the convergence *rate*, and nothing about accuracy on a problem
whose exact solution is not linear.

The penalty method passes only to about `1 / beta` — five orders worse than
elimination — exactly as its theory says. That is why elimination is the
default, and the test asserts the penalty error is **greater** than `1e-14` so
that a change which silently routed the penalty path to elimination would fail
rather than quietly pass.

---

## 2. Manufactured-solution convergence

This is the most important case in the suite: it measures the *order* of the
method, not merely that it runs.

### Methodology

Domain: the unit square, plane strain, `E = 200 GPa`, `nu = 0.3`
(`lambda = 115.385 GPa`, `mu = 76.923 GPa`).

Manufactured displacement, metres:

$$u_x = u_y = \sin(\pi x)\sin(\pi y)$$

which vanishes on the entire boundary, so the Dirichlet data contributes no
boundary-approximation error of its own. Substituting into the plane-strain
Navier equation

$$(\lambda + \mu)\nabla(\nabla \cdot u) + \mu \nabla^2 u + f = 0$$

gives the body force, newton per cubic metre:

$$f_x = f_y = \pi^2 \left[ (\lambda + 3\mu)\sin(\pi x)\sin(\pi y) - (\lambda + \mu)\cos(\pi x)\cos(\pi y) \right]$$

Errors are integrated with a **richer quadrature rule than assembly uses**,
because $(u_h - u)^2$ is not a polynomial and the assembly rule would measure
its own quadrature error instead of the discretisation error. Each refinement
halves $h$, so the observed order is $\log_2(e_\text{coarse} / e_\text{fine})$.

For the three-dimensional cases the same field is used on the unit cube with
$u_z = 0$; it remains an exact solution because every $z$ derivative vanishes.
The exact field — not zero — is imposed on all six faces, since it does not
vanish on $z = 0$ or $z = 1$.

**Pass criterion:** finest observed order within 0.15 of theory ($h^{p+1}$ in
$L_2$, $h^p$ in $H_1$).

### Results

**Quad4** (`p = 1`):

| n | dofs | L2 | order | H1 | order |
|---|---|---|---|---|---|
| 4 | 50 | 4.37275e-2 | | 7.09755e-1 | |
| 8 | 162 | 1.11518e-2 | 1.971 | 3.55844e-1 | 0.996 |
| 16 | 578 | 2.80446e-3 | 1.991 | 1.78034e-1 | 0.999 |
| 32 | 2178 | 7.02209e-4 | **1.998** | 8.90303e-2 | **1.000** |

**Tri3** (`p = 1`):

| n | dofs | L2 | order | H1 | order |
|---|---|---|---|---|---|
| 4 | 50 | 8.22110e-2 | | 1.19874e+0 | |
| 8 | 162 | 2.12183e-2 | 1.954 | 6.12430e-1 | 0.969 |
| 16 | 578 | 5.34688e-3 | 1.989 | 3.07871e-1 | 0.992 |
| 32 | 2178 | 1.33938e-3 | **1.997** | 1.54143e-1 | **0.998** |

**Tri6** (`p = 2`):

| n | dofs | L2 | order | H1 | order |
|---|---|---|---|---|---|
| 2 | 50 | 3.40645e-2 | | 6.69263e-1 | |
| 4 | 162 | 4.82246e-3 | 2.820 | 1.84008e-1 | 1.863 |
| 8 | 578 | 6.33194e-4 | 2.929 | 4.72934e-2 | 1.960 |
| 16 | 2178 | 8.03541e-5 | **2.978** | 1.19117e-2 | **1.989** |

**Hex8** (`p = 1`, three-dimensional):

| n | dofs | L2 | order | H1 | order |
|---|---|---|---|---|---|
| 2 | 81 | 2.03345e-1 | | 1.45631e+0 | |
| 4 | 375 | 5.35370e-2 | 1.925 | 7.18005e-1 | 1.020 |
| 8 | 2187 | 1.35460e-2 | **1.983** | 3.56977e-1 | **1.008** |

**Tet4** (`p = 1`, three-dimensional):

| n | dofs | L2 | order | H1 | order |
|---|---|---|---|---|---|
| 2 | 81 | 2.77936e-1 | | 2.20001e+0 | |
| 4 | 375 | 7.92357e-2 | 1.811 | 1.19951e+0 | 0.875 |
| 8 | 2187 | 2.04129e-2 | **1.957** | 6.12571e-1 | **0.969** |

### Interpretation

All five elements converge at the theoretical rates, and the observed orders
approach theory **monotonically from below**. That is the signature of a
correctly implemented method whose higher-order terms are vanishing, rather than
of a rate that happens to look right on one pair of meshes. Orders coming in
*above* theory would be the suspicious result, since that usually means the
error norm itself is being under-integrated.

At equal degrees of freedom (2178), Quad4 is 1.91 times more accurate than Tri3
in $L_2$, and Tri6 is 8.7 times more accurate than Quad4 and 16.7 times more
accurate than Tri3. The ordering is the expected one and is a free consistency
check on three implementations at once.

Tet4 is visibly pre-asymptotic on its coarse-to-medium pair (1.811, 0.875) and
catches up by the finest pair, which is unsurprising given the Kuhn subdivision
produces six rather elongated tetrahedra per cell.

At equal $h$, Hex8's $L_2$ error (1.355e-2 at $n = 8$) is 21 % larger than the
two-dimensional Quad4 result (1.115e-2) even though the exact solution is
$z$-independent. That is not an inconsistency: a trilinear hexahedron restricted
to a $z$-independent field is not the same discrete space as a bilinear
quadrilateral, because the $z$ interpolation participates in the volume
integrals.

**Limitations.** One material, well away from the incompressible limit where
these elements lock; one smooth solution; uniform meshes; straight-sided
quadratic triangles, so no curved isoparametric mapping is exercised; and a
$z$-extruded rather than genuinely three-dimensional manufactured field in 3-D.
Each of those would need its own study before the corresponding claim could be
made.

---

## 3. Thick-walled cylinder against the Lame solution

### Methodology

Inner radius `a = 0.05 m`, outer `b = 0.10 m`, quarter model, `E = 200 GPa`,
`nu = 0.3`, plane strain, internal pressure `p = 100 MPa` applied as a
consistent nodal traction on the bore facets, each using its own chord normal.
Symmetry conditions `u_y = 0` on the `theta = 0` edge and `u_x = 0` on the
`theta = pi/2` edge remove all three rigid-body modes. Quad4 elements, 2x2
Gauss.

Reference, in pascals, tension positive:

$$\sigma_r = \frac{a^2 p}{b^2 - a^2}\left(1 - \frac{b^2}{r^2}\right), \quad
\sigma_\theta = \frac{a^2 p}{b^2 - a^2}\left(1 + \frac{b^2}{r^2}\right)$$

$$u_r = \frac{(1+\nu)a^2 p}{E(b^2 - a^2)}\left[(1 - 2\nu) r + \frac{b^2}{r}\right]$$

Stresses are compared at every quadrature point, rotated into polar components,
and normalised by $|\sigma_\theta(a)| = 166.67$ MPa. The radial displacement is
compared at every node, normalised by $u_r(a) = 5.4167 \times 10^{-5}$ m.

### Results

| n_r | n_theta | dofs | err sigma_r | err sigma_t | order | err u_r | order |
|---|---|---|---|---|---|---|---|
| 4 | 8 | 90 | 1.62736e-1 | 6.25276e-2 | | 1.28351e-2 | |
| 8 | 16 | 306 | 9.03228e-2 | 3.63722e-2 | 0.849 | 3.29054e-3 | 1.964 |
| 16 | 32 | 1122 | 4.77112e-2 | 1.97779e-2 | 0.921 | 8.28051e-4 | 1.991 |
| 32 | 64 | 4290 | 2.45398e-2 | 1.03375e-2 | **0.959** | 2.07356e-4 | **1.998** |

On the finest mesh: **max relative error 2.45 % in `sigma_r`, 1.03 % in
`sigma_theta`, 0.021 % in `u_r`**.

### Interpretation

The two measures converge at **different orders, and both are the orders theory
requires**: the primary unknown at second order (1.998) and the stress at first
order (0.959), because the stress is a first derivative of a bilinear
interpolation and so loses one order. A 2.45 % worst-case stress error on a
4290-degree-of-freedom bilinear mesh is what a Quad4 does; it is not a defect.

The magnitude is predictable, which is the real check. The worst stress error
sits on the innermost quadrature row where the gradient is steepest:
$|d\sigma_\theta / dr| = 2 k b^2 / r^3 = 5.33 \times 10^9$ Pa/m at the bore,
times the roughly $h/2 = 7.8 \times 10^{-4}$ m offset of the quadrature point
from the element centre, gives 4.2 MPa, or 2.5 % of 166.67 MPa. The measured
2.45 % is that estimate. An error that could not be accounted for this way would
be the thing to worry about.

**The faceted geometry is reported, not meshed away.** Straight-sided elements
inscribe polygons in both circles and apply the pressure on chords rather than
arcs, an $O(h^2)$ geometric error that always has the same sign. At
`n_theta = 64` the chord-to-arc radius defect is $d\theta^2/24 = 2.5\times
10^{-5}$ relative, propagating to about $5 \times 10^{-5}$ in
$\sigma_\theta$ — three orders below the discretisation error, so it is not what
limits this case. It would matter on a mesh refined radially but not
circumferentially, which is why the sweep refines both.

**Gap:** the annulus generator emits Quad4 only, so no quadratic-element
comparison is made here. A quadratic element would give second-order stresses
and a far smaller error.

---

## 4. Cantilever beam against Euler-Bernoulli and Timoshenko theory

### Methodology

Plane-strain cantilever, height `H = 0.1 m`, unit thickness, `E = 200 GPa`,
**`nu = 0`**, clamped over its whole `x = 0` edge, loaded by a uniform shear
traction on the `x = L` edge totalling `P = 1000 N` downwards. Tri6 elements,
four through the depth and `8 L/H` along the span (element aspect ratio 2 at
every slenderness). Deflection read at the mid-height node of the loaded edge.

References, unit thickness, $I = H^3/12$, $A = H$:

$$\delta_\text{EB} = \frac{P L^3}{3 E I}, \quad
\delta_\text{Timo} = \frac{P L^3}{3 E I} + \frac{P L}{k G A}, \quad k = 5/6$$

with $G = E / 2$ at $\nu = 0$. The ratio of shear to bending term is
$0.6 (H/L)^2$.

**`nu = 0` is deliberate.** At zero Poisson's ratio plane strain, plane stress
and beam theory all use the same $E$, so any discrepancy is shear deformation,
end effects or discretisation error — not a plane-strain stiffening that would
have to be corrected for separately. At $\nu = 0.3$ the plane-strain model is
stiffer by $1 - \nu^2 = 0.91$, and comparing it with an $E$-based beam formula
without that correction would produce a 9 % "discrepancy" that is purely a
modelling mismatch.

**Pass criterion: the trend, not a tolerance.** The ratio
$\delta_\text{FEM} / \delta_\text{EB}$ must decrease monotonically towards 1 as
the beam gets slenderer and must sit above 1 at every slenderness. No tolerance
was loosened to obtain the numbers below.

### Results

| L/H | nx x ny | dofs | delta_FEM (m) | delta_EB (m) | delta_Timo (m) | FEM/EB | FEM/Timo |
|---|---|---|---|---|---|---|---|
| 4 | 32x4 | 1170 | 1.327069e-6 | 1.280000e-6 | 1.328000e-6 | 1.03677 | 0.99930 |
| 8 | 64x4 | 2322 | 1.033503e-5 | 1.024000e-5 | 1.033600e-5 | 1.00928 | 0.99991 |
| 16 | 128x4 | 4626 | 8.211095e-5 | 8.192000e-5 | 8.211200e-5 | 1.00233 | 0.99999 |

### Interpretation

The excess over Euler-Bernoulli is **3.677 %, 0.928 % and 0.233 %**. The
shear-deformation estimate $0.6 (H/L)^2$ predicts **3.750 %, 0.938 %, 0.234 %**.
The measured discrepancy is therefore explained by shear deformation to within
2 % of itself at the stubbiest beam and 0.5 % at the slenderest, and it falls as
$(H/L)^2$ exactly as theory says. This is the trend the case exists to show.

Against Timoshenko theory the ratios are 0.99930, 0.99991, 0.99999 — converging
to 1 from **below**. The finite-element beam being marginally stiffer is the
expected direction for a displacement-based discretisation, which can only
over-constrain. Two effects pushing the other way are present but smaller: the
quadratic triangle's residual shear locking, and the extra compliance of a
clamped end in two-dimensional elasticity, which beam theory does not model at
all. At `L/H = 4` the net is 7 parts in 10 000.

**Element choice is stated, not buried.** Quadratic triangles were used because
low-order quadrilaterals lock severely in bending; a Quad4 mesh of this
coarseness would give a visibly too-stiff beam, and reporting that as agreement
would require many more elements through the depth or reduced integration.
Neither was done.

---

## 5. Uniaxial J2 plasticity

### 5a. Against the closed-form elastic-plastic response

#### Methodology

A single eight-node hexahedron of unit side, symmetry conditions on the three
coordinate planes, prescribed axial displacement on the `x = 1` face, other
faces traction free — so the element contracts laterally as the constitutive law
dictates and the state is genuinely uniaxial **stress**, not uniaxial strain,
and the answer is produced by the full finite-element machinery rather than by a
material-point driver.

Material: `E = 200 GPa`, `nu = 0.3`, `sigma_y0 = 250 MPa`, `H = 2 GPa`. Yield
strain `1.25e-3`; plastic tangent modulus
`E_t = E H / (E + H) = 1.980198 GPa`.

Load path: axial strain stepped to `1e-4, 2e-4, ..., 5e-3` (50 steps, four of
them elastic), then unloaded in the same steps all the way back to zero. Each
step is a full Newton solve on the consistent tangent, linear solves to `1e-13`.

References: `sigma = E eps` below yield, `sigma_y0 + E_t (eps - eps_y)` above;
on unloading, elastic from the turning point until the stress reaches
`-sigma_y(alpha_peak)` (isotropic hardening, so the surface is a sphere of that
radius), then the hardening slope in compression.

#### Results

Loading branch, selected points:

| strain | sigma_FEM (Pa) | sigma_exact (Pa) | rel err | Newton solves |
|---|---|---|---|---|
| 1.0000e-4 | 2.0000000e7 | 2.0000000e7 | 0 | 2 |
| 1.0000e-3 | 2.0000000e8 | 2.0000000e8 | 0 | 2 |
| 2.0000e-3 | 2.5148515e8 | 2.5148515e8 | 3.555e-16 | 3 |
| 3.0000e-3 | 2.5346535e8 | 2.5346535e8 | 0 | 3 |
| 4.0000e-3 | 2.5544554e8 | 2.5544554e8 | 8.167e-16 | 3 |
| 5.0000e-3 | 2.5742574e8 | 2.5742574e8 | 4.631e-16 | 3 |

Unloading from `eps = 5.0e-3`; reverse yield at `eps = 2.425743e-3`, where the
stress reaches `-sigma_y(alpha) = -2.574257e8 Pa`:

| strain | sigma_FEM (Pa) | sigma_exact (Pa) | rel err | Newton solves |
|---|---|---|---|---|
| 4.0000e-3 | 5.7425743e7 | 5.7425743e7 | 3.503e-15 | 3 |
| 3.0000e-3 | -1.4257426e8 | -1.4257426e8 | 1.045e-15 | 3 |
| 2.0000e-3 | -2.5826880e8 | -2.5826880e8 | 4.616e-16 | 2 |
| 1.0000e-3 | -2.6024900e8 | -2.6024900e8 | 2.290e-16 | 2 |
| 0 | -2.6222919e8 | -2.6222919e8 | 1.136e-16 | 2 |

| Quantity | Value |
|---|---|
| worst relative error, loading | 8.167e-16 |
| worst relative error, elastic unloading | 1.954e-14 |
| worst relative error, reverse yielding | 5.761e-16 |
| equivalent plastic strain at peak | 3.7128712871e-3 |
| equivalent plastic strain just before reverse yield | 3.7128712871e-3 |
| equivalent plastic strain at `eps = 0` | 6.1145966082e-3 |

#### Interpretation

The finite-element answer reproduces the closed form **to round-off** on all
three branches. That is the correct outcome, not merely a close one: with linear
isotropic hardening the consistency condition is linear, the return map has a
closed-form root, and the only remaining error is floating-point arithmetic.

Three further properties are verified.

**Unloading is exactly elastic.** The equivalent plastic strain is bit-identical
between peak load and the last step before reverse yield, so the yield check is
not leaking spurious plastic flow into an elastic step.

**Reverse yielding happens where isotropic hardening says.** The surface is a
sphere of radius 257.4257 MPa after loading, and the model yields again in
compression at exactly that magnitude — no Bauschinger effect, which is correct
for isotropic hardening and would be wrong for kinematic hardening. Modelling
the Bauschinger effect needs a back stress, which this law does not have and
does not claim to.

**The closed-form plastic strain is recovered:**
`alpha = eps - sigma/E = 5.0e-3 - 2.5742574e8 / 2.0e11 = 3.7128713e-3`, matching
to nine digits.

Newton takes two solves per elastic step and three per plastic step. The extra
solve is the one that discovers the point has yielded and re-linearises about
the returned state.

### 5b. Consistent tangent by numerical perturbation

#### Methodology

At a quadrature point driven well into the plastic range — with prior history, a
non-proportional strain increment and a non-zero shear, so no entry of the
tangent is trivially zero — the algorithmic tangent is compared entry by entry
with the numerical derivative of the same stress update.

The numerical derivative is taken with `outram-foam-basic-lib`'s
`math::differentiate::jacobian` in central-difference mode — the workspace's own
implementation of `code_aster`'s perturbed-difference scheme, with its
$\epsilon^{1/3}$-scaled step — rather than a hand-rolled difference, so the
step-size policy is one that has been verified in its own right.

#### Results

| Quantity | Value |
|---|---|
| analytic tangent scale (largest entry) | 1.973815e11 Pa |
| worst relative entry error vs central difference | **1.055e-7**, at entry (1, 2) |
| algorithmic vs **elastic** tangent, same point | 4.986e-1 relative |

#### Interpretation

Agreement to about seven digits, which is what a central difference at
$h \sim \epsilon^{1/3}$ can deliver: truncation $O(h^2) \sim 10^{-11}$ relative
to the function plus round-off $\epsilon/h \sim 10^{-11}$, amplified by the
$10^{11}$ scale of the tangent. The residual $10^{-7}$ is the *difference
scheme's* error, not the tangent's.

The third row is the control that makes the first mean something. The
algorithmic tangent differs from the purely elastic one by 50 % of its largest
entry at this point, so a run that mistakenly returned $C_e$ would have failed
by five orders of magnitude rather than passing narrowly. The test asserts that
gap explicitly.

### 5c. Newton convergence order

#### Methodology

A single-element uniaxial cell is too easy to show a convergence *order*: the
state is uniform and the return map is closed form. A partially plastic
structure is needed, so that some quadrature points yield during the step and
others do not and the residual is genuinely nonlinear.

The thick-walled cylinder of case 3 is re-used with the elastic material
replaced by J2 (`sigma_y0 = 250 MPa`, `H = 2 GPa`) and the pressure raised to
`p = 140 MPa`. Mesh `n_r = 12`, `n_theta = 24` Quad4, plane strain, four equal
load steps. The order is estimated from the residuals of the final step that are
above `1e-10`; trailing entries of a converged history sit at the round-off
floor and the ratio between two such numbers carries no information.

#### Results

| load factor | iterations | yielding points | residual history |
|---|---|---|---|
| 0.250 | 2 | 0 | 1.000e0, 8.856e-14, 1.255e-14 |
| 0.500 | 2 | 0 | 5.000e-1, 4.569e-14, 1.201e-14 |
| 0.750 | 2 | 0 | 3.333e-1, 3.351e-14, 1.380e-14 |
| 1.000 | 5 | 192 of 1152 | 2.500e-1, 2.654e-1, 5.848e-3, 1.641e-6, 1.253e-13, 1.262e-14 |

**Observed Newton convergence order on the final step: 2.004.**

#### Interpretation

The descent `5.85e-3 -> 1.64e-6 -> 1.25e-13` squares the exponent each time,
with a nearly constant ratio $r_{k+1}/r_k^2$ of 0.048 then 0.046 — the
definition of quadratic convergence, and a direct measurement of the consistent
tangent being the exact derivative of the discrete internal force. A
modified-Newton iteration on the elastic tangent would give order 1 and would
need tens of iterations to reach `1e-13` instead of three.

The first three load steps are still elastic: at `p = 140 MPa` the bore reaches a
von Mises stress of 324 MPa against a 250 MPa yield, so plasticity appears only
in the last quarter of the load. The plastic step's residual **rises** on its
first iteration (2.500e-1 to 2.654e-1), which is expected: the first iterate is
taken on the tangent inherited from a state where nothing had yielded, so it
overshoots, and the iteration recovers immediately.

**What this case does not claim.** Nothing about the accuracy of the
elasto-plastic stresses. Full-integration Quad4 elements lock volumetrically
once a zone flows plastically, because plastic flow is incompressible, and the
collapse pressure this mesh would predict is too high. The convergence order is
a property of the linearisation and is unaffected, which is why it is the only
thing measured here.

**That "too high" is now a number, not an assertion:** case 7 measures it as
+1.67 % on an `8 x 16` mesh and +23.9 % on a `2 x 4` one, against the closed-form
limit pressure. This case is still run under full integration, deliberately —
the convergence order it measures is the property of the *linearisation*, and
re-running it under B-bar would change nothing about that claim while losing the
continuity of the recorded result. Case 7 is where the stress accuracy is
measured, and `Formulation::BBar` is what a user should select for a plastic
collapse calculation.

---

## 6. Volumetric locking: nearly incompressible elasticity

Added 2026-09-11 with the B-bar formulation (bead `op-vrtt.1`). This is the case
that measures whether the element library can be trusted near the incompressible
limit, which is where *every* fully plastic zone lives.

### Methodology

The manufactured solution of case 2, run at **two** Poisson's ratios and **two**
formulations so that the contrast is the evidence:

- `nu = 0.3` ($\lambda/\mu = 1.5$), the control, where nothing should lock;
- `nu = 0.499` ($\lambda/\mu = 332.7$), the nearly incompressible case.

**Case 6a — Quad4** on the unit square, plane strain, exact field
`u_x = u_y = sin(pi x) sin(pi y)`, meshes $n = 4, 8, 16, 32$ per side.

**Case 6b — Hex8** on the unit cube, exact field
`u_x = u_y = u_z = sin(pi x) sin(pi y) sin(pi z)`, meshes $n = 2, 4, 8, 16$.
That three-dimensional body force is new, and it is checked independently
against $-\text{div} \ \sigma$ of its own exact field by
`assembly::tests::manufactured_body_forces_are_minus_divergence_of_their_own_stress`
(worst relative residual 8.5e-9 in two dimensions, 1.3e-8 in three), so a lost
order here cannot be blamed on a mis-derived forcing term.

`E = 200 GPa` throughout; homogeneous Dirichlet data on the whole boundary;
errors integrated with the richer `error_rule`.

**Pass criteria** are orders and ratios, never tuned tolerances: B-bar within
0.15 of order 2 at `nu = 0.499`; full integration demonstrably *below* order
1.5; B-bar's error within a factor 2 of its own `nu = 0.3` error; full
integration at least 5x (Quad4) or 4x (Hex8) worse than B-bar.

### The linear solver had to change, and that is a finding

ILU(0)-preconditioned conjugate gradients — the preconditioner every other case
in this report uses — **stagnates** on these systems. At `nu = 0.499` on the
32x32 Quad4 mesh it reached a relative residual of 1.23 (full integration) and
0.153 (B-bar) after 20 000 iterations and stopped improving, while Jacobi-CG
solved both to `1e-10`. The same B-bar system ILU(0) cannot solve converges
cleanly under Jacobi, so this is a preconditioner defect, not a formulation one.
Recorded as bead `op-ldaz`; the locking cases use Jacobi and say so.

### Results, case 6a (Quad4), measured 2026-09-11

L2 errors of the displacement (absolute; the study takes ratios).

**`nu = 0.3`, the control:**

| n | dofs | L2, full | order | L2, B-bar | order |
|---|---|---|---|---|---|
| 4 | 50 | 4.37275e-2 | | 3.03801e-2 | |
| 8 | 162 | 1.11518e-2 | 1.971 | 7.61116e-3 | 1.997 |
| 16 | 578 | 2.80446e-3 | 1.991 | 1.90510e-3 | 1.998 |
| 32 | 2178 | 7.02209e-4 | 1.998 | 4.76440e-4 | 1.999 |

**`nu = 0.499`:**

| n | dofs | L2, full | order | L2, B-bar | order |
|---|---|---|---|---|---|
| 4 | 50 | 4.79622e-2 | | 2.49554e-2 | |
| 8 | 162 | 2.03534e-2 | **1.237** | 5.88305e-3 | 2.085 |
| 16 | 578 | 1.25968e-2 | **0.692** | 1.45262e-3 | 2.018 |
| 32 | 2178 | 6.14098e-3 | **1.037** | 3.62084e-4 | **2.004** |

### Results, case 6b (Hex8), measured 2026-09-11

**`nu = 0.3`, the control:**

| n | dofs | L2, full | order | L2, B-bar | order |
|---|---|---|---|---|---|
| 2 | 81 | 1.61098e-1 | | 1.27103e-1 | |
| 4 | 375 | 4.10679e-2 | 1.972 | 2.74336e-2 | 2.212 |
| 8 | 2187 | 1.04584e-2 | 1.973 | 6.70978e-3 | 2.032 |
| 16 | 14739 | 2.62991e-3 | 1.992 | 1.67044e-3 | 2.006 |

**`nu = 0.499`:**

| n | dofs | L2, full | order | L2, B-bar | order |
|---|---|---|---|---|---|
| 2 | 81 | 1.61098e-1 | | 3.12185e-1 | |
| 4 | 375 | 4.78075e-2 | **1.753** | 4.49880e-2 | 2.795 |
| 8 | 2187 | 2.32814e-2 | **1.038** | 9.76937e-3 | 2.203 |
| 16 | 14739 | 1.50022e-2 | **0.634** | 2.35856e-3 | **2.050** |

### Interpretation

**Full integration locks, and it shows in the rate, not only the constant.** On
Quad4 the observed order at `nu = 0.3` is 1.971, 1.991, 1.998 — textbook. The
same element at `nu = 0.499` gives 1.237, 0.692, 1.037; it is not converging at
second order anywhere in this range of $h$, and the wobble is the signature of a
pre-asymptotic regime whose error constant scales with $\lambda/\mu$. On Hex8
the collapse is monotone: 1.753, 1.038, **0.634**, so the element is losing
ground with refinement rather than gaining it.

**B-bar does not lock.** Observed orders 2.085, 2.018, 2.004 (Quad4) and 2.795,
2.203, 2.050 (Hex8) at `nu = 0.499` — the theoretical rate, approached from
above and settling on it.

**The `nu`-sensitivity numbers are the cleanest statement of all.** Comparing
each formulation's finest-mesh error at `nu = 0.499` with its own error at
`nu = 0.3`, for a problem whose exact solution did not change:

| | Quad4 | Hex8 |
|---|---|---|
| full integration, `nu` sensitivity | **8.75x worse** | **5.70x worse** |
| B-bar, `nu` sensitivity | 0.76x | 1.41x |
| full / B-bar error, `nu = 0.499` | **16.96x** | **6.36x** |

B-bar's error is essentially independent of Poisson's ratio, which is what
"locking-free" means. (It is not a contradiction that the Quad4 figure is
slightly *below* 1: the exact solution is fixed while the material changes, so
the two errors need not be ordered.)

Two honest notes. B-bar is also mildly better than full integration at
`nu = 0.3`; that is not a general guarantee and full integration remains the
default for compressible materials. And on the coarsest Hex8 mesh at
`nu = 0.499`, B-bar's error (3.12e-1) is *worse* than full integration's
(1.61e-1) — a 2x2x2 mesh has two elements across a full sine wave and resolves
nothing, so both numbers are pre-asymptotic and neither is evidence. That row is
left in the table rather than trimmed.

---

## 7. Volumetric locking: fully plastic limit load

### Methodology

Thick-walled cylinder, `a = 0.05 m`, `b = 0.10 m`, quarter model with symmetry
on both cut faces. **Elastic-perfectly-plastic** J2: `E = 200 GPa`, `nu = 0.3`,
`sigma_y = 250 MPa`, `H = 0` exactly, because the closed form assumes perfect
plasticity.

**Reference:** the von Mises plane-strain limit pressure

$$p_L = \frac{2}{\sqrt{3}} \sigma_y \ln\frac{b}{a} = 200.0944 \ \text{MPa}$$

obtained by integrating $d\sigma_r/dr = (\sigma_\theta - \sigma_r)/r$ with
$\sigma_\theta - \sigma_r = 2\sigma_y/\sqrt{3}$ from $\sigma_r(b) = 0$.

**Loading is displacement controlled**, not force controlled. Under force
control the problem has a limit point, the tangent of a perfectly plastic
structure loses definiteness at $p_L$ and Newton diverges, so the "collapse
load" measured would depend on the solver tolerance as much as on the mechanics.
Under displacement control the path stays single-valued and the pressure simply
plateaus. The bore radial displacement is ramped in 32 increments of `2.5e-5 m`
to `8.0e-4 m` (16x the first-yield displacement), and the pressure is recovered
from the support reaction — the internal force at the constrained degrees of
freedom — divided by the **polygon** perimeter of the discrete bore, not the arc
length, since straight-sided elements make the polygon the boundary the load
actually acts on.

Meshes `n_r = 2, 4, 8, 16` by `n_theta = 2 n_r` Quad4, each run with both
formulations.

**Pass criteria:** both formulations must *over*-predict at every level (the
upper-bound theorem of limit analysis: a displacement-based element admits only
kinematically admissible fields, so it cannot collapse below the exact load);
B-bar's over-prediction at least 5x smaller at every level; B-bar within 0.5 %
of $p_L$ on the finest mesh.

### Results, measured 2026-09-11

Collapse pressure at `u_r = 8.0e-4 m`, in megapascals, against
`p_L = 200.0944 MPa`:

| n_r x n_theta | dofs | full integration | p/p_L | B-bar | p/p_L | excess ratio |
|---|---|---|---|---|---|---|
| 2 x 4 | 30 | 247.8937 | **1.23888** | 202.2194 | 1.01062 | 22.5x |
| 4 x 8 | 90 | 213.1640 | 1.06532 | 200.6581 | 1.00282 | 23.2x |
| 8 x 16 | 306 | 203.4431 | 1.01674 | 200.2354 | 1.00070 | 23.7x |
| 16 x 32 | 1122 | 200.9348 | 1.00420 | 200.1275 | **1.00017** | 25.4x |

The excess ratio is $(p_{\text{full}} - p_L)/(p_{\text{B-bar}} - p_L)$.

Pressure-displacement path on a `10 x 20` mesh, showing the plateau the
measurement relies on (MPa):

| u_r (m) | full | B-bar | yielding points |
|---|---|---|---|
| 5.0e-5 | 105.036 | 104.929 | 0 (still elastic) |
| 1.0e-4 | 166.946 | 166.696 | 192 / 205 |
| 2.0e-4 | 198.706 | 198.198 | 414 / 420 |
| 4.0e-4 | 201.070 | 200.046 | 447 / 445 |
| 6.0e-4 | 201.708 | 200.167 | 456 / 420 |

### Interpretation

Full-integration Quad4 over-predicts the collapse load by **23.9 %** on a coarse
mesh, falling to 6.5 %, 1.7 % and 0.42 % with refinement. That is volumetric
locking measured plastically: J2 flow is volume preserving, and a
full-integration bilinear element imposes that constraint at four points per
element, over-constraining the displacement field so the structure carries load
it should not.

B-bar over-predicts by **1.06 %, 0.28 %, 0.07 % and 0.017 %** on the same four
meshes — between 22 and 25 times closer at every level, and on the finest mesh
reproducing the closed-form limit pressure to 170 parts per million.

Read across, the practical statement is that B-bar on the **2 x 4** mesh (30
degrees of freedom, 1.1 % error) beats full integration on the **8 x 16** mesh
(306 degrees of freedom, 1.7 % error): ten times the unknowns for a worse
answer.

The plateau is real and not an artefact of stopping: between `u_r = 4e-4` and
`6e-4 m` the B-bar pressure moves by 0.06 %, having risen 90 % over the path.
Both curves still creep upward, which is expected as the plastic zone finishes
spreading through the outermost element, and is why the numbers are quoted at a
stated displacement rather than as "the" collapse load.

**Limitations, stated rather than buried.** Small strain with no geometry
update, at `u_r/a = 1.6 %`. Straight-sided elements bias the pressure by about
0.05 % at `n_theta = 16`, which is comparable to the B-bar error on the finest
mesh only — so `1.00017` should be read as "indistinguishable from $p_L$ at this
mesh's geometric fidelity", not as five-digit agreement. And the reference
assumes a perfectly plastic von Mises material in plane strain with no
hardening, which is what was run but is not steel.

---

## 8. Shear locking, which B-bar does **not** cure

### Why this case exists

Bead `op-vrtt.1` asserted that "a Quad4 mesh of the same coarseness would give a
visibly too-stiff beam" — an unmeasured claim used to justify case 4's choice of
quadratic triangles. This measures it, and measures whether the B-bar work of
cases 6 and 7 fixes it. **It does not.**

### Methodology

The cantilever of case 4 re-meshed with bilinear quadrilaterals: `H = 0.1 m`,
unit thickness, `E = 200 GPa`, `nu = 0` (so plane strain, plane stress and beam
theory share the same `E`), clamped at `x = 0`, uniform shear traction on
`x = L` totalling `P = 1000 N`, deflection read at the mid-height node of the
loaded edge.

Slendernesses `L/H = 4` and `8`; elements through the depth `ny = 2, 4, 8, 16`
with **square** elements; plus one deliberately span-coarse mesh at `ny = 2` with
element aspect ratio 4. Both formulations on every mesh.

**Reference:** Timoshenko, `delta = P L^3 / (3 E I) + P L / (k G A)` with
`I = H^3/12`, `A = H`, `k = 5/6`, `G = E/2`. Timoshenko rather than
Euler-Bernoulli, because the question is how much stiffer than the *correct*
answer the element is.

### Results, measured 2026-09-11

$\delta / \delta_{\text{Timoshenko}}$; below 1 means too stiff.

**`L/H = 4`, $\delta_{\text{Timo}} = 1.328000 \times 10^{-6}$ m:**

| ny | nx | aspect | dofs | full integration | B-bar |
|---|---|---|---|---|---|
| 2 | 2 | 4 | 18 | **0.3313** | 0.3399 |
| 2 | 8 | 1 | 54 | 0.8835 | 0.9518 |
| 4 | 16 | 1 | 170 | 0.9673 | 0.9867 |
| 8 | 32 | 1 | 594 | 0.9911 | 0.9962 |
| 16 | 64 | 1 | 2210 | 0.9973 | 0.9986 |

**`L/H = 8`, $\delta_{\text{Timo}} = 1.033600 \times 10^{-5}$ m:**

| ny | nx | aspect | dofs | full integration | B-bar |
|---|---|---|---|---|---|
| 2 | 4 | 4 | 30 | **0.3328** | 0.3421 |
| 2 | 16 | 1 | 102 | 0.8875 | 0.9579 |
| 4 | 32 | 1 | 330 | 0.9692 | 0.9890 |
| 8 | 64 | 1 | 1170 | 0.9920 | 0.9972 |

For comparison, the **Tri6** mesh of case 4 reaches 0.99930 at `L/H = 4` on 1170
degrees of freedom — better than Quad4 with B-bar manages on 2210.

### Interpretation

**The bead's claim is confirmed and quantified.** With two square elements
through the depth a full-integration Quad4 cantilever is **11.25 % too stiff**
(`L/H = 8`); with elements of aspect ratio 4 it is **66.7 % too stiff** — two
thirds of the deflection simply missing. The aspect-ratio sensitivity is the
tell: shear locking comes from the bilinear element's inability to represent
pure bending without a parasitic shear strain whose magnitude grows with the
element's length-to-depth ratio.

**B-bar helps, but it does not cure this, and it cannot.** It lifts `ny = 2` from
0.8875 to 0.9579 — recovering about 63 % of the deficit — and leaves **4.2 %
still missing**. The share it recovers is the *volumetric* part of the parasitic
constraint: even at `nu = 0` the bulk modulus is `E/3`, not zero, so bending
excites a spurious dilatational stiffness that the mean-dilatation modification
relaxes. The share it does not recover is the parasitic *shear* strain itself, a
deviatoric mode that B-bar leaves untouched by construction, since it only ever
substitutes the volumetric part of `B`. On the aspect-ratio-4 mesh, where the
shear share dominates, B-bar moves 0.3328 only to 0.3421 and 65.8 % of the
deflection is still missing.

**Shear locking therefore remains an open defect in this crate**, tracked as bead
`op-uqqg`. The fixes are a different family from B-bar — incompatible modes
(Wilson), enhanced assumed strain (Simo-Rifai), assumed natural strain, or
reduced integration with hourglass control — and each needs verifying in its own
right. Nothing in cases 6 and 7 bears on it, and the B-bar work must not be
described as having addressed it.

The practical advice this case supports, and the reason case 4 uses Tri6: **do
not bend a low-order quadrilateral.** If the mesh must be quadrilateral, four or
more elements through the depth at an aspect ratio near 1 keeps the error near
1 %; two long elements through the depth loses a third of the deflection.

---

## 9. Plane stress

Added 2026-09-11 (bead `op-vrtt.2`). Before this, two-dimensional analysis in
Farrer Park was plane strain only.

Plane strain is a *kinematic* condition — `eps_zz = gamma_yz = gamma_xz = 0` —
which the strain-displacement operator imposes by writing zeros into three Voigt
slots, after which the ordinary three-dimensional law gives the correct
`sigma_zz` with no special case. Plane stress is a *constitutive* condition,
`sigma_zz = 0`, so it is not a matter of zeroing different slots: `eps_zz` has to
be solved for and condensed out. For linear elasticity that is a static
condensation of the constitutive matrix; for J2 it is a nested scalar Newton
inside the return map, plus a condensed algorithmic tangent. Both are
implemented in `material.rs` and selected by the explicit
`PlaneCondition { PlaneStrain, PlaneStress }` enum carried on `System`.

### 9a. Thin plate in uniaxial tension

#### Methodology

A square plate `[0,1]^2` m of unit thickness, Quad4, 4x4, `E = 200 GPa`,
`nu = 0.3`. Symmetry on `x = 0` and `y = 0`; uniform traction
`T = 100 MPa` on `x = 1`; everything else traction free. Solved twice, once in
each idealisation.

Exact, plane stress: `sigma_xx = T`, `sigma_yy = sigma_zz = 0`,
`u_x = T x / E`, `u_y = -nu T y / E`.
Exact, plane strain: the same in-plane stresses, `sigma_zz = nu T`, and
`u_y = -nu (1 + nu) T y / E`.

The exact field is linear, so a bilinear element reproduces it *exactly*; this is
a patch test with a physical reference attached.

#### Results

| Quantity | Plane stress | Plane strain |
|---|---|---|
| max relative error in `u_x` | 4.337e-16 | 3.574e-16 |
| max relative error in `u_y` | 5.421e-16 | 2.780e-16 |
| max relative error in `sigma_xx` | 8.941e-16 | 1.043e-15 |
| max `abs(sigma_yy)` / T | 3.353e-16 | 8.196e-16 |
| `sigma_zz` / T | **0.000e0 exactly** | **3.000e-1** |
| `u_y` at `y = 1` (m) | -1.500000e-4 | -1.950000e-4 |

#### Interpretation

Both idealisations reproduce their own exact solution to round-off, and they
differ where they must and only there. The in-plane stress is identical
(`sigma_xx = T` in both, as it must be for a statically determinate bar); the
through-thickness stress is 0 under plane stress and `nu T` under plane strain;
and the lateral contraction differs by exactly `1 + nu = 1.3`.

That last number is the one worth keeping. It is a **30 % difference in a
quantity an engineer would measure**, and before this work the crate could only
produce the plane-strain one.

### 9b. The exact plane-stress / plane-strain equivalence

#### Why this is the sharper test

A single problem with a known answer verifies one point of the constitutive map.
The equivalence below is an **algebraic identity** that must hold for every
geometry, load and mesh, so checking it on a non-trivial problem exercises the
whole condensed matrix rather than one entry.

For an isotropic material the plane-stress matrix at $(E^*, \nu^*)$ equals the
plane-strain matrix at $(E, \nu)$ when

$$E^* = \frac{E}{1 - \nu^2}, \quad \nu^* = \frac{\nu}{1 - \nu}$$

verified by hand entry by entry:
$D_{00} = E^*/(1-\nu^{*2}) = E(1-\nu)/((1+\nu)(1-2\nu)) = \lambda + 2\mu$,
$D_{01} = \lambda$, and $\mu^* = \mu$. Note $\nu^* < 1/2$ requires $\nu < 1/3$.

#### Methodology

The quarter thick-walled cylinder of case 3 — `a = 0.05 m`, `b = 0.10 m`,
`p = 100 MPa`, `6 x 12` Quad4 — chosen because its solution is neither uniform
nor a linear field, so every entry of the constitutive matrix is exercised at
every quadrature point. Run at `nu = 0.3` and `nu = 0.25`, each time comparing
the transformed pair against a **control**: the same two idealisations at the
*untransformed* `(E, nu)`, which must differ.

#### Results

| nu | nu* | E* (GPa) | max relative difference | control: untransformed pair |
|---|---|---|---|---|
| 0.30 | 0.428571 | 219.780 | **2.860e-16** | 6.372e-2 |
| 0.25 | 0.333333 | 213.333 | **7.265e-16** | 4.488e-2 |

#### Interpretation

The transformed pair agrees to round-off — below the `1e-13` linear-solve
tolerance, because the two runs assemble numerically identical matrices and the
conjugate-gradient iterations then track each other exactly.

The control column shows what "different" looks like here: 6.4 % and 4.5 %. That
is modest, because the in-plane solution of a pressurised cylinder depends on
Poisson's ratio only weakly — which is all the more reason to have the control
rather than assume a match at `1e-16` must be meaningful.

---

## 10. Plane-stress J2, uniaxial tension

### Methodology

A unit square plate, 2x2 Quad4, plane stress, J2 with linear isotropic
hardening: `E = 200 GPa`, `nu = 0.3`, `sigma_y0 = 250 MPa`, `H = 2 GPa`, yield
strain `1.25e-3`, plastic tangent modulus `E_t = 1.980198 GPa`.

Symmetry on `x = 0` and `y = 0`, prescribed `u_x = eps` on `x = 1`, `y = 1`
traction free. With `sigma_yy = 0` from the free edge and `sigma_zz = 0` from the
condensation, the state is genuinely uniaxial **stress**, and the reference is
the same closed form case 5 checks in three dimensions — now reached through the
condensed path. Strain stepped to `2.5e-4, ..., 5.0e-3` (20 steps).

### Results

| eps | sigma_xx (Pa) | closed form (Pa) | rel err | sigma_zz (Pa) | alpha |
|---|---|---|---|---|---|
| 1.0000e-3 | 2.0000000e8 | 2.0000000e8 | 0 | 0 | 0 |
| 1.2500e-3 | 2.5000000e8 | 2.5000000e8 | 3.576e-16 | 0 | 0 |
| 2.5000e-3 | 2.5247525e8 | 2.5247525e8 | 3.541e-16 | 0 | 1.2376238e-3 |
| 5.0000e-3 | 2.5742574e8 | 2.5742574e8 | 0 | 0 | 3.7128713e-3 |

| Quantity | Value |
|---|---|
| worst relative error in `sigma_xx`, all 20 steps | **3.576e-16** |
| worst `abs(sigma_yy)` | 8.941e-8 Pa |
| worst `abs(sigma_zz)` | **0.000e0 Pa** |
| worst relative error in `alpha` | 1.752e-15 |

### Interpretation

Round-off agreement, which is the correct outcome and not merely a close one:
linear hardening makes the return map a closed-form root, and the nested
`eps_zz` Newton is solving a scalar equation whose root the elastic predictor
already lands on within one iteration.

`sigma_zz` is **identically zero**, not small — the local Newton runs to the
floating-point root. `sigma_yy` reaches 8.9e-8 Pa, the residual of the global
equilibrium solve on a traction-free edge (3.5e-16 relative to `sigma_xx`).

The measurement that matters most is `alpha`, because it is the history the
condensation writes: at `eps = 5e-3` it is `3.7128713e-3`, **identical to the
three-dimensional uniaxial cell of case 5 to every digit printed**. The condensed
path and the unconstrained path reach the same physical state by different
routes, which is the strongest statement this case can make.

---

## 11. Plane-stress J2, equibiaxial tension

### Why a second plastic case

Uniaxial tension is the state the return map is easiest to get right in: the
deviator has one dominant component and `eps_zz` stays near its elastic value.
Equibiaxial tension is the opposite corner of the von Mises ellipse — the
through-thickness plastic strain is **twice** either in-plane component and
carries the whole plastic volume change — so it is where a wrong condensation
shows up largest. Verifying only uniaxial would leave the nested solve barely
exercised.

### Methodology

The same plate, with `u_x = eps` on `x = 1` **and** `u_y = eps` on `y = 1`.

Reference, with the biaxial modulus $E_b = E/(1-\nu) = 285.714$ GPa. By symmetry
$\sigma_{xx} = \sigma_{yy} = \sigma$ and $\sigma_{zz} = 0$, so the deviator is
$(\sigma/3, \sigma/3, -2\sigma/3)$ and $q = \sigma$: the equibiaxial point of the
von Mises ellipse sits at the uniaxial yield stress. The plastic strain is
$(e_p, e_p, -2e_p)$, so $\alpha = 2 e_p$, and the two conditions
$\sigma = E_b(\epsilon - e_p)$ and $\sigma = \sigma_{y0} + 2 H e_p$ give

$$e_p = \frac{E_b \epsilon - \sigma_{y0}}{E_b + 2H}, \quad
\sigma = \sigma_{y0} + 2 H e_p$$

with yield at $\epsilon_y = \sigma_{y0}/E_b = 8.750 \times 10^{-4}$.

### Results

| eps | sigma_xx (Pa) | closed form (Pa) | rel err | alpha | closed form |
|---|---|---|---|---|---|
| 7.5000e-4 | 2.1428571e8 | 2.1428571e8 | 0 | 0 | 0 |
| 1.0000e-3 | 2.5049310e8 | 2.5049310e8 | 0 | 2.4654832e-4 | 2.4654832e-4 |
| 2.5000e-3 | 2.5641026e8 | 2.5641026e8 | 1.162e-16 | 3.2051282e-3 | 3.2051282e-3 |
| 5.0000e-3 | 2.6627219e8 | 2.6627219e8 | 1.119e-16 | 8.1360947e-3 | 8.1360947e-3 |

| Quantity | Value |
|---|---|
| worst relative error in `sigma_xx` | **6.766e-16** |
| worst relative error in `alpha` | 1.099e-15 |
| worst `abs(sigma_xx - sigma_yy)` / `sigma_xx` | 6.974e-16 |
| worst `abs(sigma_zz)` | **0.000e0 Pa** |
| worst `abs(q - sigma_xx)` / `sigma_xx` | 3.487e-16 |

### Interpretation

Round-off agreement on the state that stresses the condensation hardest. Three
separate facts are confirmed, each of which would fail differently if the nested
solve were wrong:

1. **The stress magnitude** matches to `6.8e-16`. A condensation converging to
   the wrong `eps_zz` would show up here first and largest, because $\sigma$
   depends on `eps_zz` through the whole deviator at this point.
2. **The equivalent plastic strain** matches to `1.1e-15`, and at `eps = 5e-3`
   it is `8.1360947e-3` — **2.19 times** the uniaxial value at the same strain
   (`3.7128713e-3`), because equibiaxial stretching reaches yield at a smaller
   strain (`8.75e-4` against `1.25e-3`) and then accumulates plastic strain
   faster.
3. **`q = sigma_xx` to `3.5e-16`**, the statement that the computed state sits
   at the equibiaxial corner of the von Mises ellipse rather than elsewhere on
   it. This is what would catch a sign error in the out-of-plane deviatoric
   component, which the uniaxial case is far less sensitive to.

---

## Cross-cutting limitations

Stated plainly, because a verification report that lists only what passed is
half a report.

1. **No validation.** Nothing here is compared against a physical measurement or
   a published benchmark. Farrer Park has no validated case.
2. **No human review.** Every line of this crate and this document is
   AI-assisted draft material, untrusted until a human has reviewed it — see
   `RESPONSIBLE_USE.md`.
3. **Small strain only.** No finite rotation, no geometric stiffness. A rigid
   rotation of a stressed body would generate spurious stress. Outside a percent
   of strain and a few degrees of rotation the model is wrong, not merely
   inaccurate.
4. **Plane stress is implemented but the combination with B-bar is refused.**
   Both two-dimensional idealisations now exist (cases 9-11) and are selected by
   an explicit enum. `Formulation::BBar` together with
   `PlaneCondition::PlaneStress` is rejected at construction: plane stress has no
   volumetric constraint to relax, because the out-of-plane strain is free to
   accommodate any volume change, so a nearly incompressible material does not
   lock there in the first place — and the interaction between the
   mean-dilatation modification and the `eps_zz` condensation is not something
   this report has verified. Refusing it is deliberate.
5. **Volumetric locking is addressed, not eliminated.** B-bar exists and is
   measured (cases 6 and 7), but **full integration remains the default**, so a
   user who does not select `Formulation::BBar` still gets a locking element.
   Case 5c was run before B-bar existed and is still run under full integration;
   its convergence-order claim is unaffected, but its stresses remain untrusted.
6. **Shear locking is present, measured, and NOT cured.** Case 8 puts a number
   on it — 11.25 % too stiff at two square elements through the depth, 66.7 % at
   element aspect ratio 4 — and shows that B-bar recovers only the volumetric
   share (63 % of the deficit at `ny = 2`, almost none at aspect ratio 4). Case 4
   avoids it by using quadratic triangles rather than by curing it. Tracked as
   bead `op-uqqg`; the fixes are incompatible modes, enhanced assumed strain,
   assumed natural strain, or reduced integration with hourglass control, and
   each needs verifying in its own right.
7. **ILU(0) stagnates on nearly incompressible systems.** The locking cases had
   to use Jacobi preconditioning because ILU(0)-CG — the default everywhere else
   in this report — stalled at relative residuals of 1.23 and 0.153 after 20 000
   iterations on the 32x32 `nu = 0.499` meshes, while Jacobi solved the same
   systems to `1e-10`. Tracked as bead `op-ldaz`.
8. **One constitutive law.** J2 with *linear* isotropic hardening. No kinematic
   hardening, no rate dependence, no creep, no damage. Crystal plasticity
   (`op-q75c`) and microstructure-sensitive fatigue (`op-q1zn`) are separate
   work items and nothing here bears on them.
9. **Straight-sided elements throughout.** No curved isoparametric mapping is
   exercised anywhere, including by Tri6.
10. **No parallelism, no performance claim.** The largest system solved here is
   4626 degrees of freedom.
11. **Serial determinism is not tested.** The suite is deterministic in practice
    but there is no test asserting byte-identical output across runs.
