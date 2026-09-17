//! IAPWS-IF97 water and steam properties — translated from `IAPWS_IF97.m`.
//!
//! # Provenance — third-party, BSD-2-Clause
//!
//! Unlike every other module in this crate, this one is **not** Than Yan Ren's
//! code. It is a translation of a third-party MATLAB implementation that the
//! BEDOK snapshot vendored in.
//!
//! - **Upstream project:** IAPWS_IF97, <https://github.com/mikofski/IAPWS_IF97>
//! - **Source file:** `IAPWS_IF97.m` (no tagged release; taken as vendored
//!   into the `main_exec_diff3d_standalone` snapshot)
//! - **Copyright:** Copyright (c) 2013, Mark Mikofski
//! - **Licence:** BSD-2-Clause, reproduced in full in the crate `NOTICE`
//! - **Compatibility:** BSD-2-Clause is GPL-3.0-compatible, so the combined
//!   work ships GPL-3.0-only while these terms continue to govern this module.
//!
//! The vendored copy renders the copyright line as "Mark Mifofski"; the
//! upstream repository and its `license.txt` both give **Mikofski**, which is
//! used here as the correct spelling.
//!
//! # What it implements
//!
//! 27 basic property functions over the IAPWS Industrial Formulation 1997,
//! plus IAPWS-IF97-S01, -S03rev, -S04, -S05, the 2008 Revised Advisory Note
//! No. 3 on thermodynamic derivatives, the 2008 viscosity release, and the
//! 2008 revised release of the 1985 thermal-conductivity formulation.
//!
//! # Naming — the `<property>_<arguments>` convention
//!
//! The reference dispatches on a string built from a property symbol, an
//! underscore, and the symbols it depends on: `k_pT` is thermal conductivity
//! as a function of pressure and temperature. Derivatives prefix `d` and
//! suffix `d<symbol>`: `dTdp_ph`. Saturation suffixes `sat`, saturated liquid
//! `L`, saturated vapour `V`. The one irregular name is `cp_ph`, which is
//! `1/dTdh_ph`.
//!
//! Function names are carried over verbatim, lowercased for Rust:
//! `dgammadtau1_pT` becomes `dgammadtau1_pt`.
//!
//! # Units — these are not SI, and that matters
//!
//! The reference works in the formulation's own units throughout, and so does
//! this translation:
//!
//! | Symbol | Quantity | Unit |
//! |---|---|---|
//! | `p` | pressure | MPa |
//! | `T` | temperature | K |
//! | `h` | specific enthalpy | kJ/kg |
//! | `v` | specific volume | m³/kg |
//! | `x` | quality | mass fraction |
//! | `k` | thermal conductivity | W/m/K |
//! | `mu` | viscosity | Pa·s |
//! | `cp` | isobaric specific heat | kJ/kg/K |
//!
//! **Pressure in MPa and enthalpy in kJ/kg** are the two that catch people out.
//! Nothing here carries `uom` types, matching the rest of the translation.
//!
//! # Scalar, where the reference is vectorised
//!
//! `IAPWS_IF97.m` is vectorised across inputs, and the entry function spends
//! its first 110 lines on shape juggling — transposing row vectors, expanding
//! a scalar against a matrix, bailing to `NaN` for rank > 2. That is MATLAB
//! array-language plumbing, not physics, and it is not reproduced: the
//! functions here are scalar, and a caller wanting elementwise behaviour maps
//! over a slice.
//!
//! The one behaviour worth noting as *lost*: mismatched input shapes return
//! `NaN` in the reference rather than raising. A caller relying on that will
//! need to handle the mismatch itself.
//!
//! # Exponentiation
//!
//! The residual sums raise to integer-valued exponents held as `f64`. These use
//! [`f64::powf`], not `powi`, because MATLAB's `.^` on doubles calls the
//! platform `pow()`. The two can differ in the last bits, and `powf` is the
//! closer match.
//!
//! # Status — partial
//!
//! `IAPWS_IF97.m` is 3,361 lines and 107 subfunctions. Translated so far:
//!
//! | Part | Status |
//! |---|---|
//! | Region 1 (`gamma` derivatives) | done — [`region1`] |
//! | Region 2 (`gamma` derivatives) | done — [`region2`] |
//! | Region 3 (`phi` derivatives) | not started |
//! | Region 4 (saturation line) | done — [`region4`] |
//! | Backward equations `T*_ph` | regions 1 and 2 done — [`backward`]; region 3 not started |
//! | Region boundaries `TB23_p`, `h2bc_p` | done — [`backward`] |
//! | Viscosity and thermal conductivity | not started |
//! | Basic properties | partial — [`basic`] has `h`, `v` and `cp` for regions 1 and 2, plus `hL_p`, `hV_p`, `vL_p`, `vV_p` |
//! | Quality `x_ph`, `x_hT`, `x_pv`, `x_vT` | not started |
//!
//! **One gap is load-bearing and worth knowing about, and it is the same gap
//! everywhere:** region 3 is not translated, so anything that would need it
//! returns `NaN` rather than a wrong number. That caps [`basic::hl_p`],
//! [`basic::hv_p`], [`basic::vl_p`], [`basic::vv_p`] and [`backward::t_ph`] at
//! the region 1/3 boundary, **16.5292 MPa**. Both BEDOK operating points — a
//! PWR at 15.5 MPa and a BWR at 7 MPa — sit below it, so the benchmark cases
//! are unaffected. The individual sub-equations ([`backward::t2b_ph`],
//! [`backward::t2c_ph`], …) carry no such cap and are verified well above it.
//!
//! The BEDOK thermal hydraulics calls this almost entirely through the `_ph`
//! entry points — `T_ph`, `v_ph`, `cp_ph`, `x_ph` — so those and their
//! dependency chains are the critical path.
//!
//! # Verification so far
//!
//! Both translated regions are checked against the published verification
//! values, measured 2026-08-12:
//!
//! - **Region 1** — IAPWS-IF97 Table 5, worst case **2.810e-9** relative.
//! - **Region 2** — IAPWS-IF97 Table 15, worst case **1.841e-9** relative.
//! - **Region 4** — IAPWS-IF97 Tables 35 and 36, worst case **1.752e-9**
//!   relative (added 2026-08-13).
//! - **[`basic`]** — the dimensioned `h1_pT` and `h2_pT` reproduce their
//!   regions' figures exactly, confirming the `R * Tstar` scaling adds nothing.
//!   The saturated-enthalpy chain `hL_p -> Tsat_p -> h1_pT` puts the normal
//!   boiling point at **373.1243 K = 99.974 °C** and the latent heat at
//!   **2256.54 kJ/kg**, both textbook.
//!
//! Per-state numbers are in the test doc comments in [`region1`] and
//! [`region2`]. The published tables carry 9 significant figures, so this is
//! agreement at the reference's own precision.
//!
//! - **Backward equations** — IAPWS-IF97 Table 7 (region 1) to **6.590e-10**
//!   and Table 24 (regions 2a, 2b, 2c, nine states) to **4.603e-9**
//!   (added 2026-08-13).
//!
//! The backward equations get a second, independent check that uses no
//! published value: round-tripping `T -> h -> T` against the forward equations.
//! Over 122 region-1 states the worst error is **23.2 mK**, and over the
//! vapour sweep through the [`backward::t_ph`] dispatcher **15.0 mK** — both
//! inside the formulation's stated 25 mK backward tolerance. Landing *near*
//! that tolerance rather than far below it is the expected signature of a
//! correct transcription: the fits are designed to be exactly this accurate.
//! The dispatcher sweep additionally verifies the 2a/2b/2c sub-region
//! selection, since the three fits agree only on their shared boundaries — and
//! those boundaries are themselves checked, `T2b` against `T2c` on
//! `h2bc_p` to within 7.5 mK.
//!
//! Region 4 additionally cross-checks its two directions against each other:
//! `Tsat_p(psat_T(T))` returns to within **4.263e-15** relative over the whole
//! line. That is machine precision, five orders tighter than the agreement with
//! the published tables — which localises the ~1e-9 table residual to the
//! printed values' own rounding rather than to either expression.
//!
//! Region 2 additionally cross-checks `dgammadpitau` against a finite
//! difference of `dgammadpi`. Those two are transcribed from separate
//! expressions, each with its own copy of the 43-term coefficient table, so
//! their agreement is an independent check on both — the published-value test
//! alone would flag a mistyped coefficient without localising it.
//!
//! No other region has been translated, so no other region is verified.

pub mod backward;
pub mod basic;
pub mod region1;
pub mod region2;
pub mod region4;
pub mod transport;
