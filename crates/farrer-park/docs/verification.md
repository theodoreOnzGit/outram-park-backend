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

Shared configuration: isotropic linear elasticity or J2 plasticity with linear
isotropic hardening; two-dimensional cases are **plane strain** (plane stress is
not implemented); linear systems solved by ILU(0)-preconditioned conjugate
gradients through `outram-foam-basic-lib`'s shared Krylov layer.

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
4. **Plane strain only in two dimensions.** Plane stress is not implemented; it
   needs `sigma_zz = 0` enforced by condensing `eps_zz` out of the constitutive
   law, which for J2 means a nested local solve.
5. **Volumetric locking is not addressed.** Full integration only; no
   B-bar, no selective reduced integration, no mixed formulation. Fully plastic
   zones and nearly incompressible elasticity (`nu -> 0.5`) will be too stiff.
   Case 5c is run in the presence of locking and says so.
6. **Shear locking is present** in the low-order elements; case 4 avoids it by
   using quadratic triangles rather than by curing it.
7. **One constitutive law.** J2 with *linear* isotropic hardening. No kinematic
   hardening, no rate dependence, no creep, no damage. Crystal plasticity
   (`op-q75c`) and microstructure-sensitive fatigue (`op-q1zn`) are separate
   work items and nothing here bears on them.
8. **Straight-sided elements throughout.** No curved isoparametric mapping is
   exercised anywhere, including by Tri6.
9. **No parallelism, no performance claim.** The largest system solved here is
   4626 degrees of freedom.
10. **Serial determinism is not tested.** The suite is deterministic in practice
    but there is no test asserting byte-identical output across runs.
