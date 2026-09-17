//! Post-parse setup: particle-pair defaults, quantum-number validation, and
//! per-channel kinematic scale factors — ported from `samm.f90`'s `ppdefs`,
//! `checkqn`, and `fxradi`.
//!
//! These run once per resonance-range section, after [`super::mf2::parse_rml_section`]
//! and before any cross section is evaluated at a specific incident energy
//! (that's [`super::mf2::RmlSection`] plus [`ChannelKinematics`] feeding the
//! not-yet-ported R-matrix inversion — see `src/samm/README.md`).
//!
//! **Scope note:** `checkqn`/`fxradi`/`betset` are called unconditionally by
//! `ppsammy` for every mode (not gated on `mode==7` the way `findsp`/
//! `rearrange` are — see `mf2.rs`'s module doc), so unlike those two, these
//! genuinely are needed here. `findsp` and `rearrange` are *not* ported: they
//! exist only to route/sort resonances read from a flat, all-groups-combined
//! list (the SLBW/MLBW-style `mode`s), and are dead code for `mode==7`, whose
//! resonances are already read per spin-group — see `mf2.rs`'s
//! [`super::mf2::SpinGroup::resonances`], which stores them that way by
//! construction. `orders` (a generic sort+dedup helper used elsewhere for
//! energy-grid node lists) is also not yet ported — it belongs to Phase 6
//! (top-level orchestration / PENDF grid generation), not this phase.

use crate::common::phys::{
    ANRATIO, CLIGHT_CM_S, DNRATIO, EV_ERG, HBAR_ERG_S, HNRATIO, INV_FINE_STRUCT, PNRATIO, TNRATIO,
};
use crate::samm::mf2::{ParticlePair, SpinGroup};
use crate::NjoyError;

/// Fill in particle-pair defaults implied by `MT` when the ENDF evaluator
/// left a field zeroed — ported from `ppdefs` (`samm.f90:1414-1468`). Mass
/// ratios and spins for standard light-particle exit channels (n, p, d, t,
/// ³He, α), fission (`MT=18`, penetrability disabled), and inelastic
/// (`51<=MT<99`, treated as a neutron-like exit channel).
pub fn apply_particle_pair_defaults(pairs: &mut [ParticlePair]) {
    for pp in pairs.iter_mut() {
        match pp.mt {
            2 => {
                pp.mass_a = 1.0;
                pp.spin_a = 0.5;
                pp.z_a = 0;
                pp.penetrability_flag = 1;
            }
            18 => {
                pp.penetrability_flag = 0;
            }
            mt if mt > 50 && mt < 99 => {
                pp.mass_a = 1.0;
                pp.spin_a = 0.5;
                pp.z_a = 0;
                pp.penetrability_flag = 1;
            }
            103 => {
                pp.mass_a = PNRATIO;
                pp.spin_a = 0.5;
                pp.z_a = 1;
                pp.penetrability_flag = 1;
            }
            104 => {
                pp.mass_a = DNRATIO;
                pp.spin_a = 1.0;
                pp.z_a = 1;
                pp.penetrability_flag = 1;
            }
            105 => {
                pp.mass_a = TNRATIO;
                pp.spin_a = 0.5;
                pp.z_a = 1;
                pp.penetrability_flag = 1;
            }
            106 => {
                pp.mass_a = HNRATIO;
                pp.spin_a = 0.5;
                pp.z_a = 2;
                pp.penetrability_flag = 1;
            }
            107 => {
                pp.mass_a = ANRATIO;
                pp.spin_a = 0.0;
                pp.parity_a = 0.0;
                pp.z_a = 2;
                pp.penetrability_flag = 1;
            }
            _ => {}
        }
    }
}

/// Per-group statistical/entrance-channel bookkeeping computed by
/// `checkqn` — `goj` is the spin statistical weight `g_J = (2|J|+1) /
/// ((2|s_a|+1)(2|s_b|+1))` of this group's (first) entrance channel, used
/// later by the cross-section formula (`crosss`, not yet ported).
#[derive(Debug, Clone, Copy, Default)]
pub struct GroupQuantumInfo {
    /// Statistical spin weight `g_J` of the entrance channel, `0.0` if this
    /// group has no entrance (`MT=2`) channel.
    pub goj: f64,
    /// Number of entrance (`MT=2`) channels in this group.
    pub n_entrance: usize,
    /// Number of exit channels with particle-pair index `>2` in this group
    /// (i.e. excluding the eliminated capture pair and the entrance pair).
    pub n_exit: usize,
}

/// Validate quantum numbers and compute [`GroupQuantumInfo`] for every spin
/// group — ported from `checkqn` (`samm.f90:1754-1851`).
///
/// Two conditions are hard errors upstream (`call error`/`call wrongi`) and
/// stay hard errors here: a group spin that is not a multiple of `1/2`, and
/// a negative channel orbital angular momentum `l`. Everything else upstream
/// is a diagnostic `write` to the output listing, not a fatal error; those
/// are ported as [`log::warn!`] calls instead (parity mismatches between
/// `J`, `l`, and the channel spin sign; `|channel_spin|` outside
/// `[|s_a-s_b|, s_a+s_b]`; and, for penetrability-enabled channels, `|J|`
/// outside `[|l-channel_spin|, l+channel_spin]`).
pub fn check_quantum_numbers(
    pairs: &[ParticlePair],
    groups: &[SpinGroup],
) -> Result<Vec<GroupQuantumInfo>, NjoyError> {
    let mut out = Vec::with_capacity(groups.len());

    for (j, group) in groups.iter().enumerate() {
        if (group.j % 0.5).abs() > 1.0e-9 {
            return Err(NjoyError::EndfParse(format!(
                "samm: spin group number {} has invalid spin J={}",
                j + 1,
                group.j
            )));
        }

        let mut info = GroupQuantumInfo::default();

        for (n, ch) in group.channels.iter().enumerate() {
            let pair = pairs.get(ch.particle_pair - 1).ok_or_else(|| {
                NjoyError::EndfParse(format!(
                    "samm: group {} channel {} references undeclared particle pair {}",
                    j + 1,
                    n + 1,
                    ch.particle_pair
                ))
            })?;

            if ch.particle_pair == 2 {
                info.n_entrance += 1;
                if info.goj == 0.0 {
                    info.goj = (2.0 * group.j.abs() + 1.0)
                        / ((2.0 * pair.spin_a.abs() + 1.0) * (2.0 * pair.spin_b.abs() + 1.0));
                }
            } else if ch.particle_pair > 2 {
                info.n_exit += 1;
            }

            if ch.l < 0 {
                return Err(NjoyError::EndfParse(format!(
                    "samm: group {} channel {} has negative orbital angular momentum l={}",
                    j + 1,
                    n + 1,
                    ch.l
                )));
            }

            if ch.channel_spin != 0.0 && group.j != 0.0 {
                let mut x = 1.0_f64;
                if ch.l % 2 != 0 {
                    x = -x;
                }
                if ch.channel_spin < 0.0 {
                    x = -x;
                }
                if group.j < 0.0 {
                    x = -x;
                }
                if x < 0.0 {
                    log::warn!(
                        "samm: parity problem in group {} channel {} (J={}, l={}, channel_spin={})",
                        j + 1,
                        n + 1,
                        group.j,
                        ch.l,
                        ch.channel_spin
                    );
                }
            }

            let smin = (pair.spin_a.abs() - pair.spin_b.abs()).abs();
            let smax = pair.spin_a.abs() + pair.spin_b.abs();
            if ch.channel_spin.abs() < smin {
                log::warn!(
                    "samm: group {} channel {}: |channel_spin|={} < |spin_a-spin_b|={}",
                    j + 1,
                    n + 1,
                    ch.channel_spin.abs(),
                    smin
                );
            } else if ch.channel_spin.abs() > smax {
                log::warn!(
                    "samm: group {} channel {}: |channel_spin|={} > spin_a+spin_b={}",
                    j + 1,
                    n + 1,
                    ch.channel_spin.abs(),
                    smax
                );
            }

            if pair.penetrability_flag > 0 {
                let smin = ((ch.l as f64) - ch.channel_spin.abs()).abs();
                let smax = ch.l as f64 + ch.channel_spin.abs();
                if group.j.abs() < smin {
                    log::warn!(
                        "samm: group {} channel {}: |J|={} < |l-channel_spin|={}",
                        j + 1,
                        n + 1,
                        group.j.abs(),
                        smin
                    );
                } else if group.j.abs() > smax {
                    log::warn!(
                        "samm: group {} channel {}: |J|={} > l+channel_spin={}",
                        j + 1,
                        n + 1,
                        group.j.abs(),
                        smax
                    );
                }
            }
        }

        out.push(info);
    }

    Ok(out)
}

/// Energy-independent, per-channel kinematic scale factors — ported from
/// `fxradi` (`samm.f90:1853-1907`).
///
/// `zke` is the wave-number scale factor `k/sqrt(E)` for this channel (so
/// `rho = zke * radius * sqrt(E)` at the actual incident energy `E`, once
/// the not-yet-ported cross-section formula needs it); `zkfe`/`zkte` are
/// `zke` pre-multiplied by the effective/true channel radius; `zeta` is the
/// analogous Coulomb `eta/sqrt(E)` scale factor (`0.0` for uncharged
/// channels); `echan` is the channel threshold energy, either taken as given
/// (always `0.0` for LRF=7 — the ENDF channel record carries no explicit
/// threshold field, see this crate's [`super::mf2`]) or derived from the
/// pair's `Q`-value.
#[derive(Debug, Clone, Copy, Default)]
pub struct ChannelKinematics {
    pub zke: f64,
    pub zkfe: f64,
    pub zkte: f64,
    pub zeta: f64,
    pub echan: f64,
}

/// `awr`: the target nuclide's mass in neutron-mass units (ENDF `AWR`,
/// read from the MF=2 section head — `samm.f90:202`, one level up from
/// [`super::mf2::parse_rml_section`]'s own CONT read, so it is threaded in
/// here as a parameter rather than stored on [`super::mf2::RmlSection`]).
pub fn compute_channel_kinematics(
    pairs: &mut [ParticlePair],
    groups: &[SpinGroup],
    awr: f64,
) -> Result<Vec<Vec<ChannelKinematics>>, NjoyError> {
    // samm.f90:1874-1881 -- generic entrance/target mass defaults, applied
    // after `apply_particle_pair_defaults`'s MT-specific ones.
    for pp in pairs.iter_mut() {
        if pp.mass_a == 0.0 {
            pp.mass_a = 1.0;
        }
        if pp.mass_b == 0.0 {
            pp.mass_b = awr;
        }
    }

    // samm.f90:1864-1872 -- physical-constant combinations, in NJOY's
    // internal units (channel radius in 1e-15 m = fm, hence `ff=1e15`).
    let hbar_ev_s = HBAR_ERG_S / EV_ERG;
    let amu_ev = crate::common::phys::AMU_G * CLIGHT_CM_S * CLIGHT_CM_S / EV_ERG;
    let cspeed_m_s = CLIGHT_CM_S / 100.0;
    let fine_struct = 1.0 / INV_FINE_STRUCT;
    const FF: f64 = 1.0e15;
    let amassn = crate::common::phys::AMASSN_AMU;
    let twomhb = (2.0 * amassn * amu_ev).sqrt() / (hbar_ev_s * FF * cspeed_m_s);
    let etac = fine_struct * amu_ev / (hbar_ev_s * FF * cspeed_m_s) * amassn;

    // samm.f90:1883-1886 -- entrance-channel (particle pair 2) lab->cm
    // conversion factor, the same for every group/channel. `pairs[1]` is
    // particle-pair index 2 (1-indexed) -- the ENDF LRF=7 convention that
    // pair 1 is always the eliminated capture pair and pair 2 is always the
    // entrance (elastic) pair.
    let entrance = pairs.get(1).ok_or_else(|| {
        NjoyError::EndfParse(
            "samm: fxradi requires at least 2 particle pairs (eliminated + entrance)".to_string(),
        )
    })?;
    let factor0 = entrance.mass_b + entrance.mass_a;
    let alabcm = entrance.mass_b / factor0;
    let factor = alabcm / entrance.mass_a;

    groups
        .iter()
        .enumerate()
        .map(|(j, group)| {
            group
                .channels
                .iter()
                .enumerate()
                .map(|(n, ch)| {
                    let pair = pairs.get(ch.particle_pair - 1).ok_or_else(|| {
                        NjoyError::EndfParse(format!(
                            "samm: group {} channel {} references undeclared particle pair {}",
                            j + 1,
                            n + 1,
                            ch.particle_pair
                        ))
                    })?;
                    let aa_sum = pair.mass_b + pair.mass_a;
                    let aa = pair.mass_b / aa_sum;
                    let redmas = aa * pair.mass_a;
                    let z = twomhb * (redmas * factor).sqrt();

                    let echan = if pair.q_value != 0.0 {
                        -pair.q_value / alabcm
                    } else {
                        0.0
                    };

                    let zeta = if pair.z_a != 0 && pair.z_b != 0 {
                        etac * (pair.z_b as f64) * (pair.z_a as f64) * redmas / z
                    } else {
                        0.0
                    };

                    Ok(ChannelKinematics {
                        zke: z,
                        zkfe: z * ch.radius_eff * 10.0,
                        zkte: z * ch.radius_true * 10.0,
                        zeta,
                        echan,
                    })
                })
                .collect::<Result<Vec<_>, NjoyError>>()
        })
        .collect()
}
