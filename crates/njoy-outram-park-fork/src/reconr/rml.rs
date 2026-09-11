//! RECONR's dispatch of an R-matrix-limited (`LRF=7`) range into
//! [`crate::samm`] — `emerge`'s `itype = 2+i` channels and `rdfil2`'s
//! resonance-node seeding for the SAMM formalism (`reconr.f90:284-290`,
//! `331-340`, `4755-4767`). Split out of `mod.rs` on 2026-09-11 (file-size
//! rule); the shared grid machinery (`rebuild_range`, `RangeDelta`) stays
//! there.

use super::mf2::EnergyRange;
use super::{rebuild_range, RangeDelta, ReconrSection, MAX_OTHER};

/// Add R-Matrix Limited (LRF=7) resonance contributions — dispatches into
/// `crate::samm`, which handles the full multichannel R-matrix rather than
/// a pole approximation (needed for light nuclides / strongly overlapping
/// resonances that SLBW/MLBW/Reich-Moore can't represent correctly).
///
/// Runs `samm::setup::setup` once per range (the one-time, per-section
/// spin/parity/penetrability/channel-amplitude setup — Phase 2 of the
/// `samm` port), then evaluates `samm::xsformula::cssammy` at every grid
/// energy, exactly mirroring [`add_rm_range`]'s shape.
pub(super) fn add_rml_range(sections: &mut Vec<ReconrSection>, range: &EnergyRange, eps: f64) {
    let Some(rml) = &range.rml else { return };
    if rml.section.spin_groups.is_empty() {
        return;
    }

    // `samm::setup::setup` mutates particle-pair defaults in place, so it
    // needs an owned, mutable copy rather than the shared `&EnergyRange`.
    let mut section = rml.section.clone();
    let setup = match crate::samm::setup::setup(&mut section, rml.awr) {
        Ok(s) => s,
        Err(e) => {
            log::error!(
                "reconr: samm::setup::setup failed for LRF=7 range [{}, {}]: {e}",
                range.el,
                range.eh
            );
            return;
        }
    };

    let mut halo = Vec::new();
    add_rml_halo_energies(&mut halo, &section, range.el, range.eh);

    // The extra particle-pair channels (reconr.f90:284-290, 331-340,
    // 4762-4767): every pair beyond (gamma, n) that is not fission gets its
    // own MF=3 section contribution, MT 103..107 remapped to 600..800.
    let other_mts: Vec<i32> = section
        .particle_pairs
        .iter()
        .skip(2)
        .map(|p| p.mt)
        .filter(|&mt| mt != 18)
        .map(|mt| match mt {
            103 => 600,
            104 => 650,
            105 => 700,
            106 => 750,
            107 => 800,
            x => x,
        })
        .take(MAX_OTHER)
        .collect();

    rebuild_range(sections, range.el, range.eh, halo, eps, &other_mts, |e| {
        let r = crate::samm::xsformula::cssammy(
            &section,
            &setup.kinematics,
            &setup.amplitudes,
            &setup.quantum_info,
            e,
        );
        let mut other = [0.0; MAX_OTHER];
        for (k, (_, v)) in r.other.iter().take(MAX_OTHER).enumerate() {
            other[k] = *v;
        }
        RangeDelta {
            total: r.total,
            elastic: r.elastic,
            fission: r.fission,
            capture: r.capture,
            other,
        }
    });
}

/// Add a halo of energy points around each R-Matrix Limited resonance peak.
///
/// Total width proxy is `|Gamma_gamma| + sum(|Gamma_c|)` over every explicit
/// channel — [`crate::samm::mf2::RmlResonance`] has no single "total width"
/// field the way SLBW/Reich-Moore resonances do (LRF=7 channels are
/// per-spin-group, not a fixed six-column layout), so this sums what's
/// available per resonance instead.
fn add_rml_halo_energies(
    grid: &mut Vec<f64>,
    section: &crate::samm::mf2::RmlSection,
    el: f64,
    eh: f64,
) {
    const OFFSETS: &[f64] = &[
        -10.0, -5.0, -2.0, -1.0, -0.5, -0.25, 0.0, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0,
    ];
    for group in &section.spin_groups {
        for res in &group.resonances {
            if res.energy <= 0.0 {
                continue;
            }
            let gt: f64 =
                res.gamma_gamma.abs() + res.channel_widths.iter().map(|w| w.abs()).sum::<f64>();
            let half_g = gt / 2.0;
            grid.push(res.energy);
            for &off in OFFSETS {
                let e = res.energy + off * half_g;
                if e > el && e < eh && e > 0.0 {
                    grid.push(e);
                }
            }
        }
    }
}

