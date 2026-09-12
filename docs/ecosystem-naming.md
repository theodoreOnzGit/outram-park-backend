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

**Settled 2026-08-05. Revised 2026-09-12: "Multi-Phase" → "Multi-Physics".**
The canonical expansion is:

> **O**pen-source **U**nified **TR**Ansient **M**ulti-**P**hysics **A**dvanced
> **R**eactor simulation **K**it

Written out: *Open-source Unified TRAnsient Multi-Physics Advanced Reactor
simulation Kit*. **"simulation" is lower-case** — it is a connecting word that
contributes no letter to the acronym, so it is not capitalised. Capitals mark
the letters that spell OUTRAM PARK.

### Why "Multi-Physics" and not "Multi-Phase" (2026-09-12, maintainer decision)

**The acronym is unaffected.** "Multi-**P**hysics" supplies the same `M` and `P`
that "Multi-**P**hase" did, so OUTRAM PARK still spells out exactly.

**It is also the more accurate word.** Multiphase flow is *one* crate of ~37 —
`outram-foam-multiphase`, itself a scaffold with no human V&V. What the
workspace actually spans is neutronics (Monte Carlo *and* deterministic),
nuclear data processing, thermal hydraulics, fuel performance, structural
mechanics and plasticity, granular DEM, molten-salt thermochemistry, depletion,
process simulation and CFD. "Multi-Phase" names a sub-capability and undersells
the rest; "Multi-Physics" names what the thing is.

That matters increasingly as the project is cited: an acronym expanded one way
in an arXiv preprint and another way in the repository is the kind of drift that
is expensive to undo later, because published papers cannot be edited.

### Sweep: DONE (2026-09-12)

The sweep flagged here as outstanding has been carried out, and it corrected two
*separate* errors beyond the Multi-Phase → Multi-Physics change:

| file | was | issue |
|---|---|---|
| `README.md` (parent) | "Open-source TRAnsient … **simulator** Kit" | no "Unified" — the `U` had no source |
| `outram-park-backend/README.md` | same | same |
| `outram-park-backend/CLAUDE.md` | same | same |
| `crates/kovan-semantics/…/agents_md.rs` | same | same |
| `crates/outram-foam-turbulence-lib/README.md` | same | same |
| `outram-park/src/main.rs` (`--help` text) | same | same |
| `RESPONSIBLE_USE.md` | "Open Source Unified … **Simulation** Kit" | over-capitalised, "Simulation" |

All seven now carry the canonical form verbatim.

**Published crate metadata is deliberately NOT swept.** Versions already on
crates.io cannot be edited, so their descriptions keep whatever wording they
shipped with; the correction applies from the next publish onward. Do not treat
an old crates.io page as evidence the sweep was missed.

---

## Open

*(none — the BEDOK/NEE SOON boundary and the acronym are both settled above)*
