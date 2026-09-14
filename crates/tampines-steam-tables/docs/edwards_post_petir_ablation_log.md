# Edwards blowdown, post-PETIR — ablation log

**Status: OPEN.** This is a running log of a debugging campaign, not a
write-up of a finished result. Entries are appended in the order they were
run, and **failed attempts are recorded as prominently as successful ones** —
a hypothesis that was measured and refuted is the main product of an ablation,
and deleting it means the next reader pays for it again.

**Companion documents.** The ground-up explanation of this solver and of the
three faults fixed before this campaign is
[`edwards_blowdown_solver_debugging.md`](./edwards_blowdown_solver_debugging.md);
read that first if the terms here (flashing plateau, `rho_cont`, the all-Mach
hybrid) are unfamiliar. Beads: `op-bgg0` (this campaign), `op-s2dc` (the
defect), `op-ppmk` (the evidence-standard finding that came out of it).

---

## 0. What started this

The Edwards–O'Brien blowdown case stopped passing. The regression bisects to a
single commit:

| commit | | result |
|---|---|---|
| `09787761` — *petir: lift the RealMath trait into the crate* | pre-sweep | **passed**, 161.79 s |
| `baf9408b` — *route exp/ln/powf through PETIR's ARM port* | post-sweep | **FAILED**, panic at 66.94 s |

Test: `edwards_hybrid_damps_ringing_vs_pimple`, release mode, back-to-back on
one machine. The panic is

```
panicked at ph_flash_eqm/mod.rs:865: p,h point below 273.15K
  ph_flash_region <- t_ph_eqm <- correct_thermo <- step
```

with the instrumented state (recorded in `op-s2dc`)

```
OUT-OF-DOMAIN cell 22: p = 4.202124e5 Pa, h = -1.712179e10 J/kg,
                       h_floor(273.15K) = 3.857666e2, rho_prev = 1.000000e-4
```

`baf9408b` is a mechanical sweep of 173 transcendental call sites onto PETIR's
ARM optimized-routines ports — a change of about one ulp per call.

**The perturbation is not the defect, and reverting it is not the fix.** A V&V
case whose outcome flips on the last bit of `exp()` is not a case anyone should
be citing; the 1 ulp merely tipped over something that was already marginal.
What this campaign is looking for is the thing that was marginal.

(The transferable lesson — that "1009 lib tests unchanged and the V&V fixtures
regenerate character-identical" is the wrong acceptance check for a ~10⁹-step
chained solve — is `op-ppmk`, and is not re-argued here.)

---

## 1. Leading hypothesis, from reading the assembly

Not a guess from the physics: this is what the energy equation actually builds.
`step()` assembles

```rust
let rho_cont = {
    let mut rc = rho_old.clone() + (-dt) * div_phi_final;
    for c in 0..n {
        if rc.internal[c] < 1e-4 { rc.internal[c] = 1e-4; }   // <-- the clamp
    }
    rc
};
let mut e_eqn = fvm::ddt_coeff_old(&rho_cont, &rho_old, &self.he, &he_old, dt)
    + fvm::laplacian(&alpha_h_f, &self.he);
```

and `ddt_coeff_old` is

```rust
mat.ldu.diag[c] += coeff_new.internal[c] * v_dt;              // rho_cont * V/dt
mat.source[c]   += coeff_old.internal[c] * phi_old.internal[c] * v_dt;
```

The module's own comment states the reason `rho_cont` exists at all: discrete
continuity

$$\frac{\rho_{cont} - \rho_{old}}{\Delta t} = -\nabla\cdot\phi$$

holds **exactly**, so the term $h_{old}(\rho_{cont}-\rho_{old})/\Delta t =
-h_{old}\nabla\cdot\phi$ cancels the $h\,\nabla\cdot\phi$ part of
$\nabla\cdot(\phi h)$ term for term. That cancellation is what reduces the
equation to the material derivative $\rho\,Dh/Dt = dp/dt$ and produces the
flashing plateau.

**The clamp breaks that identity.** The moment it fires, $\rho_{cont} \neq
\rho_{old} - \Delta t\,\nabla\cdot\phi$, the cancellation is no longer exact,
and an uncancelled $h\,\nabla\cdot\phi$ of perfectly ordinary magnitude is left
in the source — divided by a diagonal that has been pinned at $10^{-4}V/\Delta
t$. Enthalpy of order $-10^{10}$ J/kg is exactly what that arithmetic produces,
and it explains why the blow-up is at the break cell and nowhere else.

**Predicted sign and magnitude, stated before measuring** (per the workspace
rule): the defect should appear only in cells that reach the floor; it should
appear first at the cell nearest the break; and the failure should be
*insensitive* to the KNP path, because nothing in the argument involves the
Mach blend.

**Candidate fix.** Not moving the floor. A cell holding no mass has no
meaningful specific enthalpy, so none should be solved for: give it an identity
row (`diag = 1`, `source = he_old`) and hold `he`, rather than dividing an
ordinary source by a floored diagonal.

---

## 2. Ablation ledger

Every run, in order. `t_end` is the simulated window, not wall clock.

| # | Configuration | Result | Notes |
|---|---|---|---|
| A0a | `09787761`, default build, `edwards_hybrid_damps_ringing_vs_pimple` (0.15 s) | **PASS** 161.79 s | pre-sweep reference |
| A0b | `baf9408b`, default build, same test | **FAIL** 66.94 s | post-sweep; panic as above |
| A0c | `09787761`, default build, `edwards_obrien_pipe_blowdown_600ms` | **PASS** 346.17 s | full transient; golden reference captured |
| A0d | `892c3898` + `--features platform-libm`, same 600 ms test | **PASS** 346.06 s | reproduces A0c byte-for-byte in all three CSVs |
| A1 | `892c3898`, default build, same 600 ms test | **FAIL** 62.59 s | pure PIMPLE; dies at t ~ 0.117-0.120 s |

Entries from A1 onward are this campaign's own and are appended below as they
are run.

---

## A1 — baseline: current develop, default build, pure PIMPLE, 600 ms

**Configuration.** `892c3898`, default features (PETIR ARM route),
`edwards_obrien_pipe_blowdown_600ms`, `EDW_DBG=1`. No code change.

**Result: FAIL at 62.59 s wall.** Same panic as A0b.

```
panicked at ph_flash_eqm/mod.rs:875: p,h point below 273.15K
  ph_flash_region <- t_ph_eqm <- correct_thermo <- step
```

**When.** Between step 3900 and step 4000 — simulated **t ≈ 0.117–0.120 s**.
The last two traced steps:

```
step  3800 t=0.1140 | break: p=2692kPa T=499.2K a=0.000 | GS-1: p=390psia ... mdot=58.25 u=16.72
step  3900 t=0.1170 | break: p=2656kPa T=499.5K a=0.000 | GS-1: p=385psia ... mdot=58.49 u=16.80
```

### What this confirms

**Pure PIMPLE fails.** The 600 ms test uses `SolverMode::Pimple` unless
`EDW_HYBRID=1` is set (`edwards_blowdown.rs:356`), and it is not set here. So
the failure is reachable with the KNP path entirely out of circuit.

**That refutes Mach gating as a candidate for this failure, before spending a
run on it.** Shifting `HYBRID_RHO_TAPER_LO`/`HI` or the blend window cannot fix
something that happens with the blend switched off. Recorded here so the branch
is not re-opened later: it is not that retuning the taper was tried and did not
help — it is that the taper is provably not in the code path.

### What this UNDERMINES — read before trusting section 1

The trace is not what the hypothesis predicts, and this is worth being blunt
about rather than quietly moving on.

`op-s2dc` records the failing cell as *drained to the density floor*
(`rho_prev = 1.000000e-4`). But right up to the last traced step the break cell
is reported at **`a = 0.000` — no void at all** — with temperature *rising*
(498.3 K at t = 0.081 s to 499.5 K at t = 0.117 s) and break pressure falling
through 2656 kPa. A cell at zero void fraction is liquid, and liquid is not a
cell that has drained to `1e-4` kg/m³.

Three readings, not yet distinguished:

1. The trace's "break" station and the cell that panics (`cell 22` in
   `op-s2dc`) are **different cells**, and the drained one is simply not the one
   being printed.
2. The blow-up is **fast** — the cell goes from liquid to floored inside the
   100 steps between traces, so no trace ever shows the intermediate state.
3. The `op-s2dc` diagnosis is **incomplete or wrong**, and the floor is a
   consequence rather than the cause.

Note also that 2656 kPa at 499.5 K is essentially `p_sat(T)`: the failure
happens exactly as the break cell reaches flashing, which is the most violent
moment in the transient and the one the whole `rho_cont` cancellation exists to
handle.

**Next (A2) is therefore instrumentation, not a fix.** Print, at the moment of
failure: the cell index, `rho`, `he`, `p`, `rho_old`, the unclamped
`rho_old - dt*div_phi`, and whether the clamp fired — and the same for its
neighbours. Guessing at a fix before that would be building on a diagnosis the
trace has just called into question.
