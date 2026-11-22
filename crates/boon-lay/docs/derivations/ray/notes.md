# Hand-Derived Analytical Solutions

## Fission-Product Diffusion in TRISO Fuel Particles

Submission to Theodore

Author: Ray W.

---

## Abstract

This file reproduces, in markdown, the equations and derivations from the TRISO fuel particle diffusion analysis that were derived independently by the author. It is a narrower excerpt of the full consolidated reference (`equations/all_equations.tex`) in this repository: the Arrhenius temperature-dependence relation, the completed eigenvalue problem and closed-form series solution beyond the raw eigencondition, and the five-layer multilayer particle model are all omitted here, since none of them were developed by the author personally. What remains is the physical system, the governing equation and its assumptions, and the homogeneous single-sphere (Part I) analytical derivation through the raw eigencondition, together with its FTCS numerical discretisation. Notation, variables, and units are defined in the Reference Frame below. Every equation is numbered, and where it also appears in the main report it carries the same label name (given in brackets after the number) so the two documents cross-reference unambiguously.

## Contents

1. The Physical System
2. Reference Frame
3. Assumptions
4. Initial and Boundary Conditions
5. The Governing Equation
6. Physical Models for the Source Term and Diffusion Coefficient
7. Part I: Homogeneous Sphere with a Robin Surface
   - Technique 1: Sturm-Liouville Analytical Solution
8. Part I, Technique 2: FTCS Discretisation

---

## The Physical System

The idealised five-layer TRISO particle modelled in this reference is made of concentric spherical shells. The TikZ figure in the LaTeX version is not reproduced here, so the layer structure is given as a table instead.

| Layer | Name | Region | Notes |
|---|---|---|---|
| 1 | Kernel | $0 \le r \le a$ | Volumetric source $S_0$ acts here |
| 2 | Buffer | shell | No source |
| 3 | IPyC | shell | No source |
| 4 | SiC | shell | No source |
| 5 | OPyC | shell, outer radius $R$ | No source |
| Coolant | Molten salt | $r > R$ | Finite-resistance convective (Robin) boundary at $r=R$ |

Coolant flows past the particle at $r>R$ and is treated as a finite-resistance convective (Robin) boundary at $r=R$ throughout, rather than an idealised perfect sink. Part I collapses all five layers into a single homogeneous region of shared diffusivity $D$. Part II (not included in this excerpt) resolves the five physical layers individually, assigning each shell its own diffusivity $D_i$. Not to scale. Adapted from the system diagram in the main report (`report/sections/02_system_overview.tex`).

---

## Reference Frame

| Symbol | Definition | Units |
|---|---|---|
| $c(r,t)$ | Concentration of the diffusing fission-product species | $\mathrm{mol\ m^{-3}}$ |
| $C_i^j$ | Discrete concentration at node $i$, time level $j$ (FTCS) | $\mathrm{mol\ m^{-3}}$ |
| $t$ | Time | $\mathrm{s}$ |
| $r$ | Radial coordinate | $\mathrm{m}$ |
| $a$, $r_1$ | Kernel (layer 1) outer radius (used interchangeably) | $\mathrm{m}$ |
| $R$ | Particle outer radius (OPyC/coolant surface) | $\mathrm{m}$ |
| $D$, $D_i$ | Diffusivity (Part I: single shared value; Part II: layer $i$) | $\mathrm{m^{2}\ s^{-1}}$ |
| $S$, $S_0$ | Volumetric source/sink term; kernel source strength | $\mathrm{mol\ m^{-3}\ s^{-1}}$ |
| $h$ | Convective mass-transfer coefficient at the surface | $\mathrm{m\ s^{-1}}$ |
| $\mathrm{Bi}$ | Biot number, $hR/D$ (Part I) or $hR/D_L$ (Part II) | dimensionless |
| $\Delta r$ | Radial mesh spacing | $\mathrm{m}$ |
| $\Delta t$ | Time-step size | $\mathrm{s}$ |
| $i,j$ | Radial node index; time-level index | dimensionless |
| $N$ | Index of the surface node (mesh-cell count, centre to surface) | dimensionless |
| $\mathrm{Fo}$ | Mesh Fourier number, $D\Delta t/\Delta r^{2}$ | dimensionless |
| $\kappa$ | Mesh-local convective ratio, $h\Delta r/D=\mathrm{Bi}/N$ | dimensionless |
| $D_k^{\mathrm{eff}}$ | Effective diffusivity at an interface node (Part II FTCS) | $\mathrm{m^{2}\ s^{-1}}$ |
| $w(r)$ | Steady-state concentration profile | $\mathrm{mol\ m^{-3}}$ |
| $\phi_n(r)$ | $n$-th eigenfunction, $u_n(r)/r$ | $\mathrm{m^{-1}}$ |
| $\mu_n$ | Dimensionless root of the Part I eigencondition $\mu\cot\mu=1-\mathrm{Bi}$ | dimensionless |
| $\lambda_n$ | Modal decay rate (eigenvalue) of mode $n$; $\lambda_n\equiv D\mu_n^2/R^2$ in Part I | $\mathrm{s^{-1}}$ |
| $N_n$ | Normalisation integral, $\int_0^R \phi_n^2 r^2\,\mathrm{d}r$ | $\mathrm{m^{3}}$ |
| $a_n$, $b_n$ | Expansion coefficients (source-active; post-shutdown) | $\mathrm{mol\ m^{-2}}$ |
| $t_c$ | Time at which the source is switched off (shutdown) | $\mathrm{s}$ |
| $P_i$, $J_i$ | Layer propagator matrix; interface jump matrix, $\det=1$ (Part II) | dimensionless |
| $F(\lambda)$ | Eigencondition function whose positive roots are the $\lambda_n$ (Part II) | dimensionless |
| $\rho_i$ | Shell resistance of layer $i$, $\frac{1}{4\pi D_i}\left(\frac1{r_{i-1}}-\frac1{r_i}\right)$ | $\mathrm{s\ m^{-3}}$ |

---

## Assumptions

Before any equation is written down, five standing physical assumptions fix the scope of this model. Each is invoked explicitly at the point in the derivation where it does the work of turning a general statement into a solvable one.

**Assumption 1 (spherical uniformity).** The kernel composition and the species source are uniform over each spherical shell, so the concentration field depends on the radial coordinate $r$ and time $t$ alone, never on the polar or azimuthal angle. This is what collapses the full spherical diffusion equation to one radial dimension in the Governing Equation section below.

**Assumption 2 (a single representative particle).** The model describes one particle in isolation, driven by a specified kernel source and a specified coolant-side condition. It does not resolve particle-to-particle variability, particle packing in the fuel compact, or the coolant-side thermal-hydraulic field that would set $h$ in a full-core calculation. Those quantities are treated as given inputs.

**Assumption 3 (kernel-confined source).** Fission events, and with them the fission-product species being tracked, occur only inside the fissile kernel. The buffer, IPyC, SiC, and OPyC layers contain no fissile material, so the volumetric source term is exactly zero there.

**Assumption 4 (piecewise-uniform diffusivity, ideal interfacial contact).** Each layer is modelled as a distinct homogeneous material with its own diffusivity, so nothing forces the diffusivity itself to be continuous from one layer to the next. What does hold, assuming ideal contact between adjacent shells with no additional interfacial resistance, is that the concentration itself does not jump and that whatever flux leaves one layer enters the next.

**Assumption 5 (convective surface condition).** The coolant is a finite, well-mixed reservoir that removes the diffusing species at a rate proportional to how far the surface concentration sits above the coolant's own concentration, rather than instantly to zero. This convective (Robin) condition is the physically realistic representation of a real coolant with finite mass-transfer resistance. An idealised perfect sink is the $\mathrm{Bi}\to\infty$ limiting case of it.

Part I below adopts a further simplification of its own, a uniform-everywhere source in place of Assumption 3's kernel-confined one, in exchange for a fully analytical solution.

---

## 1. Initial and Boundary Conditions

The initial condition and boundary conditions close the governing equation once its form is fixed, before any particular solution technique is applied. The centre condition follows from Assumption 1: spherical symmetry rules out a kink in the concentration field at the coordinate singularity $r=0$, so its radial derivative there must vanish. The outer condition follows from Assumption 5, the convective surface: the flux leaving the particle is proportional to how far the surface concentration sits above the (zero) coolant concentration, rather than being fixed outright. The initial condition reflects a freshly fabricated particle carrying none of the diffusing species yet.

$$
c(r,0) = 0 \tag{1, eq:b:ic}
$$

$$
\frac{\partial c}{\partial r}(0,t) = 0 \quad \text{(spherical symmetry, Assumption 1)} \tag{2, eq:b:bc0}
$$

$$
-D\frac{\partial c}{\partial r}(R,t) = h\,c(R,t) \quad \text{(convective release, Assumption 5)} \tag{3, eq:b:bcR}
$$

where:

- $c(r,0)$ = initial concentration field ($\mathrm{mol\,m^{-3}}$)
- $r$ = radial coordinate ($\mathrm{m}$)
- $t$ = time ($\mathrm{s}$)
- $D$ = diffusivity ($\mathrm{m^{2}\,s^{-1}}$)
- $R$ = particle outer radius ($\mathrm{m}$)
- $h$ = convective mass-transfer coefficient at the surface ($\mathrm{m\,s^{-1}}$)
- $c(R,t)$ = surface concentration ($\mathrm{mol\,m^{-3}}$)

**Biot number.**

$$
\mathrm{Bi} = \frac{hR}{D} \tag{4, eq:b:biot}
$$

where:

- $\mathrm{Bi}$ = Biot number, ratio of surface convective transport to internal diffusive transport (dimensionless)
- $h$ = convective mass-transfer coefficient ($\mathrm{m\,s^{-1}}$)
- $R$ = particle outer radius ($\mathrm{m}$)
- $D$ = diffusivity ($\mathrm{m^{2}\,s^{-1}}$)

---

## 2. The Governing Equation

**Fick's second law with a volumetric source term (general form).**

$$
\frac{\partial c}{\partial t} = D\,\nabla^{2} c + S \tag{5, eq:fick}
$$

where:

- $c$ = concentration of the diffusing fission-product species ($\mathrm{mol\,m^{-3}}$)
- $t$ = time ($\mathrm{s}$)
- $D$ = diffusion coefficient ($\mathrm{m^{2}\,s^{-1}}$)
- $\nabla^{2}$ = Laplacian (divergence-of-gradient) operator ($\mathrm{m^{-2}}$)
- $S$ = volumetric source/sink term ($\mathrm{mol\,m^{-3}\,s^{-1}}$)

**Fick's second law in spherical coordinates.**

$$
\frac{\partial c}{\partial t}
= D \left[
    \frac{1}{r^{2}} \frac{\partial}{\partial r}\!\left( r^{2}\,\frac{\partial c}{\partial r} \right)
    + \frac{1}{r^{2}\sin\theta} \frac{\partial}{\partial \theta}\!\left( \sin\theta \, \frac{\partial c}{\partial \theta} \right)
    + \frac{1}{r^{2}\sin^{2}\theta} \frac{\partial^{2} c}{\partial \varphi^{2}}
  \right] + S \tag{6, eq:fick-spherical}
$$

where:

- $c$ = concentration field ($\mathrm{mol\,m^{-3}}$)
- $t$ = time ($\mathrm{s}$)
- $D$ = diffusion coefficient ($\mathrm{m^{2}\,s^{-1}}$)
- $r$ = radial coordinate ($\mathrm{m}$)
- $\theta$ = polar (colatitude) angle ($\mathrm{rad}$)
- $\varphi$ = azimuthal angle ($\mathrm{rad}$)
- $S$ = volumetric source/sink term ($\mathrm{mol\,m^{-3}\,s^{-1}}$)

**Reduction to one radial dimension (spherical uniformity, Assumption 1).** The TRISO particle is built from concentric, materially uniform shells, and both the source term and the diffusivity in Eq. (6) depend on $r$ alone, so nothing in the geometry or the physics picks out one polar or azimuthal direction over another. The concentration field inherits that symmetry, $c=c(r,t)$, so $\partial c/\partial\theta=\partial c/\partial\varphi=0$ and the two angular terms in Eq. (6) vanish identically. What is left is a genuinely one-dimensional problem in $r$ alone:

$$
\frac{\partial c}{\partial t}
= D \left[ \frac{1}{r^{2}} \frac{\partial}{\partial r}\!\left( r^{2}\,\frac{\partial c}{\partial r} \right) \right]
+ S(r,t) \tag{7, eq:radial-conservative}
$$

where:

- $c$ = concentration field, now a function of $r,t$ only under spherical symmetry ($\mathrm{mol\,m^{-3}}$)
- $t$ = time ($\mathrm{s}$)
- $D$ = diffusion coefficient ($\mathrm{m^{2}\,s^{-1}}$)
- $r$ = radial coordinate ($\mathrm{m}$)
- $S(r,t)$ = volumetric source/sink term ($\mathrm{mol\,m^{-3}\,s^{-1}}$)

**Expansion of the conservative flux term.**

$$
\frac{1}{r^{2}} \frac{\partial}{\partial r}\!\left( r^{2}\,\frac{\partial c}{\partial r} \right)
= \frac{1}{r^{2}} \left[ 2r\,\frac{\partial c}{\partial r} + r^{2}\,\frac{\partial^{2} c}{\partial r^{2}} \right]
= \frac{\partial^{2} c}{\partial r^{2}} + \frac{2}{r}\,\frac{\partial c}{\partial r} \tag{8, eq:product-rule}
$$

where:

- $c$ = concentration field ($\mathrm{mol\,m^{-3}}$)
- $r$ = radial coordinate ($\mathrm{m}$)

**The governing equation used throughout this reference.**

$$
\boxed{\;
\frac{\partial c}{\partial t}
= D \left( \frac{\partial^{2} c}{\partial r^{2}} + \frac{2}{r}\,\frac{\partial c}{\partial r} \right) + S(r,t)
\;} \tag{9, eq:simplified-triso}
$$

where:

- $c$ = concentration field ($\mathrm{mol\,m^{-3}}$)
- $t$ = time ($\mathrm{s}$)
- $D$ = diffusion coefficient ($\mathrm{m^{2}\,s^{-1}}$)
- $r$ = radial coordinate ($\mathrm{m}$)
- $S(r,t)$ = volumetric source/sink term ($\mathrm{mol\,m^{-3}\,s^{-1}}$)

**Kernel-localised volumetric source term (Assumption 3).** Fission events, and with them the generation of the fission-product species being tracked, occur only inside the fissile kernel. The buffer, IPyC, SiC, and OPyC layers contain no fissile material, so $S(r)$ is exactly zero there regardless of how each layer's diffusivity is modelled. This is a statement about where the species is created, not a simplifying approximation:

$$
S(r) =
\begin{cases}
  S_0, & 0 \le r \le a \quad \text{(layer 1, kernel)}, \\
  0,   & a < r \le R \quad \text{(layers 2 to 5, buffer and coatings)},
\end{cases} \tag{10, eq:kernel-source}
$$

where:

- $S(r)$ = volumetric source term at radius $r$ ($\mathrm{mol\,m^{-3}\,s^{-1}}$)
- $S_0$ = kernel source strength ($\mathrm{mol\,m^{-3}\,s^{-1}}$)
- $r$ = radial coordinate ($\mathrm{m}$)
- $a$ = kernel (layer 1) outer radius ($\mathrm{m}$)
- $R$ = particle outer radius ($\mathrm{m}$)

---

## 3. Physical Models for the Source Term and Diffusion Coefficient

**Decomposition of the volumetric source term.**

$$
S(c, r, t) = S_{\text{gen}} + S_{\text{decay}} + S_{\text{trap}} \tag{11, eq:source-decomposition}
$$

where:

- $S(c,r,t)$ = total volumetric source term ($\mathrm{mol\,m^{-3}\,s^{-1}}$)
- $S_{\text{gen}}$ = generation term from fission ($\mathrm{mol\,m^{-3}\,s^{-1}}$)
- $S_{\text{decay}}$ = radioactive-decay loss term ($\mathrm{mol\,m^{-3}\,s^{-1}}$)
- $S_{\text{trap}}$ = trapping/release term ($\mathrm{mol\,m^{-3}\,s^{-1}}$)
- $c$ = concentration ($\mathrm{mol\,m^{-3}}$)
- $r$ = radial coordinate ($\mathrm{m}$)
- $t$ = time ($\mathrm{s}$)

---

## 4. Part I: Homogeneous Sphere with a Robin Surface

**Governing equation, homogeneous-sphere simplification.** Part I treats the whole sphere as a single homogeneous region generating species uniformly, rather than confining generation to the kernel as Eq. (10) does (Assumption 3): trading a more realistic source geometry for a fully analytical, self-contained solution. The general governing equation, Eq. (9), specialises here to a single diffusivity $D$ and a uniform source $S_0$ over the whole domain:

$$
\frac{\partial c}{\partial t}
= D\left(\frac{\partial^2 c}{\partial r^2} + \frac{2}{r}\frac{\partial c}{\partial r}\right) + S_0,
\qquad 0 < r < R,\quad t > 0 \tag{12, eq:b:pde}
$$

where:

- $c$ = concentration field ($\mathrm{mol\,m^{-3}}$)
- $t$ = time ($\mathrm{s}$)
- $D$ = diffusivity of the homogeneous sphere ($\mathrm{m^{2}\,s^{-1}}$)
- $r$ = radial coordinate ($\mathrm{m}$)
- $S_0$ = uniform volumetric source term ($\mathrm{mol\,m^{-3}\,s^{-1}}$)
- $R$ = particle outer radius ($\mathrm{m}$)

### Technique 1: Sturm-Liouville Analytical Solution

**Governing equation restated in divergence (Sturm-Liouville) form, via** $c_{rr}+\tfrac2r c_r = \tfrac1{r^2}(r^2c_r)_r$.

$$
\frac{\partial c}{\partial t}
= \frac{D}{r^2}\frac{\partial}{\partial r}\!\left(r^2 \frac{\partial c}{\partial r}\right) + S_0 \tag{13, eq:b:pde-divergence}
$$

where:

- $c$ = concentration field ($\mathrm{mol\,m^{-3}}$)
- $t$ = time ($\mathrm{s}$)
- $D$ = diffusivity ($\mathrm{m^{2}\,s^{-1}}$)
- $r$ = radial coordinate ($\mathrm{m}$)
- $S_0$ = uniform volumetric source term ($\mathrm{mol\,m^{-3}\,s^{-1}}$)

**Sturm-Liouville form: $p$, $q$, $\sigma$ for the radial operator.**

$$
p(r) = r^2, \qquad q(r) = 0, \qquad \sigma(r) = r^2 \tag{14, eq:b:pqsigma}
$$

where:

- $p(r)$ = Sturm-Liouville leading-coefficient function ($\mathrm{m^{2}}$)
- $q(r)$ = Sturm-Liouville potential-term function, here identically zero (dimensionless)
- $\sigma(r)$ = Sturm-Liouville weight function ($\mathrm{m^{2}}$)
- $r$ = radial coordinate ($\mathrm{m}$)

**Steady problem: first integration, boundedness fixes the constant.**

$$
\left(r^2w'\right)' = -\frac{S_0r^2}{D} \;\Longrightarrow\; r^2w' = -\frac{S_0r^3}{3D} + A \;\Longrightarrow\; A=0 \ (\text{boundedness at } r=0) \tag{15, eq:b:w-firstint}
$$

where:

- $w(r)$ = steady-state concentration profile ($\mathrm{mol\,m^{-3}}$)
- $r$ = radial coordinate ($\mathrm{m}$)
- $S_0$ = uniform volumetric source term ($\mathrm{mol\,m^{-3}\,s^{-1}}$)
- $D$ = diffusivity ($\mathrm{m^{2}\,s^{-1}}$)
- $A$ = constant of integration, forced to zero by boundedness at the origin ($\mathrm{mol\,m^{-1}}$)

**Second integration and the Robin condition fix the remaining constant.**

$$
w' = -\frac{S_0r}{3D}, \quad w = B - \frac{S_0r^2}{6D}; \qquad -Dw'(R) = h\,w(R) \;\Longrightarrow\; B = \frac{S_0R}{3h} + \frac{S_0R^2}{6D} \tag{16, eq:b:w-secondint}
$$

where:

- $w$ = steady-state concentration profile ($\mathrm{mol\,m^{-3}}$)
- $r$ = radial coordinate ($\mathrm{m}$)
- $S_0$ = uniform volumetric source term ($\mathrm{mol\,m^{-3}\,s^{-1}}$)
- $D$ = diffusivity ($\mathrm{m^{2}\,s^{-1}}$)
- $B$ = constant of integration, fixed by the Robin condition ($\mathrm{mol\,m^{-3}}$)
- $h$ = convective mass-transfer coefficient ($\mathrm{m\,s^{-1}}$)
- $R$ = particle outer radius ($\mathrm{m}$)

**Steady-state concentration profile (Robin surface condition).**

$$
\boxed{\;w(r) = \frac{S_0 R}{3h} + \frac{S_0}{6D}\left(R^2 - r^2\right)\;} \tag{17, eq:b:w}
$$

where:

- $w(r)$ = steady-state concentration profile, Robin surface condition ($\mathrm{mol\,m^{-3}}$)
- $S_0$ = uniform volumetric source term ($\mathrm{mol\,m^{-3}\,s^{-1}}$)
- $R$ = particle outer radius ($\mathrm{m}$)
- $h$ = convective mass-transfer coefficient ($\mathrm{m\,s^{-1}}$)
- $D$ = diffusivity ($\mathrm{m^{2}\,s^{-1}}$)
- $r$ = radial coordinate ($\mathrm{m}$)

**Cross-check: global mass balance (independent of the integration above).**

$$
\frac{4}{3}\pi R^{3}S_0 = 4\pi R^{2}h\,w(R) \;\Longrightarrow\; w(R) = \frac{S_0R}{3h} \tag{18, eq:b:massbalance}
$$

where:

- $R$ = particle outer radius ($\mathrm{m}$)
- $S_0$ = uniform volumetric source term ($\mathrm{mol\,m^{-3}\,s^{-1}}$)
- $h$ = convective mass-transfer coefficient ($\mathrm{m\,s^{-1}}$)
- $w(R)$ = steady-state concentration at the surface ($\mathrm{mol\,m^{-3}}$)

**Homogeneous transient problem for the deviation $v=c-w$.** Eq. (17) solves the boundary-value problem but not the initial-value problem: it satisfies the steady PDE and the Robin condition, but not $c(r,0)=0$. Subtracting it off, $v=c-w$, leaves a new unknown that satisfies the same linear, homogeneous PDE and the same homogeneous boundary conditions, but starts from a nonzero, known initial condition, exactly the form an eigenfunction expansion needs:

$$
v_t = \frac{D}{r^2}\left(r^2 v_r\right)_r, \quad
v_r(0,t)=0,\quad -Dv_r(R,t) = h\,v(R,t), \quad
v(r,0) = -w(r) \tag{19, eq:b:vproblem}
$$

where:

- $v$ = deviation of the concentration field from steady state, $v=c-w$ ($\mathrm{mol\,m^{-3}}$)
- $t$ = time (subscript denotes a partial derivative) ($\mathrm{s}$)
- $D$ = diffusivity ($\mathrm{m^{2}\,s^{-1}}$)
- $r$ = radial coordinate (subscript denotes a partial derivative) ($\mathrm{m}$)
- $h$ = convective mass-transfer coefficient ($\mathrm{m\,s^{-1}}$)
- $R$ = particle outer radius ($\mathrm{m}$)
- $w(r)$ = steady-state concentration profile ($\mathrm{mol\,m^{-3}}$)

**Separation of variables, $v(r,t)=\phi(r)\,h(t)$.** Because the equation for $v$ is linear and every one of its boundary conditions is homogeneous, a product solution $v(r,t)=\phi(r)h(t)$ is a valid ansatz. Substituting it in and dividing through by $\phi(r)h(t)$ separates the equation into a function of $t$ alone equal to a function of $r$ alone. That can only hold for every $r$ and $t$ if both sides equal the same constant, written $-\lambda D$:

$$
h'(t) = -\lambda D\,h(t), \qquad \frac{d}{dr}\!\left(r^2\frac{d\phi}{dr}\right) + \lambda r^2\phi = 0 \tag{20, eq:b:sl-ode}
$$

where:

- $h(t)$ = time factor of the separated solution (dimensionless)
- $\lambda$ = separation-of-variables eigenvalue ($\mathrm{s^{-1}}$)
- $D$ = diffusivity ($\mathrm{m^{2}\,s^{-1}}$)
- $\phi(r)$ = spatial eigenfunction ($\mathrm{m^{-1}}$)
- $r$ = radial coordinate ($\mathrm{m}$)

**Substitution $u(r)=r\phi(r)$ collapses the spatial equation to harmonic form.**

$$
u'' + \lambda u = 0 \tag{21, eq:b:harmonic}
$$

where:

- $u(r)$ = auxiliary variable, $u=r\phi$ (dimensionless)
- $\lambda$ = eigenvalue ($\mathrm{s^{-1}}$)

**Eigenfunction of the radial Sturm-Liouville problem.**

$$
\phi(r) = \frac{\sin\!\left(\sqrt{\lambda}\,r\right)}{r} \tag{22, eq:b:eigfun}
$$

where:

- $\phi(r)$ = eigenfunction of the radial Sturm-Liouville problem ($\mathrm{m^{-1}}$)
- $\lambda$ = separation-of-variables eigenvalue ($\mathrm{s^{-1}}$)
- $r$ = radial coordinate ($\mathrm{m}$)

**Robin condition applied to the eigenfunction (pre-simplified form).**

$$
\sin\mu - \mu\cos\mu = \mathrm{Bi}\sin\mu, \qquad \mu \equiv \sqrt{\lambda}\,R \tag{23, eq:b:eigcond-raw}
$$

where:

- $\mu$ = dimensionless trial variable, $\mu=\sqrt{\lambda}R$ (dimensionless)
- $\lambda$ = eigenvalue ($\mathrm{s^{-1}}$)
- $R$ = particle outer radius ($\mathrm{m}$)
- $\mathrm{Bi}$ = Biot number (dimensionless)

---

## 5. Part I, Technique 2: FTCS Discretisation

The finite-difference method depends only on the governing equation's own form. The mesh runs $r_i = i\,\Delta r$, $i=0,\dots,N$, $\Delta r = R/N$, with node $N$ sitting exactly at the surface $r=R$.

**Central-difference approximations of the spatial derivatives.**

$$
\frac{\partial^{2} c}{\partial r^{2}} \approx
\frac{C_{i-1}^{j} - 2C_{i}^{j} + C_{i+1}^{j}}{\Delta r^{2}},
\qquad
\frac{\partial c}{\partial r} \approx
\frac{C_{i+1}^{j} - C_{i-1}^{j}}{2\,\Delta r} \tag{24, eq:central-2nd}
$$

where:

- $C_i^{j}$ = discrete concentration at node $i$, time level $j$ ($\mathrm{mol\,m^{-3}}$)
- $i$ = radial node index (dimensionless)
- $j$ = time-level index (dimensionless)
- $\Delta r$ = radial mesh spacing ($\mathrm{m}$)

**FTCS explicit update rule, regrouped by stencil coefficient.**

$$
C_{i}^{j+1}
= C_{i}^{j}\!\left( 1 - 2\frac{D\Delta t}{\Delta r^{2}} \right)
+ C_{i+1}^{j}\!\left( \frac{D\Delta t}{\Delta r^{2}} + \frac{D\Delta t}{\Delta r^{2}}\,\frac{1}{i} \right)
+ C_{i-1}^{j}\!\left( \frac{D\Delta t}{\Delta r^{2}} - \frac{D\Delta t}{\Delta r^{2}}\,\frac{1}{i} \right)
+ S_i\,\Delta t \tag{25, eq:explicit-coeffs}
$$

where:

- $C_i^{j+1},\,C_i^{j},\,C_{i+1}^{j},\,C_{i-1}^{j}$ = discrete concentration at node $i$ (and its neighbours), time levels $j$ and $j{+}1$ ($\mathrm{mol\,m^{-3}}$)
- $D$ = diffusion coefficient ($\mathrm{m^{2}\,s^{-1}}$)
- $\Delta t$ = time-step size ($\mathrm{s}$)
- $\Delta r$ = radial mesh spacing ($\mathrm{m}$)
- $i$ = radial node index (dimensionless)
- $S_i$ = volumetric source term at node $i$ ($\mathrm{mol\,m^{-3}\,s^{-1}}$)

**Ghost-point relation from centre symmetry.** The point $r=0$ is a coordinate singularity of the spherical Laplacian, not a physical wall, and the particle continues smoothly through its centre. Spherical symmetry then forces the concentration profile to be an even function of $r$ about the origin, so a fictitious node placed one spacing on the far side of the centre must carry the same value as the real node at $r=+\Delta r$:

$$
C_{-1}^{\,j} = C_{1}^{\,j} \tag{26, eq:centre-ghost}
$$

where:

- $C_{-1}^{\,j}$ = discrete concentration at the fictitious ghost node one spacing inside the centre ($\mathrm{mol\,m^{-3}}$)
- $C_1^{\,j}$ = discrete concentration at node 1, time level $j$ ($\mathrm{mol\,m^{-3}}$)

**Collapsed second-derivative stencil at the centre node.**

$$
\left.\frac{\partial^{2} c}{\partial r^{2}}\right|_{i=0} = \frac{2\left(C_1^{\,j} - C_0^{\,j}\right)}{\Delta r^{2}} \tag{27, eq:centre-stencil}
$$

where:

- $c$ = concentration field ($\mathrm{mol\,m^{-3}}$)
- $r$ = radial coordinate ($\mathrm{m}$)
- $C_1^{\,j},\,C_0^{\,j}$ = discrete concentration at node 1 and at the centre node, time level $j$ ($\mathrm{mol\,m^{-3}}$)
- $\Delta r$ = radial mesh spacing ($\mathrm{m}$)

**L'Hopital limit of the curvature term at the centre.**

$$
\lim_{r\to 0} \frac{2}{r}\,\frac{\partial c}{\partial r}
= 2\,\left.\frac{\partial^{2} c}{\partial r^{2}}\right|_{r=0} \tag{28, eq:lhopital}
$$

where:

- $c$ = concentration field ($\mathrm{mol\,m^{-3}}$)
- $r$ = radial coordinate ($\mathrm{m}$)

**Combined radial operator at the centre (curvature limit plus collapsed stencil).**

$$
\left.\left(\frac{\partial^{2} c}{\partial r^{2}} + \frac{2}{r}\frac{\partial c}{\partial r}\right)\right|_{r=0} = 3\left.\frac{\partial^{2} c}{\partial r^{2}}\right|_{r=0} = \frac{6\left(C_1^{\,j} - C_0^{\,j}\right)}{\Delta r^{2}} \tag{29, eq:centre-curvature-limit}
$$

where:

- $c$ = concentration field ($\mathrm{mol\,m^{-3}}$)
- $r$ = radial coordinate ($\mathrm{m}$)
- $3$ = combined coefficient: the curvature term's l'Hopital factor of two, plus the existing second-derivative term itself (dimensionless)
- $6$ = the resulting centre-node stencil coefficient, $3\times2$ from the collapsed stencil (dimensionless)
- $C_1^{\,j},\,C_0^{\,j}$ = discrete concentration at node 1 and at the centre node, time level $j$ ($\mathrm{mol\,m^{-3}}$)
- $\Delta r$ = radial mesh spacing ($\mathrm{m}$)

**Explicit centre-node update (the "factor of six").**

$$
C_{0}^{j+1} = C_{0}^{j}
+ 6\,D\Delta t\,\frac{C_{1}^{j} - C_{0}^{j}}{\Delta r^{2}}
+ S_0\,\Delta t \tag{30, eq:centre-update}
$$

where:

- $C_0^{j+1},\,C_0^{j}$ = discrete concentration at the centre node (node 0), time levels $j{+}1$ and $j$ ($\mathrm{mol\,m^{-3}}$)
- $C_1^{j}$ = discrete concentration at the first node (node 1), time level $j$ ($\mathrm{mol\,m^{-3}}$)
- $D$ = diffusion coefficient ($\mathrm{m^{2}\,s^{-1}}$)
- $\Delta t$ = time-step size ($\mathrm{s}$)
- $\Delta r$ = radial mesh spacing ($\mathrm{m}$)
- $S_0$ = kernel source strength ($\mathrm{mol\,m^{-3}\,s^{-1}}$)
- $6$ = curvature-limit coefficient from l'Hopital's rule at $r=0$ in spherical coordinates (dimensionless)

Only the outer boundary needs a treatment beyond the general interior stencil above, since the coolant here is a finite convective resistance rather than a fixed concentration: the Robin condition, Eq. (3).

**Ghost-point relation from the Robin surface condition.** The centre-node ghost value was fixed above by a zero-derivative condition, Eq. (26). The surface ghost value here is fixed instead by the nonzero-derivative Robin condition, Eq. (3), so it comes out as an expression in the current solution rather than a constant. Approximating $\partial c/\partial r|_{r=R}$ with the same central difference used everywhere else and substituting it into Eq. (3):

$$
-D\,\frac{C_{N+1}^{j}-C_{N-1}^{j}}{2\Delta r} = h\,C_{N}^{j}
\;\Longrightarrow\;
C_{N+1}^{j} = C_{N-1}^{j} - \frac{2h\Delta r}{D}\,C_{N}^{j} \tag{31, eq:b:ftcs-ghost}
$$

where:

- $C_{N+1}^{\,j}$ = discrete concentration at the fictitious ghost node one spacing beyond the surface, time level $j$ ($\mathrm{mol\,m^{-3}}$)
- $C_{N-1}^{\,j}$ = discrete concentration one spacing inside the surface, time level $j$ ($\mathrm{mol\,m^{-3}}$)
- $C_N^{\,j}$ = discrete concentration at the surface node, time level $j$ ($\mathrm{mol\,m^{-3}}$)
- $D$ = diffusivity ($\mathrm{m^{2}\,s^{-1}}$)
- $h$ = convective mass-transfer coefficient ($\mathrm{m\,s^{-1}}$)
- $\Delta r$ = radial mesh spacing ($\mathrm{m}$)
- $N$ = index of the surface node (dimensionless)

**Explicit surface-node update with the Robin ghost eliminated.** Substituting Eq. (31) into the interior update rule, Eq. (25) evaluated at $i=N$, removes the fictitious node, leaving an update written entirely in terms of real mesh values. Writing the result in terms of the mesh-local convective ratio $\kappa\equiv h\Delta r/D$, which is just the physical Biot number, Eq. (4), rescaled by the number of mesh cells, $\kappa=\mathrm{Bi}/N$:

$$
C_{N}^{j+1} = C_{N}^{j}\Bigl[1-2\,\mathrm{Fo}\bigl(1+\kappa(1+\tfrac1N)\bigr)\Bigr]
+ 2\,\mathrm{Fo}\,C_{N-1}^{j} + S_0\Delta t,
\qquad \kappa \equiv \frac{h\Delta r}{D} = \frac{\mathrm{Bi}}{N} \tag{32, eq:b:ftcs-surface}
$$

where:

- $C_N^{\,j+1},\,C_N^{\,j}$ = discrete concentration at the surface node, time levels $j{+}1$ and $j$ ($\mathrm{mol\,m^{-3}}$)
- $C_{N-1}^{\,j}$ = discrete concentration one spacing inside the surface, time level $j$ ($\mathrm{mol\,m^{-3}}$)
- $\mathrm{Fo}$ = mesh Fourier number, $D\,\Delta t/\Delta r^{2}$ (dimensionless)
- $\kappa$ = mesh-local convective ratio, $h\Delta r/D$, equal to the physical Biot number divided by $N$ (dimensionless)
- $N$ = index of the surface node, and total mesh-cell count from centre to surface (dimensionless)
- $S_0$ = uniform volumetric source term ($\mathrm{mol\,m^{-3}\,s^{-1}}$)
- $\Delta t$ = time-step size ($\mathrm{s}$)

**Stability bound tightened by the convective boundary.** An explicit scheme stays bounded only while every coefficient multiplying a known value in an update rule remains non-negative. A negative coefficient would let a local excess amplify rather than spread out. The interior nodes already require $\mathrm{Fo}\le\tfrac12$, from the stencil derived above. The surface update in Eq. (32) carries an extra loss term proportional to $\kappa$, so its own coefficient turns negative sooner:

$$
\mathrm{Fo} \le \frac{1}{2\bigl(1+\kappa(1+\tfrac1N)\bigr)} < \frac12 \qquad (\kappa>0) \tag{33, eq:b:ftcs-stability}
$$

where:

- $\mathrm{Fo}$ = mesh Fourier number (dimensionless)
- $\kappa$ = mesh-local convective ratio, $h\Delta r/D$ (dimensionless)
- $N$ = index of the surface node (dimensionless)

Any finite surface resistance ($\kappa>0$) therefore forces a smaller time step than an idealised perfect-sink boundary would, where $\kappa=0$ and the bound relaxes back to exactly $\mathrm{Fo}\le\tfrac12$.
