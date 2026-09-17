<!--
PROVENANCE / AI-ASSISTED EXTRACTION NOTICE
==========================================
Source document : "Coupled neutronics and thermal-hydraulics modeling for
                   pebble-bed Fluoride-Salt-Cooled, High-Temperature Reactor (FHR)"
Author          : Xin Wang
Degree / year   : Ph.D. in Engineering — Nuclear Engineering, 2018
Institution     : University of California, Berkeley (Chair: Prof. Per F. Peterson)
Permalink       : https://escholarship.org/uc/item/40q3985m
Access terms    : Open-access, peer-reviewed dissertation (UC eScholarship /
                  California Digital Library). Used here as public literature only.

This file is an AI-ASSISTED extraction of the *core* methodology and the
Figure-4.29 case from the dissertation, produced to seed the OUTRAM PARK
`nee_soon` reproduction workflow. It is a condensed transcription, NOT the full
thesis. It is UNVERIFIED draft material: every equation, number, and claim must
be checked by a human against the source PDF before it is relied upon (per the
workspace AI_USAGE.md / RESPONSIBLE_USE.md rules). Page/equation/table/figure
numbers refer to the printed dissertation.
-->

# Xin Wang (2018) — Overview and reading map

This directory is a committed markdown extraction of the *core* of Xin Wang's
2018 UC Berkeley PhD dissertation, kept so that future OUTRAM PARK work needs no
re-indexing of the 9 MB source PDF. The raw PDF is intentionally **git-ignored**;
this markdown is the committed deliverable.

## What the dissertation is about

The thesis develops **coupled neutronics + thermal-hydraulics (TH) numerical
models for pebble-bed Fluoride-salt-cooled High-temperature Reactors (PB-FHRs)**
at several fidelity levels:

- a 0-D **multi-point kinetics** unit-cell model (reflector-corrected point
  kinetics, implemented in the `PyRK` Python package);
- a 3-D **full-core multi-group neutron diffusion** model coupled to a
  **porous-media CFD** TH model, with an **$SP_3$** correction near strong
  absorbers (control rods) and a **multi-scale** fuel-temperature treatment,
  implemented in **COMSOL** via its user-defined-PDE ("General Form PDE")
  interface;
- **Monte Carlo** reference models built in **Serpent** (via an in-house Python
  input generator, "FIG"), used both to generate homogenised group constants for
  the deterministic models and as code-to-code verification references.

Two FHR designs are studied: **TMSR SF-1** (Chapter 3) and the **Mk1 PB-FHR**
(Chapter 4, a 236 MW(th) pre-conceptual UC Berkeley design). Steady state plus
reactivity-insertion and overcooling (ATWS-class) transients are simulated. The
headline finding is that FHR cores are highly resilient to the investigated
transients (large negative Doppler feedback, high thermal margins, low excess
reactivity from online refuelling).

## Why it matters to OUTRAM PARK

The `nee_soon` coupling crate is scaffolding a re-implementation of Wang's
full-core coupled model on the OUTRAM PARK open-source stack. The tool mapping is
**not one-to-one** and is documented honestly in
[`03-njoy-openmc-genfoam-workflow.md`](03-njoy-openmc-genfoam-workflow.md):

| Wang (2018) tool | Role | OUTRAM PARK re-implementation |
|---|---|---|
| Serpent (Monte Carlo) | group-constant generation + reference | `outram-mc-libs` (transport) fed by `njoy-outram-park-fork` (nuclear data / MGXS) |
| COMSOL user-PDE ($SP_3$ + diffusion) | deterministic neutronics | GeN-Foam $SP_3$ port in `outram-foam-appbuilder-lib` |
| COMSOL porous-media CFD | thermal-hydraulics | GeN-Foam porous-media TH in `outram-foam-appbuilder-lib` |
| PyRK | 0-D reflector-corrected point kinetics | `teh-o-prke` |

The concrete reproduction target is **Figure 4.29** — the maximum fuel
temperature during a Mk1 control-rod-removal transient.

## Reading map

| File | Contents |
|---|---|
| [`00-overview.md`](00-overview.md) | this file |
| [`02-methodology-sp3.md`](02-methodology-sp3.md) | Chapter 2 methodology: multi-point kinetics, multi-group diffusion, the **$SP_3$** equations, MGXS generation from Monte Carlo (Eq. 2.23), cross-section parametrisation for feedback (Eqs. 2.24–2.26), porous-media TH, multi-scale fuel temperature; Appendix D COMSOL $SP_3$ implementation |
| [`03-njoy-openmc-genfoam-workflow.md`](03-njoy-openmc-genfoam-workflow.md) | the njoy → openmc → genfoam workflow: how Wang's Serpent + COMSOL pipeline maps onto the OUTRAM PARK crates, stage by stage |
| [`04-transients-fig4-29.md`](04-transients-fig4-29.md) | the **Mk1 PB-FHR** case: geometry, materials, 8-group structure, control-rod-removal transient definition, and the digitised Fig. 4.27–4.29 reference curves |
| [`references.md`](references.md) | citation + the key third-party references named in the extracted sections |

## Thesis table of contents (for orientation)

- **Ch. 1** Introduction (background; neutronics/TH methods; coupling; numerical tools)
- **Ch. 2** Multiphysics modeling methodology (Monte Carlo + Serpent "FIG" generator; unit-cell multi-point kinetics; full-core diffusion + $SP_3$ + porous-media TH + multi-scale temperature)
- **Ch. 3** TMSR SF-1 core analysis (design, mesh study, code-to-code verification, steady state, transients)
- **Ch. 4** Mark-1 PB-FHR core analysis (design; Serpent MC model; multiphysics model; steady-state optimisation; **transient results incl. Fig. 4.29**)
- **Ch. 5** Conclusions and future work
- **App. A** Serpent input generator example
- **App. B** FHR material thermo-physical properties (flibe; fuel-pebble/TRISO)
- **App. C** Mk1 Monte Carlo geometry + material specifications
- **App. D** Implementing diffusion and $SP_N$ in COMSOL user-PDEs (the $SP_3$ coefficient matrices)
