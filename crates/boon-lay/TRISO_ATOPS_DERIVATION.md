# TRISO-ATOPS fission-product-release model — a step-by-step derivation

> ⚠️ **Unverified until validated.** All code in this workspace is **unverified
> and untrusted** unless a specific verification & validation (V&V) case
> demonstrates otherwise. Not for nuclear facility operation, reactor control,
> safety-critical, or licensing decisions. See the workspace
> `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`.

This document derives the **TRISO-ATOPS** fission-product-release model from
first principles — from Fick's law of diffusion and the law of radioactive
decay — and shows, step by step, how each equation becomes a line of the
**upstream INL Python** implementation.

- It documents the **upstream Python** model in
  `upstream_source/TRISO-ATOPS/trisoatops/utility_functions/calculation_functions.py`
  (plus `trisoatops.py`). Every derivation step names the exact Python function
  that implements it.
- For the **Rust port** (`boon_lay::triso_atops_fork`) the same narrative is
  reproduced in the module-level rustdoc and cross-referenced from
  `docs/triso-atops-derivation.md`. This file is the *physics-and-Python* view;
  that file is the *Rust-port* view.

TRISO-ATOPS is a fork of Idaho National Laboratory's *TRISO Analysis TOol for
Predictive Source terms*. The physics equations are those of the NP-MHTGR New
Production Reactor Program (ref. [1]); half-lives are from the IAEA Live Chart
of Nuclides (ref. [2]). Note that the TRISO-ATOPS **User Manual** deliberately
does *not* reproduce the equations or the theory (Manual §3), so the closed-form
solutions below are derived here from the standard diffusion literature (Booth
ref. [3], Crank ref. [4]) and matched term-by-term to the code.

## Contents

1. [First principles: diffusion and radioactive decay](#1-first-principles-diffusion-and-radioactive-decay)
2. [Diffusion out of a sphere; the equivalent-sphere idealisation](#2-diffusion-out-of-a-sphere-the-equivalent-sphere-idealisation)
3. [The Booth model and the effective diffusion coefficient `D'`](#3-the-booth-model-and-the-effective-diffusion-coefficient-d)
4. [Fractional release of a stable species](#4-fractional-release-of-a-stable-species)
5. [Adding decay: release of a decaying species](#5-adding-decay-release-of-a-decaying-species)
6. [The assembled model: from release fraction to source term](#6-the-assembled-model-from-release-fraction-to-source-term)
7. [The transient (accident) model](#7-the-transient-accident-model)
8. [Upstream quirks and approximations flagged during this derivation](#8-upstream-quirks-and-approximations-flagged-during-this-derivation)
9. [References](#9-references)

---

## 1. First principles: diffusion and radioactive decay

Two physical laws underlie the entire model.

**Fick's second law** governs how a diffusing species spreads. If $C(\mathbf{x},
t)$ is the concentration (atoms per m³) of a fission product in a solid, and $D$
is its diffusion coefficient (m²/s), then in the absence of sources or sinks

$$\frac{\partial C}{\partial t} = D \nabla^2 C .$$

The diffusion coefficient $D$ measures how fast the species migrates through the
lattice; it is strongly temperature-dependent (Step 3).

**The law of radioactive decay** governs how an unstable nuclide disappears. If
$N(t)$ is the number of atoms of a nuclide with decay constant $\lambda$ (s⁻¹),

$$\frac{dN}{dt} = -\lambda N ,$$

whose solution is $N(t) = N_0 \, e^{-\lambda t}$. The decay constant is fixed by
the half-life $t_{1/2}$ through

$$\lambda = \frac{\ln 2}{t_{1/2}} .$$

**Code correspondence.** The decay constant is computed once per nuclide in the
Python `Nuclide.__init__`:

```python
self.lam = np.log(2) / hl        # lambda = ln(2) / t_half,  s^-1
```

A fission product inside a TRISO fuel kernel obeys **both** laws at once. Adding
a uniform production (birth) rate $B$ (atoms per m³ per s) from fission and the
decay sink to Fick's law gives the governing equation for a radioactive species
diffusing in the fuel,

$$\frac{\partial C}{\partial t} = D \nabla^2 C - \lambda C + B .$$

Everything that follows is a solution of this one equation under different
geometries, boundary conditions, and limits.

## 2. Diffusion out of a sphere; the equivalent-sphere idealisation

A real TRISO particle is a fuel kernel (UCO or UO₂) wrapped in concentric
coating layers — a porous carbon buffer, an inner pyrolytic-carbon layer, a
silicon-carbide (SiC) pressure boundary, and an outer pyrolytic-carbon layer.
Fuel is a compact of many such particles held in a graphite matrix. Solving the
diffusion equation in that full multi-shell geometry is intractable in closed
form.

**The Booth equivalent-sphere idealisation** (ref. [3]) replaces the real
microstructure with a single uniform sphere of radius $a$ and a single effective
diffusion coefficient. The species is assumed born uniformly throughout the
sphere and to escape instantly once it reaches the surface (surface
concentration held at zero — a perfect sink). This reduces the problem to
Fick's law in a sphere with spherical symmetry,

$$\frac{\partial C}{\partial t}
   = D \left( \frac{\partial^2 C}{\partial r^2}
              + \frac{2}{r} \frac{\partial C}{\partial r} \right)
     - \lambda C + B ,
   \qquad C(a, t) = 0 .$$

TRISO-ATOPS applies this idealisation in three different ways depending on the
chemistry of the fission product (chosen by atomic number $Z$):

- **Metals** (Sr, Rb, Cs, Ba, Eu, and "other" fission metals) diffuse out of the
  fuel grain. Their equivalent-sphere radius blends the fuel grain size
  $a_\text{grain}$ and the kernel radius $r$:

  $$a_\text{booth} = \sqrt{2 \, a_\text{grain} \, r} .$$

- **Silver** (Ag) and palladium (Pd) are limited not by the kernel but by the
  SiC layer they must permeate; they use the *breakthrough* (membrane) solution
  of Step 5 with the SiC thickness $a_\text{SiC}$ as the barrier.

- **Volatiles** (noble gases Kr, Xe; halogens I, plus Se and Te grouped with
  them) are released so readily from failed particles that TRISO-ATOPS uses an
  *empirical* release-to-birth correlation rather than a diffusion solution
  (Step 5).

**Code correspondence.** The grouping and the equivalent-sphere radius are set in
`R_B_fail`:

```python
a_booth = (a_grain * 2 * r) ** 0.5
if z in noble_gases or z in halogens:  ...   # empirical correlation
elif z in special_metals:              ...   # Booth sphere, radius a_booth
elif z == 47 or z == 46:               ...   # breakthrough through SiC
else:                                  ...   # nominal 1e-5
```

## 3. The Booth model and the effective diffusion coefficient `D'`

Every sphere solution below depends on $D$ and the radius $a$ only through the
single combination

$$D' \equiv \frac{D}{a^2} ,$$

which has units of s⁻¹ and is the **reduced (equivalent) diffusion
coefficient**. It sets the characteristic timescale of release: $1/D'$ is
roughly the time for the sphere to empty. Collapsing the two parameters $D$ and
$a$ into one $D'$ is the essence of the Booth model — the geometry is folded into
an effective rate.

**Temperature dependence.** Solid-state diffusion is thermally activated, so $D$
follows an **Arrhenius law**

$$D(T) = D_0 \, \exp\!\left( -\frac{Q}{R \, T} \right) ,$$

with pre-exponential $D_0$ (m²/s), activation energy $Q$ (J/mol), molar gas
constant $R = 8.31447 \ \text{J}\,\text{mol}^{-1}\text{K}^{-1}$, and absolute
temperature $T$ (K). Some species need a sum of two Arrhenius terms (a low- and
a high-temperature mechanism). The NP-MHTGR correlations (ref. [1]) are valid
roughly $700$–$2400\ ^\circ\text{C}$; below a species' valid range the
temperature is **clamped** to the boundary rather than extrapolated (Manual §5,
"Results look incorrect").

**Code correspondence.** `diffusion_coefficient(z, T, T_graph)` selects the
correlation family by $Z$ and evaluates the Arrhenius term(s). For the volatile
group, for example,

```python
if T < 1500: D = 1.3e-12 * np.exp(-126 * 1000 / (T + 273.15) / 8.31447)
else:        D = 8.8e-15 * np.exp(-54  * 1000 / (T + 273.15) / 8.31447) \
               + 6e-1    * np.exp(-480 * 1000 / (T + 273.15) / 8.31447)
```

The `T + 273.15` is the °C → K conversion; the `* 1000` turns kJ/mol into J/mol.
Silver's rate-limiting SiC coefficient is a separate correlation,
`diffusion_coefficient_SiC_Ag(T)`:

$$D_\text{Ag,SiC}(T) = 3.6\times10^{-9}
   \exp\!\left( -\frac{215\,000}{R\,T} \right) .$$

The reduced coefficient $D' = D/a^2$ itself is formed inside each release model
(e.g. `Dp = D / a / a` in `breakthrough_model` and `booth_longlived`).

## 4. Fractional release of a stable species

Consider first a **stable** species ($\lambda = 0$), born uniformly in the Booth
sphere up to time $t = 0$ and then released with the surface held at zero
concentration. Solving Fick's law in the sphere by separation of variables gives
the classic result for the **fractional release** $f$ — the fraction of the
initial inventory that has escaped by time $t$ (ref. [4], Crank §6):

$$f(t) = 1 - \frac{6}{\pi^2}
   \sum_{n=1}^{\infty} \frac{1}{n^2}
   \exp\!\left( -n^2 \pi^2 D' t \right) ,
   \qquad D' = \frac{D}{a^2} .$$

**Limits.**

- As $D' t \to \infty$ every exponential vanishes and $f \to 1$ (the sphere
  empties completely).
- For small $D' t$ the series is slow to converge; its closed-form **short-time
  expansion** (ref. [4]) is

  $$f(t) \approx 6 \sqrt{\frac{D' t}{\pi}} - 3 D' t .$$

  The leading $\sqrt{t}$ behaviour is the signature of one-dimensional diffusion
  out of a surface.

**Code correspondence.** The full series is `booth_longlived(D, t, a)`:

```python
Dp = D / a / a
i = np.arange(1, num_terms)
terms = np.exp(-(i * np.pi) ** 2 * Dp * t) * (i * np.pi) ** -2
RF_2 = 1 - 6 * np.sum(terms)
```

Note that $6/\pi^2 \cdot \sum 1/n^2 (\dots)$ is written as $6 \sum
1/(n\pi)^2(\dots)$ — the $\pi^2$ is folded into the denominator. This function is
used for **long-lived** metals, whose inventory keeps accumulating over the
irradiation time so that the time-dependent fractional release (evaluated at the
irradiation time $t$) is the physically relevant quantity. The short-time
expansion is the analytic check exercised by the port's
`booth_longlived_early_time_matches_analytic` test.

## 5. Adding decay: release of a decaying species

Now restore the decay term. Two distinct regimes matter.

### 5a. Short-lived species — the steady-state release-to-birth ratio

A **short-lived** nuclide reaches secular equilibrium during irradiation: it is
produced and decays fast enough that its concentration profile stops changing,
$\partial C / \partial t \to 0$. Setting the sphere equation to steady state
with uniform birth $B$, decay $\lambda$, and $C(a) = 0$,

$$D \left( \frac{d^2 C}{dr^2} + \frac{2}{r}\frac{dC}{dr} \right)
   - \lambda C + B = 0 ,$$

and integrating the diffusive surface flux against the total birth rate gives the
steady **release-to-birth ratio** (ref. [1], [3]),

$$\left\langle \frac{R}{B} \right\rangle
   = \frac{3}{\mu}\left( \coth \mu - \frac{1}{\mu} \right) ,
   \qquad \mu = \sqrt{\frac{\lambda a^2}{D}} = \sqrt{\frac{\lambda}{D'}} .$$

The dimensionless group $\mu$ compares the decay rate to the diffusion rate.

**Limits.**

- Fast diffusion or long half-life ($\mu \to 0$): $\langle R/B \rangle \to 1$
  (everything born escapes before decaying).
- Slow diffusion or short half-life ($\mu \to \infty$): $\coth \mu \to 1$ and
  $\langle R/B \rangle \to 3/\mu$ (decay wins; only a thin surface layer
  escapes).

**Code correspondence.** `booth_shortlived_fastdiffuse(D, lam, a)`:

```python
x = (lam * a * a / D) ** 0.5          # x = mu
RF = 3 / x * (1 / np.tanh(x) - 1 / x) # coth = 1/tanh
```

### 5b. Silver through the SiC barrier — the breakthrough (membrane) model

Silver is not held up by the kernel but by the SiC layer, which it must permeate
like a gas through a membrane. The relevant solution is the **Daynes–Barrer
time-lag** solution (ref. [4], Crank §4) for the cumulative amount that has
diffused through a plane barrier of thickness $a$, with a fixed upstream source
and a perfect downstream sink, starting from an empty barrier:

$$\frac{Q(t)}{a\,C_0}
   = \frac{D t}{a^2} - \frac{1}{6}
     - \frac{2}{\pi^2} \sum_{n=1}^{\infty}
        \frac{(-1)^n}{n^2}\exp\!\left(-n^2\pi^2 D' t\right) .$$

The first term is the steady permeation rate; the constant $-1/6$ is the
**time lag** (breakthrough delay) before steady flow is established; the series
is the decaying transient. TRISO-ATOPS converts this per-area permeation into a
**release fraction of the spherical kernel** by multiplying by the kernel
surface-to-volume ratio $S/V = 3/r$ (kernel radius $r$). Carrying the $3/r$
through every term gives exactly the code's expression:

$$RF = \frac{3 D t}{r\,a} - \frac{a}{2 r}
   - \frac{6 a}{r}\sum_{n=1}^{\infty}
      \frac{(-1)^n}{(n\pi)^2}\exp\!\left(-(n\pi)^2 D' t\right) .$$

(The constant maps as $\frac{1}{6}\cdot a \cdot \frac{3}{r} = \frac{a}{2r}$;
the series prefactor as $\frac{2}{\pi^2}\cdot a \cdot \frac{3}{r} =
\frac{6a}{r\pi^2}$.) The result is clamped to $[0, 1]$.

**Code correspondence.** `breakthrough_model(D, t, a, r)`:

```python
Dp = D / a / a
sum = 0
for n in range(1, num_terms):
    sum += (-1) ** n / ((n * np.pi) ** 2) * np.exp(-(n * np.pi) ** 2 * Dp * t)
RF = 3 * (D * t) / r / a - a / 2 / r - 6 * a / r * sum
if RF > 1: RF = 1
if RF < 0: RF = 0
```

For silver the barrier coefficient is the SiC one, and the result is scaled by
$\sqrt{\lambda_{\text{Ag-110m}} / \lambda}$ to reference all silver isotopes to
Ag-110m (in `R_B_fail`).

### 5c. Volatiles — the empirical noble-gas / halogen correlation

For noble gases and halogens released from **failed** particles, TRISO-ATOPS does
not solve a diffusion equation at all; it uses the NP-MHTGR **empirical**
release-to-birth fit (ref. [1]),

$$\left\langle \frac{R}{B} \right\rangle_\text{fail}
   = \exp\!\left( n \ln\frac{1}{\lambda} + \frac{B}{T} + C \right) ,$$

with $(n, B, C)$ one set of constants for krypton and another for
xenon/halogens. This is a correlation, not a first-principles result — it
captures the observed $\lambda$- and $T$-dependence of volatile release from
exposed fuel.

**Code correspondence.** `RB_fail_Noble_Gases(z, lam, T)`:

```python
if z == 36:            n, B, C = 0.325, -8572, -1.41   # Kr
elif z == 54 or z in halogens: n, B, C = 0.302, -7793, -2.73  # Xe, halogens
return np.exp(n * np.log(1 / lam) + B / (T + 273.15) + C)
```

### 5d. Graphite hold-up — the attenuation factor

Metals released from the fuel do not reach the coolant instantly; they must
still diffuse through the surrounding graphite, which **attenuates** (delays)
the release. Treating the graphite as a plane slab of thickness $a$ with
transient diffusion, TRISO-ATOPS forms a hold-up series and defines an
**attenuation factor**

$$S = \sum_{i\ \text{odd}} \frac{4}{i\pi}\sin\!\frac{i\pi}{2}
      \exp\!\left(-\frac{(i\pi)^2 D_\text{graph}\, t}{4 a^2}\right) ,
   \qquad Af = \frac{1}{1 - S} .$$

At $t = 0$ the series is the Leibniz sum $S = \frac{4}{\pi}(1 - \frac{1}{3} +
\frac{1}{5} - \dots) = 1$, so $Af \to \infty$ (total hold-up — nothing has emerged
yet); this singular case is capped at $10^8$. As $t \to \infty$, $S \to 0$ and
$Af \to 1$ (the graphite is saturated and no longer attenuates). The coolant
source rate is then $S_\text{coolant} = R / Af$: large $Af$ early means almost
nothing reaches the coolant.

**Code correspondence.** `attenuation_factor(D_graph, t, a)`:

```python
i = np.arange(1, num_terms, 2)                      # odd i only
terms = 4 / i / np.pi * np.sin(i * np.pi / 2) \
        * np.exp(-(i * np.pi) ** 2 * D_graph * t / 4 / a / a)
Af = 1 / (1 - np.sum(terms))
if Af > 1e8 or Af < 0: Af = 1e8
```

## 6. The assembled model: from release fraction to source term

The per-nuclide release models above are composed into a full normal-operation
source term. For each nuclide at each reactor node, `trisoatops.py::normal_operation`
runs the following chain.

**(i) Release-to-birth at failure — `R_B_fail`.** Dispatch by group (Step 2):
volatiles use §5c, special metals use §5a (short-lived) or §4 (long-lived),
silver uses §5b, and anything else gets a fixed nominal $\langle R/B
\rangle_\text{fail} = 10^{-5}$.

**(ii) Release rate — `release_rate`.** The actual release-to-birth ratio is the
failure fraction times $\langle R/B \rangle_\text{fail}$, applied to the nuclide
birth rate. For a short-lived nuclide the birth rate equals the inventory
activity $A$ (secular equilibrium):

$$R = \left\langle \frac{R}{B} \right\rangle \cdot A
   \qquad (\text{short-lived}) .$$

For a long-lived nuclide the inventory has not saturated, so the birth rate is
$A / (1 - e^{-\lambda t})$:

$$R = \left\langle \frac{R}{B} \right\rangle \cdot
      \frac{A}{1 - e^{-\lambda t}}
   \qquad (\text{long-lived}) .$$

In code:

```python
if sl is True: didt = inventories * 3.7e10
else:          didt = inventories * 3.7e10 / (1 - np.exp(-t * lam))
return didt * RB
```

The `* 3.7e10` converts the curie inventory to becquerels (atoms/s); $1\ \text{Ci}
= 3.7\times10^{10}\ \text{Bq}$.

**(iii) Source rate and graphite hold-up — `base_activities`.** The release rate
$R$ is split into the part that reaches the coolant, $S = R / Af$ (using the
attenuation factor of §5d), and the part retained in the graphite,
$G = R\,(1 - 1/Af)\,(1 - e^{-\lambda t})/\lambda$. Volatiles bypass the graphite
entirely ($S = R$, $G = 0$).

**(iv) Primary-loop pools — `circulating` / `plate_out` / `clean_up`.** The
coolant source rate $S$ feeds three linear activity balances sharing the total
removal rate $\beta = \lambda + k_\text{plate} + k_\text{clean}$:

$$C = \frac{S\,(1 - e^{-\beta t})}{\beta}
      + \frac{\lambda\,C_\text{parent}}{\beta}
   \qquad (\text{circulating}) ,$$

$$P = \frac{k_\text{plate}}{\beta - \lambda}
      \left( \frac{S}{\lambda}(1 - e^{-\lambda t}) - C \right)
      + P_\text{parent}
   \qquad (\text{plate-out}) ,$$

$$H\!P\!S = \frac{k_\text{clean}}{\beta - \lambda}
      \left( \frac{S}{\lambda}(1 - e^{-\lambda t}) - C \right)
      + H\!P\!S_\text{parent}
   \qquad (\text{clean-up}) .$$

Here $C$ is the activity circulating in the coolant, $P$ the activity plated onto
loop surfaces (rate constant $k_\text{plate}$), and $H\!P\!S$ the activity
removed by an optional helium-purification system (rate constant
$k_\text{clean}$). The `_parent` terms chain a parent nuclide's pools into its
daughter. Group-dependent routing (in `higher_activities`) zeroes $k_\text{plate}$
for noble gases, applies clean-up only to volatiles, and so on.

**(v) Report in curies.** Every pool is finally multiplied by $\lambda / 3.7
\times 10^{10}$ to convert atom counts (or atoms/s) back to curies (or Ci/s), the
reported units.

The composition is exactly the per-node loop body of
`trisoatops.py::normal_operation`.

## 7. The transient (accident) model

During an accident the temperature — and therefore $D$ — changes with time, so a
single $D' t$ is no longer meaningful. TRISO-ATOPS replaces the products $D t$
and $D' t$ everywhere by their **time integrals** along the temperature history:

$$\int_0^t D\,dt' \quad (\text{units } \text{m}^2), \qquad
  \int_0^t D'\,dt' = \frac{1}{a^2}\int_0^t D\,dt' \quad (\text{dimensionless}) .$$

**Code correspondence.** `integrate` accumulates the trapezoidal cumulative
integral of $D(T(t))$ over the temperature history, then the transient models
reuse the Step 4/5 series with $D' t \to \int D'\,dt'$:

- `booth_transient(int_Dp)` — the transient form of `booth_longlived` (§4):
  $RF = 1 - 6\sum 1/(i\pi)^2 \exp(-(i\pi)^2 \int D'\,dt')$.
- `breakthrough_model_transient(int_Dp, int_Dt, a, r)` — the transient form of
  `breakthrough_model` (§5b), with $D t \to \int D\,dt'$ and $D' t \to \int
  D'\,dt'$.
- `RF_Graph(int_Dt, a)` — the graphite **release** fraction (the complement of the
  hold-up of §5d), $RF = \sum_{i\ \text{odd}} 8/(i\pi)^2\,(1 - \exp(-(i\pi)^2
  \int D\,dt' / 4a^2))$. Since $\sum_{i\ \text{odd}} 8/(i\pi)^2 = 1$, this
  saturates to 1 as the graphite fully releases.

The dispatcher `release_fraction(z, ..., material)` routes kernel vs. graphite
and silver vs. non-silver. The release fractions are then turned into release
activities (`release_activity`) and combined with a coolant-venting fraction
(`coolant_release`) and a lift-off contribution from the plated-out inventory to
give the total accident release (`accident_case`).

## 8. Upstream quirks and approximations flagged during this derivation

These are recorded so the port faithfully reproduces upstream behaviour (a
*verification* port matches the reference, quirks included) rather than silently
"fixing" it.

- **`clean_up_steadystate` ignores `HPS_parent`.** Unlike
  `circulating_steadystate` and `plate_out_steadystate`, which add their parent
  pool, the steady-state clean-up form omits the `HPS_parent` term (it computes
  `k_clean * S / lam / beta` only). The time-dependent `clean_up` *does* add it.
  The Rust port preserves this asymmetry deliberately (see
  `coolant_activity::clean_up_steadystate` doc comment).

- **`nuclide_import` never actually sets `parent_decay`.** In the parent-in-list
  branch the code writes `nuclide_out[nuclide].parent_decay == True` (a `==`
  comparison whose result is discarded) instead of `=` (assignment). The flag is
  therefore left at its default in that branch — a genuine upstream typo. The
  effect is that parent-decay chaining may not switch on where the author
  intended; downstream code that reads `parent_decay` should be understood in
  that light.

- **The Booth short-time expansion** $6\sqrt{D' t/\pi} - 3 D' t$ is an
  approximation valid only for small $D' t$; the code always evaluates the full
  series, and the expansion is used only as an analytic verification check.

- **Temperature clamping, not extrapolation.** Below a species' valid Arrhenius
  range the temperature is clamped to the boundary (e.g. Cs/Rb kernel clamped to
  $\ge 700\ ^\circ\text{C}$). Results in the clamped region are boundary values,
  not physical extrapolations (Manual §5).

- **Graph-read / nominal constants.** The "other" fission-metal groups get a
  fixed $\langle R/B \rangle_\text{fail} = 10^{-5}$ and a fixed graphite
  attenuation $Af = 10^8$; these are engineering placeholders, not derived
  quantities.

- **`release_activity` silver test uses `z == 47 or z == 48`.** The
  accident-path `release_activity` tests silver with `z == 48` (cadmium) where
  the transport grouping elsewhere uses `z == 46` (palladium). This looks like an
  upstream inconsistency; it only affects the still-scaffolded accident-activity
  path and is flagged here for whoever ports it.

## 9. References

1. E. Anderson, E. Arbtin, B. Barnes, C. Barnes, A. Bowman, M. Carboneau, G.
   Dinneen, K. Moor, R. Moore, D. Petti, S. Thurmond, C. Bendixsen, K. Bulhman,
   R. Henry, A. Roeh, L. H. G. Goldman, S. Langer, P. Lobner, P. Voilleque, and
   D. Rosholt, *Generic Reactor Plant Description and Source Terms Volume 1*,
   EG&G Idaho Inc., Idaho Falls, 1989. (The NP-MHTGR New Production Reactor
   Program equations; TRISO-ATOPS User Manual ref. [0].)
2. IAEA, *Live Chart of Nuclides*, IAEA, November 2023.
   <https://www-nds.iaea.org/relnsd/vcharthtml/VChartHTML.html> (nuclide
   half-lives; User Manual ref. [2]).
3. A. H. Booth, *A Method of Calculating Fission Gas Diffusion from UO₂ Fuel and
   its Application to the X-2-f Loop Test*, Atomic Energy of Canada Limited,
   AECL-496 (CRDC-721), 1957. (The equivalent-sphere model.)
4. J. Crank, *The Mathematics of Diffusion*, 2nd ed., Oxford University Press,
   1975. (Diffusion out of a sphere, §6; the plane-membrane time-lag solution,
   §4.)
5. TRISO-ATOPS User Manual, INL/MIS-26-90986 Rev. 0, B. D. Stoyer, K. E. Egan,
   A. C. Raichart, D. A. Petti, Idaho National Laboratory, 2026. (Program
   structure, parameter definitions, and valid ranges;
   `upstream_source/TRISO-ATOPS/TRISOATOPS User Manual.pdf`.)

---

*Rust-port view:* see [`docs/triso-atops-derivation.md`](docs/triso-atops-derivation.md)
for the same derivation mapped onto the `boon_lay::triso_atops_fork` modules and
types, and the module-level rustdoc (`cargo doc -p boon-lay --no-deps`).
