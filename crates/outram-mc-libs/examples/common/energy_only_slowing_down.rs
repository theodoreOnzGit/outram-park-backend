// SPDX-License-Identifier: GPL-3.0

//! **Slowing down in energy only**: the collision kernel of the transport
//! loop with the geometry taken away, shared by the examples that need it.
//!
//! Moved out of `u238_resonance_escape.rs` on 2026-10-04 (GitHub #524) and
//! generalised from two nuclides to any mixture, so that
//! `ugraphite_four_factor.rs` can compare its transport `p` against the same
//! energy-only walk at the same composition, rather than against a second copy
//! of this loop. Each example pulls it in with
//! `#[path = "common/energy_only_slowing_down.rs"] mod energy_only_slowing_down;`.
//! A subdirectory without `main.rs` is not an example target, so Cargo does
//! not build this file on its own.
//!
//! # What it mirrors, and what it leaves out
//!
//! A neutron starts at `e_source` and is followed **in energy only** until it
//! is absorbed or falls to `e_cut`. Each collision is the one
//! `physics::transport_csg::transport_history_vr` makes:
//!
//! - the nuclide is sampled by `N_i σ_t,i` (`Material::sample_nuclide_urr`);
//! - the reaction by partitioning `ξ·σ_t` into absorption / inelastic /
//!   (n,2n) / elastic, in that order;
//! - inelastic levels take their own MF=4 CM cosine, the continuum the
//!   evaluated MF=6 law (as transport does);
//! - elastic below a nuclide's S(α,β) cutoff goes through
//!   `Nuclide::sample_thermal`; otherwise free gas with the target's thermal
//!   motion below `400 kT`, through `free_gas_elastic_scatter_dbrc` with the
//!   nuclide's DBRC table (on by default since 2026-09-20);
//! - unresolved-resonance self-shielding through `xs_at_energy_urr`, with the
//!   band drawn from a separate stream advanced after every collision
//!   (`transport_csg`'s `urr_seed` handling, OpenMC `calculate_urr_xs`).
//!
//! **Left out:** space (no geometry, no leakage), fission as a source (a
//! fission ends the history like a capture), and the `(n,2n)` Q-value (see the
//! inline comment). DBRC and URR can be switched off with [`KernelOptions`],
//! which is how the pre-2026-10-04 behaviour of `u238_resonance_escape.rs` is
//! reproduced.

#![allow(dead_code)]

use outram_mc_libs::geometry::position::Direction;
use outram_mc_libs::material::material::urr_xi;
use outram_mc_libs::material::nuclide::{Inelastic, Nuclide};
use outram_mc_libs::physics::scatter::{
    continuum_inelastic_scatter_evaluated, free_gas_elastic_scatter_dbrc, two_body_scatter,
    two_body_scatter_with_mu, K_BOLTZMANN_EV_PER_K,
};
use outram_mc_libs::rng::lcg::{future_seed, prn};

/// ENDF MT of the continuum inelastic channel, whose evaluated MF=6 emission
/// law the kernel looks up. U-238's MT=91 threshold is 435.6 keV, so below a
/// 100 keV source this arm is unreachable; it is wired anyway so the kernel
/// matches transport's if the source is raised.
const MT_CONTINUUM_INELASTIC: i32 = 91;

/// ENDF MT of the (n,2n) channel, likewise unreachable below ~6 MeV on U-238.
const MT_N2N: i32 = 16;

/// Which optional physics the kernel applies. The [`Default`] is everything
/// on, matching transport (workspace rule: correct physics is the default).
#[derive(Debug, Clone, Copy)]
pub struct KernelOptions {
    /// DBRC (resonance elastic scattering) through each nuclide's own table.
    pub dbrc: bool,
    /// URR probability-table self-shielding.
    pub urr: bool,
    /// Free-gas `kT` from the nuclide's data temperature
    /// (`Nuclide::free_gas_kt`, as transport) when true; from
    /// `K_BOLTZMANN_EV_PER_K * temp_k` when false. Identical for an ENDF build
    /// at `temp_k`; kept so the pre-2026-10-04 arithmetic is reproducible bit
    /// for bit.
    pub kt_from_data: bool,
}

impl Default for KernelOptions {
    fn default() -> Self {
        Self {
            dbrc: true,
            urr: true,
            kt_from_data: true,
        }
    }
}

impl KernelOptions {
    /// The kernel `u238_resonance_escape.rs` ran until 2026-10-04: no DBRC, no
    /// URR, `kT = k_B T`.
    pub fn legacy() -> Self {
        Self {
            dbrc: false,
            urr: false,
            kt_from_data: false,
        }
    }
}

/// Where the source neutrons ended up, as counts.
#[derive(Debug, Clone)]
pub struct SlowDownCounts {
    /// Absorptions (capture or fission) in each mixture component, in the
    /// caller's order.
    pub absorbed_by: Vec<usize>,
    /// Neutrons (source and (n,2n) secondaries) that reached `e_cut` alive.
    pub escaped: usize,
    /// Source neutrons followed.
    pub histories: usize,
}

/// Follow `histories` neutrons in **energy only** from `e_source` down to
/// `e_cut`, through the mixture `nuclides[i]` at `densities[i]`
/// \[atoms/barn·cm\], and count where they end up.
///
/// With two components `[moderator, absorber]` and [`KernelOptions::legacy`]
/// this is draw-for-draw the loop `u238_resonance_escape.rs` carried before
/// 2026-10-04 (the nuclide choice `prn·Σ_t < Σ_mod` is the first step of the
/// cumulative search below, and no other draw moved).
#[allow(clippy::too_many_arguments)]
pub fn slow_down(
    nuclides: &[Nuclide],
    densities: &[f64],
    temp_k: f64,
    e_source: f64,
    e_cut: f64,
    histories: usize,
    seed: &mut u64,
    opts: KernelOptions,
) -> SlowDownCounts {
    assert_eq!(nuclides.len(), densities.len(), "one density per nuclide");
    let n = nuclides.len();
    let u = Direction::new(0.0, 0.0, 1.0);
    let mut absorbed_by = vec![0usize; n];
    let mut escaped = 0usize;
    // A separate URR stream, advanced once per collision, as transport does
    // (`urr_seed = future_seed(n_nuclides, urr_seed)`). Kept off the main
    // stream so switching URR on or off moves no other draw.
    let mut urr_seed: u64 = 0x5552_525F_5345_4544;

    for _ in 0..histories {
        let mut stack: Vec<f64> = vec![e_source];
        while let Some(mut e) = stack.pop() {
            loop {
                if e <= e_cut {
                    escaped += 1;
                    break;
                }
                if opts.urr {
                    urr_seed = future_seed(n as u64, urr_seed);
                }
                let xs: Vec<_> = nuclides
                    .iter()
                    .enumerate()
                    .map(|(i, nuc)| {
                        if opts.urr && nuc.needs_urr_draw(e) {
                            nuc.xs_at_energy_urr(e, temp_k, urr_xi(i, urr_seed))
                        } else {
                            nuc.xs_at_energy(e, temp_k)
                        }
                    })
                    .collect();
                let st: f64 = densities.iter().zip(&xs).map(|(d, x)| d * x.total).sum();
                if !(st > 0.0) {
                    break;
                }
                // Which nuclide: proportional to N_i sigma_t,i.
                let mut pick = prn(seed) * st;
                let mut i = n - 1;
                for (k, (d, x)) in densities.iter().zip(&xs).enumerate() {
                    let w = d * x.total;
                    if pick < w {
                        i = k;
                        break;
                    }
                    pick -= w;
                }
                let nuc = &nuclides[i];
                let x = &xs[i];

                let xi = prn(seed) * x.total;
                if xi < x.absorption {
                    absorbed_by[i] += 1;
                    break;
                } else if xi < x.absorption + x.inelastic {
                    e = match nuc.sample_inelastic(e, seed) {
                        Inelastic::Level { q, mt } => match nuc.sample_inelastic_mu_cm(mt, e, seed)
                        {
                            Some(mu_cm) => {
                                two_body_scatter_with_mu(e, u, nuc.awr, q, mu_cm, seed).0
                            }
                            None => two_body_scatter(e, u, nuc.awr, q, seed).0,
                        },
                        Inelastic::Continuum { q } => {
                            continuum_inelastic_scatter_evaluated(
                                e,
                                u,
                                nuc.awr,
                                q,
                                nuc.continuum_law(MT_CONTINUUM_INELASTIC),
                                seed,
                            )
                            .0
                        }
                    };
                } else if xi < x.absorption + x.inelastic + x.n2n {
                    // (n,2n) takes its own evaluated MF=6 law, like transport.
                    // The `0.0` Q is the one thing still approximated: MT=16's
                    // QI is not carried on this path (GitHub #192), and it only
                    // bounds the outgoing energy, which the evaluated law
                    // already respects.
                    let e2 = continuum_inelastic_scatter_evaluated(
                        e,
                        u,
                        nuc.awr,
                        0.0,
                        nuc.continuum_law(MT_N2N),
                        seed,
                    )
                    .0;
                    stack.push(e2); // yield - 1 = 1 secondary
                    e = e2;
                } else if let Some((e_out, _mu_lab)) = nuc.sample_thermal(e, seed) {
                    // Bound-atom S(alpha,beta) below the table's cutoff, as
                    // transport. `None` (no table, or above the cutoff) draws
                    // nothing, so a mixture without tables is unaffected.
                    e = e_out;
                } else {
                    let mu_cm = nuc
                        .sample_elastic_mu_cm(e, seed)
                        .unwrap_or_else(|| 2.0 * prn(seed) - 1.0);
                    let kt = if opts.kt_from_data {
                        nuc.free_gas_kt(temp_k)
                    } else {
                        K_BOLTZMANN_EV_PER_K * temp_k
                    };
                    let table = if opts.dbrc { nuc.dbrc_table() } else { None };
                    e = free_gas_elastic_scatter_dbrc(e, u, nuc.awr, kt, mu_cm, seed, table).0;
                }
                if !(e > 0.0) || !e.is_finite() {
                    break;
                }
            }
        }
    }
    SlowDownCounts {
        absorbed_by,
        escaped,
        histories,
    }
}
