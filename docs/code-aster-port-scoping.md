# Scoping: porting code_aster's constitutive laws and fracture mechanics to Rust

**Date:** 2026-08-04 · **Status:** scoping analysis, not an approved plan.
**Nothing has been ported.** This is a map.
**Related:** `docs/melcor-scoping.md` §3 (lower-head creep failure),
`docs/offbeat-port-scoping.md` (the protocol this follows).

> Every count below was taken on 2026-08-04 from a shallow read-only clone of
> the upstream repository at commit `b504ea08` (2026-08-03), kept **outside**
> this working tree at `~/dev/codeaster/src` per the workspace rule on upstream
> clones. Line counts are over `.F90 .f90 .c .h .cxx .py .mfront`.

---

## 1. Why code_aster specifically

| Property | Value | Why it matters here |
|---|---|---|
| Licence | **GPL-3.0-or-later** — root `LICENSE` is the GPLv3 text; every source header reads *"either version 3 of the License, or (at your option) any later version"* | Identical to this workspace, with the "or later" upgrade path. No compatibility work. |
| Copyright | `Copyright (C) 1991 - 2026 - EDF` | Live, actively maintained (HEAD one day old at survey time). |
| Domain | Nonlinear structural and thermo-mechanical analysis, built by EDF to justify the integrity and remaining life of its own reactor fleet | The constitutive laws are *specifically* the nuclear ones — irradiation creep, Zircaloy anisotropy, vessel steels — not generic mechanical-engineering fare. |
| Law catalogue | **231 declarative behaviour records** in `code_aster/Behaviours/*.py`, each carrying `num_lc`, state-variable names, supported modelisations, and integration algorithms | Machine-readable. The Rust registry can be **generated** from it rather than hand-transcribed. |
| Dispatch | `bibfor/lc/lc0000.F90 … lcNNNN.F90`, indexed by the catalogue's `num_lc` | A numbered dispatch table — maps 1:1 onto a Rust enum, exactly what the workspace's no-trait-objects rule wants. |
| Dual payoff | The catalogue carries both severe-accident laws (creep rupture, damage) **and** fuel-performance laws (Zircaloy, irradiation creep) | One port serves both `docs/melcor-scoping.md` phase 5 and `outram-park-fork-offbeat`'s rheology. |

The dual payoff is the strongest argument. A port justified only by lower-head
creep would be speculative; the same 231-law catalogue also contains `ZIRC`,
`ZIRC_MECA`, `META_LEMA_ANI`, `LEMAITRE_IRRA`, `VISC_IRRA_LOG`, `GRAN_IRRA_LOG`
and `IRRAD3M` — anisotropic and irradiation creep for cladding, which OFFBEAT
needs for normal operation and does not currently have.

---

## 2. Module inventory (verified)

### Upstream totals — for scale

| Directory | Files | Lines | Content |
|---|---|---|---|
| `bibfor/` | 16,291 | 1,730,463 | Fortran core |
| `code_aster/` | 1,468 | 284,806 | Python command and catalogue layer |
| `astest/` | 358 | 201,415 | Testcases |
| `catalo/` | 742 | 196,946 | Element/command catalogues |
| `bibcxx/` | 731 | 101,818 | C++ layer |
| *(remainder)* | ~170 | ~68,000 | build, docs, extern, run_aster |
| **Total** | **~19,800** | **~2,584,000** | |

**A full port is not on the table.** 2.58 MLOC is an order of magnitude beyond
anything in this workspace, and most of it — the element library, the Newton
drivers, JEVEUX memory management, the WAF build, the bespoke command language
— duplicates roles `outram-foam-basic-lib` already fills.

### In scope

| Module | Files | Lines | Content |
|---|---|---|---|
| `bibfor/comport/` | 241 | 60,818 | The constitutive laws themselves |
| `bibfor/fracture/` | 72 | 14,954 | G-theta method, K/G extraction, crack propagation |
| `bibfor/lc/` | 102 | 11,171 | `lcNNNN` numbered dispatch layer |
| `bibfor/comport_prep/` | 89 | 10,576 | Material-parameter preparation |
| `mfront/` | 14 | 4,449 | MFront-declared laws (Burger creep, `VISC_ISOT_PLAS`, …) |
| `code_aster/Behaviours/` | 231 | — | Declarative law catalogue (metadata, no algorithm) |
| **Total** | **~518** | **~102,000** | |

≈ **4% of upstream by volume**, and the part with the highest physics density
per line.

### The behaviour catalogue, by kind

From `lc_type` across all 231 declarations:

| `lc_type` | Count |
|---|---|
| `MECANIQUE` | **151** |
| `KIT_THM` | 12 |
| `COUPLAGE_THM` | 8 |
| `HYDRAULIQUE` | 6 |
| `DEFORMATION` | 6 |
| `UTILITAIRE` | 5 |
| `SECHAGE` (drying) | 5 |
| `PHASE` | 5 |
| `THERMIQUE` | 3 |
| `MODELE_METALLURGIQUE` | 3 |
| `DIVERS` | 3 |
| `KIT_*`, `META_G_*`, `ELEMENT` | 1 each |

**151 mechanical laws** is the real target. The THM/hydraulic/drying subsets
are concrete- and geomechanics-oriented and can be deferred or skipped.

### Integration algorithms

`algo_inte` across the catalogue: `ANALYTIQUE` 68, `SPECIFIQUE` 32, `NEWTON`
23, `NEWTON_PERT` 16, `SECANTE` 12, `BRENT` 10, `NEWTON_1D` 8, plus
`RUNGE_KUTTA` and `VERIFICATION` on individual laws (`SANS_OBJET` 52 marks
laws needing no local solve).

This is a **small, closed set of shared machinery** every law calls into.
Porting it once, first, unblocks all 151 laws — which is why it is phase P1
below rather than something each law drags along.

---

## 3. Substrate assessment — what we already have

`outram-park-fork-offbeat/src/rheology/` already contains a working
constitutive-law framework in the workspace's own idiom:

- `ConstitutiveLaw` — a 3-variant enum (`Elastic`, `MisesPlasticity`,
  `MisesPlasticCreep`) with `correct()` and a radial `return_map()`
- `CreepModel`, `YieldStressModel`, `RheologyState`, `RheologyInputs`,
  `StressCorrection`

`outram-foam-basic-lib` provides `primitives/symm_tensor.rs` and the tensor
algebra the stress/strain update needs, plus `ldu_matrix` and `krylov` if a law
ever needs a local linear solve.

So the **shape** is established and proven — a Rust enum of constitutive laws
with a stress-update method, hung on a `SymmTensor` algebra. What code_aster
adds is **volume and depth**: 151 mechanical laws against OFFBEAT's 3, with
irradiation, anisotropy, metallurgical phase transformation, and damage that
OFFBEAT does not model at all.

**This is a clean extension of an existing pattern, not a new architecture.**
That is the single most favourable fact in this document.

---

## 4. Naming convention (decided — applies to every ported item)

Upstream names are 1970s French six-character Fortran: `betfpp`, `dpvpcr`,
`cjspla`, `lc0032`. Transliterating them would violate the workspace's
**Human interface layer** rule, which requires that a Rust developer navigate
the API with rust-analyzer alone.

**Every ported law therefore carries three names, with distinct jobs:**

1. **The Rust name — descriptive English.** What rust-analyzer surfaces and
   what a reader sees first. `NortonViscoplastic`, not `Dpvpcr`.
2. **The ASTER behaviour name — preserved verbatim.** `NORTON`,
   `VISC_CIN2_CHAB`, `META_LEMA_ANI`. This is what a code_aster user types in
   a deck and what the literature cites, so it must stay searchable — as a
   documented alias and as the string an `as_aster_name()` accessor returns.
   **Do not "improve" these.**
3. **The legacy Fortran symbol — in the doc comment and attribution header
   only.** `dpvpcr`, `lc0032`, with its upstream file path, so any line of the
   port can be traced back to the subroutine it came from.

Worked example of the intended doc-comment shape:

```rust
/// Isotropic viscoplastic flow after Norton.
///
/// ASTER behaviour name: `NORTON` (`num_lc = 32`, 7 state variables).
/// Upstream: `bibfor/lc/lc0032.F90`, `bibfor/comport/` — legacy symbol
/// `lc0032`. Integration: Runge-Kutta or perturbed Newton.
///
/// Strain rate follows a power law in deviatoric stress, ...
```

This satisfies the interface rule and the provenance rule at once, and it is
the same pattern `outram-park-fork-offbeat` already uses when its doc comments
name the upstream class *and* the dictionary `TypeName` string.

---

## 5. Phased port plan

Sequenced so the shared machinery lands before the bulk laws, and so the two
consumers (severe accident, fuel performance) each get a usable result early.

### P0 — Catalogue ingestion and registry generation
Parse the 231 `code_aster/Behaviours/*.py` declarations into a generated Rust
registry: enum variants, `num_lc`, state-variable names and counts, supported
modelisations and deformations, integration algorithm. No physics yet.
**Acceptance:** the registry compiles, round-trips every catalogue entry, and
`NORTON` dispatches end-to-end to a stub.

### P1 — Integration-algorithm layer
`NEWTON`, `NEWTON_1D`, `NEWTON_PERT`, `SECANTE`, `BRENT`, `RUNGE_KUTTA`, and
the `ANALYTIQUE` closed-form path. Shared by every law; must precede them.
**Acceptance:** each solver reproduces a published convergence result on an
analytical test problem.

### P2 — Viscoplastic creep family *(drives the severe-accident case)*
`NORTON`, `NORTON_HOFF`, `LEMAITRE`, `LEMAITRE_IRRA`, `VISCOCHAB`,
`VISC_TAHERI`, `VISC_CIN1_CHAB`, `VISC_CIN2_CHAB`, `VISC_ISOT_*`.
**Acceptance:** a vessel-wall creep-rupture case matched against a published
result — the model `docs/melcor-scoping.md` phase 5 needs.

### P3 — Damage and rupture
`VENDOCHAB` (Lemaitre-Chaboche damage), `ROUSS_VISC` (Rousselier),
`VISC_GTN` (Gurson-Tvergaard-Needleman), `CRIT_RUPT`, the `ENDO_*` family.

### P4 — Metallurgy and irradiation *(drives the fuel-performance case)*
`ZIRC`, `ZIRC_MECA`, `META_LEMA_ANI`, the `META_*` family, `GRAN_IRRA_LOG`,
`VISC_IRRA_LOG`, `IRRAD3M`. **Acceptance:** OFFBEAT's rheology can select a
code_aster law and reproduce a cladding-creepdown case.

### P5 — Fracture mechanics
`bibfor/fracture/` — the G-theta method (`cgComputeGtheta` and the `cg*`
family), K/G extraction (`cakg2d`, `cakg3d`), crack propagation. 72 files.

### P6 — Remaining mechanical laws
The balance of the 151 `MECANIQUE` entries. High volume, low individual risk,
independently testable — the natural candidate for a partitioned agent fleet,
one law family per agent.

**Deferred / possibly out of scope:** `KIT_THM`, `COUPLAGE_THM`,
`HYDRAULIQUE`, `SECHAGE` (concrete and geomechanics), and the MFront laws,
which need a decision on generator-vs-hand-port first (§7.3).

---

## 6. Effort

| Phase | Effort (py) |
|---|---|
| P0 catalogue and registry | 0.1–0.2 |
| P1 integration algorithms | 0.2–0.4 |
| P2 viscoplastic creep | 0.3–0.5 |
| P3 damage and rupture | 0.3–0.5 |
| P4 metallurgy and irradiation | 0.3–0.5 |
| P5 fracture mechanics | 0.2–0.4 |
| P6 remaining mechanical laws | 0.4–0.9 |
| **Total** | **1.8–3.4** |

Comparable to the OFFBEAT port (1.5–3.2 py) — fewer files, but Fortran-77
legacy code is denser and harder to read per line than OFFBEAT's OpenFOAM C++,
and the renaming discipline in §4 is per-law work that does not amortise.

As with OFFBEAT, these are conventional-development figures and are **not**
calibrated to this workspace's AI-assisted throughput; the per-commit
`API-Usage` trailers and `docs/historian/` reports are the local data for
deriving a multiplier.

---

## 7. Constraints and obligations

- **Licence provenance.** GPL-3.0-or-later, compatible with this workspace.
  Every ported file keeps an attribution header naming the upstream project,
  source file, commit (`b504ea08`), copyright (`EDF 1991–2026`) and licence —
  the pattern `outram-park-fork-offbeat` already uses.
- **Do not vendor.** Upstream stays outside this tree at `~/dev/codeaster/src`,
  read-only. Never add it as a workspace member or commit its source here.
- **Restricted upstream data is out of scope.** code_aster is distributed as
  three repositories; the README states `validation: few testcase files with
  proprietary data` and `data: material data that can not be freely
  distributed`. **Only `src` may be cloned or referenced.** Do not obtain the
  `validation` or `data` repositories — that is a `DATA_POLICY.md` line, not a
  preference. (`src/data/` itself is build templates and config, and is fine.)
  `src/astest/` **is** in scope and is licence-clean — see §8.2.
- **V&V oracle.** `astest/` (358 files, 201 kLOC, 4,590 GPL-headered testcase
  files) is the port's regression suite, playing the role `Cases/Verification`
  plays for the OFFBEAT port. Each ported law should name the `astest` cases it
  is checked against.
- **Android/Termux.** Pure-Rust, no BLAS, no FFI — the port should be
  Android-clean by construction. Verify with
  `cargo check -p <crate> --all-targets --target aarch64-linux-android`.
- **Workspace Rust rules.** Enum dispatch, never trait objects — the `lcNNNN`
  table makes this natural. No `Box<T>`, no lifetime parameters, `Arc` for
  shared state.
- **V&V.** Per the workspace rule, each ported law documents *methodology* and
  *measured results with uncertainty* — not merely "ported".
- **AI output is untrusted draft material** until human review, per
  `RESPONSIBLE_USE.md`. A ported law is not a validated law.

---

## 8. Open questions — **re-ask these before any port work begins**

> **These four are maintainer decisions, not assistant defaults.** Anyone —
> human or AI — picking this document up to start work must **put every
> question below back to the maintainer and get an answer**, rather than
> adopting the "leaning" note as a settled choice. The leanings record what the
> evidence suggested on 2026-08-04; they are not approvals.
>
> Each one changes work that is expensive to redo: the crate boundary (Q1) is
> hard to move once laws are written against it, and the tensor convention (Q3)
> and strain measure (Q4) are **silent-wrong-answer risks** — code that
> compiles, runs, and produces plausible but incorrect stresses.

1. **Relationship to OFFBEAT's `ConstitutiveLaw`.** Extend that enum in place,
   or stand up a separate crate that OFFBEAT later delegates to? *Leaning
   separate crate* — `outram-park-fork-code-aster`, per the `op-ahi`
   trademark-compliance naming rule. The laws serve vessel, piping and
   containment as well as fuel, and a separate crate is independently
   publishable. Needs deciding before P0.
2. **MFront: port the 14 laws, or the generator?** MFront/TFEL is a separate
   CEA/EDF project with its own licence (GPL or CeCILL — **unverified**).
   Porting 14 laws by hand is bounded; porting a code generator is not.
   *Leaning hand-port*, but the licence must be checked either way before the
   MFront laws are touched at all.
3. **Tensor conventions.** code_aster uses a specific Voigt ordering with
   `sqrt(2)` factors on the shear components. Mapping that onto
   `outram-foam-basic-lib`'s `SymmTensor` is a **silent-wrong-answer risk** if
   done casually — pin it down with a round-trip test in P0, before any law is
   ported.
4. **Large strain.** Is `deformation=PETIT` (small strain) sufficient for the
   target cases, or is `GDEF_LOG` needed? Creep rupture of a lower head
   involves large deformation, so this may not be optional — and retrofitting a
   finite-strain measure after the laws are written is a rewrite, not a patch.

---

## 9. Resolved questions

1. **Is `astest/` freely distributable?** — **RESOLVED 2026-08-04: yes.**
   Three independent lines of evidence:
   - The upstream README states `src` contains *"Python, C/C++, Fortran source
     files, its build scripts and **most of the testcases**"*, and names the
     exclusions as two **separate** repositories — `validation` (*"few testcase
     files with proprietary data"*) and `data` (*"material data that can not be
     freely distributed"*). The held-back cases are, by construction, not in
     `src`.
   - **Every** `.comm` and `.py` file in `astest/` carries the full
     GPL-3.0-or-later grant in its header — 4,268 / 4,268 and 322 / 322
     respectively, verified by header grep. The licence is applied to the
     testcase files themselves, not merely to the surrounding repository.
   - No file under `astest/` mentions "proprietary", "confidential",
     "restricted distribution", or "not be freely distributed".

   **Consequence:** `astest/` **is** usable as the port's V&V oracle, the same
   way `Cases/Verification` serves the OFFBEAT port. Residual caveat: the
   binary mesh and data files (`.mmed`, `.med`, `.mail`, `.datg` — ~2,970
   files) cannot carry inline headers and are covered only by the repository
   `LICENSE`; that is normal for binary assets, but it is a repository-level
   rather than per-file assurance.

2. **Is the licence GPLv3-compatible?** — **RESOLVED 2026-08-04: yes,
   GPL-3.0-or-later.** The root `LICENSE` is the GPLv3 text ("Version 3,
   29 June 2007"), and every source header reads *"either version 3 of the
   License, or (at your option) any later version"*. An earlier web-page
   summary suggesting GPL-3.0-**only** was wrong; the "or later" grant is
   present in the files themselves. No compatibility work is needed.

---

## 10. Provenance

- code_aster source — <https://gitlab.com/codeaster/src> (GPL-3.0-or-later;
  `LICENSE` and source headers retrieved 2026-08-04; commit `b504ea08`,
  2026-08-03; module counts from a local shallow clone, same date)
- code_aster project site — <https://code-aster.org>
- Upstream README's three-repository split (`src` / `validation` / `data`) is
  the basis for the data-scope constraint in §7.
