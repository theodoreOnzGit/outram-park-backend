// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! Atmosphere reactions: hydrogen and carbon-monoxide combustion at rates
//! the caller sets (gh:#1100, the burns prerequisite; MELCOR 2.2's BUR
//! package decides *whether* and *how fast* a mixture burns, BUR RM §2,
//! cited by section only; here only the stoichiometry and the energy are
//! closed, the criteria and rates staying with the caller, `sembawang`).
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! ```text
//! H2 + ½ O2 → H2O(g)        (extent ξ_H, mol)
//! CO + ½ O2 → CO2           (extent ξ_C, mol)
//! ```
//!
//! ## Limiting reactant
//!
//! The extents a step asks for, `rate · Δt`, are capped by the fuel present
//! (`ξ_H ≤ n_H2`, `ξ_C ≤ n_CO`) and then, together, by the oxygen
//! (`½ (ξ_H + ξ_C) ≤ n_O2`): when oxygen limits, both extents are scaled by
//! the same factor (the caller's rate ratio is kept; neither fuel is given
//! priority), and the limited reactant is set to exactly zero, so no amount
//! ever goes negative.
//!
//! ## The heat of reaction, on the gas mixture's own references
//!
//! Each species' energy in `tampines_steam_tables::gas_mixture` is on its
//! own reference: the non-condensables' `u(T)` from 0 K by `sembawang`'s
//! convention (MELCOR's NCG fits for N2 and H2, GRI-Mech 3.0 NASA-7 `c_p`
//! polynomials for O2, CO, CO2 and Ar; **below each fit's lower bound,
//! `c_v(T_low) T`**, so "from 0 K" is a convention, not JANAF's
//! `H°(T) − H°(0)`), and steam on IAPWS-IF97's (the triple-point liquid's
//! `u = s = 0`). A reaction moves moles between species, so the energy it
//! releases has to be put on those same references. With `h_i^code(T)` the
//! code's molar enthalpy of species `i` (`u_i(T) + R T` for a
//! non-condensable; `M_H2O h_IF97(T, p → 0)` for steam, region 2 at 1 Pa,
//! where the residual part is below 1e-5 J/kg), each species' absolute
//! (formation-referenced) enthalpy is
//!
//! ```text
//! h̃_i(T) = h_i^code(T) + c_i,   c_i = Δ_f H_i°(298.15 K) − h_i^code(298.15 K)
//! ```
//!
//! which anchors every species to its NIST-JANAF standard enthalpy of
//! formation at 298.15 K (Chase, *NIST-JANAF Thermochemical Tables*, 4th ed.,
//! J. Phys. Chem. Ref. Data Monograph 9, 1998: H2O(g) −241.826, CO −110.527,
//! CO2 −393.522 kJ/mol; the elements H2, O2, N2, Ar zero). The constant `c_i`
//! shifts `u` exactly as it shifts `h` (`h − u` is `R T`, or IF97's `p v`, on
//! both references), so the conserved absolute energy of the gas is
//! `Σ n_i (u_i^code + c_i)`. A reaction of extents `ξ` changes the amounts by
//! `ν ξ` at fixed absolute energy, so the stored (code-reference) energy rises
//! by
//!
//! ```text
//! Q = −Σ_i ν_i c_i ξ
//!   = ξ_H [−Δ_f H°(H2O) + h_H2O^code − h_H2^code − ½ h_O2^code]_298.15
//!   + ξ_C [Δ_f H°(CO) − Δ_f H°(CO2) + h_CO2^code − h_CO^code − ½ h_O2^code]_298.15
//! ```
//!
//! a constant per mole on each closure
//! ([`super::ControlVolume::reaction_energy_per_mole`]). The
//! temperature the products reach then follows from the stored energy by the
//! mixture's own `U(T)`, so the heat of reaction at any other temperature is
//! Kirchhoff's law on the code's own heat capacities. By construction the
//! enthalpy of reaction at 298.15 K is the JANAF one: 241.826 kJ/mol for H2
//! (the lower heating value) and 282.995 kJ/mol for CO; the V&V gate
//! (`tests/control_volume_reactions.rs`, R1) measures it through a step, from
//! the stored energy and the code's own `U(298.15 K)` of the products, which
//! tests the offsets, their sign, the mol-to-kg conversion of the steam and
//! the IF97 steam's real-gas `p v` at its partial pressure.
//!
//! **Why 298.15 K, not 0 K.** JANAF also tabulates `Δ_f H°(0)` and
//! `H°(298.15) − H°(0)`, and anchoring there looks natural for references
//! "from 0 K". It would be wrong here: the code's `u(T)` below each fit's
//! lower bound is the `c_v(T_low) T` extrapolation, not the real
//! `H°(T) − H°(0)`, so a 0 K anchor would carry the extrapolation's error
//! (measured 2026-11-01, the code's `h(298.15)` "from 0 K" less JANAF's
//! `H°(298.15) − H°(0)`: H2 −896 J/mol on MELCOR's fit, CO2 +518, N2 +83,
//! CO +14, O2 +12, Ar +0.4; `tests/control_volume_reactions.rs` R1) into
//! every heat of reaction (H2's alone is 0.37 % of the H2 burn's), and steam
//! on IF97 has no 0 K value to anchor at all. At 298.15 K every fit is
//! inside its range, or (N2, Ar, from 300 K) 1.85 K below it, where an inert
//! gas's offset cancels.

use uom::si::f64::{CatalyticActivity, MolarEnergy};
use uom::si::catalytic_activity::mole_per_second;
use uom::si::molar_energy::joule_per_mole;

/// A molar flow rate, mol/s (uom names the dimension `CatalyticActivity`;
/// the alias follows `outram_park_fork_dwsim_libs::separator::MolarFlowRate`).
pub type MolarFlowRate = CatalyticActivity;

/// NIST-JANAF standard enthalpy of formation of H2O(g) at 298.15 K, J/mol.
pub const DELTA_HF_H2O_GAS: f64 = -241_826.0;
/// NIST-JANAF standard enthalpy of formation of CO at 298.15 K, J/mol.
pub const DELTA_HF_CO: f64 = -110_527.0;
/// NIST-JANAF standard enthalpy of formation of CO2 at 298.15 K, J/mol.
pub const DELTA_HF_CO2: f64 = -393_522.0;
/// The formation enthalpies' reference temperature, K.
pub const T_FORMATION_K: f64 = 298.15;

/// Combustion rates in a volume's atmosphere, set by the caller (whose burn
/// model decides them) and applied inside [`super::ControlVolume::step`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AtmosphereReactions {
    /// `H2 + ½ O2 → H2O(g)`, moles of H2 per second, ≥ 0.
    pub hydrogen_burn: MolarFlowRate,
    /// `CO + ½ O2 → CO2`, moles of CO per second, ≥ 0.
    pub carbon_monoxide_burn: MolarFlowRate,
}

impl AtmosphereReactions {
    /// No reaction (the default).
    pub fn none() -> Self {
        Self::new(MolarFlowRate::new::<mole_per_second>(0.0), MolarFlowRate::new::<mole_per_second>(0.0))
    }
    /// Burn H2 and CO at these rates (moles of fuel per second).
    pub fn new(hydrogen_burn: MolarFlowRate, carbon_monoxide_burn: MolarFlowRate) -> Self {
        Self { hydrogen_burn, carbon_monoxide_burn }
    }
}

impl Default for AtmosphereReactions {
    fn default() -> Self {
        Self::none()
    }
}

/// The extents one step applies, mol, and the amounts left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Extents {
    /// H2 burned.
    pub xi_h: f64,
    /// CO burned.
    pub xi_c: f64,
    /// H2, CO and O2 left.
    pub h2: f64,
    pub co: f64,
    pub o2: f64,
}

/// Cap the asked extents `want_h`, `want_c` (mol) by the fuel and then the
/// oxygen present (see the module docs); a limited reactant ends exactly 0.
pub(crate) fn capped_extents(n_h2: f64, n_co: f64, n_o2: f64, want_h: f64, want_c: f64) -> Extents {
    let (mut xi_h, mut xi_c) = (want_h.min(n_h2), want_c.min(n_co));
    let need = 0.5 * (xi_h + xi_c);
    let o2_limited = need > 0.0 && need >= n_o2;
    if o2_limited {
        let s = n_o2 / need;
        xi_h *= s;
        xi_c *= s;
    }
    let left = |n: f64, x: f64| if x >= n { 0.0 } else { n - x };
    let o2 = if o2_limited { 0.0 } else { (n_o2 - 0.5 * xi_h - 0.5 * xi_c).max(0.0) };
    Extents { xi_h, xi_c, h2: left(n_h2, xi_h), co: left(n_co, xi_c), o2 }
}

/// The stored-energy release per mole of each reaction, `[H2, CO]`, J/mol,
/// given the code's molar enthalpies at 298.15 K of H2O(g), H2, O2, CO and
/// CO2 (see the module docs for the derivation).
pub(crate) fn energy_per_mole(h_h2o: f64, h_h2: f64, h_o2: f64, h_co: f64, h_co2: f64) -> [f64; 2] {
    let c = |dhf: f64, h: f64| dhf - h;
    let (c_h2o, c_h2, c_o2) = (c(DELTA_HF_H2O_GAS, h_h2o), c(0.0, h_h2), c(0.0, h_o2));
    let (c_co, c_co2) = (c(DELTA_HF_CO, h_co), c(DELTA_HF_CO2, h_co2));
    [-(c_h2o - c_h2 - 0.5 * c_o2), -(c_co2 - c_co - 0.5 * c_o2)]
}

/// `[H2, CO]` as uom quantities.
pub(crate) fn as_molar_energy(q: [f64; 2]) -> [MolarEnergy; 2] {
    q.map(MolarEnergy::new::<joule_per_mole>)
}
