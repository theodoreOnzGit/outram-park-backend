// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! Steam methane reforming in a continuous stirred-tank reactor.
//!
//! # What this models
//!
//! Methane and steam over a reforming catalyst, in a perfectly-mixed tank at
//! steady state. Two **independent** reactions:
//!
//! ```text
//!   R1  reforming   CH4 + H2O  <=>  CO  + 3 H2     strongly endothermic
//!   R2  water-gas   CO  + H2O  <=>  CO2 +   H2     mildly exothermic
//!       shift
//! ```
//!
//! # Why two reactions and not three
//!
//! Textbooks usually write a third, `CH4 + 2 H2O <=> CO2 + 4 H2`. It is
//! **R1 + R2 exactly** — same atoms, same enthalpy, same entropy — so adding
//! it to the reaction list does not add chemistry. It adds a redundant extent:
//! the outlet composition becomes a function of three unknowns constrained by
//! two, the Jacobian in [`Cstr`]'s Newton solve goes singular, and the solver
//! either wanders or fails. The set here is deliberately the independent one,
//! and [`smr_reactions`] returns exactly two.
//!
//! This is checked, not asserted: `the_overall_reaction_is_the_sum_of_the_two`
//! confirms the third reaction's enthalpy and entropy are reproduced by
//! summing R1 and R2, which is *why* it is redundant.
//!
//! # The model hierarchy: what is derived and what is fitted
//!
//! The workspace's model-hierarchy rule requires the physics-derived layer to
//! be built and run **uncalibrated** first, with the disagreement reported,
//! before anything is fitted. This module is built to make that separation
//! explicit rather than to assert it:
//!
//! | quantity | where it comes from | free? |
//! |---|---|---|
//! | `ΔH°`, `ΔS°` per reaction | summed from [`crate::species`] formation data | **no** — derived |
//! | `K_eq(T)` | van 't Hoff on those: `K = exp(−(ΔH° − TΔS°)/RT)` | **no** — derived |
//! | reverse rate `A_r`, `E_r` | [`consistent_reverse`], forced to satisfy `k_f/k_r = K_eq` | **no** — derived |
//! | forward rate `A_f`, `E_f` | the deck | **yes** — the *only* fitted inputs |
//!
//! So the **equilibrium limit of this reactor is pure thermodynamics**: as the
//! residence time grows, the outlet approaches the equilibrium composition
//! regardless of what `A_f` and `E_f` are. The forward rate sets only how
//! quickly that limit is approached. That is the uncalibrated model, and
//! `long_residence_time_approaches_thermodynamic_equilibrium` measures it.
//!
//! **No published kinetic parameter set is embedded here.** The Xu–Froment
//! (1989) Langmuir–Hinshelwood constants are the usual choice and are
//! deliberately *not* typed in: they are fitted to one specific
//! Ni/MgAl₂O₄ catalyst, and the workspace requires any document informing the
//! code to be catalogued in `kovan-literature` first — which, as
//! [`crate::species`] records, has not been done. A deck supplies its own
//! `A_f`/`E_f` and owns that choice.
//!
//! # Stiffness: why the bundled deck's shift pre-exponential is 1e2
//!
//! The residual [`Cstr`] solves is `ζ_r − V·rate_r(C(ζ)) = 0`. It is only
//! well-scaled while `V·rate` is comparable to the extents themselves. A
//! reaction fast enough that `V·rate` runs many orders of magnitude above the
//! feed rate is, physically, *at equilibrium* — and numerically it makes that
//! residual unsolvable.
//!
//! This bit, and the measurement is recorded here because the symptom looks
//! like a solver bug and is not. An earlier draft of the bundled deck used
//! `A = 1e6` for the shift. At the base operating point that is
//! `V·rate_shift = 1.3e6 mol/s` once CO reaches 0.1 mol/s — against a
//! **1 mol/s** methane feed, six orders of magnitude out. Measured 2026-09-25:
//! the solve then failed for 4 of 13 temperatures between 800 K and 1400 K and
//! for 4 of 6 volumes between 2 m³ and 2e5 m³, and raising `max_iter` to
//! 100 000 fixed none of them, because the failure is not a budget shortfall —
//! `‖g‖` has a strict local minimum at `ζ ≈ 0` that is not a root, so no step
//! in any direction reduces it.
//!
//! **The choice of `A` costs nothing physically, which is what licenses making
//! it on conditioning grounds.** The shift is equilibrium-limited over the
//! whole range, so the answer is invariant (measured 2026-09-25, base case,
//! `max_iter = 20 000`):
//!
//! | `A_shift` | methane conversion | shift `Q/Kc` |
//! |---|---|---|
//! | 1e0 | 0.41151567 | 0.921970 |
//! | 1e1 | 0.41172225 | 0.991650 |
//! | **1e2** | **0.41174540** | **0.999159** |
//! | 1e3 | 0.41174774 | 0.999916 |
//! | 1e4 | 0.41174797 | 0.999992 |
//! | 1e6 | 0.41174800 | 1.000000 |
//! | 1e7 | 0.41174800 | 1.000000 |
//!
//! From `1e2` up the reported conversion moves by 6e-6 relative while the
//! conditioning improves by four orders of magnitude. `1e2` is the deck's
//! value for that reason, and `shift_kinetics_only_set_the_approach_rate`
//! pins the invariance so the claim can fail. This is **not** a parameter
//! tuned to reproduce a target: the target is unchanged across the range.
//!
//! # Honest scope — read before using a number from this
//!
//! - **Isothermal.** The feed temperature is held through the reactor. The
//!   heat of reaction is *reported* ([`SmrOutcome::heat_duty`]) but not fed
//!   back into an energy balance. Real reforming is violently endothermic and
//!   a real reactor's temperature drops along it; this does not model that.
//! - **Ideal gas, constant volumetric flow.** `Cᵢ = Fᵢ/Q` with `Q` fixed, the
//!   inherited assumption from `dwsim-libs`' reactors. Reforming increases
//!   the mole count substantially (1 + 1 → 1 + 3), so a real isobaric reactor
//!   expands and `Q` rises; holding it fixed overstates the product
//!   concentrations and therefore the reverse rates. Documented, not modelled.
//! - **Power-law kinetics, no adsorption.** `dwsim-libs` has
//!   `LangmuirHinshelwood` ready for the surface-coverage denominator real
//!   reforming needs; wiring it requires the catalyst constants above, so it
//!   is left for when they can be catalogued.
//! - **No catalyst, no diffusion, no pressure drop, no carbon formation.**
//!
//! **Untrusted AI-assisted draft, no human V&V.** Education, research and V&V
//! only, per the workspace `RESPONSIBLE_USE.md`.

use outram_park_fork_dwsim_libs::reactions::{
    EquilibriumConstant, Reaction, ReactionBasis, ReactionComponent, ReactionKind,
};
use outram_park_fork_dwsim_libs::reactors::{Cstr, ReactorError, ReactorFeed};

use crate::species::Species;

/// Molar gas constant `R` [J/(mol·K)] for **thermodynamics** — CODATA, exact
/// by the 2019 SI redefinition.
///
/// Used for `ΔG° = ΔH° − TΔS°`, for `K = exp(−ΔG°/RT)`, and for the ideal-gas
/// volumetric flow. Not used for rate constants — see [`R_KINETIC`].
pub const R_GAS: f64 = 8.314_462_618_153_24;

/// The gas constant **`dwsim-libs` evaluates Arrhenius rate constants with**,
/// re-exported so this crate cannot drift from it.
///
/// # Why there are two of these
///
/// `outram-park-fork-dwsim-libs` carries three gas constants:
/// `reactions::R_GAS = 8.314` (truncated, as DWSIM upstream has it) and
/// `thermo::flash_sle::R_GAS` / `thermo::activity::R_GAS`, both the full
/// CODATA `8.31446261815324`. `Reaction::forward_rate_constant` uses the
/// truncated one.
///
/// That 5.6e-5 relative difference is invisible almost everywhere and is
/// emphatically not invisible here: `consistent_reverse` has to predict what
/// `forward_rate_constant` will return, and an activation energy of 240 kJ/mol
/// at 700 K puts `E/(RT)` near 41, so a 5.6e-5 error in `R` becomes a
/// **2e-3 relative error in `k`**. That is what the first run of
/// `reverse_rate_is_thermodynamically_consistent` measured, to within a
/// factor of one — the test found it immediately.
///
/// So: thermodynamic quantities use [`R_GAS`], and anything that has to agree
/// with the rate law uses this. Mixing them silently breaks the consistency
/// guarantee that is the point of the model.
pub const R_KINETIC: f64 = outram_park_fork_dwsim_libs::reactions::R_GAS;

/// Newton iteration budget handed to [`Cstr`], overriding its default of 200.
///
/// # Why it is raised, and why that is a budget and not a loosened criterion
///
/// [`Cstr`]'s tolerance is untouched — a solve still has to reach the same
/// residual, it is merely allowed more steps to get there. The cases that need
/// them are the genuinely harder ones: a large reactor sitting close to
/// equilibrium, where the residual surface is flat near the root.
///
/// **Measured 2026-09-25** over a 936-point grid (6 volumes from 2 m³ to
/// 2e5 m³ × 13 temperatures from 800 K to 1400 K × 4 pressures from 5 bar to
/// 40 bar × 3 steam-to-carbon ratios), with the deck's kinetics: **0 failures**,
/// and the worst case needed 20 000 iterations (V = 2e4 m³, 800 K, 40 bar,
/// S/C = 1). The base case converges in 200 and takes about 0.3 ms; the
/// slowest in the grid takes a few milliseconds. The budget is therefore set
/// at the measured worst case rather than at a round number.
pub const MAX_NEWTON_ITERATIONS: usize = 20_000;

/// Reference temperature for the standard-state data, 298.15 K.
pub const T_REF: f64 = 298.15;

/// Which of the two independent reactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmrReaction {
    /// `CH4 + H2O <=> CO + 3 H2`.
    Reforming,
    /// `CO + H2O <=> CO2 + H2`.
    WaterGasShift,
}

impl SmrReaction {
    /// Both, in the order [`smr_reactions`] returns them.
    pub const BOTH: [SmrReaction; 2] = [SmrReaction::Reforming, SmrReaction::WaterGasShift];

    /// Signed stoichiometry `νᵢ` — negative reactant, positive product.
    #[must_use]
    pub fn stoichiometry(self) -> [(Species, f64); 4] {
        match self {
            SmrReaction::Reforming => [
                (Species::Methane, -1.0),
                (Species::Steam, -1.0),
                (Species::CarbonMonoxide, 1.0),
                (Species::Hydrogen, 3.0),
            ],
            SmrReaction::WaterGasShift => [
                (Species::CarbonMonoxide, -1.0),
                (Species::Steam, -1.0),
                (Species::CarbonDioxide, 1.0),
                (Species::Hydrogen, 1.0),
            ],
        }
    }

    /// The reaction's base reactant — the species whose conversion normalises
    /// the extent.
    #[must_use]
    pub fn base_reactant(self) -> Species {
        match self {
            SmrReaction::Reforming => Species::Methane,
            SmrReaction::WaterGasShift => Species::CarbonMonoxide,
        }
    }

    /// Standard reaction enthalpy `ΔH°` at 298.15 K [J/mol of extent],
    /// summed from [`crate::species`] formation enthalpies.
    ///
    /// Positive is endothermic. Nothing here is a tabulated reaction
    /// enthalpy — it is `Σ νᵢ ΔH°f,ᵢ`, so it cannot disagree with the species
    /// table it is built from.
    #[must_use]
    pub fn delta_h(self) -> f64 {
        self.stoichiometry()
            .iter()
            .map(|&(sp, nu)| nu * sp.enthalpy_formation())
            .sum()
    }

    /// Standard reaction entropy `ΔS°` at 298.15 K [J/(mol·K)], summed from
    /// absolute entropies as `Σ νᵢ S°ᵢ`.
    #[must_use]
    pub fn delta_s(self) -> f64 {
        self.stoichiometry()
            .iter()
            .map(|&(sp, nu)| nu * sp.entropy())
            .sum()
    }

    /// Standard Gibbs energy of reaction at `temperature_k` [J/mol], on the
    /// constant-`ΔH°`/`ΔS°` (van 't Hoff) approximation: `ΔG° = ΔH° − T ΔS°`.
    ///
    /// **This is the model's main thermodynamic approximation.** Treating
    /// `ΔH°` and `ΔS°` as temperature-independent ignores the `∫Cp dT` and
    /// `∫Cp/T dT` corrections, which over 298 K → 1200 K are not negligible.
    /// It is the approximation `EquilibriumConstant::GibbsVantHoff` embodies,
    /// and it is kept because the alternative — integrating heat capacities —
    /// needs `Cp(T)` polynomials this crate would have to source and
    /// catalogue. Stated so a reader does not mistake the equilibrium curve
    /// for an exact one.
    #[must_use]
    pub fn delta_g(self, temperature_k: f64) -> f64 {
        self.delta_h() - temperature_k * self.delta_s()
    }

    /// Equilibrium constant `Kp(T)` [-] on the **standard-state partial
    /// pressure** basis: `Kp = exp(−ΔG°(T)/(R T))`, with each partial
    /// pressure referred to `P° = 1 bar`.
    ///
    /// This is the thermodynamic equilibrium constant. It is **not** the one
    /// a concentration-basis rate law equilibrates to unless `Δn = 0` — see
    /// [`SmrReaction::kc`].
    #[must_use]
    pub fn equilibrium_constant(self, temperature_k: f64) -> f64 {
        (-self.delta_g(temperature_k) / (R_GAS * temperature_k)).exp()
    }

    /// Change in mole count, `Δn = Σ νᵢ` [-].
    ///
    /// `+2` for reforming (1 + 1 → 1 + 3) and `0` for the shift. It is the
    /// whole reason [`kc`](Self::kc) exists.
    #[must_use]
    pub fn delta_n(self) -> f64 {
        self.stoichiometry().iter().map(|&(_, nu)| nu).sum()
    }

    /// Equilibrium constant on the **molar concentration** basis,
    /// `Kc = Π Cᵢ^νᵢ` in `(mol/m³)^Δn`.
    ///
    /// # Why this is not `Kp`, and why getting it wrong is expensive
    ///
    /// `Reaction::net_rate` evaluates `k_f·ΠC^d − k_r·ΠC^r` in
    /// **concentrations**, so the composition its rate law settles at
    /// satisfies `k_f/k_r = Kc`, not `Kp`. For an ideal gas `pᵢ = Cᵢ R T`,
    /// so
    ///
    /// ```text
    ///   Kp = Kc · (R T / P°)^Δn      =>      Kc = Kp · (P° / (R T))^Δn
    /// ```
    ///
    /// For the shift reaction `Δn = 0` and the two coincide. For reforming
    /// `Δn = +2`, and at 1123 K the factor `(R T / P°)²` is **about 115** —
    /// so using `Kp` where `Kc` belongs would put the reforming equilibrium
    /// out by two orders of magnitude, in the direction of far too little
    /// conversion. That is not a subtle error, but it is a silent one: the
    /// reactor still converges, and still returns a plausible-looking number.
    ///
    /// `P°` is 1 bar = 100 kPa, the standard state the formation data in
    /// [`crate::species`] is tabulated at.
    #[must_use]
    pub fn kc(self, temperature_k: f64) -> f64 {
        const P_STANDARD: f64 = 100_000.0;
        self.equilibrium_constant(temperature_k)
            * (P_STANDARD / (R_GAS * temperature_k)).powf(self.delta_n())
    }
}

/// The forward Arrhenius pair a deck supplies for one reaction — the **only**
/// fitted inputs in the model.
///
/// `A` carries whatever units make `k·∏Cᵈ` come out in mol/(m³·s) for that
/// reaction's orders; with the mass-action orders used here (orders equal to
/// `|ν|` for reactants) that is `(m³/mol)^(n−1)·s⁻¹` for an `n`-th order
/// reaction. `E` is in J/mol.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ForwardRate {
    /// Pre-exponential factor `A_f`.
    pub a: f64,
    /// Activation energy `E_f` [J/mol].
    pub e: f64,
}

/// Derive the reverse Arrhenius pair that makes a reversible rate law
/// **thermodynamically consistent** at the reactor temperature.
///
/// # Why this function has to exist
///
/// `dwsim-libs`' `Reaction::net_rate` evaluates
/// `rate = k_f(T)·ΠCᵈ − k_r(T)·ΠCʳ` with `k_f` and `k_r` as two *independent*
/// Arrhenius pairs. It never consults `k_eq`. So nothing in that API stops a
/// caller specifying a reaction whose kinetics settle at a composition its own
/// equilibrium constant says is wrong — the rate law and the thermodynamics
/// simply disagree, silently, and the reactor converges to the kinetic answer.
///
/// Mass-action orders make the equilibrium condition `k_f/k_r = Kc`, so the
/// reverse constant is pinned once `k_f` and the thermodynamics are known:
///
/// ```text
///   k_r(T) = k_f(T) / Kc(T)
/// ```
///
/// # Why it is anchored at one temperature rather than derived once
///
/// The tempting move is to match the Arrhenius forms symbolically —
/// `A_r = A_f·exp(−ΔS°/R)`, `E_r = E_f − ΔH°` — which makes `k_f/k_r` equal
/// `Kp` at *every* temperature. That is exactly right, and exactly wrong
/// here: the rate law needs `Kc`, and `Kc = Kp·(P°/RT)^Δn` carries a factor
/// of `T^{−Δn}` that **no Arrhenius pair can reproduce**. For reforming
/// (`Δn = +2`) that factor changes by 2.6x across 800–1300 K, so a single
/// pair cannot be consistent over a range.
///
/// Since this model is isothermal — one temperature per solve — the honest
/// resolution is to make consistency **exact at that temperature**. The
/// activation energy still takes its physically meaningful value
/// `E_r = E_f − ΔH°` (so an endothermic reaction gets the lower reverse
/// barrier, as it should), and `A_r` is then whatever makes the ratio come
/// out right:
///
/// ```text
///   A_r = (k_f(T) / Kc(T)) · exp(E_r / (R T))
/// ```
///
/// Verified to 1e-12 relative over 700–1400 K by
/// `reverse_rate_is_thermodynamically_consistent`, which rebuilds the pair at
/// each temperature exactly as a solve does.
///
/// **A sweep therefore rebuilds its reactions per point**, which
/// [`smr_reactions`] takes the temperature for. Reusing one reactor across a
/// temperature sweep would silently break consistency everywhere but the
/// anchor point.
#[must_use]
pub fn consistent_reverse(
    forward: ForwardRate,
    delta_h: f64,
    kc: f64,
    temperature_k: f64,
) -> ForwardRate {
    let e_r = forward.e - delta_h;
    // R_KINETIC, not R_GAS: this has to reproduce exactly what
    // `Reaction::forward_rate_constant` will compute, and that uses
    // dwsim-libs' own truncated constant. See R_KINETIC's docs.
    let k_f = forward.a * (-forward.e / (R_KINETIC * temperature_k)).exp();
    let k_r = k_f / kc;
    ForwardRate {
        a: k_r * (e_r / (R_KINETIC * temperature_k)).exp(),
        e: e_r,
    }
}

/// Build the two independent SMR reactions, ready for a
/// [`Cstr`](outram_park_fork_dwsim_libs::reactors::Cstr).
///
/// `forward` supplies the fitted forward Arrhenius pair for each reaction, in
/// [`SmrReaction::BOTH`] order. Everything else — stoichiometry, orders,
/// `ΔH°`, `ΔS°`, `K_eq(T)` and the whole reverse pair — is derived.
///
/// `temperature_k` is required because the reverse pair is anchored to it;
/// see [`consistent_reverse`]. Build the reactions at the temperature you
/// intend to solve at.
///
/// Reaction orders are mass-action: each reactant's forward order is `|ν|`,
/// each product's reverse order is `ν`. That is an assumption, not a
/// measurement — real reforming is not mass-action, it is
/// Langmuir–Hinshelwood — and it is the assumption that makes the rate law
/// self-consistent without catalyst data.
#[must_use]
pub fn smr_reactions(forward: [ForwardRate; 2], temperature_k: f64) -> Vec<Reaction> {
    SmrReaction::BOTH
        .iter()
        .zip(forward)
        .map(|(&rxn, fwd)| {
            let (dh, ds) = (rxn.delta_h(), rxn.delta_s());
            let rev = consistent_reverse(fwd, dh, rxn.kc(temperature_k), temperature_k);
            let base = rxn.base_reactant();

            let components = rxn
                .stoichiometry()
                .iter()
                .map(|&(sp, nu)| {
                    // Mass-action: reactants carry the forward order, products
                    // the reverse one.
                    let (direct, reverse) = if nu < 0.0 { (-nu, 0.0) } else { (0.0, nu) };
                    ReactionComponent::new(sp.index(), nu, direct, reverse, sp == base)
                })
                .collect();

            Reaction::new(
                ReactionKind::Kinetic,
                ReactionBasis::MolarConcentration,
                components,
            )
            .with_forward(fwd.a, fwd.e)
            .with_reverse(rev.a, rev.e)
            .with_k_eq(EquilibriumConstant::GibbsVantHoff {
                delta_h: dh,
                delta_s: ds,
            })
            .with_reaction_heat(dh)
        })
        .collect()
}

/// A fully-specified steady-state SMR CSTR case.
#[derive(Debug, Clone, PartialEq)]
pub struct SmrCase {
    /// Inlet methane molar flow [mol/s].
    pub methane_feed: f64,
    /// Steam-to-carbon ratio `S/C` [-] — inlet steam per inlet methane.
    /// Industrial reforming runs 2.5–3.5 to suppress carbon laydown; this
    /// model has no carbon chemistry, so a low `S/C` here is merely
    /// optimistic rather than visibly wrong.
    pub steam_to_carbon: f64,
    /// Reactor temperature [K], held constant (see the module's scope note).
    pub temperature: f64,
    /// Reactor pressure [Pa]. Enters only through the ideal-gas volumetric
    /// flow.
    pub pressure: f64,
    /// Tank volume `V` [m³].
    pub volume: f64,
    /// Forward Arrhenius pairs, in [`SmrReaction::BOTH`] order.
    pub forward: [ForwardRate; 2],
}

impl SmrCase {
    /// Inlet molar flows, by [`Species::index`]. Only methane and steam are
    /// fed; the rest start at zero.
    #[must_use]
    pub fn inlet_flows(&self) -> Vec<f64> {
        let mut f = vec![0.0; Species::INDEX_ORDER.len()];
        f[Species::Methane.index()] = self.methane_feed;
        f[Species::Steam.index()] = self.methane_feed * self.steam_to_carbon;
        f
    }

    /// Inlet volumetric flow `Q` [m³/s] from the ideal gas law,
    /// `Q = Σ F · R T / P`.
    ///
    /// Held constant through the reactor, which is the inherited assumption
    /// this model does not fix — see the module scope note.
    #[must_use]
    pub fn volumetric_flow(&self) -> f64 {
        let total: f64 = self.inlet_flows().iter().sum();
        total * R_GAS * self.temperature / self.pressure
    }

    /// Assemble the reactor feed.
    #[must_use]
    pub fn feed(&self) -> ReactorFeed {
        ReactorFeed {
            molar_flows: self.inlet_flows(),
            temperature: self.temperature,
            pressure: self.pressure,
            volumetric_flow: self.volumetric_flow(),
        }
    }

    /// Nominal residence time `τ = V/Q` [s].
    #[must_use]
    pub fn residence_time(&self) -> f64 {
        self.volume / self.volumetric_flow()
    }

    /// Solve the steady-state CSTR.
    ///
    /// # Errors
    ///
    /// Propagates [`ReactorError`] from the underlying Newton solve — a
    /// non-positive volumetric flow, or failure to converge.
    pub fn solve(&self) -> Result<SmrOutcome, ReactorError> {
        let mut reactor = Cstr::new(smr_reactions(self.forward, self.temperature), self.volume);
        reactor.max_iter = MAX_NEWTON_ITERATIONS;
        let feed = self.feed();
        let out = reactor.solve(&feed)?;

        let inlet = self.inlet_flows();
        let ch4 = Species::Methane.index();
        let methane_conversion = if inlet[ch4] > 0.0 {
            (inlet[ch4] - out.molar_flows[ch4]) / inlet[ch4]
        } else {
            0.0
        };

        Ok(SmrOutcome {
            outlet_flows: out.molar_flows,
            extents: out.extents,
            heat_duty: out.heat_of_reaction,
            methane_conversion,
            residence_time: self.residence_time(),
        })
    }
}

/// What a solved case reports.
#[derive(Debug, Clone, PartialEq)]
pub struct SmrOutcome {
    /// Outlet molar flows [mol/s], by [`Species::index`].
    pub outlet_flows: Vec<f64>,
    /// Per-reaction extent [mol/s], in [`SmrReaction::BOTH`] order.
    pub extents: Vec<f64>,
    /// Net heat of reaction [W]. **Positive means heat must be supplied** to
    /// hold the feed temperature — reforming is endothermic, so a converged
    /// case with meaningful conversion should report a positive duty. This is
    /// reported, not enforced: the model is isothermal and does not solve an
    /// energy balance.
    pub heat_duty: f64,
    /// Fractional methane conversion `X = (F_in − F_out)/F_in` [-].
    pub methane_conversion: f64,
    /// Nominal residence time `τ = V/Q` [s].
    pub residence_time: f64,
}

impl SmrOutcome {
    /// Outlet mole fraction of `species` [-].
    #[must_use]
    pub fn mole_fraction(&self, species: Species) -> f64 {
        let total: f64 = self.outlet_flows.iter().sum();
        if total > 0.0 {
            self.outlet_flows[species.index()] / total
        } else {
            0.0
        }
    }

    /// Hydrogen yield, moles of H₂ per mole of methane fed.
    ///
    /// The stoichiometric ceiling is **4** (the overall reaction
    /// `CH4 + 2H2O -> CO2 + 4H2`), reached only at complete conversion with
    /// complete shift.
    #[must_use]
    pub fn hydrogen_yield(&self, methane_feed: f64) -> f64 {
        if methane_feed > 0.0 {
            self.outlet_flows[Species::Hydrogen.index()] / methane_feed
        } else {
            0.0
        }
    }
}
