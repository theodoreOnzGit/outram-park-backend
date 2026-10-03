# Collisions, fission and delayed neutrons

## The problem

The neutron has arrived at a collision site. Three questions follow, in order:
**which nuclide** did it hit, **what happened** (scatter, capture, fission, …), and
if it was a fission, **how many** new neutrons are born, **where**, and **at what
energy**? Each answer is a sample from a distribution the nuclear data supplies.

## Which nuclide, at which temperature

[`transport_csg.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1680-L1708)
samples the target nuclide in proportion to its share of the material's total
cross section, then looks up that nuclide's cross sections **at the collision
material's own temperature**. The comment there explains the defect this
replaced: the flight was sampled at each material's temperature but the collision
was partitioned at one run-wide temperature. That was invisible while every
material sat at one temperature, and wrong as soon as they differed, which is
exactly what a temperature-coefficient study needs.

In the unresolved-resonance range the same probability-table band is used for
the flight, the nuclide choice and the reaction (GitHub #407, OpenMC's
`calculate_urr_xs`), so the three are consistent with one another.

## What happened: one uniform, one ladder

The reaction is chosen with one uniform `ξ·Σ_t` and a ladder of cumulative
partial cross sections: fission, then capture, inelastic, (n,2n), (n,3n), other
channels, and elastic last
([`transport_csg.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1709-L1830)).

Survival biasing reuses the **same** ladder. It banks fission sites from the
*expected* production, reduces the weight by the absorption probability, then
offsets the draw by `Σ_a` so the fission and capture rungs simply cannot be
reached. Reusing one ladder rather than writing a second one with different
bounds avoids a second copy that could drift. Variance reduction is the subject of
the next chapter.

Some of the physics on the rungs, each with its own issue history:

- **Inelastic level scattering** uses the level's own ENDF MF=4 angular
  distribution and falls back to isotropic only when the evaluation has none. The
  comment records why: sampling every inelastic collision isotropically
  understates the mean cosine, which suppresses leakage and raises `k`.
- **(n,2n)** emits two neutrons drawn **independently** from the evaluated law.
  Copying the first neutron's outgoing state, which is what it used to do,
  correlates the pair perfectly (GitHub #192).

## How many fission neutrons

The expected number banked is `ν̄/k`. Dividing by the running eigenvalue keeps
the bank size steady from generation to generation. The integer is a floor plus a
Bernoulli draw on the remainder
([`fission.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/fission.rs#L32-L56)):

```rust,ignore
{{#include ../../../src/physics/fission.rs:45:56}}
```

Under weight windows each banked neutron carries the parent's weight: the call
passes `w·ν̄`, as OpenMC's `create_fission_sites` does (GitHub #461). For an
analog run `w` is exactly 1, so that change was bit-identical there.

## At what energy: prompt and delayed spectra

A fission neutron's direction is isotropic. Its energy is drawn from the
nuclide's **own** fission spectrum χ (ENDF MF=5), not a generic Watt shape. The
Watt function survives only as a stand-in where a nuclide carries no law, and to
seed the *initial* source
([`fission.rs` module doc](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/fission.rs#L1-L30)).

### Case study: delayed neutrons born with the prompt spectrum (GitHub #365)

Roughly 0.65 % of thermal U-235 fission neutrons (the textbook β) are *delayed*. They are emitted by
precursor decay, with a much softer spectrum: around 0.5 MeV against around
2 MeV prompt. For an eigenvalue calculation it is a standard approximation to
count them in ν̄ and birth them at the same instant. But until the #365 audit
the crate also birthed them **with the prompt χ**, on both the ENDF and ACE
routes, even though the data to do better was already parsed.

The fix is a port of OpenMC's `sample_fission_neutron`
([`Nuclide::sample_fission_energy_below`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L3738-L3807)):
with probability `β = ν_d(E)/ν_t(E)` the neutron is delayed; its precursor group
is drawn by yield; its energy comes from that group's spectrum.

```rust,ignore
{{#include ../../../src/material/nuclide.rs:3775:3807}}
```

It is on by default; `Nuclide::without_delayed_spectra` is the ablation arm.

**How it is verified**
([`tests/delayed_spectra_vs_openmc.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/tests/delayed_spectra_vs_openmc.rs#L1-L51)):
the reference is the exact mean birth energy under the mixture, computed from
OpenMC's own readers. With two million draws per case, the sampled mean must sit
within 5σ of the mixture reference, the *ablated* arm within 5σ of the prompt-only
reference, and the two references must be more than 5σ apart, **so the test is
capable of failing**. The test's own doc records how its first version got that
last condition wrong and was corrected: a power criterion was being judged on
noisy data.

## A note on the doc comments themselves

The module doc of `fission.rs` carries two strike-throughs dated 2026-10-03. It
had said fission-site energies came from the Watt sampler and that delayed
neutrons were "treated as prompt". Both were true once, and the code had since
moved on. They are corrected in place, with the old text struck rather than
deleted, so a reader can see what changed. This book follows the same rule.
