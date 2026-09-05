<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

# Interface transmission rule — uniform equilibrium density

**Generated:** 2026-07-23T00:28:53Z
**Crate version / commit:** boon-lay 0.1.2, working tree atop `ae84a04` (Phase 4)

## Methodology

**What is computed, and why.** The Walk-on-Spheres engine crosses a material
interface by transmitting or reflecting a walker with a probability that must
make the scheme reproduce Fickian diffusion. The physically correct equilibrium
for continuity of concentration and flux (partition ratio $K = 1$) is a
**uniform concentration**: with zero net flux the profile is flat, independent of
the diffusivity contrast. This test checks that the implemented transmission
rule reproduces that uniform equilibrium — the property that *fixes* the rule.

**The rule under test.** In Walk-on-Spheres the walker is reinserted a fixed
distance from an interface and then takes a hop of duration $\tau \sim R^2/D$, so
the rate at which it encounters the interface from side $i$ scales as $D_i$.
Detailed balance for a uniform equilibrium then requires the **linear** rule

$$p_{\text{transmit}} = \frac{K\,D_2}{D_1 + K\,D_2},$$

*not* the $\sqrt{D}$-ratio rule appropriate to a fixed-timestep walk (whose
step length scales as $\sqrt{D}$ and encounter rate as $1/\sqrt{D}$). Using the
$\sqrt{D}$ rule in this scheme would give a non-uniform (wrong) equilibrium.

**Setup.** A single ergodic walker in a two-region reflecting sphere: inner
region $[0, a)$ with $D_{\text{in}}$, outer region $[a, b)$ with $D_{\text{out}}$,
a transmitting interface at $a$, and a reflecting wall at $b$. With
$a = 50\ \mu\text{m}$, $b = 100\ \mu\text{m}$ the inner-region **volume**
fraction is $(a/b)^3 = 0.125$. A diffusivity contrast of $10\times$
($D_{\text{in}} = 10^{-8}$, $D_{\text{out}} = 10^{-9}\ \text{m}^2/\text{s}$) is
imposed, $K = 1$, interface capture $\varepsilon = 20\ \text{nm}$, reinsertion at
$3\varepsilon$. The walker is stepped $3\times10^6$ times; the first-passage time
of every hop is accumulated into a per-region time tally. At a uniform
equilibrium the **time fraction** spent in a region equals its **volume**
fraction.

**Pass criterion.** $|f_{\text{in}} - 0.125| < 0.02$, where $f_{\text{in}}$ is
the measured inner-region time fraction. (Implemented as the
`interface_rule_gives_uniform_equilibrium_density` test in
`first_passage/walk_on_spheres.rs`.)

## Reference

The reference is the analytical statement that Fickian diffusion with
continuity of concentration and flux has a uniform equilibrium concentration
(flat profile at zero net flux) regardless of the piecewise diffusivity. The
scheme-dependence of the interface transmission probability — linear-in-$D$ for
first-passage/Green's-function walks versus $\sqrt{D}$ for fixed-step walks —
follows from the detailed-balance argument in the module documentation of
`first_passage/interface.rs`.

```bibtex
@article{ovaskainen2003biased,
  title   = {Biased movement at a boundary and conditional occupancy times for diffusion processes},
  author  = {Ovaskainen, Otso and Cornell, Stephen J.},
  journal = {Journal of Applied Probability},
  volume  = {40},
  number  = {3},
  pages   = {557--580},
  year    = {2003},
  note    = {Boundary/interface rules and their equilibria for diffusion}
}
```

## Results

Measured on the generation date above (`cargo test -p boon-lay --release
interface_rule_gives_uniform_equilibrium_density -- --nocapture`), seed
`0x13579BDF2468ACE0`, $3\times10^6$ steps:

```csv
quantity,value
inner_volume_fraction,0.1250
measured_inner_time_fraction,0.1216
absolute_error,0.0034
diffusivity_contrast,10x
pass_threshold,0.02
```

**Interpretation.** With the linear transmission rule the walker's time-in-region
matches the volume fraction to $3.4\times10^{-3}$ across a tenfold diffusivity
contrast — i.e. the equilibrium is uniform, as Fickian continuity requires, and
comfortably inside the $0.02$ criterion. The small residual (a slight
under-population of the fast inner region) is consistent with the finite
$\varepsilon$ interface-capture discretisation and Monte-Carlo noise. This
verifies the interface treatment that turns the SiC layer (whose $D$ is ~$10^6$
times smaller than the surrounding pyrolytic carbon) into the TRISO containment
barrier. Had the $\sqrt{D}$ rule been used, the predicted inner time fraction
would have been biased well outside the threshold.

**Scope / limitations.** Verified for $K = 1$ (concentration continuity) at a
single $10\times$ contrast in a two-region geometry. The full five-layer TRISO
release with the extreme PyC/SiC contrast is heavier and is exercised
qualitatively by the multilayer-transmission test; a quantitative multilayer
release record is future work. Partition ratios $K \ne 1$ are supported by the
rule and unit-tested for the transmission probability, but their equilibrium
$c_2/c_1 = K$ is not separately measured here.
