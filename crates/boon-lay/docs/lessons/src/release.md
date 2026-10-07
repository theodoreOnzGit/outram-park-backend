# Getting out: diffusion and release

**The problem.** Even an intact particle is not a perfect seal. Cesium,
strontium and silver diffuse through the kernel and coatings, and they do it
faster as temperature rises. A failed particle releases much more. For each
nuclide we want the fraction that leaves the fuel, and we can get it two
ways.

## The continuum route: TRISO-ATOPS

### Sort nuclides by how they move

TRISO-ATOPS does not treat every element alike. It sorts each element into
one of five transport groups by atomic number
([`ElementGroup`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/triso_atops_fork/nuclide_model/mod.rs#@@L:crates/boon-lay/src/triso_atops_fork/nuclide_model/mod.rs:enum=ElementGroup@@)):

| Group | Elements | Release model |
|---|---|---|
| Noble gas | He, Ne, Ar, Kr, Xe, Rn | empirical `<R/B>` correlation |
| Halogen (TRISO-ATOPS's grouping) | F, Cl, Br, I, At, **plus Se, Te** | same correlation; these plate out |
| Special metal | Rb, Sr, Cs, Ba, Eu | Booth equivalent sphere |
| Silver | Ag, Pd | breakthrough through the SiC |
| Other | everything else | fixed `<R/B> = 1e-5` |

The dispatcher
[`rb_fail`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/triso_atops_fork/release_models/mod.rs#@@L:crates/boon-lay/src/triso_atops_fork/release_models/mod.rs:fn=rb_fail@@)
picks the model by group.

### The Booth equivalent sphere

Booth's idea (1957) is to replace the real multi-shell particle, for one
chemical group, with a single uniform sphere of radius `a`. Everything then
depends on `D` and `a` only through `D' = D/a²`. `D` itself follows an
Arrhenius law, `D(T) = D₀·exp(−Q/RT)`
([`diffusion_coefficient`](../../api/boon_lay/triso_atops_fork/diffusion/fn.diffusion_coefficient.html)).

For a **long-lived** species with a uniform start and a perfect-sink surface,
the released fraction is Crank's series
([`booth_longlived`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs#@@L:crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs:fn=booth_longlived@@)):

```rust,ignore
{{#include ../../../src/triso_atops_fork/release_models/steady_state.rs:180:197}}
```

For a **short-lived** species, decay competes with diffusion, and the release
settles to a steady release-to-birth ratio
`<R/B> = (3/μ)(coth μ − 1/μ)`, with `μ = √(λa²/D)`
([`booth_shortlived_fast_diffuse`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs#@@L:crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs:fn=booth_shortlived_fast_diffuse@@)).
When `μ` is small (diffusion fast or decay slow), everything born gets out and
`<R/B> → 1`. When `μ` is large, `<R/B> → 3/μ`.

> **Predict.** Halve the half-life of a species at fixed `D` and `a`. Does
> `<R/B>` go up or down, and by roughly what factor when `μ` is large?

For an **accident**, the products `D·t` and `D'·t` become time integrals
`∫D dt` and `∫D' dt` over the temperature history
([`integrate_diffusion_over_time`](../../api/boon_lay/triso_atops_fork/diffusion/fn.integrate_diffusion_over_time.html)),
and the same series are reused in
[`release_models::transient`](../../api/boon_lay/triso_atops_fork/release_models/transient/index.html).

### A truncation floor inherited from upstream

The series stops after a fixed number of terms (`BOOTH_SERIES_TERMS = 5000`,
upstream's `num_terms`;
[source](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs#@@L:crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs:const=BOOTH_SERIES_TERMS@@)).
At very small `D't` the cut-off series does not go to zero. It floors at
about `1.216·10⁻⁴`. That over-states Sr, Ba and Eu kernel release by up to
120× at HTR-10 normal-operation temperatures (gh:#385). The port keeps
upstream's behaviour, because it is a port and is verified code to code; the
floor is measured and pinned in
[`tests/triso_atops_booth_vs_crp6.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/tests/triso_atops_booth_vs_crp6.rs).

## The particle route: one atom at a time

The Lagrangian side follows individual atoms. Each one does a random walk
through the layers, using the `D` of whichever layer it is in.

### Case study: why the first walk was wrong

The first version moved each atom by a Gaussian step with
`σ = √(2DΔt)`. That is the exact propagator in an **unbounded** medium,
by the central limit theorem. A TRISO particle is a set of thin shells. In the
buffer, `D ≈ 10⁻⁸ m²/s`, so with `Δt = 1 s`, `σ ≈ 141 µm`. The buffer is only
100 µm thick. At `Δt = 10 s`, `σ ≈ 447 µm`, which is larger than the whole
particle. One step could carry an atom straight across the buffer and the
IPyC and onto or through the SiC, without ever meeting the interface that
should have reflected it. Shrinking `Δt` fixes it, but then an atom in the
buffer needs hundreds of sub-steps per second of simulated time. The full
diagnosis, with the numbers, is in
[`docs/buffer_clt_failure_analysis.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/docs/buffer_clt_failure_analysis.md).

The lesson generalises: **the central limit theorem gives the free-space
answer, and a bounded domain is not free space.**

### Walk-on-Spheres: exact hops that never cross an interface

The replacement does not step in time. At each move it:

1. finds the distance `R` to the nearest interface;
2. jumps to a uniformly random point on the sphere of radius `R` around the
   atom (inside that sphere the medium is uniform, so this is exact);
3. adds a first-passage **time** drawn from the exact exit-time distribution
   for that sphere, with mean `R²/(6D)`
   ([`sample_first_passage_time`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/lagrangian_decay_simulator/lagrangian_diffusion/first_passage/sphere_fpt.rs#@@L:crates/boon-lay/src/lagrangian_decay_simulator/lagrangian_diffusion/first_passage/sphere_fpt.rs:fn=sample_first_passage_time@@)).

Hops are large in the bulk and shrink as the atom nears an interface. A hop
touches the nearest interface but never crosses it, so the buffer overshoot
cannot happen. The single-region version, used for the CRP-6 benchmark, is
short enough to read whole
([`walk_to_absorbing_sphere`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/lagrangian_decay_simulator/lagrangian_diffusion/first_passage/walk_on_spheres.rs#@@L:crates/boon-lay/src/lagrangian_decay_simulator/lagrangian_diffusion/first_passage/walk_on_spheres.rs:fn=walk_to_absorbing_sphere@@)):

```rust,ignore
{{#include ../../../src/lagrangian_decay_simulator/lagrangian_diffusion/first_passage/walk_on_spheres.rs:217:237}}
```

### The interface rule that makes SiC a barrier

When the atom reaches an interface between `D₁` (its side) and `D₂`, it
transmits with probability
([`transmission_probability`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/lagrangian_decay_simulator/lagrangian_diffusion/first_passage/interface.rs#@@L:crates/boon-lay/src/lagrangian_decay_simulator/lagrangian_diffusion/first_passage/interface.rs:fn=transmission_probability@@))

```text
p_transmit = K·D₂ / (D₁ + K·D₂)
```

```rust,ignore
{{#include ../../../src/lagrangian_decay_simulator/lagrangian_diffusion/first_passage/interface.rs:70:83}}
```

Why linear in `D` and not `√D`? It depends on how often *this* walk visits an
interface. In Walk-on-Spheres, the encounter rate from each side scales as
that side's `D`. Detailed balance at equilibrium then gives the rule above. A
fixed-time-step walk visits at a rate that scales as `1/√D` and would need a
`√D` rule; using that one here would give the wrong equilibrium
([derivation](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/lagrangian_decay_simulator/lagrangian_diffusion/first_passage/interface.rs#L13-L32)).

~~`D` in SiC is about 10⁶ times smaller than in the pyrolytic carbon around
it.~~ **CORRECTED 2026-10-04 (gh:#531):** with the Arrhenius factors
included, `D` in SiC is 13 to 4100 times smaller than in the pyrolytic
carbon around it, by element and temperature (Cs: 120 at 1000 °C, 440 at
1600 °C), as measured in
[rung 4](../../tutorials/triso-atops/layers.html#step-1-how-fast-does-an-atom-move-in-each-layer);
the 10⁶ compared prefactors only. So an atom arriving from PyC transmits with probability of order
`D_SiC/D_PyC` and is reflected nearly every time. That is SiC's containment
role, coming out of one line of arithmetic.

### Decay while walking

An atom also decays, and under a neutron field it can transmute. The
[`depletion`](../../api/boon_lay/lagrangian_decay_simulator/lagrangian_diffusion/first_passage/depletion/index.html)
module runs these as competing exponential clocks beside each hop. If an
event falls inside a hop's time, the atom changes nuclide (and `D`) at the
hop's start. The histogram of identities over time is the depleted inventory,
with no Bateman matrix. Decay uses the real ENDF/B-VIII.0 library [(Brown & others, 2018)](#ref-brown2018endf8). The neutron
side is a framework with a single explicit `(n,γ)` channel; per-nuclide cross
sections and fission yields are not wired in yet
([scope](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/lagrangian_decay_simulator/lagrangian_diffusion/first_passage/depletion.rs#L23-L39)).

<!-- references:begin -->
## References

<p class="csl-entry" id="ref-brown2018endf8" style="padding-left: 2em; text-indent: -2em;">Brown, D. A., & others. (2018). ENDF/B-VIII.0: The 8th Major Release of the Nuclear Reaction Data Library with CIELO-project Cross Sections, New Standards and Thermal Scattering Data. <span style="font-style: italic;">Nuclear Data Sheets</span>, <span style="font-style: italic;">148</span>, 1–142. <a href="https://doi.org/10.1016/j.nds.2018.02.001">https://doi.org/10.1016/j.nds.2018.02.001</a></p>

<!-- references:end -->
