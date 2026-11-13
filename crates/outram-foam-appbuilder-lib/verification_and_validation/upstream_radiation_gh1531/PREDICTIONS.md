# Predictions, written 2026-11-13 BEFORE any ESI run of the radiation case and BEFORE any test run (gh:#1531)

Case (fixed before running): the gh:#1363 two-region case (water fluid 20x5x1, steel-like solid 10x5x1,
dt 0.1 s, 50 steps, fluid at rest, frozenFlow) with P1 radiation switched on in the FLUID only:
a = e = 50 /m, E = 0, G initial uniform 0, wall emissivities left 0.9 / interface 0.8 / walls 0.5,
`laplacian(gammaRad,G)` Gauss linear uncorrected, G solver PCG/DIC 1e-15 (the port replays ESI's per-step
iteration count with tolerance 0). Interface `qr none` on both sides (the port has no qr term in its
coupled BC; stated as not covered).

Derivations (from the equations, before data):

1. Step 1, G starts at 0 so Ru = 0: the medium emits and does not absorb. Cooling of a cell away from
   the walls, ESI = port: dT = -Rp T^4 dt/(rho cp) / (1 + 4 Rp T^3 dt/(rho cp)), Rp = 4 e sigma,
   sigma = 5.670408558e-8: Rp T^4 = 200*5.670408558e-8*8.1e9 = 9.186e4 W/m3, dT = -2.197e-3 K,
   implicit factor 1 - 2.9e-5. Predicted fluid mean dT at step 1 = -2.197e-3 K (+-2 %; cells next to the
   interface/left wall differ by wall exchange, so the MEAN over the 100 cells is quoted).
2. Radiation effect is detectable: port-without-radiation vs ESI-with-radiation fluid cell T differs by
   >= 1e-3 K at some step (step 1 alone gives 2.2e-3 K) and <= 0.12 K at step 50 (upper bound: the medium
   seeing 360 K everywhere, a*4 sigma (360^4-300^4) dt_total/(rho cp) = 0.12 K). If outside, the case cannot discriminate.
3. Port with radiation vs ESI, fluid cell T: <= 5e-11 K at every step (the no-radiation floor was 1.05e-11 K, from
   T-form vs h-form representation; radiation adds a source of size 2e-3 K per step whose own round-off is
   ~1e-16 K, so no new floor expected). Solid T: <= 2e-11 K.
4. G (cells): relative worst <= 1e-9 at every step with ESI's iteration counts replayed (G inherits the T difference
   through 4 e sigma T^4: dG/G ~ 4 dT/T ~ 1e-13). qr on the left wall: <= 1e-8 of max|qr| (qr = difference of
   close numbers near equilibrium, so a looser bound).
5. WHERE IT FIRST DIVERGES (expected, not yet measured): the Marshak refValue 4 sigma Tw^4 on the INTERFACE
   patch. The gh:#1363 record found ESI's stored T.boundaryField() at the coupled interface lags the BC coefficients
   by one step (the port's `evaluate_boundary_values` yields the current-step value). Radiation reads the stored
   boundary value, so I predict the first divergence at step 2, in G of the cells next to the interface, of
   size 4 sigma 4 T^3 dTf ~ up to 10 W/m2 with dTf = the one-step change of the interface face temperature, unless the
   port is made to use the pre-solve boundary value. Left wall (fixedValue) and `walls` (zeroGradient) cannot diverge this way.
   If instead step 1 diverges, the suspect order is: initial-G convention, Tref/he conversion, then warm start.
6. Defaults: radiation off, every existing pin (gh:#1363 1.05e-11, gh:#1156 energy balance) unchanged bit for bit
   (the new field is None; no arithmetic is added to the off path).
7. `solverFreq 3` ablation: G recomputed only on steps 3,6,... (and the first call): the port reproduces ESI's
   `timeIndex % solverFreq` rule (unit test, ESI not re-run for it).
