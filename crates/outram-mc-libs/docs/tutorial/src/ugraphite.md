# Uranium in graphite: why a reactor needs a moderator

> **Research, education and V&V only.** Not for reactor operation, licensing,
> safety decisions or emergency response
> ([`RESPONSIBLE_USE.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/RESPONSIBLE_USE.md)).
> **Review status: first draft, 2026-10-05, AI-assisted, not yet reviewed by a
> human.** Built from
> [`@@COMMIT_SHORT@@`](https://github.com/theodoreOnzGit/outram-park-backend/commit/@@COMMIT@@)
> on @@BUILD_DATE@@; every code link points at that commit.

<div class="mcw-demo" data-mc-widget="demo" data-src="../../demos/monte-carlo/?rung=ugraphite&amp;mode=watch" data-label="▶ Start the uranium-in-graphite demo here (Watch mode)"></div>

*The demo downloads five ENDF/B-VIII.0 tapes and graphite's thermal-scattering
law and processes them in your browser before the first neutron flies: allow a
minute or two. Each track is a real history from the transport code; chaining
them one after another is an illustration. Watch the colour: a neutron is born
fast (yellow-white), and it is slow and dark long before it is absorbed.*

## The problem

Godiva (rung 1) was **93.8 % U-235** by atoms (`vv::godiva`), and every one of
its fissions was caused by a fast neutron. Natural uranium is only
**0.72 % U-235** (the IUPAC composition used here, `vv::ugraphite::NAT_U`).
A lump of natural uranium alone never goes critical: its fast neutrons are
lost to U-238 before they find enough U-235. Yet the first reactors were
built from natural uranium and graphite.

> *History placeholder: the 1942 Chicago pile (natural uranium and uranium
> oxide in graphite) is the obvious hook here. No source on it is in the
> literature corpus yet (`crates/kovan-literature/CATALOGUE.md`, searched
> 2026-10-05), so nothing about it is stated as fact on this page. A sourced
> account, with page numbers, is waiting for the maintainer.*

**The question of this lesson:** *what does the graphite do?* Try the
simplest version first: uranium mixed **evenly** through graphite, an infinite
homogeneous mixture. (History did not do it this way. Step 7 shows why.)

The model is a cube of one mixture with **reflective walls**: a neutron that
reaches a wall comes back, so nothing leaks and the cube behaves as an infinite
medium. Its multiplication factor is $k_\infty$. The mixture of the main case is
the uranium and carbon of one HTR-10 fuel pebble (17 wt% U-235) smeared evenly:
**767.2 carbon atoms per uranium atom** ($N_C/N_U = 767.2$), computed from the
published pebble specification (`vv::ugraphite::htr10_pebble_mix`) and not
chosen to give any particular $k$.

The model the code runs is these few lines:

```rust,ignore
{{#include ../../../examples/ugraphite_four_factor.rs:model}}
```

([`ugraphite_four_factor.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#@@L:crates/outram-mc-libs/examples/ugraphite_four_factor.rs:anchor=model@@);
the compositions are in
[`vv::ugraphite`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/vv/ugraphite.rs#@@L:crates/outram-mc-libs/src/vv/ugraphite.rs:fn=htr10_pebble_mix@@).)

---

## 1. Why slow the neutrons down at all?

**Answer.** Because U-235 is far more willing to fission when the neutron is
slow. Its fission cross section rises roughly as $1/v$ as the neutron slows,
by orders of magnitude between a fast fission neutron and a thermal one, while
U-238 at thermal energies only captures weakly. Slow the neutrons down to
thermal energies, and the 0.72 % of U-235 can win against the 99.3 % of
U-238. Carbon is there to do the slowing, and itself absorbs very little.

The catch is the middle of the energy range. On the way down, U-238 has
**resonances**: narrow energies where its capture cross section spikes by
thousands of times. A neutron that happens to land on one is likely to be
captured. Seeing the curves is the best start: open the nuclear data demo's
reconstructed cross sections and look at U-235 fission, U-238 capture and
carbon scattering on a log-log scale.

<div class="mcw-demo" data-mc-widget="demo" data-src="../../demos/nuclear-data/?rung=reconr" data-label="▶ Look at the cross sections (nuclear data demo, RECONR)"></div>

*No numbers are quoted here on purpose: read them off the curves, which come
from our own processing of the ENDF/B-VIII.0 evaluations (the nuclear data
track explains where they come from).*

In the code, every cross section the transport uses is one call:
`Nuclide::xs_at_energy(e, temp_k)`, which reads the pointwise table that the
workspace's NJOY port reconstructed and Doppler-broadened from the ENDF tape.
Step 5 walks to it.

<div class="predict">

**Predict.** A 2 MeV neutron hits a carbon nucleus head-on. What fraction of
its energy can it lose in one collision: all of it, about a quarter, or about
2 %? And off a U-238 nucleus?

</div>

## 2. One collision with carbon

**Answer.** In an elastic collision with a nucleus of mass ratio $A$ (target
at rest), the neutron leaves with an energy $E'$ between $\alpha E$ and $E$:

$$\alpha = \left(\frac{A-1}{A+1}\right)^2$$

Where it lands in that range depends on the scattering angle in the
**centre-of-mass** frame, $\mu_{cm} = \cos\theta_{cm}$:

$$\frac{E'}{E} = \frac{(1+\alpha) + (1-\alpha)\,\mu_{cm}}{2}$$

so a head-on collision ($\mu_{cm} = -1$) gives $\alpha E$ and a grazing one
($\mu_{cm} = +1$) gives $E$. If $\mu_{cm}$ is uniform (isotropic in the centre
of mass), $E'/E$ is uniform on $[\alpha, 1]$.

| nuclide | AWR (tape) | $\alpha$ | most energy lost in one collision |
|---|---|---|---|
| H-1 | 0.9991673 | 1.7 × 10⁻⁷ | all of it |
| C-12 | 11.89365 | 0.7138 | 28.6 % |
| U-238 | 236.0058 | 0.9832 | 1.7 % |

*AWR from each ENDF/B-VIII.0 tape, the same table as rung 4's, computed
2026-10-04.*

<div class="mcw" data-mc-widget="collision" data-target="1"></div>

*Illustration (JavaScript's own random numbers). Left: velocity space. The
neutron arrives with velocity 1 along the grey line; after the collision its
velocity ends on the circle, centred on the centre-of-mass velocity. Right: the
lab energy bar $[\alpha E, E]$ and the histogram of $E'/E$, flat for isotropic
scattering. Try the two $\mu_{cm}$ buttons, and switch the target.*

**The code walk.** The transport loop (`transport_history_vr` in
`transport_csg.rs`, the same kernel LCT-008 runs) does not call a function
named "elastic scatter" for carbon. It samples the centre-of-mass cosine
inline, from the nuclide's ENDF angular law (MF=4) where the evaluation gives
one and isotropically ($\mu_{cm} = 2\xi - 1$) where it does not, then calls the
scattering kernel:

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/src/physics/transport_csg.rs::transport_history_vr to=crates/outram-mc-libs/src/physics/scatter.rs::free_gas_elastic_scatter_dbrc depth=3 -->
<!-- /code-walk -->

</div>

which, above the free-gas threshold, holds the target at rest and lands in
the two-body kinematics:

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/src/physics/scatter.rs::free_gas_elastic_scatter_dbrc to=crates/outram-mc-libs/src/physics/scatter.rs::cm_to_lab depth=3 -->
<!-- /code-walk -->

</div>

```rust,ignore
{{#include ../../../src/physics/scatter.rs:cm_to_lab}}
```

([`scatter.rs`, `cm_to_lab`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/scatter.rs#@@L:crates/outram-mc-libs/src/physics/scatter.rs:anchor=cm_to_lab@@).)
With `e_cm_out` $= E A^2/(A+1)^2$ for elastic scattering, `e_out` is
$E (A^2 + 2A\mu_{cm} + 1)/(A+1)^2$: set `mu_cm` to −1 and it is exactly
$\alpha E$.

*Two footnotes.* Real carbon is not isotropic in the centre of mass at MeV
energies: the code uses the evaluation's MF=4 law, which is why the walk
passes through `sample_elastic_mu_cm`. And "target at rest" fails near
thermal energies, where the carbon atom's own motion matters: that is step 4.

<div class="predict">

**Predict.** About how many collisions with carbon does it take to bring a
2 MeV neutron down to thermal (0.025 eV): ten, a hundred, or a thousand?

</div>

## 3. Lethargy: counting the collisions

**Answer.** Energy is the wrong scale for slowing down: each collision removes
a *fraction* of the energy, not an amount. So measure progress in
**lethargy**, $u = \ln(E_0/E)$. Each collision then adds, on average, the
same amount of lethargy, whatever the energy:

$$\xi = \langle \ln(E/E') \rangle = 1 + \frac{\alpha \ln \alpha}{1-\alpha}$$

and the number of collisions to go from $E_0$ to $E$ is about

$$n \approx \frac{\ln(E_0/E)}{\xi}$$

From 2 MeV to 0.025 eV, $\ln(E_0/E) = 18.2$:

| nuclide | $\xi$ | collisions, 2 MeV → 0.025 eV |
|---|---|---|
| H-1 | 0.9999973 | 18.2 |
| C-12 | 0.1591 | **114** |
| U-238 | 0.00845 | 2153 |

<div class="mcw" data-mc-widget="slowdown" data-target="1"></div>

*Illustration (target at rest, isotropic). One neutron's energy against
collision number, on a log scale: a ragged staircase around the straight
dashed line of slope $\xi$. "1000 neutrons" gives the spread of the count.*

**There is no function in the code that computes $\xi$, lethargy, or "114
collisions".** They *emerge* from the transport loop repeating one collision.
What the code has is a **fork**, taken afresh at every scattering collision,
and it is the subject of the next step:

```rust,ignore
{{#include ../../../src/physics/transport_csg.rs:scatter_fork}}
```

([`transport_csg.rs`, the scatter fork](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#@@L:crates/outram-mc-libs/src/physics/transport_csg.rs:anchor=scatter_fork@@).)

**Measured check (verification).** `examples/epithermal_slowing_down.rs`
samples the transport's own collision kernel 200 000 times per energy and
compares $\xi$ and $\langle E'/E\rangle$ with the formulas above. Re-measured
2026-10-04 on `develop` at `bfeb81a083` (the record is in that example's doc
comment): 104 rows, eight nuclides, from 0.0253 eV to 10 keV, all inside the
envelope, worst $|\xi - \xi_0| = 1.08 \times 10^{-3}$ (Li-7 at 100 eV); for
graphite's C-12 between 6.674 eV and 10 keV, $\xi/\xi_0$ is 0.9972 to 0.9998.
Below about 5 eV the bound-atom law takes over and the formula no longer
applies.

<div class="predict">

**Predict.** If every collision holds the carbon atom still, a neutron can
only lose energy. After 400 collisions in graphite at 600 K, where does it
end up: at about $kT$ (0.05 eV), at a millionth of that, or at essentially
zero?

</div>

## 4. The bottom of the slope: the carbon atom moves

**Answer.** Near thermal energies the neutron is no faster than the atoms it
hits. A carbon atom at room temperature jiggles with an energy of order $kT$
(0.026 eV at 296 K), so a collision can **give** the neutron energy as well as
take it. The neutron population ends in balance with the moderator, a
Maxwellian at the moderator's temperature. Getting this wrong is not a small
error.

**4a. The bug this code once had.** Until bead `op-50vu` (September 2026),
the transport held every target at rest at every energy. Each collision on its
own was valid, but the sequence has **no equilibrium**: a neutron that can
only lose energy cools without limit.

**Measured, re-run for this page** (2026-10-05, `epithermal_slowing_down.rs`
with its explicit ablation `TARGET_AT_REST=1`, which puts the old kernel
back; 20 000 neutrons × 400 collisions from 1 eV at 600 K, where the correct
equilibrium of this walk is $\langle E\rangle = 2kT = 0.103$ eV):

| medium | $\langle E\rangle$ after 400 collisions |
|---|---|
| carbon, target held at rest | **1.5 × 10⁻²⁷ eV** |
| oxygen-16, target held at rest | 2.9 × 10⁻²¹ eV |
| carbon in graphite, S(α,β) | 0.102 eV ($\langle E\rangle/2kT = 0.989$) |

Twenty-six decades too cold. Each collision was right; the physics that was
missing was the target's own motion, and only a whole history shows it. With
the fix (below), free-gas carbon in the same walk ends at
$\langle E\rangle/2kT = 0.981$ (re-measured 2026-10-04, the example's doc
comment).

**4b. Free gas.** The fix samples the target nucleus's thermal velocity from
a Maxwellian at the material temperature, below $400\,kT$ (OpenMC's
`FREE_GAS_THRESHOLD`), and does the collision in the frame where the target
was moving. For U-238 the same function applies **DBRC** (Doppler-broadened
rejection), which samples the target velocity weighted by the resonance's own
shape so that a neutron near a resonance scatters correctly.

**4c. Graphite is not a gas: S(α,β).** Carbon in graphite is bound in a
crystal. It cannot recoil freely; it exchanges energy with the lattice in
**phonons**, and at low energy the crystal planes diffract neutrons
coherently (**Bragg edges**: sudden steps in the elastic cross section). The
evaluated **thermal scattering law** $S(\alpha,\beta)$ describes all of this,
and the code samples it below its cutoff (a few eV). The law is processed
from `tsl-crystalline-graphite.endf` by the workspace's THERMR port.

**The fork, in the order the code tests it:**

1. **S(α,β):** the nuclide carries a thermal law and $E$ is below its cutoff →
   `Nuclide::sample_thermal`, lab-frame energy and angle from the bound-atom
   law;
2. **free gas:** otherwise, below $400\,kT$ (and inside a resonant nuclide's
   DBRC window) → the target's motion is sampled;
3. **target at rest:** above that → step 2's two-body kinematics.

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/src/physics/transport_csg.rs::transport_history_vr to=crates/outram-mc-libs/src/material/nuclide.rs::Nuclide::sample_thermal depth=3 -->
<!-- /code-walk -->

</div>

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/src/material/nuclide.rs::Nuclide::sample_thermal to=crates/outram-mc-libs/src/material/thermal.rs::ThermalScattering::sample depth=3 -->
<!-- /code-walk -->

</div>

The demo labels nothing by branch yet; in this graphite mixture every
collision below a few eV takes branch 1, carbon above it branch 3, and U-238
near its resonances branch 2.

**Measured checks (verification against NJOY2016's THERMR,** re-measured
2026-10-04 on `develop` at `bfeb81a083`; the records are in the examples'
doc comments**):**

- graphite cross sections, `graphite_vs_njoy_thermr.rs`: incoherent inelastic
  worst **+0.060 %** (at 1 meV), coherent elastic worst **−0.085 %** (at
  3.0 eV), and exactly zero below the first Bragg edge, as it must be;
- the sampled outgoing energy, `graphite_kernel_vs_njoy_thermr.rs`:
  $\langle E'\rangle/E$ within **+0.30 %** at 0.01 eV (inside its 1σ of
  0.35 %) and closer above;
- the sampled angle, `graphite_sab_angle_and_width_vs_njoy_thermr.rs`:
  $\bar\mu$ worst **−0.0040** (inelastic at 5 meV); the outgoing-energy
  **width** is narrower than NJOY's at every energy, worst **−1.14 %** at
  2.6 meV. That is a known, one-signed, open defect of the tabulation, kept
  inside a 4 % envelope.
- the thermal equilibrium of the walk with S(α,β) in graphite at 600 K,
  `epithermal_slowing_down.rs`: $\langle E\rangle / 2kT = 0.9889$ after 400
  collisions (the fixed point of a walk over collisions is $2kT$).

**What is S(α,β) worth in $k$?** A run of the main case with the graphite law switched off (the example's explicit ablation `GRAPHITE_SAB=0`, carbon as free gas) answers it. *Running on 2026-10-05; the measured result is recorded here in the next update of this page.*

<div class="predict">

**Predict.** U-238's biggest resonance, at 6.67 eV, has a capture cross
section of thousands of barns at its peak. If you double the amount of U-238
in the mixture, does resonance capture double?

</div>

## 5. Resonances, and the probability of escaping them

**Answer.** On its way down, a neutron's energy steps down by a random
fraction each collision, so it can land on a resonance peak. The probability
of getting past all of them is the **resonance escape probability** $p$.
With little U-238 (very dilute), the capture adds up over the resonances as
the **resonance integral**,

$$RI_\infty = \int \sigma_\gamma(E)\,\frac{dE}{E}$$

and with $N_{238}$ U-238 atoms per cm³ among moderator atoms that scatter
with $\Sigma_s$,

$$p \approx \exp\left(-\frac{N_{238}\,RI_{\text{eff}}}{\xi \Sigma_s}\right)$$

Why $RI_{\text{eff}}$ and not $RI_\infty$: when there is a lot of U-238, the
neutrons at a resonance's peak energy are absorbed so quickly that the flux
there **dips** (a notch in the flux spectrum). Fewer neutrons are there to be
captured than the dilute formula assumes. That is **self-shielding in
energy**, and it is why doubling the U-238 does not double the capture.

**There is no function that computes $p$.** It emerges, like $\xi$. Each
collision picks a nuclide in proportion to its $N\sigma_t$ at the neutron's
energy, then a reaction in proportion to that nuclide's cross sections; at a
resonance U-238's capture dominates both choices. In U-238's **unresolved**
resonance range (above about 20 keV, where the resonances are too dense to
measure one by one) the cross sections come from **probability tables** (URR),
sampled once per neutron per energy:

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/src/physics/transport_csg.rs::transport_history_vr to=crates/outram-mc-libs/src/material/material.rs::Material::sample_nuclide_urr depth=3 -->
<!-- /code-walk -->

</div>

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/src/physics/transport_csg.rs::transport_history_vr to=crates/outram-mc-libs/src/material/nuclide.rs::Nuclide::xs_at_energy_urr depth=3 -->
<!-- /code-walk -->

</div>

`xs_at_energy_urr` falls through to `xs_at_energy` outside the unresolved
range, and `xs_at_energy` reads the resonances RECONR reconstructed (the
nuclear data track's
[RECONR rung](../../deep-dives/nuclear-data/reconr.html)).

**Measured check (verification against an analytic limit).**
`examples/u238_resonance_escape.rs` follows neutrons in energy only (no
geometry) through U-238 in carbon, with the transport's own collision physics,
and turns the absorbed fraction into $RI_{\text{eff}}$. Re-measured 2026-10-04
(record in that example's doc comment), at this lesson's 296 K, with DBRC and
URR on (the transport's defaults), as a fraction of $RI_\infty = 274.637$ b
(from our own data, checked against NJOY by `u238_resonance_integral.rs`):

| carbon per U-238, as $\sigma_0$ (b) | $RI_{\text{eff}} / RI_\infty$ |
|---|---|
| 379 580 (trace U-238) | 0.982 ± 0.011 |
| 37 958 | 0.903 |
| 3 796 | 0.524 |
| 380 | 0.188 |

At trace U-238 it is 1 within 1.7σ: the dilute limit. As the U-238 grows,
self-shielding cuts the effective integral to a fifth. (The commonly quoted
experimental value of the resonance integral is **not** used here: no source
for it was found, see `u238_resonance_integral.rs`.)

<div class="predict">

**Predict.** For every 100 neutrons born in the main-case mixture (17 wt%
U-235), how many get past the resonances to thermal energies: 50, 70 or 95?
And for every thermal neutron absorbed, how many new neutrons come out?

</div>

## 6. The four-factor formula: a neutron's life, in four ratios

**Answer.** Follow one generation in an infinite medium (nothing leaks).
Split the life of a neutron at two energies (100 keV and the cadmium cutoff
0.625 eV) and count:

$$k_\infty = \eta \, f \, p \, \varepsilon$$

- $\varepsilon$, **fast fission factor**: all fission neutrons divided by
  those from thermal fissions. The bonus Godiva set up: a fast neutron
  occasionally fissions before it slows down.
- $p$, **resonance escape probability**: the fraction that reaches thermal
  energies instead of being absorbed on the way.
- $f$, **thermal utilisation**: the fraction of thermal absorptions that
  happen in uranium rather than in carbon.
- $\eta$: neutrons produced per thermal absorption in uranium.

**The code** gets all four from one run's **track-length** tallies (the flux
summed over every flight, times the cross section: a different estimator from
rung 1's counting, and why it is used is a later rung's question), absorption
and production in three energy groups, then:

```rust,ignore
{{#include ../../../src/physics/reactor_physics.rs:six_factors}}
```

([`reactor_physics.rs`, `assemble_six_factors`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#@@L:crates/outram-mc-libs/src/physics/reactor_physics.rs:anchor=six_factors@@).)
With nothing leaking, $P_{FNL} = P_{TNL} = 1$ and the six factors are the
four. The product **telescopes**: every intermediate count cancels, so
$\eta f p \varepsilon$ is exactly total production over total absorption,
whatever the energy boundaries. The factors depend on the convention (where
you split the energy range, and what you call "fuel"); $k$ does not.

**One subtlety the homogeneous mixture forces.** The library calls a
*material* "fuel" when it holds uranium. Here the only material holds
uranium, so $f$ would be exactly 1. The example therefore splits the thermal
absorption by **nuclide** from the run's own fine-group flux, and hands the
library that split (its module docs say how, and print a binning check).

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/examples/ugraphite_four_factor.rs::main to=crates/outram-mc-libs/src/physics/transport_csg.rs::run_keff_csg_reactor_physics depth=6 -->
<!-- /code-walk -->

</div>

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/examples/ugraphite_four_factor.rs::main to=crates/outram-mc-libs/src/physics/reactor_physics.rs::assemble_six_factors depth=6 -->
<!-- /code-walk -->

</div>

**The result to quote** (main case, 17 wt%, $N_C/N_U = 767.2$, 296 K,
recorded 2026-10-04 at `c199b7dc1f` in the doc comment of
[`ugraphite_four_factor.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#@@L:crates/outram-mc-libs/examples/ugraphite_four_factor.rs:text=Main+case+(2026-10-04@@)
and in `verification_and_validation/tutorial_rung2/README.md`; 20 000
neutrons × [20 + 100] generations):

| | value |
|---|---|
| $k_\infty$ (power iteration) | **1.56777 ± 0.00081** |
| $\eta$ | 2.02809 ± 0.00132 |
| $f$ | 0.97470 ± 0.00063 |
| $p$ | 0.71398 ± 0.00044 |
| $\varepsilon$ | 1.11144 ± 0.00093 |
| $\eta f p \varepsilon$ | 1.56867 ± 0.00256 (= production / absorption to $2 \times 10^{-16}$) |

- **The prediction held.** Written before the run: $k_\infty \approx 1.4$–1.6,
  with $\eta \approx 2.0$, $f \approx 0.98$, $p \approx 0.70$,
  $\varepsilon \approx 1.0$–1.1. Measured 1.568, each factor inside its range.
- **$p$ against the energy-only walk of step 5** at the same composition:
  0.71274 ± 0.00072, against the transport's 0.71398 ± 0.00044 (1.5σ). They are
  not quite the same quantity (the transport's resonance group also holds the
  ~1 % of fission neutrons born below 100 keV), so agreement at this level is
  what is expected.
- **Against another code (verification, not validation; no experiment exists
  for a homogeneous mixture):** OpenMC 0.16.1-dev25 on the openmc.org
  ENDF/B-VIII.0 library, same mixture and histories: $k$ 1.56711 ± 0.00134,
  a difference of **+66 ± 157 pcm (0.4σ)**, and every factor within 0.15 %.
  The deck is committed:
  `verification_and_validation/tutorial_rung2/openmc_inputs/ugraphite_openmc.py`.
- The ± on $k$ is the standard error over the 100 active generations. Why
  that can be trusted, and when it cannot, is a later rung.

**Even at 17 % enrichment, 29 % of the neutrons are lost to the resonances**
($p = 0.714$). With natural uranium there is 24 times less U-235 per U-238.

<div class="predict">

**Predict, before reading on.** Natural uranium, mixed evenly into graphite.
Can *any* ratio of carbon to uranium reach $k_\infty = 1$? If you add more
carbon, which factor improves, and which gets worse?

</div>

## 7. Why nobody built it this way

**The expectation, written down before the run** (2026-10-04, in the
example's doc comment, commit `c199b7dc1`, not edited since): homogeneous
natural uranium in graphite **never reaches $k_\infty = 1$ at any ratio**,
because U-238 resonance capture is too strong. With little carbon, neutrons
reach the resonances with too few collisions in between and $p$ is small;
with a lot of carbon, carbon's own capture takes the thermal neutrons and $f$
falls. So $k_\infty$ rises, peaks and falls. The hand estimate put the
**maximum at about 0.75–0.8, near $N_C/N_U \approx 500$–800**.

*Running on 2026-10-05; the measured result is recorded here in the next update of this page.*

**So the graphite does two jobs that pull against each other** in a
homogeneous mixture: more of it slows the neutrons past the resonances
(higher $p$), and more of it absorbs them once they are slow (lower $f$). The
way out is not a better ratio. It is to **stop mixing**: gather the uranium
into lumps, so that the U-238 shields itself in *space*. That is the
[next rung](lumped.md).

**Run it yourself.** On your own machine, from a clone of the repository:

```text
cargo run --release -p outram-mc-libs --features endf-pebble-cases --example ugraphite_four_factor
MODE=sweep cargo run --release -p outram-mc-libs --features endf-pebble-cases --example ugraphite_four_factor
```

The first runs the main case and the $p$ check; the second the natural-uranium
sweep (`RATIOS=300,600,1000` picks your own points; `PARTICLES`, `INACTIVE`,
`ACTIVE`, `THREADS` set the size).

**Modify.** `CU=400` runs the main case's 17 wt% uranium at another ratio.
Predict first: does $k_\infty$ go up or down, and which factor moves most?

**Create.** Find the enrichment at which a homogeneous mixture at
$N_C/N_U = 600$ is just critical ($k_\infty = 1$). You need a `Mix` with your
own U-235 fraction (copy `natural_mix` in `vv::ugraphite`) and a bisection on
it. How would you put an error bar on that enrichment?

## The whole call tree

Everything `ugraphite_four_factor.rs`'s `main` reaches inside the workspace,
three calls deep, as an architecture map. Generated by `kovan-cli code-walk`
(std and dependency calls left out; each function expanded once).

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/examples/ugraphite_four_factor.rs::main depth=3 -->
<!-- /code-walk -->

</div>

In the demo, the browser's worker reaches the same transport through
[`ugraphite::sim::Chain::run_next`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/dhoby-ghaut/examples/monte_carlo_web/ugraphite/sim.rs#@@L:crates/dhoby-ghaut/examples/monte_carlo_web/ugraphite/sim.rs:fn=run_next@@)
(`run_fixed_source_traced`, one neutron per call), called from
[`Loaded::serve`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/dhoby-ghaut/examples/monte_carlo_web/engine.rs#@@L:crates/dhoby-ghaut/examples/monte_carlo_web/engine.rs:fn=serve@@)
when the page asks for the next neutron *(filled by hand: page and worker
talk by messages, not calls)*.

---

**Deliberate liberties.** The main case is one HTR-10 pebble's uranium and
carbon only: the oxygen of the UO₂, the silicon of the SiC and the boron
impurities are left out (this rung is uranium and graphite, by design), the
gaps between pebbles are not modelled, and the TRISO particles are smeared,
which is exactly the heterogeneity rungs 3 and 5 put back. None of these is
measured here. Natural uranium is the IUPAC composition, recalled and not
page-checked (`vv::ugraphite::NAT_U`). Everything is at 296 K, the lowest
tabulated temperature of the graphite law. The demo's Watch mode processes
the data at the loosened tolerance 0.01 (NJOY's is 0.001).

**Literature.** ENDF/B-VIII.0 (Brown et al., *Nuclear Data Sheets* 148, 2018)
for every cross section and the graphite thermal scattering law; the HTR-10
pebble specification as cited in `pebble_beds::htr10` (Li, Yu & Wei 2014,
Table 2; IAEA-TECDOC-1382, Table 4-2, private corpus, cited by page only).

**Doesn't tally?** If anything here disagrees with the code it links to, the
page is wrong:
[report it](https://github.com/theodoreOnzGit/outram-park-backend/issues/new?title=Uranium-in-graphite%20lesson%20doesn%27t%20tally%3A%20&labels=bug).
This page changes whenever `develop` does; it was built from
`@@COMMIT_SHORT@@` on @@BUILD_DATE@@. Tracking issue
[#524](https://github.com/theodoreOnzGit/outram-park-backend/issues/524).
