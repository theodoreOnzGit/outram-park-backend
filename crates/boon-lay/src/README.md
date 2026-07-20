# Intro 

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


The purpose of this crate is to build libraries for a neutron bombardment 
simulator for nuclei where the user can watch nuclei transmute and decay 
in real-time.

Basically, it also holds all the libraries necessary for decay simulation,
neutron capture and so on. 

I intend to build a simulator that demonstrates these libraries as a testing 
ground.

# Decay Data 

Decay data was provided by OpenMC depletion chains based on endfb 8. These 
were xml files. However, the files are huge, about 27 Mb in size.

Not only that, there are thousands of Nuclides. How will this work?

We are going to use serde-xml-rs. 

This will take the Nuclide, then access the data library. 
The serde should return the nuclide decay data.


# Scattering Data and Cross Sections

For diffusion, in neutron theory,

D = 1/(3 Sigma Transport) = 1/(3 * Sigma scatter *(1-mubar))

mubar is average scattering cosine.

But for isotropic scattering, mubar = 0



D = 1/(3 Sigma Scatter) 

This helps me correlate diffusion coefficient vs macroscopic 
scattering cross section.

However, one must note though, that neutron flux diffusion coefficient  (m)
is in different units than diffusion coefficient for materials (m^2/s)

For this, we use 

D =  1/6 (lambda^2) * nu 

nu is collision frequency in per second 
lambda is called jump distance (similar to mean free path).

Jump distances are on the order of 2-3 angstroms for SiC. And about 2 
angstroms for PyC


# TRISO-ATOPS Eulerian fork (`triso_atops_fork`)

Everything above describes boon-lay's **Lagrangian** (single-atom Monte-Carlo)
view of TRISO fission-product transport. The `triso_atops_fork` module adds the
complementary **Eulerian / continuum-diffusion** view: a Rust fork of Idaho
National Laboratory's MIT-licensed
[TRISO-ATOPS](https://github.com/IdahoLabResearch/TRISO-ATOPS) (commit
`de374c8`).

Instead of tracking atoms, it uses closed-form analytical solutions to the
Fickian diffusion equation — the **Booth** equivalent-sphere model, a
**breakthrough** model (for silver through SiC), and a graphite **attenuation**
model — to compute per-nuclide release fractions directly. The equations come
from the NP-MHTGR New Production Reactor Program (EG&G Idaho, 1989); half-lives
are from the IAEA Live Chart of Nuclides.

What is ported and verified this pass (uom-typed, tested):

- `nuclide_model` — the TRISO-ATOPS nuclide record, the five transport element
  groups, and the 84-nuclide supported table.
- `diffusion` — Arrhenius diffusion coefficients `D(T)` for the kernel, matrix
  graphite, and Ag-in-SiC, plus the time-integrated `∫D dt`.
- `release_models` — Booth (long/short-lived), breakthrough, graphite
  attenuation, their transient (accident) variants, and the group dispatchers
  `rb_fail` / `release_fraction_transient`.

Scaffolded / deferred (see beads op-b4a.2.2 / op-b4a.2.3): the coolant
**activity** bookkeeping (circulating / plate-out / HPS) and the nodal
orchestration + JSON run-file driver — their upstream unit conventions mix
atoms/Ci/Bq and need a dimensional-analysis pass before uom wiring. The
**GUI was intentionally not ported** (headless-library + Android rule).

Provenance/attribution: `LICENSE.triso-atops`, `NOTICE.triso-atops`, per-file
headers, and `upstream_source/TRISO-ATOPS/PROVENANCE.md` (gitignored,
reference-only clone). Full Python→Rust module map and V&V results:
[`docs/triso-atops-fork.md`](../docs/triso-atops-fork.md).
