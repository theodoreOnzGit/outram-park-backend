# Thermophysics against OpenFOAM-v2506 (ESI), gh:#1362 row 4

Upstream line: **ESI v2506** (`/home/user/upstream-build/OpenFOAM-v2506`, the
runnable build). The Foundation line was not read or run. Maintainer
direction "ESI preferred"; where the port follows Foundation the ESI form is
added as a named option and both are recorded.

Driver: `driver/thermoDump.C` (unmodified `libspecie` /
`libthermophysicalProperties` templates, built from dictionaries as a case file
would). Rebuild: `scripts/rebuild_reference.sh`. Reference:
`reference/thermo_esi_v2506.txt.gz` (2560 rows: 64 combinations x 10 T x 4 p;
Cp, Ha, Hs, Hc, S, rho, psi, Z, CpMCv, mu, kappa, alphah as hex bit patterns).
Test: `tests/openfoam_thermo_code_to_code.rs`.

Inputs (all literature-style values, none tuned): air `W = 28.96`, CO2
`W = 44.01, Tc = 304.12 K, Pc = 7.374 MPa, omega = 0.2239` for Peng-Robinson;
hConst `Cp = 1004.5, Hf = 2.5e5`; N2 JANAF coefficients; hPolynomial
`CpCoeffs<8> (1000 0.1 -2e-5 3e-9 0 0 0 0), Hf = 1e5, Sf = 200`; Sutherland
`As = 1.458e-6, Ts = 110.4`. Grid: T = 250..1900 K (10), p = 1e5..8e6 Pa (4).

## Findings read from the source before any run

- **Upstream cannot compile `hTabulatedThermo::S`** in v2506: it calls
  `EquationOfState::Sp`, which no equation of state in
  `specie/equationOfState/` defines (`hTabulatedThermoI.H:106`). The driver
  prints 0 for S on those 16 combinations; the port's S for hTabulated is
  therefore not comparable.
- `hTabulatedThermo` upstream is one 1-D `Cp(T)` table with `Hs = int Cp dT`,
  `Ha = Hs + Hf`; the port takes three independent tables (Cp, Ha, S).
- `hPolynomialThermo` upstream: `hCoeffs` offset to `Href` at `Tref`
  (defaults 0 and Tstd), `Ha = hCoeffs(T)`, `Hs = Ha - Hf`, `S = sCoeffs(T)`;
  `Hf` and `Sf` are NOT added to Ha / S (v2506 reads `Sf` but only uses
  `Sref`). The port follows the Foundation form (Hf added at Tstd).
- `Pstd`: upstream `1e5` Pa (`etc/controlDict`, `standard`); port `P_REF = 101325`.

## Predictions (written before the Rust side ran)

Sign and size stated per group; wrong ones will be corrected in place.

- P1: hConst x perfectGas: Cp, Hs, Ha, Hc bit-identical (no libm, same
  operation order); rho, psi differ at ulp level (R = RR/W against R/(W/1000));
  **S differs by `R ln(101325/1e5)` = +3.8 J/kg/K** from the `Pstd` value, so
  not identical, relative size ~1e-2.
- P2: janaf: Cp, Ha, Hs, Hc within a few ulp (coefficients scaled by a port R
  that is ulp-different); S differs by the `Pstd` term.
- P3: hPolynomial: Ha, Hs, S differ **structurally** (kJ/kg size), Cp identical.
- P4: rhoConst: Z differs (upstream 0, port `p/(rho R T)`); rho identical;
  icoPolynomial: rho identical, Z identical (0).
- P5: sutherland / polynomial / tabulated / const transport: mu bit-identical
  for polynomial and tabulated, ulp-level for sutherland (`sqrt` is exact);
  kappa follows Cp/Cv so inherits their differences.
- P6: PengRobinson: Z, rho, psi, Cp, H, S differ above ulp level (the port
  solves the cubic with `CubicEqn`, upstream uses the trigonometric / cbrt
  closed form and `RR/W` units); size unknown, predicted below 1e-10 relative
  away from the three-root region and larger inside it.
- P7: hTabulated: not comparable until an ESI-form table option exists.
