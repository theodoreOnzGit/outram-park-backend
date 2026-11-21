// SPDX-License-Identifier: AGPL-3.0-only
//
// Attribution (RESEARCH_INTEGRITY_AND_PROVENANCE.md): port of OpenMC.
//   Upstream project: OpenMC, https://github.com/openmc-dev/openmc
//   Upstream files:   src/physics.cpp (sample_photon_reaction, sample_electron_reaction, sample_positron_reaction, process_charged_secondary)
//   Upstream commit:  8f79d9d0
//   Upstream licence: MIT, Copyright (c) 2011-2025 Massachusetts Institute of
//                     Technology, UChicago Argonne LLC, and OpenMC contributors
//   This port:        AGPL-3.0-only, with the MIT notice preserved here.
//! **OpenMC photon / electron / positron reaction sampling** (gh:#1683).
//!
//! **Status: first port, not differentially tested (gh:#1683).**
//!
//! Ports of `sample_photon_reaction` (`src/physics.cpp:295-468`),
//! `process_charged_secondary` (`:470-497`), `sample_electron_reaction`
//! (`:499-510`) and `sample_positron_reaction` (`:512-529`).
//!
//! # Seam toward `punggol::egsnrc`
//!
//! Upstream OpenMC does not transport charged particles: `process_charged_secondary`
//! either radiates thick-target bremsstrahlung or deposits locally. Here that
//! routine is [`process_charged_secondary`] and every charged secondary a photon
//! reaction produces is **also recorded** in [`ReactionOutcome::charged`] before
//! it is processed, so a caller that wants condensed-history transport
//! (`punggol::egsnrc`) can take those instead of calling TTB: set
//! [`ElectronTreatment::Deferred`]. Nothing from `egsnrc` is called from here.
//!
//! # Correct physics is the default
//!
//! [`PhotonSettings::new`] turns TTB ([`ElectronTreatment::Ttb`], upstream
//! `settings.cpp:113`) and atomic relaxation on. Ablations are explicit:
//! [`PhotonSettings::without_ttb`], [`PhotonSettings::without_atomic_relaxation`].
//! Pinned by `tests::correct_physics_is_default`.
//!
//! # Differences from upstream
//!
//! - `p.create_secondary` becomes a push onto [`ReactionOutcome::secondaries`].
//! - `sample_element` (choosing the element in a material) is the caller's: the
//!   element and its microscopic cross sections are arguments.
//! - `p.bank_second_E() -= 2 m_e` (the positron annihilation heating offset) is
//!   reported as [`ReactionOutcome::bank_second_e_offset`].

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::bremsstrahlung::{BremsstrahlungData, thick_target_bremsstrahlung};
use super::{MASS_ELECTRON_EV, PhotonInteraction, PhotonMicroXs, RelaxationSecondary};
use alloc::vec::Vec;
use core::f64::consts::PI;
use outram_mc_libs::geometry::position::Direction;
use outram_mc_libs::particle::particle::ParticleType;
use outram_mc_libs::physics::scatter::rotate_angle_openmc_unguarded as rotate_angle;
use petir::mathf::RealMath;
use petir::real::Real;
use petir::rng::lcg::prn;

/// `ElectronTreatment` (`settings.h`): how charged secondaries are handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElectronTreatment {
    /// Thick-target bremsstrahlung, local deposition of the rest (default).
    Ttb,
    /// Local deposition only: ablation of TTB.
    LocalDeposit,
    /// Record charged secondaries for the caller and do nothing with them (the
    /// seam for `punggol::egsnrc`).
    Deferred,
}

/// The settings `physics.cpp` reads here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhotonSettings {
    /// `settings::energy_cutoff[photon]`, eV (upstream default 1000).
    pub photon_cutoff: f64,
    /// `settings::energy_cutoff[electron]`, eV (default 1000).
    pub electron_cutoff: f64,
    /// `settings::energy_cutoff[positron]`, eV (default 1000).
    pub positron_cutoff: f64,
    /// `settings::electron_treatment`.
    pub electron_treatment: ElectronTreatment,
    /// `settings::atomic_relaxation`.
    pub atomic_relaxation: bool,
}

impl PhotonSettings {
    /// Upstream's defaults: TTB on, atomic relaxation on, 1 keV cutoffs
    /// (`settings.cpp`: `energy_cutoff = {0, 1000, 0, 0}` for neutrons and
    /// charged particles is overridden by `settings::energy_cutoff` defaults of
    /// 1000 eV for photon/electron/positron, `settings.cpp:113`).
    pub fn new() -> Self {
        Self {
            photon_cutoff: 1000.0,
            electron_cutoff: 1000.0,
            positron_cutoff: 1000.0,
            electron_treatment: ElectronTreatment::Ttb,
            atomic_relaxation: true,
        }
    }

    /// Ablation: no bremsstrahlung, charged energy deposited locally.
    pub fn without_ttb(mut self) -> Self {
        self.electron_treatment = ElectronTreatment::LocalDeposit;
        self
    }

    /// Ablation: no Auger electrons or fluorescent photons.
    pub fn without_atomic_relaxation(mut self) -> Self {
        self.atomic_relaxation = false;
        self
    }

    pub(crate) fn cutoff(&self, t: ParticleType) -> f64 {
        match t {
            ParticleType::Photon => self.photon_cutoff,
            ParticleType::Electron => self.electron_cutoff,
            ParticleType::Positron => self.positron_cutoff,
            ParticleType::Neutron => 0.0,
        }
    }
}

impl Default for PhotonSettings {
    fn default() -> Self {
        Self::new()
    }
}

/// `TallyEvent` outcomes this module sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// Nothing happened (photon below cutoff is killed with this).
    None,
    /// `TallyEvent::SCATTER`.
    Scatter,
    /// `TallyEvent::ABSORB`.
    Absorb,
}

/// The ENDF MT numbers `physics.cpp` records: 502 coherent, 504 incoherent,
/// 533 + subshell photoelectric, 516 pair production.
pub mod mt {
    /// `COHERENT`.
    pub const COHERENT: i32 = 502;
    /// `INCOHERENT`.
    pub const INCOHERENT: i32 = 504;
    /// `PAIR_PROD`.
    pub const PAIR_PROD: i32 = 516;
}

/// The particle state the reactions read and write (`Particle` subset).
#[derive(Debug, Clone, Copy)]
pub struct PhotonState {
    /// Energy, eV.
    pub e: f64,
    /// Direction.
    pub u: Direction,
    /// Weight.
    pub wgt: f64,
    /// Last scattering cosine.
    pub mu: f64,
    /// Whether the particle is in a void material (TTB skips void).
    pub in_void: bool,
    /// Last event.
    pub event: Event,
    /// Last event's MT.
    pub event_mt: i32,
}

/// A secondary particle created by a reaction (`p.create_secondary`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Secondary {
    /// Type.
    pub particle: ParticleType,
    /// Direction.
    pub u: Direction,
    /// Energy, eV.
    pub e: f64,
    /// Weight.
    pub wgt: f64,
}

/// Everything a reaction produced besides the change to [`PhotonState`].
#[derive(Debug, Clone, Default)]
pub struct ReactionOutcome {
    /// Secondaries banked by `create_secondary`.
    pub secondaries: Vec<Secondary>,
    /// Charged secondaries presented to `process_charged_secondary` (the seam).
    pub charged: Vec<Secondary>,
    /// Sum of `p.bank_second_E() -= ...` offsets, eV (negative).
    pub bank_second_e_offset: f64,
}

/// The bremsstrahlung table to use for a charged type (material's `ttb_`).
#[derive(Debug, Clone, Copy)]
pub struct TtbTables<'a> {
    /// `data::ttb_e_grid`, ln eV.
    pub e_grid: &'a [f64],
    /// Electron table.
    pub electron: &'a BremsstrahlungData,
    /// Positron table.
    pub positron: &'a BremsstrahlungData,
}

/// `isotropic_direction` (azimuth first, as `photon.cpp` uses it).
fn isotropic(seed: &mut u64) -> Direction {
    let (a, b, c) = outram_mc_libs::rng::distributions::isotropic_direction_openmc(seed);
    Direction::new(a, b, c)
}

fn neg(u: Direction) -> Direction {
    Direction::new(-u.u, -u.v, -u.w)
}

/// `Particle::create_secondary` (`particle.cpp:86-122`): a secondary whose
/// energy is below its type's cutoff is not created (`E < energy_cutoff`).
/// Every secondary of this module goes through here, as upstream's do.
fn create_secondary(out: &mut ReactionOutcome, settings: &PhotonSettings, particle: ParticleType, u: Direction, e: f64, wgt: f64) -> bool {
    if e < settings.cutoff(particle) {
        return false;
    }
    out.secondaries.push(Secondary { particle, u, e, wgt });
    true
}

/// `process_charged_secondary` (`physics.cpp:470-497`).
pub fn process_charged_secondary(
    p: &PhotonState,
    u: Direction,
    e: f64,
    particle: ParticleType,
    settings: &PhotonSettings,
    ttb: Option<&TtbTables<'_>>,
    out: &mut ReactionOutcome,
    seed: &mut u64,
) {
    out.charged.push(Secondary { particle, u, e, wgt: p.wgt });
    if settings.electron_treatment == ElectronTreatment::Deferred {
        return;
    }
    if e >= settings.cutoff(particle) && settings.electron_treatment == ElectronTreatment::Ttb {
        if let Some(t) = ttb {
            let data = if particle == ParticleType::Positron { t.positron } else { t.electron };
            for ph in thick_target_bremsstrahlung(t.e_grid, data, p.in_void, settings.photon_cutoff, e, seed) {
                create_secondary(out, settings, ParticleType::Photon, u, ph.energy, p.wgt);
            }
        }
    }
    if particle == ParticleType::Positron {
        annihilate(p.wgt, settings, out, seed);
        out.bank_second_e_offset -= 2.0 * MASS_ELECTRON_EV;
    }
}

fn annihilate(wgt: f64, settings: &PhotonSettings, out: &mut ReactionOutcome, seed: &mut u64) {
    let u = isotropic(seed);
    create_secondary(out, settings, ParticleType::Photon, u, MASS_ELECTRON_EV, wgt);
    create_secondary(out, settings, ParticleType::Photon, neg(u), MASS_ELECTRON_EV, wgt);
}

/// Hand one relaxation secondary to the particle bank / charged seam, at the
/// moment upstream creates it (so Auger-electron bremsstrahlung draws land in
/// the same place in the random stream).
fn apply_relaxation_secondary(
    p: &PhotonState,
    s: RelaxationSecondary,
    settings: &PhotonSettings,
    ttb: Option<&TtbTables<'_>>,
    out: &mut ReactionOutcome,
    seed: &mut u64,
) {
    match s {
        RelaxationSecondary::Photon { u, energy } => {
            create_secondary(out, settings, ParticleType::Photon, Direction::new(u.0, u.1, u.2), energy, p.wgt);
        }
        RelaxationSecondary::Electron { u, energy } => process_charged_secondary(
            p,
            Direction::new(u.0, u.1, u.2),
            energy,
            ParticleType::Electron,
            settings,
            ttb,
            out,
            seed,
        ),
    }
}

/// `sample_photon_reaction` (`physics.cpp:295-468`).
///
/// `micro` are the element's cross sections at `p.e`
/// ([`PhotonInteraction::calculate_xs`]); the element has been chosen by the
/// caller. `bremsstrahlung` is `None` for a material without TTB data (no
/// photons are then radiated).
pub fn sample_photon_reaction(
    p: &mut PhotonState,
    element: &PhotonInteraction,
    micro: &PhotonMicroXs,
    settings: &PhotonSettings,
    ttb: Option<&TtbTables<'_>>,
    seed: &mut u64,
) -> ReactionOutcome {
    let mut out = ReactionOutcome::default();
    if p.e < settings.photon_cutoff {
        p.e = 0.0;
        p.wgt = 0.0;
        return out;
    }
    let alpha = p.e / MASS_ELECTRON_EV;
    let mut prob = 0.0;
    let cutoff = prn(seed) * micro.total;

    // Coherent
    prob += micro.coherent;
    if prob > cutoff {
        p.mu = element.rayleigh_scatter(alpha, seed);
        p.u = rotate_angle(p.u, p.mu, None, seed);
        p.event = Event::Scatter;
        p.event_mt = mt::COHERENT;
        return out;
    }

    // Incoherent
    prob += micro.incoherent;
    if prob > cutoff {
        let (alpha_out, mu, i_shell) = element.compton_scatter(alpha, true, seed);
        p.mu = mu;
        let e_b = if i_shell == -1 { 0.0 } else { element.binding_energy[i_shell as usize] };
        let mut phi = 0.0 + (2.0 * PI - 0.0) * prn(seed);
        let e_electron = (alpha - alpha_out) * MASS_ELECTRON_EV - e_b;
        if e_electron >= settings.electron_cutoff {
            let mu_electron = (alpha - alpha_out * p.mu)
                / Real::sqrt(alpha * alpha + alpha_out * alpha_out - 2.0 * alpha * alpha_out * p.mu);
            let u = rotate_angle(p.u, mu_electron, Some(phi), seed);
            process_charged_secondary(p, u, e_electron, ParticleType::Electron, settings, ttb, &mut out, seed);
        }
        if settings.atomic_relaxation
            && element.has_atomic_relaxation
            && i_shell >= 0
            && element.subshell_map[i_shell as usize] >= 0
        {
            element.atomic_relaxation_with(
                element.subshell_map[i_shell as usize] as usize,
                p.e,
                settings.photon_cutoff,
                seed,
                |s, seed| apply_relaxation_secondary(p, s, settings, ttb, &mut out, seed),
            );
        }
        phi += PI;
        p.e = alpha_out * MASS_ELECTRON_EV;
        p.u = rotate_angle(p.u, p.mu, Some(phi), seed);
        p.event = Event::Scatter;
        p.event_mt = mt::INCOHERENT;
        return out;
    }

    // Photoelectric
    let prob_after = prob + micro.photoelectric;
    if prob_after > cutoff {
        let i_grid = micro.index_grid;
        let f = micro.interp_factor;
        let xs_lower = element.cross_sections.row(i_grid);
        let xs_upper = element.cross_sections.row(i_grid + 1);
        for i_shell in 0..element.shells.len() {
            let shell = &element.shells[i_shell];
            if xs_lower[i_shell] == 0.0 {
                continue;
            }
            prob += RealMath::r_exp(xs_lower[i_shell] + f * (xs_upper[i_shell] - xs_lower[i_shell]));
            if prob > cutoff {
                let binding_energy =
                    if element.has_atomic_relaxation { shell.binding_energy } else { element.binding_energy[i_shell] };
                let e_electron = p.e - binding_energy;
                let mu;
                loop {
                    let r = prn(seed);
                    if 4.0 * (1.0 - r) * r >= prn(seed) {
                        let rel_vel = Real::sqrt(e_electron * (e_electron + 2.0 * MASS_ELECTRON_EV))
                            / (e_electron + MASS_ELECTRON_EV);
                        mu = (2.0 * r + rel_vel - 1.0) / (2.0 * rel_vel * r - rel_vel + 1.0);
                        break;
                    }
                }
                let phi = 0.0 + (2.0 * PI - 0.0) * prn(seed);
                let s = Real::sqrt(1.0 - mu * mu);
                let u = Direction::new(mu, s * RealMath::r_cos(phi), s * RealMath::r_sin(phi));
                process_charged_secondary(p, u, e_electron, ParticleType::Electron, settings, ttb, &mut out, seed);
                if settings.atomic_relaxation {
                    element.atomic_relaxation_with(i_shell, p.e, settings.photon_cutoff, seed, |s, seed| {
                        apply_relaxation_secondary(p, s, settings, ttb, &mut out, seed)
                    });
                }
                p.event = Event::Absorb;
                p.event_mt = 533 + shell.index_subshell;
                p.wgt = 0.0;
                p.e = 0.0;
                return out;
            }
        }
    }
    prob = prob_after;

    // Pair production
    prob += micro.pair_production;
    if prob > cutoff {
        let (e_el, e_po, mu_el, mu_po) = element.pair_production(alpha, seed);
        let u = rotate_angle(p.u, mu_el, None, seed);
        process_charged_secondary(p, u, e_el, ParticleType::Electron, settings, ttb, &mut out, seed);
        let u = rotate_angle(p.u, mu_po, None, seed);
        process_charged_secondary(p, u, e_po, ParticleType::Positron, settings, ttb, &mut out, seed);
        p.event = Event::Absorb;
        p.event_mt = mt::PAIR_PROD;
        p.wgt = 0.0;
        p.e = 0.0;
    }
    out
}

/// `sample_electron_reaction` (`physics.cpp:499-510`) for an electron that has
/// reached the end of its (TTB) life: radiates, then is absorbed.
pub fn sample_electron_reaction(
    p: &mut PhotonState,
    settings: &PhotonSettings,
    ttb: Option<&TtbTables<'_>>,
    seed: &mut u64,
) -> ReactionOutcome {
    let mut out = ReactionOutcome::default();
    if settings.electron_treatment == ElectronTreatment::Ttb {
        if let Some(t) = ttb {
            for ph in thick_target_bremsstrahlung(t.e_grid, t.electron, p.in_void, settings.photon_cutoff, p.e, seed) {
                create_secondary(&mut out, settings, ParticleType::Photon, p.u, ph.energy, p.wgt);
            }
        }
    }
    p.e = 0.0;
    p.wgt = 0.0;
    p.event = Event::Absorb;
    out
}

/// `sample_positron_reaction` (`physics.cpp:512-529`): TTB, then two 511 keV
/// annihilation photons back to back.
pub fn sample_positron_reaction(
    p: &mut PhotonState,
    settings: &PhotonSettings,
    ttb: Option<&TtbTables<'_>>,
    seed: &mut u64,
) -> ReactionOutcome {
    let mut out = ReactionOutcome::default();
    if settings.electron_treatment == ElectronTreatment::Ttb {
        if let Some(t) = ttb {
            for ph in thick_target_bremsstrahlung(t.e_grid, t.positron, p.in_void, settings.photon_cutoff, p.e, seed) {
                create_secondary(&mut out, settings, ParticleType::Photon, p.u, ph.energy, p.wgt);
            }
        }
    }
    annihilate(p.wgt, settings, &mut out, seed);
    p.e = 0.0;
    p.wgt = 0.0;
    p.event = Event::Absorb;
    out
}
