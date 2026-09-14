//! Parse ENDF secondary-neutron **energy distributions** and convert them to ACE
//! law form for the DLW block. **Phase 4d.**
//!
//! This is the energy-distribution counterpart of [`super::angular`]. It produces
//! the per-reaction laws stored in the ACE **DLW** block (located by **LDLW**),
//! which a transport code samples to pick a secondary neutron's outgoing energy.
//!
//! Split into a directory (file-size cap, see this crate's `CLAUDE.md`) by
//! functional group:
//! - [`core`] — the shared [`EnergyLaw`]/[`Law4`]/[`OutgoingEnergy`] types, the
//!   [`Emission`] record, [`build_emissions`], and the pdf/cdf normalisation
//!   both files below use.
//! - [`mf5`] — ENDF **MF=5** (plain secondary-energy laws): `acelf5`.
//! - [`mf6`] — ENDF **MF=6** (coupled energy-angle laws): `acelf6` + `skip6`.
//!
//! ## Status
//!
//! Implemented and wired (via [`build_emissions`] + `AceTable::from_reconr_full`):
//! - [`parse_mf5_law4`] — MF=5 LF=1 tabulated secondary energy (fission
//!   χ(E→E')) → **ACE Law 4** (faithful to `acelf5`).
//! - [`parse_mf6_law1_neutron`] — MF=6 LAW=1 neutron energy pdf `f₀` → **Law 4**.
//! - [`law3_discrete_level`] — discrete two-body levels (MT51–90) → **ACE Law 3**
//!   from Q + AWR (the inline `acelod` branch).
//! - [`EnergyLaw::serialize`] lays these into the LDLW/DLW blocks with the correct
//!   law-validity header and internal locators.
//!
//! Implemented, **parsing only** (not yet wired into [`build_emissions`]/the ACE
//! DLW writer — see each function's own doc for what it returns):
//! - [`mf5::parse_mf5_section`] — full MF=5, any `NK` (probability-weighted
//!   mixtures), dispatching `LF∈{1,7,9,11}` (`LF=5` mirrors `acelf5`'s own hard
//!   refusal; `LF=12` Madland-Nix remains genuinely unported).
//! - [`mf6::parse_mf6_law1_neutrons`] now finds a neutron subsection **after** a
//!   non-neutron one (`skip6a` ported as [`mf6::skip_mf6_subsection`]), and
//!   handles LAW=1's discrete-line case (`ND>0`).
//! - [`mf6::parse_mf6_law6_phase_space`] — MF=6 LAW=6 (n-body phase space).
//! - [`mf6::parse_mf6_law7_lab_angle_energy`] — MF=6 LAW=7 (lab angle-energy).
//!
//! Not yet implemented:
//! - **Non-elastic angular**: producers are written isotropic. MF=6 LANG=1
//!   Legendre → **Law 61**, LANG=2 Kalbach-Mann → **Law 44** (`acelf6`).
//! - **MF=5 LF=12** (Madland-Nix) — needs `acelf5`'s adaptive-linearization
//!   integral; out of the LF=1/5/7/9/11 scope this pass closed.
//! - **Fission** (MT=18) secondaries — coupled to the ν̄ (NU) block.
//!
//! ## How the DLW block is assembled (the wiring this feeds — `acelod`)
//!
//! Structural facts reverse-engineered from `acelod`/`change`, recorded here so
//! the wiring step is unambiguous:
//!
//! - **NXS(5) = NR** is the count of reactions that **produce secondary neutrons**
//!   (a *subset* of the NTR reactions in MTR), e.g. (n,2n) MT16, (n,n') levels
//!   MT51–90, continuum MT91, (n,anything) MT5, fission MT18. Elastic is separate.
//! - **TYR** (one entry per MTR reaction) is `0` for non-producers, else the
//!   neutron yield with sign = frame: `> 0` lab, `< 0` centre-of-mass, `|TYR| >
//!   100` ⇒ energy-dependent yield given in a yield table.
//! - **LDLW** has NR entries (1-based locators into DLW, in MTR order, filtered
//!   to producers). **LAND** has NR+1 entries (elastic + the NR producers).
//! - A **DLW** entry is `[LNW, LAW, IDAT, NR_app, (NBT,INT)·NR_app, NE, E(NE),
//!   P(NE)]` — the law-applicability header (LNW = next-law locator or 0; P = the
//!   probability this law applies vs incident energy) — followed at **IDAT** by
//!   the law-specific data below.
//!
//! **Consequence:** every neutron producer needs a valid DLW law, so a *partial*
//! DLW cannot yield a loadable table. Wiring Law 4 in therefore waits until the
//! MF=6 laws (44/61) cover the remaining producers for a given nuclide.
//!
//! ## ACE Law 4 data layout (what [`Law4`] serialises to, per `acelf5`)
//!
//! ```text
//! NR_in, (NBT,INT)·NR_in,        ! incident-energy interpolation
//! NE, E_in(1..NE),               ! incident energies [MeV]
//! L(1..NE),                      ! locator to each E_in's distribution
//! for each E_in:
//!   INTT, NP, E_out(1..NP), pdf(1..NP), cdf(1..NP)
//! ```
//! `INTT` is the outgoing interpolation (1 histogram, 2 lin-lin); the pdf is per
//! MeV and the cdf runs 0 → 1.

pub mod madland_nix;
mod core;
mod mf5;
mod mf6;
#[cfg(test)]
mod tests;

pub use core::{
    build_emissions, law3_discrete_level, Emission, EnergyLaw, Law4, OutgoingEnergy,
};
pub use mf5::{parse_mf5_law4, parse_mf5_section, Applicability, Mf5Law, Mf5Subsection, TabFn};
pub use mf6::{
    parse_mf6_law1_neutron, parse_mf6_law1_neutrons, parse_mf6_law6_phase_space,
    parse_mf6_law7_lab_angle_energy, Law7Incident, Law7MuTable, Mf6LabAngleEnergy, Mf6Neutron,
    Mf6PhaseSpace,
};
