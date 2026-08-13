# Heat-exchanger temperature-cross fallback (design note)

**Status: DESIGN ONLY — not implemented.** Recorded 2026-08-13 from the
maintainer's proposal. Tracked as `op-3lay`. Nothing in the codebase behaves
this way yet; `htgr_sim_v1` currently avoids the problem by sub-stepping the
steam generator 8x per plant step.

## The problem this solves

A nodalised counter-flow exchanger in this workspace is **three separate
matrix systems** — hot fluid array, tube-metal `SolidColumn`, cold fluid array
— coupled through conductance terms evaluated at the previous iterate. That
coupling is **Lie-split (explicit)** even when each array solves its own energy
equation implicitly.

Making convection implicit *within* an array (`EnergyBalanceMode::Implicit`,
landed 2026-08-13) does nothing for this. It removes the *intra*-array Courant
limit; the *inter*-array coupling lag is untouched.

Consequence, measured 2026-08-13 on `htgr_sim_v1`'s steam generator with the
helium side already implicit:

| SG substeps / plant step | `Co_hot` | real-time ratio | outcome |
|---|---|---|---|
| 8 | 0.222 | 1.027 | ok |
| 4 | 0.444 | 2.051 | ok |
| 2 | 0.888 | 4.101 | ok |
| 1 | 1.776 | — | **temperature cross** |

The 1-substep failure is the second-law assertion at
`examples/htgr_sim_v1/physics/mod.rs:946`
(`worst_node_cross_kelvin() <= 1e-6`), **not** a divergence of the helium
array. At one exchange per 0.1 s plant step the counter-flow coupling cannot
resolve and the cold stream overtakes the hot.

**This is expected behaviour of an explicitly-coupled exchanger at a coarse
coupling step, not a bug.** A fully implicit (monolithic) solve spanning all
three arrays would remove it, at the cost of abandoning three cheap
tridiagonal solves for one large coupled system. The maintainer has explicitly
rejected that trade.

## The proposed fallback — a three-tier escalation

The governing idea, in the maintainer's words: **adopt the steady-state
temperature profile if a temperature cross is observed.** Thermodynamics is
then not violated, even though the transient differs from a fully-resolved
one. Degrade to a physically admissible state rather than crash or emit
nonsense.

### Tier 0 — normal operation

Three arrays, Lie-split coupling, as today. No cross detected, nothing
happens. This must remain the overwhelmingly common path.

### Tier 1 — direct array-to-array coupling — **REMOVED 2026-08-13**

> **Maintainer decision: "eliminate metal is eliminated."** Implemented,
> verified, and then removed. Kept here as the record of why, and because the
> idea is still viable in a different form (see below).

On detecting a cross, couple the hot and cold arrays **directly** through an
appropriate series thermal resistance, bypassing the metal as a lagged
intermediary. The tube metal is then **set to the temperature implied at that
steady state** rather than integrated.

This is the Schur-complement idea in concrete form: the metal has no
advection, so it is a per-node capacitance with two conductances and can be
eliminated analytically. Eliminating it removes one full lag from the coupling
loop, which in a counter-flow arrangement is where the cross originates.

**What was built and what it measured.** The algebra
`T_m* = (G_h·T_h + G_c·T_c)/(G_h + G_c)`, `1/G_series = 1/G_h + 1/G_c` was
verified three ways: against a numerically integrated metal ODE to
**5.684e-11 K**, both-sides equality to **1 ulp**, and the dropped axial
conduction measured at `G_axial/(G_h+G_c)` = **1.704e-6** (safe at 8 nodes, not
at a much finer mesh).

**Why it was removed anyway, on two independent grounds:**

1. **It could not clear a cross, structurally.** It rewrites only `T_m`, and is
   invoked only on an already-crossed state, so it returned "did not converge"
   every time. Eliminating the metal changes *how the next step is integrated*;
   a post-hoc state repair cannot undo a lag already taken.
2. **Its transient cost was the largest of the three.** Setting the metal rather
   than integrating it collapses `tau_exchanger` from **39.78 s** to zero and
   `tau_node` from **7.46 s** to zero, delivering **31.78 MJ** early on a 100 K
   hot-inlet step. Not a different transient — a plant whose steam generator has
   no tube mass, which is the isothermal-sink behaviour the nodalisation exists
   to replace.

**The idea is not dead, but it is not a remedy.** As an *integration-time*
change — replacing the four metal links with one direct hot-to-cold link at
`G_series`, in `steam_generator.rs` — it would remove a real lag from the
coupling loop. That is separate work, tracked in `op-3lay`.

### Tier 2 — impose an LMTD profile

If a cross is *still* observed after Tier 1, compute the steady-state outlet
temperatures from the **log-mean temperature difference**, and adjust the
temperature profiles in both arrays to that LMTD-consistent profile.

### Tier 3 — implicit

Whatever the tier, the invariant to preserve is: **the state handed to the
next timestep must not contain a temperature cross.**

## Outcome (2026-08-13)

Implemented as `physics/temperature_cross/`, with the selector
`TemperatureCrossRemedy { None, Bedok, Lmtd }`. **Default `None`** — the shipped
2 sub-steps produce no cross, so nothing engages in normal operation.

**Order of preference, per maintainer decision:**

1. **`Bedok`** — the corrector of choice. The only method verified against an
   analytic reference (closed-form ε-NTU: hot outlet within 0.135 K / 0.086% of
   span, duty within 0.086%), the only one that handles the boiling transition
   without zoning, and the cheapest at **~425 µs** per repair — 3.4% of one
   exchanger sub-step, which answers concern 6 below.
2. **`Lmtd`** — last resort, and carrying a flagged defect: the steady state it
   imposes is not the plant's operating point (**−73.8 K** on steam outlet).
   See the variant docs and its own bead.

`EliminateMetal` was removed — see Tier 1 above.

## Concerns to resolve before implementing

These are not objections to the approach — the heuristic is reasonable and the
maintainer has already accepted the transient-fidelity trade explicitly. They
are the parts that need a decision or they will bite later.

1. **LMTD is not valid across a phase change, which is exactly where this
   exchanger lives.** The cold side is feedwater → steam through IF97, and
   LMTD assumes constant specific heats, constant `U`, and no phase
   transition. A steam generator violates all three across the boiling
   boundary. Standard practice is to **zone** the exchanger (subcooled /
   two-phase / superheat) and apply LMTD per zone with its own `U`. A
   single-zone LMTD across a boiling transition will produce a confidently
   wrong profile. This is the single biggest technical risk in Tier 2.

2. **Snapping to steady state discards the transient, which is what a digital
   twin is for.** The cross will occur *during* transients, not at steady
   state — so the fallback fires precisely when the interesting dynamics are
   happening. It must be **loud**: counted, surfaced in the GUI, and available
   to tests. The precedent is `hot_enthalpy_clamp_events`, which is currently
   zero everywhere and asserted `== 0` in three tests. Do the same here.

3. **Energy conservation takes a discontinuous hit.** Overwriting both arrays'
   temperature profiles changes their stored energy instantaneously. The
   second law is respected pointwise, but the **first law is violated across
   the jump** unless the discarded energy is accounted for. At minimum,
   compute and log the energy discrepancy per event; a running total makes it
   auditable rather than invisible.

4. **It can mask a genuine bug.** If a cross arises from a coding error rather
   than a coarse coupling step, this heuristic converts a loud failure into a
   quiet wrong answer. The event counter is the mitigation — a cross at a
   substep count that *should* be fine is a bug signal, not a fallback
   trigger.

5. **Chatter.** Alternating between "crossed → snap to steady" and "fine →
   integrate transient" can limit-cycle. Consider hysteresis, or holding the
   fallback for a minimum dwell once engaged.

6. **The real-time budget is the reason this exists, so it must be measured.**
   The maintainer's hope is "all these steps can be done with sufficient time
   to spare". Tier 1 is cheap. Tier 2 with zoning is not free, and it fires
   during transients — i.e. when the plant is already working hardest. If the
   fallback costs more than the substeps it saves, it is a net loss. Measure
   the cost of a fallback event before adopting it.

## V&V framing — mandatory

Per `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`: this is a
**heuristic that deliberately trades transient fidelity for thermodynamic
admissibility.** Any results produced while the fallback is engaged are **not**
a resolved transient and must never be presented as one. If this ships:

- the doc comment on every affected public item must say so;
- any V&V case that engages the fallback must record how many times it fired;
- a validation claim against a measured transient is **void** for any run in
  which the fallback engaged, and the report must state it.

## Alternatives deliberately not taken

- **Monolithic implicit solve across all three arrays.** Removes the problem
  at the root. Rejected by the maintainer: it would replace three cheap
  tridiagonal solves with one large coupled matrix.
- **More inter-array outer correctors.** *Not yet tried* — deferred on the
  maintainer's instruction, 2026-08-13. Worth noting the theory is favourable
  here in a way it is not for advection: the inter-array Picard iteration's
  contraction factor is governed by roughly `UA·dt/(m·c_p)`, **not** by the
  cell Courant number, so unlike the advective corrector loop it can genuinely
  converge. Cheaper than either tier above if it works.
- **Keeping 8 substeps.** Always available, costs the real-time buffer
  (1.027 against 4.101 at 2 substeps).
