# Results (gh:#1407, 2026-11-13)

Measured with `cargo test --release -p outram-park-fork-thermochimica --lib --tests`,
Thermochimica `0c35c8d`, gfortran build, same ChemSage `.dat` files from upstream `data/`.
Relative differences are `|port - upstream| / |upstream|`; "major" = species with at least
1e-3 mol upstream. Port options: rung 0 unless noted (see README).

## The defect found and fixed first

Before any number could be compared, **the port solved 0 of the 8 ideal cases**: a three-species
ideal gas (H2, O2, H2O at 3000 K) did not converge in 5000 iterations with Thermochimica's
absolute `g0/RT`, but converged in 48 once the energies were referenced to the elements.

- First divergent operation: the line-search acceptance in `GemSystem::minimize`. The merit
  `G/RT + mu * sum|b - q|` had a constant `mu = 10`; an L1 exact penalty needs `mu >= max|pi|`,
  and ChemSage energies give `|pi| = 11-38`, so the merit preferred infeasible points. The
  `minimize` doc claimed "a common additive offset cancels"; true of the solution, false of the
  solver.
- Upstream (`GEMLineSearch.f90`, `CompFunctionNorm.f90`) has no penalty merit: it line-searches
  the residual norm, which is offset-free. The repair keeps the port's merit but makes it
  offset-free: (1) energies are referenced to a fitted per-element shift for the merit only,
  (2) `mu` is raised (never lowered) to `2 max|pi - c|` (Han's condition), as a strongly exothermic
  reaction (C + 1/2 O2 -> CO at 1000 K: -24 RT per atom) needs more than 10 even after the shift.
- Moved numbers (old -> new, port solves of the 8 ideal cases vs Thermochimica): **0 -> 7**
  (H1's reference itself is unconverged, see below). Test pinned by
  `gem::tests::merit_is_invariant_to_a_per_atom_energy_offset` (absolute and referenced energies
  agree to <= 1e-9 on every species).
- `descent_merit_history` documentation changed: monotone while `mu` is constant. The six earlier
  unit tests pass unchanged.

## Tightened upstream (1e-10), mode A, free candidates

| case | system | port rung | assemblage | pi | n major | G/RT | iterations up / port |
|---|---|---|---|---|---|---|---|
| C1 | CO 1000 K 1 atm, C2 O1 | 0 | same | 1.8e-12 | 1.6e-11 | 1.7e-12 | 8 / 53 |
| C2 | CO 300 K | 0 | same | 6.9e-15 | 1.3e-13 | 2.3e-15 | 7 / 172 |
| C3 | CO 2500 K | 0 | same | 1.9e-10 | 6.0e-15 | 3.4e-15 | 1 / 1233 |
| C4 | CO 1500 K 10 atm | 0 | same | 6.1e-13 | 8.3e-9 | 5.4e-13 | 14 / 43 |
| H2 | HO 1000 K | **1** | same | 1.8e-8 | 1.5e-14 | 3.0e-15 | 53 / 31 |
| H3 | HO 3000 K | 0 | same | 9.2e-12 | 6.1e-10 | 2.7e-12 | 19 / 42 |
| H4 | HO 2000 K 0.01 atm | 0 | same | 3.1e-12 | 7.2e-9 | 2.8e-12 | 10 / 39 |
| W1 | WAuArO-1 1455 K, 4-species RKMP | 0 | same | 5.5e-11 | 1.6e-15 | 1.2e-11 | 35 / 1 |

Bit-identical solved quantities: **1 of 50** (a mole count fixed by the feed in W1).
Default-tolerance upstream (1e-5), same cases: pi <= 9.0e-7 (W1), n major <= 8.5e-6 (C4),
G/RT <= 2.0e-7 (W1), 1 of 50 bit-identical.

"Given assemblage" adds the CsI liquid + solid system: S3 (800 K, Cs 3, I 1; liquid RKMP with
exponent-0 and exponent-1 binary terms plus CsI solid) tightened: pi 3.5e-13, n major 1.3e-9,
G/RT 2.1e-15, 187 port iterations vs 45 upstream, rung 0. S2 (1100 K, liquid only) solves only on
rung 2 (`mole_floor = 1e-50`): pi 2.8e-8, n 5.3e-15, G/RT 6.2e-14. S1 (400 K, CsI solid only)
is not run (no single-element species to seed from in the given set).

Mode B (literal J/mol, `R_up/R_port - 1 = 1.1e-6`): pi 1.1e-6 to 1.2e-6 and G/RT 0.7e-6 to
1.2e-6 on every solved case: the gas-constant difference, nothing else. Two cases need a looser
rung in mode B (C3 rung 1, H2 rung 3) and S2 does not solve at all.

## Predictions scored (PREDICTIONS.md)

| # | outcome |
|---|---|
| P1 assemblage identical, 8 ideal cases | **Held** on the 7 comparable (H1 excluded: upstream stopped at its iteration cap, `iterGlobal 1001`, still reporting `INFO 0`) |
| P2 default upstream: pi <= 1e-5, n <= 1e-4, G/RT <= 1e-8 | pi and n **held** (9.0e-7, 1.7e-5 worst incl. S3); G/RT **missed**: 2.0e-7 (W1), 2.2e-8 (H3). The issue's "< 1e-8" was expected to fail and does |
| P3 tightened: all <= 1e-8 | **Held** except H2 pi 1.8e-8: the port could not reach its own default `tol = 1e-10` there (below) and converged at `1e-8` |
| P4 ~no bit-identical | **Held** (1/50) |
| P5 W1/W2 | W1 **held** (n 1.6e-15, pi 9.0e-7 default). **W2 prediction wrong**: W2 (and W3, W4) carry ternary/quaternary RKMP terms (types 3/4), which the port lacks |
| P6 R offset 1.1e-6 | **Held** |
| P7 W4 gap | **Not tested**: ternary terms, and the tightened reference stopped at the 1001 cap |
| P8 iterations | Upstream 1-66 (default), port 29-187 on most, **C3 1233 vs 1** |
| P9 | not separately compared |

## Port findings not repaired (not localised beyond what is stated)

1. **Default `tol = 1e-10` is not always attainable.** H2: `max|delta|` stalls at 2.95e-9,
   `atom residual` 5e-15; converges at `tol = 1e-8`. Cause not localised (round-off in
   `delta = a.pi + u - mu` for trace species is the leading hypothesis, untested).
2. **Default `mole_floor = 1e-12` stalls S2** (a species at 1e-42 mol; max|delta| 1.4e-5 after 5000
   iterations, 8.8e-5 after 100000). `mole_floor = 1e-50` converges in 76 iterations. Changing a
   default is the maintainer's call; deferred.
3. **No assemblage selection.** With more candidate phases than elements the saddle system is
   singular (S1-S3 free, and two pure phases of identical composition, graphite/diamond). Upstream
   solves this in `gem/CheckPhaseAssemblage`, `CheckSolnPhaseAdd/Rem`, leveling (#1409).
4. **No ternary/quaternary RKMP terms** (`CompExcessGibbsEnergyRKMP.f90`, `iParam` types 3 and 4):
   W2, W3, W4 out of reach.
5. **Standard state.** Upstream's gas `g0` carries `ln P + 0.10945/R` (1 atm to 1 bar); a data
   pipeline feeding the port needs that constant. `R` differs: 8.314472 upstream vs CODATA
   8.31446261815324 here.
6. Upstream notes (not port defects): H1 and W4 end at `iterGlobal 1001` with `INFO 0`, so
   upstream can return an unconverged state without an error code.

## Gaps against the epic (#1408)

The parser, Gibbs-polynomial evaluation (needed to produce `g0` at all), leveling, SUBG/SUBL/QKTO/
SUBI, magnetic term and miscibility gap are the existing rows. Added by this comparison: RKMP types
3/4, assemblage selection robustness, and the minimiser findings above.
