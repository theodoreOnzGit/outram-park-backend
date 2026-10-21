# Step 12 — Into the coolant: fission products, tramp uranium and dust

> **Research, education and V&V only.** Not for reactor operation, licensing, safety decisions or emergency response.
> **Review status: first draft, 2026-10-18, AI-assisted (Claude Sonnet), not yet reviewed by a human.**

**The question this page answers.** Steps 1 to 11 stayed inside the
particle. Atoms that leave it enter a flowing coolant, helium in an HTGR or
a molten salt in a salt-cooled reactor. *What is in that coolant, where did
it come from, and what carries it?*

**The question from the previous step.** [Step 11](./10-stacy-boon-lay.md)
said the aim is one tool from generation to the source term. This page opens
the second half of the book: the part after the particle wall.

## 1. Three sources, not one

Fission products reach the coolant by three routes. The first is the one the
earlier steps described.

| Source | What it is | Where this book treated it |
|---|---|---|
| **Release through intact coatings** | Atoms diffuse out of the kernel and through the layers | Steps 2 to 4 and 8 to 10 |
| **Failed particles** | A particle whose SiC is breached lets the kernel inventory out | Step 7 |
| **Tramp uranium** | Uranium that sits *outside* an intact kernel, so its fission products are born outside the SiC | below |

"Tramp uranium" is the maintainer's term. The code's term is **heavy-metal
contamination**, the fraction `f_hm`, defined as "uranium outside intact
kernels"
([`FailureFractions`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs)).
It is one of four failure fractions: `f_hm`, `f_sic` (as-manufactured
defective SiC), `f_inc` (in-service failure) and `f_inc_sic` (in-service
SiC-only failure).

~~For HTR-10 the repo records a free-uranium value of $3 \times 10^{-4}$~~
**CORRECTED 2026-10-18** (found by the Step 21 review of `htgr_sim_v1`): there
are two HTR-10 numbers. The sembawang bounding example uses a design
specification of $3 \times 10^{-4}$ for `f_hm`, from Liu & Cao (2002)
section 2.1
(the plant model in the digital twin instead uses the measured $5.0 \times 10^{-5}$,
the average over 25 fuel-element lots by the burn-leach method, Tang et al.
2002, `HTR10_FREE_URANIUM_FRACTION` in `fission_product_release.rs`)
([`htr10_air_ingress_kora_bound.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/sembawang/examples/htr10_air_ingress_kora_bound.rs)).
The paper is catalogued as proprietary
([`CATALOGUE.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/kovan-literature/CATALOGUE.md),
key `liu2002htr10fp`), so only the number is quoted here, not its text.

**Why tramp uranium matters more than its size suggests.** Its products are
born with no SiC to cross. A noble gas born there reaches the coolant at once,
so for noble gases and halogens the release rate is proportional to
$f_{hm} + f_{inc}$, not to the leaky-but-intact particle fraction
([`release_rate`](../../code-review/#crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs::release_rate)).
A small $f_{hm}$ is therefore a large share of the normal-operation noble-gas
source. Whether it is a *dominant* share for a given reactor is a number to
take from that reactor's data, not from this page.

## 2. The fewest-assumptions model of the coolant

Treat the coolant as a fluid carrying many species at once. Each species
$s$ (a nuclide, or a chemical form of it) has a concentration $c_s(\mathbf{x}, t)$
in the helium, and obeys a transport equation

$$\frac{\partial c_s}{\partial t} + \nabla \cdot (\mathbf{u} c_s) = \nabla \cdot (D_s \nabla c_s) + S_s - \lambda_s c_s + \sum_p \lambda_p c_p - R_s$$

with flow velocity $\mathbf{u}$, molecular and turbulent diffusivity $D_s$,
sources $S_s$ (the three routes above), decay $\lambda_s$, ingrowth from
parents $p$, and removal $R_s$ to walls, filters and leaks (Step 13). The
pages that follow say what each term costs, and what is thrown away to avoid
paying it.

That equation is an assumption-light statement of the physics, not something
the repo solves. Step 14 lists what solving it would take.

## 3. Graphite dust

In a pebble-bed HTGR the fuel elements are graphite spheres that move and
rub. A common physical picture is that abrasion makes fine graphite dust,
that dust carries sorbed fission products, and that it is carried round the
circuit with the helium and can settle or be stirred up again. That picture is
general knowledge, and **this repo has no source for dust generation, size or
loading**. ~~[citation needed]~~ **Partly sourced 2026-10-21
(proprietary-folder sources).** IAEA-TECDOC-978 (IAEA, Vienna, 1997, p. 265)
says that in a pebble-bed HTGR graphite dust forms by abrasion during
fuel-ball recirculation and is a major transport medium for fission products
besides the coolant itself. For the German reactors it reports (p. 269) an AVR
dust production of about 3 kg per operating year and about 60 kg in the primary
circuit at the end of 1988, 0.5 to 2 % of the gas-borne dust deposited per
recirculation, and a THTR-300 estimate of 13 to 400 kg (expected 25 kg) of fine
dust in 300 days at full power. IAEA-TECDOC-1674 (IAEA, Vienna, 2012, p. 209)
gives AVR cold-gas-filter dust concentrations of 4 to 40 µg/Nm³, and (p. 537)
notes that a MELCOR-type calculation needs a dust generation rate and size
distribution as input. These are AVR and THTR-300 values, not HTR-10's. What
the repo does have:

- **A dust inventory fraction in the HTR-10 accident stage of the digital
  twin, not in boon-lay.** The `htgr_sim_v1` blowdown stage takes the fraction
  of an element's *deposited* activity that sits on dust (Cs 5 %, Sr, Rb and
  Ag 20 %, I 1 %) and releases 10 % of that dust, citing Liu & Cao (2002)
  section 4.1.1.3
  ([`depressurisation.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/depressurisation.rs),
  `dust_share`, `DUST_RELEASED_FRACTION`).
- **No dust term in TRISO-ATOPS.** The port's own limitations page says its
  accident release has "no building, dust or helium-purification term"
  ([limitations](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/docs/lessons/src/limitations.md)).
  The only place the fork's source mentions dust is that same statement, that
  there is none (module doc of `triso_atops_fork`). Dust is
  therefore **not modelled** by the cheap model of Step 15.
- **No dust physics at all**: no generation rate, no size distribution, no
  deposition or resuspension model, anywhere in the workspace I searched.

So the honest position is that dust is an *input fraction* in one example, and
absent as physics.

## 4. How the species are grouped

The cheap models do not track chemistry species by species. They sort
nuclides into groups and give each group one set of rules. TRISO-ATOPS uses
five
([`ElementGroup`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/triso_atops_fork/nuclide_model/mod.rs)):

| Group | Elements |
|---|---|
| noble gas | He, Ne, Ar, Kr, Xe, Rn |
| halogen | F, Cl, Br, I, At, plus Se and Te (upstream's choice, kept) |
| special metal | Rb, Sr, Cs, Ba, Eu |
| silver | Ag, Pd |
| other | everything else |

Se and Te sit in the halogen group because upstream put them there, not
because their chemistry says so. The chemistry behind the groups is in
[Step 3](./03-layers-chemistry.md). The next page uses the groups to say
how each is removed.

## Why each simplification is allowed

**Kinds of justification used below.** (a) negligible effect, with an estimate; (b) computationally too expensive to be practical; (c) bounding or conservative, with the side it errs on; (d) data not available. **Open** means this page cannot justify it.

| Simplification | Kind | Justification |
|---|---|---|
| Groups stand in for chemical species | (b) and (d) | Species-resolved chemistry needs data the repo lacks, and one equation per species is costly (Step 14). Error is **open** |
| Tramp uranium as one fraction | (d) | Only the HTR-10 design value is recorded; no spatial or time dependence is available |
| Dust as a fixed share, or absent | (d) | The repo has no dust generation or loading data. Called neither negligible nor small: **open** |
| Species do not react with each other | **Open** | Not examined here |

## What we assumed

- The coolant is the only medium considered, and it carries species that do
  not react with each other.
- Tramp uranium is a single fraction of the fuel, with no dependence on
  location or time.
- Dust is, at best, a fixed share of the plated-out activity. It has no
  physics of its own in this workspace.
- Chemical groups stand in for chemical species.

## Next question

Atoms in the coolant leave it again, by several routes at different rates.
[Step 13](./12-escape-and-removal.md) lists the routes and what drives each.
