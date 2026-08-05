# Port scoping assessment — Modified UNIFAC (NIST)

> **Status: SCOPING ASSESSMENT, not a validated design.** This document is a
> research/writing deliverable for bead `op-qo2.25`. It surveys the remaining
> UNIFAC-family property package in DWSIM (NIST-Modified UNIFAC) and estimates
> the cost of a Rust port. **No Rust code was written or modified.** Any code
> written from this scope would still be untrusted AI-assisted draft material
> requiring human verification and validation per `RESPONSIBLE_USE.md`.
>
> Reference source read: DWSIM (GPL-3.0), gitignored clone under
> `upstream_source/DWSIM/`, commit `1abf72d`.

## 1. What NIST-Modified UNIFAC is

**Modified UNIFAC (NIST)** is a re-fit and extension of **Modified UNIFAC
(Dortmund)** — same model family, same functional form, a different (larger,
more recently fitted) published parameter set. It is the NIST TRC group's
critically-evaluated re-parameterisation, published as:

> Kang, Diky & Frenkel, *"New modified UNIFAC parameters using critically
> evaluated phase equilibrium data"*, **Fluid Phase Equilibria 388** (2015)
> 128–141, <https://doi.org/10.1016/j.fluid.2014.12.042>.

DWSIM's own model documentation string (`Models/NISTMFAC.vb:181-196`) states it
plainly: *"This model is similar to the Modified UNIFAC (Dortmund), with new
modified UNIFAC parameters reported for 89 main groups and 984 group–group
interactions using critically evaluated phase equilibrium data including VLE,
LLE, SLE, excess enthalpy (HE), infinite dilution activity coefficient (AINF)
and excess heat capacity (CPE) data."*

**Functional form vs. Dortmund — identical.** Reading DWSIM's two model files
side by side, `Models/NISTMFAC.vb` and `Models/MODFAC.vb` implement the *same*
equations. Every routine matches line-for-line in structure:

| Quantity | NIST `NISTMFAC.vb` | Dortmund `MODFAC.vb` | Same? |
|---|---|---|---|
| Molecular volume `r_i = Σ_k ν_k^i R_k` (`RET_Ri`) | 403-415 | 398-410 | identical |
| Molecular area `q_i = Σ_k ν_k^i Q_k` (`RET_Qi`) | 417-429 | 412-424 | identical |
| Group area fraction `e_ki` (`RET_EKI`) | 431-443 | 426-438 | identical |
| Interaction `a_mn(T) = a + bT + cT²`, `τ = exp(−a/T)` (`TAU`) | 363-401 | 358-396 | identical form |
| Modified combinatorial `ln γ_i^C` (3/4-power volume fraction) | 291 | 286 | identical |
| Residual `ln γ_i^R` (compact `β/θ/s`) | 296 | 291-296 | identical |
| Assembly `γ_i = exp(ln γ^C + ln γ^R)` | 302 | 297 | identical |
| Excess `HEX_MIX` / `CPEX_MIX` / `DLNGAMMA_DT` | 475-521 | 461-507 | identical |

Both use:

- the **modified (Dortmund) combinatorial term** — Flory–Huggins volume
  fraction raised to the `3/4` power, Staverman–Guggenheim correction
  `1 − J_i/L_i + ln(J_i/L_i)` with `z/2 = 5`;
- **temperature-dependent group interactions** `a_mn(T) = a + bT + cT²`
  entering `τ_mn = exp(−a_mn(T)/T)`;
- fitted (non-Bondi) `R_k` / `Q_k`.

The only structural differences are **data-ingestion details, not algebra**:

1. **Compound-group fallback.** NIST reads a molecule's group counts from
   `NISTMODFACGroups` if present, else falls back to the Dortmund `MODFACGroups`
   assignment (`NISTMFAC.vb:454-468`, and the property-package wrapper
   `PropertyPackages/NISTMFAC.vb:124-132`). Dortmund only reads `MODFACGroups`.
2. **Single directional interaction map.** NIST stores one directional
   `InteracParam_aij/bij/cij` triple (`NISTMFAC.vb:546-548`) and looks up
   `(g1,g2)` then `(g2,g1)`. Dortmund carries a second `aji/bji/cji` mirror set.
   The NIST layout is in fact a *closer* match to the Rust port's existing
   single-map `ModfacParameters` than Dortmund's own is (see §3).
3. **`c` coefficient is stored ×1000.** The NIST IP file column is labelled
   `1000anm,3`; the loader divides by 1000 on read (`NISTMFAC.vb:600, 605, 609`).
   A pure data-scaling step at table-build time.
4. **Asset file format** — tab-delimited with a two-line header and
   `(N) Main Group Name` section markers (see §2), versus Dortmund's
   `;`-delimited `modfac.txt` and space-delimited `modfac_ip.txt`.

**Bottom line for scoping:** a NIST port is **≈ a new parameter table plus a
new enum arm** on top of the already-ported Dortmund algebra. No new equations.

## 2. Parameter tables

Two tab-delimited GPL-3.0 asset files under
`upstream_source/DWSIM/DWSIM.Thermodynamics/Assets/`:

### `NIST-MODFAC_RiQi.txt` — subgroup R/Q + main-group assignment (13.5 KB)

- 291 lines total: a 2-line header, then **89 main-group section markers** of
  the form `(N) Main Group Name`, interleaved with **202 subgroup rows**.
- Subgroup row columns: `No.` (subgroup id), `Sub-group Name`, `Ri`, `Qi`,
  `Example`, and an example decomposition. Example first data rows:

  | No. | Sub-group | Ri | Qi | (main group) |
  |---|---|---|---|---|
  | 1 | CH3 | 0.6325 | 1.0608 | (1) CH2 |
  | 2 | CH2 | 0.6325 | 0.7081 | (1) CH2 |
  | 5 | CH2=CH | 1.2832 | 1.6016 | (2) C=C |

- The loader (`NISTMFAC.vb:567-585`) tracks the current main group from the
  `(N)` marker line and assigns every following subgroup to it.

### `NIST-MODFAC_IP.txt` — main-group interaction matrix (68 KB)

- 1970 lines: a 1-line header (`Main Group n  Main Group m  anm,1  anm,2
  1000anm,3  Tmin  Tmax`), then **1969 directional interaction rows**.
- Columns: source main group `n`, target main group `m`, then
  `a` (K), `b`, `1000·c`, and a validity window `Tmin`, `Tmax` (K).
- Directional: each unordered main-group pair appears as two rows (`n→m` and
  `m→n`). 1969 directional rows ≈ **984 unordered pairs** stored both
  directions — matching DWSIM's stated "984 group–group interactions". **79**
  distinct main groups appear as a source (of 89 defined; the remainder appear
  only as targets or have no fitted interactions).
- Note the extra `Tmin`/`Tmax` validity columns, which the Dortmund
  `modfac_ip.txt` does **not** carry. DWSIM ignores them at load time, but they
  document each pair's fitted temperature range and are worth preserving.

### Size contrast with the already-ported Dortmund tables

| File | NIST | Dortmund (`modfac*.txt`) |
|---|---|---|
| Subgroup R/Q table | 202 subgroups / 89 main groups (13.5 KB) | ~108 rows (6.6 KB) |
| Interaction matrix | 1969 directional rows / ~984 pairs (68 KB) | 1179 rows (44.6 KB) |

The NIST set is roughly **1.5–2× larger** than the Dortmund asset already in the
tree.

### Data provenance / licensing

- **Underlying parameters:** publicly-published NIST TRC data (Kang, Diky &
  Frenkel 2015, DOI above) — open literature.
- **As shipped here:** DWSIM's `Assets/NIST-MODFAC_IP.txt` and
  `NIST-MODFAC_RiQi.txt`, both **GPL-3.0** (DWSIM is GPL-3.0; headers on
  `NISTMFAC.vb` confirm). GPL-3.0-compatible with this workspace's default
  license.
- **Verdict under `DATA_POLICY.md`:** permitted. Same footing as the
  already-ported Dortmund (`unifac_dortmund.rs`) and LLE (`unifac_lle.rs`)
  tables — open, published, properly-cited literature data carried as GPL
  assets. No NUS-restricted, proprietary, or operational data involved. Any
  bundled table must carry a provenance header citing the 2015 paper and the
  DWSIM asset origin, exactly as the Dortmund/LLE ports already do.

## 3. Reuse assessment

**The whole algebra of `src/thermo/unifac_dortmund.rs` is reusable as-is.**
Because NIST and Dortmund share an identical functional form (§1), the existing
Dortmund module already provides every routine a NIST port needs:

- `molecular_r_q` — `r_i` / `q_i`;
- `ln_gamma_combinatorial` — the 3/4-power modified combinatorial;
- `group_ln_gamma` / `ln_gamma_residual` — the temperature-dependent residual;
- `activity_coefficients` — the `exp(ln γ^C + ln γ^R)` assembly;
- `ModfacParameters` (subgroup map + **single directional** `(m,n)`
  interaction map with `a + bT + cT²`), `ModfacSubgroup`, `ModfacInteraction`,
  `ModfacComponent`.

Crucially, `ModfacParameters` already stores interactions in a **single
directional map** — which is precisely NIST's on-disk layout (§1 item 2), so the
NIST data drops in without even the structural mismatch the Dortmund loader has.

**Recommended pattern — identical to how `unifac_lle.rs` reused `unifac.rs`.**
`unifac_lle` adds no new algebra: it `use super::unifac::{...}`, supplies a
different parameter table (`magnussen_lle_subset`), adds an enum arm
(`UnifacLleTable`), and forwards to the base `activity_coefficients`. Nothing in
`unifac.rs` is modified. A NIST port should mirror this exactly:

- a new `unifac_nist.rs` that `use super::unifac_dortmund::{activity_coefficients,
  ln_gamma_combinatorial, ln_gamma_residual, molecular_r_q, ModfacParameters,
  ModfacComponent, ModfacSubgroup}`;
- a table builder (e.g. `nist_modfac_subset()` or a full-table loader) returning
  a `ModfacParameters` populated with NIST R/Q + `a/b/c` (remembering the ÷1000
  on `c`);
- a `NistModfacTable` enum (or a new `ModfacTable::NistModfac` arm) for dispatch,
  per the workspace no-`dyn` rule;
- forwarding wrapper fns, mirroring `unifac_lle`'s `*_lle` functions.

**Reuse fraction: ≈ 100% of the algebra; ~0% new math.** New code is essentially
the parameter table plus thin forwarding wrappers and tests. The `unifac_lle.rs`
port that did the same thing for the LLE table is ~490 lines, most of which is
the parameter subset and its verification tests — a realistic size envelope for
a subset-scoped NIST port too.

One caveat if the **full** NIST matrix is transcribed (rather than a small
literature subset): 202 subgroups + ~1969 interaction rows is a large data
transcription. It should be **script-generated** from the two asset files (a
deterministic parse into Rust `const` arrays or a bundled asset), not
hand-typed, and then spot-verified against the source rows. This is the bulk of
the effort; the Dortmund and LLE ports both sidestepped it by bundling a small
public-literature subset sufficient for their tests.

## 4. Effort estimate + recommendation

### Effort: **Small (subset) to Medium (full table)**

| Component | Effort | Notes |
|---|---|---|
| Model algebra | ~none | Reuse `unifac_dortmund.rs` verbatim (§3). |
| Enum arm + forwarding wrappers | Small | Copy the `unifac_lle.rs` shape. |
| Parameter table — **small subset** (a few groups, like Dortmund/LLE) | Small | Hand-pick ~4–8 subgroups + a handful of pairs; ≈ 1 focused session. |
| Parameter table — **full** (202 subgroups, ~1969 rows) | Medium | Must be script-generated from the two asset files + spot-verified; the dominant cost, plus the ÷1000 `c`-scaling and `Tmin`/`Tmax` handling. |
| Verification tests | Small | Same recipe as Dortmund: pure-component `γ = 1`, identical-molecule ideality, an independent second-implementation cross-check. No experimental validation is claimed. |

### Relevance to OUTRAM PARK

**★ Low relevance.** NIST-Modified UNIFAC is a general chemical-engineering
VLE/LLE activity-coefficient model for organic mixtures. OUTRAM PARK's domain is
reactor physics and reactor thermal-hydraulics; liquid-phase organic-mixture
activity coefficients are not on the critical path for the neutronics / TH /
coolant-property work. It is a completeness item in the DWSIM thermodynamics
tier, not a reactor-chemistry need. It sits at the same low-priority tier as the
already-ported Dortmund and LLE UNIFAC variants.

### Recommendation: **PORT LATER (defer) — low priority, cheap when it lands**

- **Do not port now.** No current OUTRAM PARK workstream needs it, and active
  effort is better spent on the reactor-relevant tiers.
- **When it is picked up, it is cheap** *if* scoped as a **small
  public-literature subset** reusing the Dortmund algebra — the `unifac_lle`
  precedent shows the pattern is a well-trodden ~half-day of work for a subset.
  Only escalate to the full-table transcription if a concrete downstream
  consumer needs broad group coverage, in which case script the asset parse.
- **Provenance discipline:** whichever scope, cite Kang, Diky & Frenkel (2015)
  and the DWSIM GPL asset origin in the module header, exactly as the Dortmund
  and LLE ports do, and keep the "untrusted AI-assisted draft, verification not
  validation" framing until a human signs off both bookkeeping axes.

This completes the UNIFAC family: **UNIFAC (VLE)**, **UNIFAC-LLE**, and
**Modified UNIFAC (Dortmund)** are ported; **Modified UNIFAC (NIST)** is the only
remaining member, scoped here.

## 5. Out of scope — Wilson (flagged separately)

The other remaining `★` low-relevance activity-model package in DWSIM,
**Wilson**, is **not a UNIFAC method** and is **out of scope for this document**.
Wilson is a two-parameter *molecular* (not group-contribution) local-composition
activity model: it has no group decomposition, no `R_k`/`Q_k` surface/volume
parameters, and no group-interaction matrix. Its energy parameters `Λ_ij` are
fitted **per binary pair of components**, not per functional group. It shares
none of the UNIFAC infrastructure reused above (`unifac.rs` /
`unifac_dortmund.rs`), so it cannot piggyback on the Dortmund algebra and needs
its own separate scoping. Tracked as a distinct item; not assessed here.
