# OUTRAM PARK ecosystem — domain naming

**Date:** 2026-08-05 · **Status:** decisions recorded; two items open.

Singapore MRT station names identify **domains**, not crates.

> **This is a naming layer, not a repository restructure.** Decided
> 2026-08-05. Crates keep their existing names and sit under a domain. In
> particular, `outram-park-fork-*` forks keep that prefix — it is what
> `RESEARCH_INTEGRITY_AND_PROVENANCE.md` requires and what the `op-ahi`
> trademark-compliance epic established. A domain name must never bury the
> identity of the upstream project a crate forks.

---

## Domains

| Domain | Scope | Rests on |
|---|---|---|
| **TUAS** | Boussinesq thermal hydraulics — incompressible, natural and forced circulation, buoyancy-driven flow, molten salt, pipe networks, heat exchangers | `tuas_boussinesq_solver` |
| **TAMPINES** | Thermophysical properties, steam tables, EOS, compressible-flow infrastructure, HEM, balance-of-plant, TH framework | `tampines`, `tampines-steam-tables`, `outram-park-fork-coolprop` |
| **NEE SOON** | **Neutronics and nuclear data** — the integration crate for that domain, and only that domain | `nee_soon`, composing `njoy-outram-park-fork`, `outram-mc-libs`, `teh-o-prke` |
| **BEDOK** | **Multiphysics coupling at system level** — TH and neutronics coupled, above 1-D neutronics fidelity but **below CFD fidelity** (CFD-level coupling stays with GeN-Foam in `outram-foam-appbuilder-lib`) | *new* |
| **SEMBAWANG** | Severe accident progression — melt behaviour, relocation, vessel failure, MCCI, hydrogen, aerosols, source term. *"What gets released?"* | *new* — scoped in `docs/melcor-scoping.md` |
| **CHANGI** | Atmospheric dispersion, plume transport, deposition, ground contamination. *"What happens after release?"* | *new* — depends on a FLEXPART port |
| **REDHILL** | Groundwater and geological transport, subsurface radionuclide migration, porous-media flow. *"What happens after deposition?"* | *new* — depends on `outram-park-fork-pflotran` |

Neutronics, fuel performance, CFD, meshing, KOVAN and the remaining crates are
not yet assigned a domain. That is deliberate: the seven names above cover
roughly a third of the workspace, and unassigned crates are not thereby
orphaned — they simply have no domain label yet.

---

## Decisions (2026-08-05)

1. **This repository's conventions take precedence over the draft** for
   **NEE SOON** and **TAMPINES**.
   - **NEE SOON stays an integration crate — scoped exclusively to neutronics
     and nuclear data.** Refined 2026-08-05. It keeps the integration role its
     README describes, but its domain is neutronics + nuclear data only: it
     composes `njoy-outram-park-fork`, `outram-mc-libs` and `teh-o-prke`.
     Thermal-hydraulic coupling is **not** its job — that moves to BEDOK. (The
     draft's plan to make NEE SOON a general neutronics *solver* umbrella is
     still rejected; it integrates, it does not implement.)
   - **TAMPINES keeps its existing identity.** No "AI" in the expansion; the
     `tampines` and `tampines-steam-tables` crates are published, and their
     names and scope stand. Surrogate/PINN work, if pursued, needs its own
     scoping and its own V&V regime — it is not a scope bullet.
2. **No repository restructure.** Domains are a label above crates.
3. **CHANGI is scoped to research, education and V&V only.** Its scope
   statement must **not** claim emergency-planning, emergency-response, dose
   assessment for real populations, or operational Level 3 PSA support —
   `RESPONSIBLE_USE.md` excludes those, and the draft's original wording
   contradicted it. The capability is in scope; that framing is not.
4. **Dependency directions fixed:**
   - **REDHILL depends on `outram-park-fork-pflotran`.**
   - **CHANGI depends on the FLEXPART port** (GPL-3.0; see
     `docs/melcor-scoping.md` §4 Tier A).
5. **BEDOK is the systems-level multiphysics coupling engine** — thermal
   hydraulics and neutronics coupled, at higher fidelity than 1-D neutronics
   but **not at CFD level**. CFD-fidelity multiphysics coupling remains
   GeN-Foam's, in `outram-foam-appbuilder-lib`.
6. **BEDOK vs NEE SOON boundary — settled by domain, not by layer.** Both are
   coupling crates; they are separated by *what* they couple. NEE SOON couples
   within neutronics and nuclear data. BEDOK couples *across* physics — TH to
   neutronics — at system level. A neutronics-only integration belongs in NEE
   SOON; anything reaching into thermal hydraulics belongs in BEDOK.

---

## The name

**Settled 2026-08-05.** The canonical expansion is:

> **O**pen-source **U**nified **TR**Ansient **M**ulti-**P**hase **A**dvanced
> **R**eactor simulation **K**it

Written out: *Open-source Unified TRAnsient Multi-Phase Advanced Reactor
simulation Kit*. **"simulation" is lower-case** — it is a connecting word that
contributes no letter to the acronym, so it is not capitalised. Capitals mark
the letters that spell OUTRAM PARK.

This supersedes both forms currently in the repository — `README.md` and
`CLAUDE.md` ("…Advanced Reactor **simulator** Kit", no "Unified") and
`RESPONSIBLE_USE.md` ("Open Source Unified … **Simulation** Kit"). **A sweep is
outstanding** across the READMEs, `CLAUDE.md`, the five compliance documents
and any published crate metadata.

---

## Open

*(none — the BEDOK/NEE SOON boundary and the acronym are both settled above)*
