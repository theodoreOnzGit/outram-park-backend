# `rhoPimpleFoam` port — register of omitted upstream terms

**Purpose.** This port is a translation of OpenFOAM's `rhoPimpleFoam`. In a
translation, the overwhelmingly likely cause of a discrepancy is that **upstream
does something the port does not** — so every term, equation, or solver stage
that upstream has and this port lacks must be written down **with its reason**.

**Why this file exists.** It was opened on 2026-09-14 after `fvc::ddt_corr` was
found to have been ported in full — with OpenFOAM's `fvcDdtPhiCoeff` limiter, a
faithful translation of `EulerDdtScheme::fvcDdtPhiCorr`, and a doc comment
stating that it "is what suppresses pressure–velocity (checkerboard)
decoupling" — and then **never called from anywhere**. Nobody noticed, because
there was no list to notice against. That omission is the direct cause of the
Edwards blowdown failure under investigation in
[`edwards_post_petir_ablation_log.md`](./edwards_post_petir_ablation_log.md)
(`bn:op-bgg0`).

**The rule this file enforces.** *Taking a term out is allowed. Taking it out
silently is not.* Every row below must carry a reason, and a reason must be one
of:

- **Deliberate — scope.** Upstream supports a case this port does not model
  (moving meshes, multiple reference frames). Safe, permanent.
- **Deliberate — physics.** The term is genuinely zero or negligible for this
  configuration, *with the argument stated*.
- **Deferred.** Wanted, not yet done, tracked by a bead.
- **UNEXPLAINED.** Present upstream, absent here, no recorded reason. **These
  are defects until shown otherwise** — an unexplained omission is precisely
  what `ddt_corr` was.

Prefer physically correct fixes over numerical band-aids. Where a band-aid is in
place (a clamp, a floor, a hold), it belongs in the "Band-aids" section below
with the physical defect it is masking named explicitly, so it can be removed
once that defect is fixed.

---

## Register

| # | Upstream term / stage | Status in port | Class | Reason |
|---|---|---|---|---|
| 1 | `rhorAUf*fvc::ddtCorr(rho, U, phi)` added to `phiHbyA` | **RESOLVED 2026-09-14** — wired in | — | Was absent with zero call sites, no reason recorded. Restoring it fixed the Edwards 30 µs failure outright AND improved agreement with experiment: GS-1 RMSE 58.6 → 42.8 psia, flashing plateau 392.7 → 359.0 psia (into the measured 350–367 band). See `bn:op-bgg0`, log A6. |
| 2 | `constrainHbyA` / `adjustPhi` | **ABSENT** (0 hits) | **UNEXPLAINED** | None recorded. Upstream uses these to make `HbyA` respect velocity BCs and to render the flux globally conservative on a closed domain. This port does hand-write a boundary flux write-back for `FixedValue` velocity patches (with a regression test), which may cover part of the same ground — but that equivalence is **asserted here as a question, not a finding**. |
| 3 | `EEqn` solved **before** the pressure-corrector loop | **REORDERED** — solved *after* the inner loop | Deliberate — documented | The module header states the ordering explicitly. The `rho_cont` construction depends on the final `self.phi` of the corrector loop, which only exists after it. Recorded as a divergence so a future reader does not "fix" it back. |
| 4 | `pcEqn.H` (consistent / SIMPLEC path, `pimple.consistent()`) | **ABSENT** (comment only) | **UNEXPLAINED** | None recorded. The port always takes the `pEqn` branch. Probably harmless at `alpha_p = 1`, but unstated. |
| 5 | `turbulence->correct()`, `divDevRhoReff` in `UEqn` | **ABSENT** (comment only) | Deliberate — physics *(inferred, NOT stated in code)* | A 1-D area-averaged pipe model has no resolved turbulence. **But no wall-friction closure appears either** (0 hits for friction/laminar), and an adiabatic blowdown pipe certainly has wall shear. Flagged: this may be a genuine missing physical term, not a scope exclusion. |
| 6 | `MRF` (multiple reference frames) | ABSENT (comment only) | Deliberate — scope | Static 1-D mesh; no rotating frame exists. Safe. |
| 7 | Mesh motion (`mesh.controlledUpdate()`, `correctPhi`, `meshCourantNo`) | ABSENT (comment only) | Deliberate — scope | Fixed mesh. Safe. |
| 8 | `fvOptions` / `fvModels` sources | ABSENT (comment only) | Deliberate — scope | Replaced by this port's explicit lateral-conductance and volumetric-heat-source terms in the `EEqn`. |
| 9 | `rhoEqn.H` on the first outer iteration | **PRESENT** | — | `self.rho = rho_old - dt*div(phi)` at the top of the outer loop. |
| 10 | Rhie–Chow *static* part (`phi = phiHbyA - rhorAUf*snGrad(p)*magSf`, same `rhorAUf` in the `pEqn` Laplacian) | **PRESENT** | — | Verified 2026-09-14. It is only the **transient** half (row 1) that is missing. |

**Row 1 is RESOLVED** (2026-09-14). **Three rows remain UNEXPLAINED or flagged (2, 4, 5).**
Rows 2, 4 and 5 need a maintainer decision or an investigation, and row 5 in
particular should not be waved through as "1-D has no turbulence" when the
question is really "where is wall friction?".

---

## Band-aids currently in place

Each of these suppresses a *symptom*. Each should be removable once the
physical defect named beside it is fixed — and each should be **loud**, not
silent, until then.

| Band-aid | Where | Masks | Status |
|---|---|---|---|
| `pressureControl::limit` — clamp `p` into `[p_min, p_max]` | after the `pEqn` solve | Pressure outside the EOS range. Was: cell 21 solving to **−27.2 kPa**, clamped to 611.8 Pa **silently**. | **KEPT and now LOUD** — one-shot warning naming the cell and the numbers, plus `pressure_bound_events()` / `pressure_bound_worst_undershoot()`. Still fires **150 times** after row 1 was fixed (worst undershoot 171.75 kPa), so a real defect remains; leading candidate is the `psi` linearisation window. Should become a hard gate once that is closed. |
| Density floor `rho.max(1e-4)` | `correct_thermo` | A cell being drained of more mass than it holds. Measured: `rho_old - dt*div(phi) = -1.643`, i.e. **negative**, not merely small. | Masks the over-drain. Cannot be fixed by moving the floor: the quantity being floored is negative. |
| `rho_cont` clamp `rc.max(1e-4)` | energy equation | Same over-drain, and it **breaks the exact discrete-continuity identity** that makes `h_old·(rho_cont − rho_old)/dt` cancel `h·div(phi)`. | This is the one that produced `he = -1.7e10 J/kg`. |
| Drained-cell enthalpy hold (identity row) | energy equation | The division above. | Added 2026-09-14 (log A3). **KEPT and now INERT.** With row 1 fixed it engages **0 times** across the full 600 ms transient (log A7) — the over-drain it masks does not occur any more. Retained per maintainer direction as a guard against recurrence, now loud + counted (`drained_hold_events()`). Its zero count is the evidence it is not currently papering over anything. |

---

## How to use this file

- **Adding a term back?** Move its row to a "Resolved" note with the date, the
  commit, and the measured effect.
- **Taking a term out?** Add a row *in the same change*, with the class and the
  reason. A commit that removes an upstream term without a row here should be
  rejected in review.
- **Found an omission?** Add it as **UNEXPLAINED** rather than guessing a
  reason. "I do not know why this is missing" is a valid and useful entry; an
  invented justification is not.
