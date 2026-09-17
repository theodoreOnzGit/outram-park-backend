# Scoping: porting OFFBEAT (and bundled SCIANTIX) to Rust

**Date:** 2026-07-29 · **Status:** scoping analysis, not an approved plan
**Parent:** `docs/type-i-digital-twin-scoping.md` §5 (Track B — fuel performance)

> Every count below was obtained from the upstream GitLab API on 2026-07-29 and
> verified per-module. An initial recursive listing silently truncated and
> undercounted; the per-module figures are the corrected ones. Nothing has been
> ported or built — this is a map.

---

## 1. Why OFFBEAT specifically

| Property | Value | Why it matters here |
|---|---|---|
| Licence | **GPL-3.0** (LICENSE file retrieved) | Identical to this workspace. No compatibility work |
| Architecture | Cell-centred finite-volume on OpenFOAM | `outram-foam-basic-lib` already translates that layer |
| Upstream project | `foam-for-nuclear` (EPFL LRS, PSI LRT, TAMU MEDAL) | **Same project as GeN-Foam**, already being ported into `outram-foam-appbuilder-lib` |
| Activity | Last commit 2026-06-09 | Live, not abandoned |
| Bundled | **SCIANTIX vendored in-tree** (MIT, own LICENSE) | One port yields both, plus the coupling |
| Test oracle | `Cases/Verification` + `Cases/testCases`, ~1500 entries | Ready-made V&V suite for the port |

The GeN-Foam overlap is the strongest argument. Both codes come from the same
group, share OpenFOAM idioms, naming and file layout, and target the same
`outram-foam-basic-lib` substrate. Porting conventions established for one carry
directly to the other.

---

## 2. Module inventory (verified)

`offbeatLib/` — roughly 700 C/H files:

| Module | C/H files | Content | Port risk |
|---|---|---|---|
| `materials` | **249** | Property correlations (UO2, Zircaloy, IAPWS, …) | **Low** risk, high volume — mechanical, parallelizable |
| `physicsSubSolvers` | **145** | thermal, mechanics, neutronics, flow, elementTransport | **High** — the core |
| `fvPatchFields` | **99** | BCs: contact, traction, coolant channel, gap, HTC | Medium |
| `rheology` | **64** | Constitutive laws: creep, plasticity, swelling | **High** — physics-dense |
| `OpenFOAMNumerics` | 42 | Interpolation schemes, log vol fields | Medium; partly maps to existing |
| `corrosion` | 20 | Corrosion model + layer addition/removal topo changer | Medium |
| `finiteVolume` | 12 | fvc extensions, grad schemes | Low |
| `heatSource` | 12 | Power/heat deposition | Low |
| `burnup` | 10 | Burnup accumulation | Low |
| `sliceMapper` | 9 | 1.5D / 2D / 3D mapping | Medium |
| `gapGasModel` | 8 | Gap conductance and gas composition | Medium |
| `accelerationSchemes` | 8 | Anderson mixing | Low |
| `fissionGasRelease` | 6 | FGR models + SCIANTIX hook | Low–medium |
| `fastFlux` | 6 | Fast neutron flux | Low |
| `SCIANTIX` (vendored) | **44** | 0D single-grain fission gas — **MIT** | Separate licence lineage |

The distribution is favourable: the single largest module (`materials`, 249
files) is the *lowest-risk* work — correlations with clear inputs and outputs,
independently testable, and well suited to parallel execution. The genuinely hard
content is concentrated in `physicsSubSolvers` + `rheology` + contact
(~210 files, ~30% of the library).

---

## 3. Substrate assessment — what we already have

`outram-foam-basic-lib` provides, verified present:

- `primitives/tensor.rs`, `primitives/symm_tensor.rs` — the stress/strain algebra
- `fv_operators/fvc/div_tensor.rs`, `fv_operators/fvm/d2dt2.rs` — tensor
  divergence and second time derivative
- `fields/vol_field.rs` + `vol_field_algebra.rs` — tensor-valued fields
- `ldu_matrix`, `krylov` — the linear-solver stack
- `mesh`, `interpolation`, `thermophysics`, `fluid_thermo`, `ode`, `polynomial`

**What is absent, and is exactly what OFFBEAT adds:**

1. A **solid-mechanics sub-solver** — displacement field, traction and fixed-
   displacement boundary conditions, momentum assembly. The *primitives* exist;
   the solver does not.
2. **Constitutive laws** — creep, plasticity, swelling, densification, relocation.
3. **Contact and gap** — `implicitContact`, gap conductance, and mapping across
   non-conformal fuel/cladding meshes. Architecturally the hardest piece.
4. **Multi-region coupling** for the fuel/gap/cladding stack.

So the FV foundation is built and the mechanics layer is not. That is a clean,
well-bounded gap rather than a diffuse one.

---

## 4. Phased port plan

Sequenced so that something runs end-to-end early, and the highest-risk work is
attempted before the highest-volume work.

### P0 — Mechanics bridgehead
Displacement field, linear-elastic constitutive law, traction and fixed-
displacement patch fields, momentum assembly on the existing `ldu_matrix`/
`krylov` stack. **Acceptance:** a 1D thermo-elastic fuel rod runs and matches an
analytical solution. Nothing else proceeds until this stands.

### P1 — Rheology
Creep, plasticity, swelling, densification, relocation (~64 files).
**Acceptance:** matches selected `Cases/Verification` results.

### P2 — Gap and contact
`implicitContact`, `gapGasModel`, non-conformal fuel/cladding mapping,
`sliceMapper`. The hardest architectural piece; deliberately before the bulk
material work so an unpleasant surprise surfaces early.

### P3 — Materials (bulk)
~249 files of property correlations. Mechanical, independently testable,
parallelizable. This is the natural candidate for a partitioned agent fleet —
one module per agent, no shared files.

### P4 — Burnup, fast flux, FGR, SCIANTIX
`burnup`, `fastFlux`, `fissionGasRelease`, then the vendored SCIANTIX (44 C/H,
MIT) with its coupling.

### P5 — Corrosion, acceleration, remaining patch fields
`corrosion` (including the layer addition/removal topo changer),
`accelerationSchemes`, residual `fvPatchFields`.

### V&V throughout
`Cases/Verification` and `Cases/testCases` (~1500 entries) become the port's
regression suite. Per the workspace V&V rule, each ported model records both
*methodology* and *measured results with uncertainty* — not merely "ported".

---

## 5. Effort

| Phase | Effort (py) |
|---|---|
| P0 mechanics bridgehead | 0.3–0.6 |
| P1 rheology | 0.3–0.5 |
| P2 gap and contact | 0.3–0.6 |
| P3 materials (bulk) | 0.3–0.8 |
| P4 burnup / flux / FGR / SCIANTIX | 0.2–0.4 |
| P5 corrosion and remainder | 0.1–0.3 |
| **Total** | **1.5–3.2** |

Consistent with the 1.5–3 py figure in the parent document. As noted there, these
are conventional-development estimates and are **not** calibrated to this
workspace's AI-assisted throughput; the per-commit `API-Usage` trailers and
`docs/historian/` reports are the local data for deriving a multiplier.

---

## 6. Constraints and obligations

- **Licence provenance.** OFFBEAT is GPL-3.0, bundled SCIANTIX is MIT. Both are
  GPLv3-compatible, but every ported file keeps its upstream attribution header
  (project, source file, version/commit, copyright, licence) — the same pattern
  `boon-lay/src/triso_atops_fork/` already uses for MIT TRISO-ATOPS under GPL-3.0.
  Record the upstream commit hash at port time.
- **Android/Termux.** The port must stay Android-clean. `outram-foam-basic-lib`
  already is (pure Rust, no system BLAS); do not introduce an Android-hostile
  dependency, and check with `cargo check -p <crate> --all-targets --target
  aarch64-linux-android`.
- **Workspace Rust rules.** Enum dispatch rather than trait objects for the model
  registries (constitutive laws, material models, FGR models are all closed sets
  — a natural fit); no `Box<T>`; no lifetime parameters; `Arc` for shared mesh.
- **Do not vendor.** OFFBEAT source stays outside this tree; port from a
  read-only clone kept elsewhere, as with kopitiam.

---

## 7. Open questions

1. **Target crate.** New `outram-park-fork-offbeat`, or a module inside
   `outram-foam-appbuilder-lib` alongside the GeN-Foam port? Leaning separate
   crate — different physics domain, independently publishable.
2. **OpenFOAM version parity.** Which upstream OpenFOAM version OFFBEAT builds
   against, and whether `outram-foam-basic-lib` matches it, needs checking before
   P0.
3. **SCIANTIX: port or FFI?** Porting keeps the pure-Rust/Android property. FFI
   would be faster to stand up but breaks Android-cleanliness. Recommend porting.
4. **TRISO reuse.** How much of P0–P2 (multi-layer spherical shells, contact,
   constitutive laws) transfers to the TRISO mechanical model that would close
   `FailureFractions.incremental`. Suspected high; worth confirming during P2.

---

## 8. Provenance

- OFFBEAT — <https://gitlab.com/foam-for-nuclear/offbeat> (GPL-3.0; LICENSE
  retrieved 2026-07-29; module counts from the GitLab tree API, same date)
- OFFBEAT documentation — <https://foam-for-nuclear.gitlab.io/offbeat/>
- The OFFBEAT multi-dimensional fuel behavior solver, *Nuclear Engineering and
  Design* — <https://www.sciencedirect.com/science/article/abs/pii/S0029549319304479>
- SCIANTIX — <https://github.com/sciantix/sciantix-official> (MIT)
