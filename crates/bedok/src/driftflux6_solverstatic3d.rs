//! Multichannel wrapper for the staggered six-equation two-fluid solver.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `driftflux6_solverstatic3d.m`,
//!   `main_exec_diff3d_standalone` snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.
//!
//! # Read this first: the solver this wraps is missing from the handover
//!
//! Every channel's actual solve is delegated to **`driftflux6_solverstatic1d.m`,
//! which is not in the snapshot** — one of the five referenced-but-absent files
//! `docs/bedok-reference-defects.md` records. So this file cannot do its job as
//! shipped, in MATLAB or here.
//!
//! What it does instead is not undefined, though, and that is why the module is
//! translated rather than skipped. The reference wraps each channel solve in a
//! `try`/`catch` that keeps the channel's previous state and warns. In MATLAB,
//! calling a missing function raises `Undefined function` — which that `catch`
//! swallows. **So the shipped snapshot's real behaviour is: every powered
//! channel fails, warns, and retains its previous state, after which the
//! derived-field tail runs over those unchanged states.** This translation
//! reproduces exactly that, and reports it through
//! [`ChannelOutcome::SolverMissing`] so a caller cannot mistake it for a
//! converged solve.
//!
//! The consequence for the layer above: `th_solverxyz.m` chooses between this
//! and [`crate::singleflow1devap`] on `params.th_model`, and **only the `'hem'`
//! branch can actually run**. The NEACRP D1 BWR case sets `th_model = 'hem'`,
//! so the benchmark path is unaffected.
//!
//! # What *is* translated and does work
//!
//! - The channel sharding and the previous-state defaults.
//! - The warm-start admission policy (below).
//! - **The whole derived-field recovery tail** — pressures, phase densities,
//!   mixture density and velocity, enthalpies, quality and the three liquid
//!   transport properties, all from the IAPWS layer. This is real, testable
//!   code and it runs over whatever states the channels hold.
//!
//! # The warm-start policy, which is the interesting part
//!
//! A channel's previous solution is reused as a starting guess **only if both**
//! hold: that solve converged (`relerr < 1e-3`), and the wall heat flux has
//! moved less than 20% since. The reference's own comment explains why — an
//! unconverged mid-march state is a "poisoned seed", and under a hard flux ramp
//! the evaporation seed rebuilt from the *current* flux tracks the problem
//! better than a stale converged one. A seed is likewise only *stored* from a
//! converged solve.
//!
//! # What is deliberately not reproduced
//!
//! **The `parfor` sharding.** The reference runs the channels over a MATLAB
//! process pool, with `params.stag6_par` and `params.stag6_nworkers` to control
//! it and an automatic serial fallback. Channels are independent, so this is a
//! pure performance choice with no effect on results; the translation runs them
//! serially. Re-introducing parallelism here is a free change whenever it is
//! worth making.
//!
//! **The `evalc` log capture.** The reference wraps the channel call in `evalc`
//! purely to swallow the JFNK solver's per-iteration printing, which it notes
//! would otherwise flood the coupled log at ~2 MB/cycle. Nothing here prints.

use crate::error::BedokError;
use crate::iapws_if97::basic::{cp1_pt, h1_pt, h2_pt, hl_p, hv_p, v1_pt, v2_pt};
use crate::iapws_if97::region4::tsat_p;
use crate::iapws_if97::transport::{k_pt, mu_pt};
use crate::matlab::Array2;
use crate::types::{Geometry, Params, Th};

/// What happened to one channel.
///
/// The reference tracks this in the `warm` / `fail` flag arrays and prints a
/// summary line; returning it lets a caller act on the fact that nothing was
/// solved, which a printed line does not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelOutcome {
    /// The column carries no power, so it was skipped and keeps its previous
    /// state. This is the reference's `if ~any(pwch); return; end`.
    Unpowered,
    /// The channel is powered and would have been solved, but
    /// `driftflux6_solverstatic1d.m` is absent from the snapshot. The previous
    /// state is retained, reproducing the reference's `catch`.
    SolverMissing,
}

/// The per-channel bookkeeping the wrapper returns alongside the updated state.
#[derive(Clone, Debug, Default)]
pub struct ChannelReport {
    /// One outcome per channel, in `ix * maxiy + iy` order.
    pub outcomes: Vec<ChannelOutcome>,
    /// How many channels carried power and so attempted a solve.
    pub powered: usize,
    /// How many would have been given a warm start, had the solver existed.
    ///
    /// Computed from the reference's admission policy against the incoming
    /// `stag6_*` store, so it is a faithful count even though no solve runs.
    pub warm_eligible: usize,
}

/// `th = driftflux6_solverstatic3d(params, geometry, th, pwrdens)`.
///
/// # Arguments
///
/// - `params` — the three extents.
/// - `geometry` — needs `Lz`.
/// - `th` — the incoming T-H state; its coolant fields supply the
///   previous-state defaults, and `stag6_ustag` / `stag6_qref` /
///   `stag6_relerr` the warm-start store.
/// - `pwrdens` — power density per node; a column with none is skipped.
///
/// # Returns
///
/// `(th, report)` — the updated state with every derived field recovered, and
/// the per-channel [`ChannelReport`].
///
/// # This does not solve anything
///
/// See the module docs. Every powered channel reports
/// [`ChannelOutcome::SolverMissing`] and keeps its previous state; the derived
/// fields are then recovered over those states, which is exactly what the
/// shipped MATLAB does. A caller wanting a channel model that works should use
/// [`crate::singleflow1devap`].
///
/// # Errors
///
/// Never — the missing solver is reported per channel, not as a call failure,
/// because that is how the reference behaves. [`missing_solver`] is provided
/// for a caller that would rather have the error value.
///
/// # Panics
///
/// If `pwrdens` or `geometry.lz` is shorter than the node count.
pub fn driftflux6_solverstatic3d(
    params: &Params,
    geometry: &Geometry,
    th: &Th,
    pwrdens: &[f64],
) -> (Th, ChannelReport) {
    let (maxix, maxiy, maxiz) = crate::handle3dcoords::handle3dcoords(params);
    let es = maxix * maxiy * maxiz;
    let nch = maxix * maxiy;

    assert!(pwrdens.len() >= es, "pwrdens is {} long, need {es}", pwrdens.len());
    assert!(
        geometry.lz.len() >= es,
        "geometry.lz is {} long, need {es}",
        geometry.lz.len()
    );

    let inletpress = th.coolant.inletpress;
    let inlett = th.coolant.inlettemp;
    let alphagin = th.coolant.inletvoid;
    let tsatin = tsat_p(inletpress);
    let ldensin = 1.0 / v1_pt(inletpress, inlett) / 1000.0;
    let gdensin = 1.0 / v2_pt(inletpress, tsatin + 2.0 * f64::EPSILON) / 1000.0;

    // `max(th.flowrate)` — the reference takes the largest channel flux for the
    // inlet velocity, whatever the per-channel values are.
    let max_flow = match &th.flowrate {
        crate::types::MassFlux::Uniform(g) => *g,
        crate::types::MassFlux::PerNode(v) => v.iter().copied().fold(f64::NEG_INFINITY, f64::max),
    };
    let vm_in = max_flow / (alphagin * gdensin + (1.0 - alphagin) * ldensin);

    // Previous-state defaults, used by unpowered columns and failed solves.
    let default_or = |field: &Vec<f64>, d: f64| -> Vec<f64> {
        if field.len() == es {
            field.clone()
        } else {
            vec![d; es]
        }
    };
    let press = default_or(&th.coolant.press, inletpress);
    let alphag = default_or(&th.coolant.alphag, alphagin.max(1e-9));
    let vliq = default_or(&th.coolant.vliq, vm_in);
    let vgas = default_or(&th.coolant.vgas, vm_in);
    let tempsliq = default_or(&th.coolant.tempsliq, inlett);
    let tempsgas = default_or(&th.coolant.tempsgas, tsatin);

    // Per-channel pass. Every powered channel would call the absent solver.
    let mut outcomes = Vec::with_capacity(nch);
    let mut powered = 0usize;
    let mut warm_eligible = 0usize;

    let seeds_present = th.stag6_ustag.rows() == 6 * maxiz && th.stag6_ustag.cols() == nch;
    let qref_present = th.stag6_qref.rows() == maxiz && th.stag6_qref.cols() == nch;
    let rel_present = th.stag6_relerr.len() == nch;

    for c in 0..nch {
        let col = c * maxiz;
        let has_power = (0..maxiz).any(|iz| pwrdens[col + iz] != 0.0);
        if !has_power {
            outcomes.push(ChannelOutcome::Unpowered);
            continue;
        }
        powered += 1;

        // The admission policy, evaluated faithfully even though the seed can
        // go nowhere: a converged previous solve whose wall flux has moved by
        // less than 20%.
        if seeds_present && qref_present && rel_present {
            let any_seed = (0..6 * maxiz).any(|r| th.stag6_ustag.get(r, c) != 0.0);
            let converged = th.stag6_relerr[c] < 1e-3;
            if any_seed && converged {
                let mut dq = 0.0;
                let mut qn = 0.0;
                for iz in 0..maxiz {
                    let qold = th.stag6_qref.get(iz, c);
                    let qnew = th.heatflux.get(col + iz).copied().unwrap_or(0.0);
                    dq += (qnew - qold) * (qnew - qold);
                    qn += qold * qold;
                }
                if dq.sqrt() <= 0.2 * qn.sqrt().max(1e-12) {
                    warm_eligible += 1;
                }
            }
        }

        outcomes.push(ChannelOutcome::SolverMissing);
    }

    // ---- recover the derived fields over the whole domain ----
    // Direct `(p, T)` IAPWS, with each phase forced onto its own branch.
    let mut ldens = vec![0.0; es];
    let mut gdens = vec![0.0; es];
    let mut dens = vec![0.0; es];
    let mut vm = vec![0.0; es];
    let mut enth = vec![0.0; es];
    let mut quality = vec![0.0; es];
    let mut pran = vec![0.0; es];
    let mut kvis = vec![0.0; es];
    let mut tcon = vec![0.0; es];

    for i in 0..es {
        let p = press[i];
        let tsat = tsat_p(p);
        // Force the liquid and vapour branches; the two-fluid model allows
        // either phase to sit on the wrong side of saturation.
        let tlk = tempsliq[i].min(tsat - 1e-3);
        let tgk = tempsgas[i].max(tsat + 1e-3);

        ldens[i] = 1.0 / v1_pt(p, tlk) / 1000.0;
        gdens[i] = 1.0 / v2_pt(p, tgk) / 1000.0;
        dens[i] = alphag[i] * gdens[i] + (1.0 - alphag[i]) * ldens[i];
        vm[i] = (alphag[i] * gdens[i] * vgas[i] + (1.0 - alphag[i]) * ldens[i] * vliq[i]) / dens[i];

        let enthliq = h1_pt(p, tlk);
        let enthgas = h2_pt(p, tgk);
        enth[i] = (alphag[i] * gdens[i] * enthgas + (1.0 - alphag[i]) * ldens[i] * enthliq)
            / dens[i];
        quality[i] = (enth[i] - enthliq) / (hv_p(p) - hl_p(p));

        pran[i] = cp1_pt(p, tlk) * mu_pt(p, tlk) / k_pt(p, tlk) * 1000.0;
        kvis[i] = mu_pt(p, tlk) * v1_pt(p, tlk) * 10000.0;
        tcon[i] = k_pt(p, tlk) / 100.0;
    }

    let mut out = th.clone();
    out.coolant.press = press;
    out.coolant.alphag = alphag;
    out.coolant.vliq = vliq;
    out.coolant.vgas = vgas;
    // `temps := liquid temp`, the reference's compatibility assignment.
    out.coolant.temps = tempsliq.clone();
    out.coolant.tempsliq = tempsliq;
    out.coolant.tempsgas = tempsgas;
    out.coolant.ldens = ldens;
    out.coolant.gdens = gdens;
    out.coolant.dens = dens;
    out.coolant.vm = vm;
    out.coolant.enth = enth;
    out.coolant.quality = quality;
    out.coolant.pran = pran;
    out.coolant.kvis = kvis;
    out.coolant.tcon = tcon;

    // No solve ran, so no seed is stored — the reference stores one only from a
    // converged state, and there are none.
    out.stag6_ustag = Array2::<f64>::zeros(6 * maxiz, nch);
    out.stag6_qref = Array2::<f64>::zeros(maxiz, nch);
    out.stag6_relerr = vec![f64::NAN; nch];

    (
        out,
        ChannelReport {
            outcomes,
            powered,
            warm_eligible,
        },
    )
}

/// The error value for the absent single-channel solver.
///
/// Provided for a caller that would rather fail than continue on stale state —
/// the wrapper itself does not return it, because the reference catches and
/// continues.
pub fn missing_solver() -> BedokError {
    BedokError::ReferenceFileMissing {
        file: "driftflux6_solverstatic1d.m",
        referenced_from: "driftflux6_solverstatic3d.m",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Coolant, FlowDirection, MassFlux};

    /// A 2x2 core of `n`-node channels at BWR conditions, with only the first
    /// channel powered.
    fn core(n: usize) -> (Params, Geometry, Th, Vec<f64>) {
        let params = Params {
            maxix: Some(2),
            maxiy: Some(2),
            maxiz: Some(n),
            g: 1,
            nc: Some(0),
            ..Default::default()
        };
        let es = 4 * n;

        let geometry = Geometry {
            lz: vec![366.0 / n as f64; es],
            ..Default::default()
        };

        let th = Th {
            coolant: Coolant {
                inlettemp: 550.0,
                inletpress: 7.0,
                inletvoid: 0.0,
                ..Default::default()
            },
            heatflux: vec![0.0; es],
            maxpow: 1e6,
            powratio: 1.0,
            nfuelpin: 1.0,
            coolheatfrac: 1.0,
            flowrate: MassFlux::Uniform(100.0),
            flowdir: FlowDirection::Up,
            ..Default::default()
        };

        // Only channel 0 (ix=0, iy=0) carries power.
        let mut pwrdens = vec![0.0; es];
        for slot in pwrdens.iter_mut().take(n) {
            *slot = 1.0 / n as f64;
        }
        (params, geometry, th, pwrdens)
    }

    /// Powered channels report the missing solver; unpowered ones report being
    /// skipped.
    ///
    /// # Methodology
    ///
    /// A 2x2 core with one powered channel. The reference would attempt a solve
    /// on that one and skip the other three. Since
    /// `driftflux6_solverstatic1d.m` is absent, the powered channel's solve
    /// raises in MATLAB and is caught, so the honest outcome is
    /// `SolverMissing` for it and `Unpowered` for the rest.
    ///
    /// Pass criterion: exactly one powered channel, its outcome
    /// `SolverMissing`, and three `Unpowered`.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// `ChannelReport { outcomes: [SolverMissing, Unpowered, Unpowered,
    /// Unpowered], powered: 1, warm_eligible: 0 }` — exactly as predicted.
    ///
    /// **Interpretation.** The call does not fail; it reports. That matters
    /// because it is the reference's own behaviour, and because a caller can
    /// now distinguish "solved" from "kept stale state", which the MATLAB's
    /// printed warning does not let a programmatic caller do.
    #[test]
    fn the_missing_solver_is_reported_per_channel() {
        let (params, geometry, th, pwrdens) = core(6);
        let (_, report) = driftflux6_solverstatic3d(&params, &geometry, &th, &pwrdens);

        eprintln!("{report:?}");
        assert_eq!(report.powered, 1);
        assert_eq!(report.outcomes.len(), 4);
        assert_eq!(report.outcomes[0], ChannelOutcome::SolverMissing);
        for o in &report.outcomes[1..] {
            assert_eq!(*o, ChannelOutcome::Unpowered);
        }
    }

    /// The derived-field tail runs, and produces a physically consistent
    /// single-phase state from the inlet defaults.
    ///
    /// # Methodology
    ///
    /// With no previous state, every channel falls back to the inlet defaults:
    /// `press = 7 MPa`, `alphag ~ 0`, both phase temperatures at their inlet
    /// values. The tail then recovers densities, mixture properties, enthalpy,
    /// quality and transport properties over the whole domain — the part of
    /// this file that does real work regardless of the missing solver.
    ///
    /// At `alphag = 1e-9` the mixture is essentially pure liquid, so: `dens`
    /// should equal `ldens` to within 1e-6 relative, `vm` should equal `vliq`,
    /// the enthalpy should be the liquid enthalpy, and the quality should be
    /// near zero. Liquid density at 7 MPa and 550 K is about 0.76 g/cm³.
    ///
    /// Pass criterion: those four identities, plus `ldens` in 0.70-0.80 g/cm³
    /// and the transport properties in the same bands
    /// [`crate::singleflow1devap`]'s tests use.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// At 7 MPa and 550 K: `ldens = 0.75721 g/cm³`, `gdens = 0.03652`,
    /// `dens = 0.75721` (indistinguishable from the liquid, as it must be at
    /// `alphag = 1e-9`), `vm = 132.064 cm/s`, `h = 1219.84 kJ/kg`,
    /// `x = 4.976e-11`. Transport: `tcon = 0.005839 W/(cm·K)`,
    /// `pran = 0.8480`, `kvis = 0.001256 cm²/s`.
    ///
    /// **Interpretation.** Every identity holds. The enthalpy matches the
    /// `h1_pt(7 MPa, 550 K) = 1219.8438` that
    /// [`crate::singleflow1devap`]'s own tests report, so the two modules agree
    /// on the inlet state through independent code paths. `vm` is
    /// `100 / 0.75721 = 132.06 cm/s`, closing the mixture-velocity definition.
    /// The three transport properties land within 0.5% of
    /// `singleflow1devap`'s, the small difference being that this file
    /// evaluates them at `Tsat - 1e-3` rather than `Tsat - 2*eps`.
    ///
    /// This verifies the part of the file that does real work, independently of
    /// the missing solver.
    #[test]
    fn the_derived_field_tail_recovers_a_consistent_liquid_state() {
        let (params, geometry, th, pwrdens) = core(6);
        let (out, _) = driftflux6_solverstatic3d(&params, &geometry, &th, &pwrdens);
        let c = &out.coolant;

        eprintln!(
            "node 0: p = {}, ldens = {:.5}, gdens = {:.5}, dens = {:.5}, vm = {:.3}, h = {:.2}, x = {:.3e}",
            c.press[0], c.ldens[0], c.gdens[0], c.dens[0], c.vm[0], c.enth[0], c.quality[0]
        );
        eprintln!(
            "        tcon = {:.6}, pran = {:.4}, kvis = {:.6}",
            c.tcon[0], c.pran[0], c.kvis[0]
        );

        for i in 0..c.press.len() {
            assert_eq!(c.press[i], 7.0);
            assert!((c.dens[i] - c.ldens[i]).abs() / c.ldens[i] < 1e-6);
            assert!((c.vm[i] - c.vliq[i]).abs() / c.vliq[i] < 1e-6);
            assert!(c.quality[i].abs() < 1e-6, "quality {}", c.quality[i]);
            assert!((0.70..0.80).contains(&c.ldens[i]), "ldens {}", c.ldens[i]);
            assert!((0.0029..0.0116).contains(&c.tcon[i]));
            assert!((0.45..1.8).contains(&c.pran[i]));
        }
        // `temps` is the liquid temperature, per the compatibility assignment.
        assert_eq!(c.temps, c.tempsliq);
    }

    /// The vapour branch is forced above saturation even when the liquid
    /// temperature is not.
    ///
    /// # Methodology
    ///
    /// The reference clamps `Tlk = min(tempsliq, Tsat - 1e-3)` and
    /// `Tgk = max(tempsgas, Tsat + 1e-3)` before evaluating properties, so that
    /// each phase is always evaluated on its own side of the saturation line.
    /// Without that, a two-fluid state with a superheated liquid or a subcooled
    /// vapour would ask region 1 for a vapour property or vice versa.
    ///
    /// Feeding both phases in at the *same* subcooled temperature must still
    /// produce a vapour density far below the liquid one, because `Tgk` is
    /// pushed above `Tsat` regardless.
    ///
    /// Pass criterion: `gdens` at least 10x smaller than `ldens`.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// With both phases handed in at 540 K — 19 K **below**
    /// `Tsat(7 MPa) = 558.980 K` — the recovered densities were
    /// `ldens = 0.77511` and `gdens = 0.03652 g/cm³`, a ratio of **21.2**.
    ///
    /// **Interpretation.** The vapour was pushed onto its own branch despite
    /// being asked for at a subcooled temperature, which is the whole purpose
    /// of the `max(tempsgas, Tsat + 1e-3)` clamp. Without it, region 2 would
    /// have been evaluated 19 K inside the liquid region and returned a density
    /// near the liquid's — silently destroying the phase separation the
    /// two-fluid model exists to represent.
    #[test]
    fn each_phase_is_forced_onto_its_own_branch() {
        let (params, geometry, mut th, pwrdens) = core(4);
        let es = 16;
        // Both phases at the same subcooled temperature.
        th.coolant.press = vec![7.0; es];
        th.coolant.tempsliq = vec![540.0; es];
        th.coolant.tempsgas = vec![540.0; es];
        th.coolant.alphag = vec![0.3; es];
        th.coolant.vliq = vec![100.0; es];
        th.coolant.vgas = vec![200.0; es];

        let (out, _) = driftflux6_solverstatic3d(&params, &geometry, &th, &pwrdens);
        let c = &out.coolant;
        eprintln!(
            "Tsat(7 MPa) = {:.3}; ldens = {:.5}, gdens = {:.5}, ratio = {:.1}",
            tsat_p(7.0),
            c.ldens[0],
            c.gdens[0],
            c.ldens[0] / c.gdens[0]
        );
        assert!(
            c.ldens[0] > 10.0 * c.gdens[0],
            "vapour was not forced onto its own branch"
        );
        // And the mixture density lies between the two.
        assert!(c.dens[0] < c.ldens[0] && c.dens[0] > c.gdens[0]);
    }

    /// The warm-start admission policy is evaluated as the reference states it.
    ///
    /// # Methodology
    ///
    /// A seed is admissible only if the previous solve converged
    /// (`relerr < 1e-3`) **and** the wall flux has moved less than 20%. This
    /// checks all four corners: no seed, unconverged seed, converged seed with
    /// a small flux change, and converged seed with a large one.
    ///
    /// The policy is exercised even though no solve can consume the seed —
    /// preserving it is the point, since it is the part of this file most
    /// likely to matter once the missing solver is supplied.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// All four corners behaved as specified: converged + unchanged admitted
    /// (1 eligible); converged + 50% flux change rejected (0); unconverged +
    /// unchanged rejected (0); converged + 10% flux change admitted (1).
    ///
    /// **Interpretation.** The two conditions are independent and both
    /// load-bearing, and the 20% threshold sits where the reference puts it.
    /// This is the piece of the file most likely to matter once
    /// `driftflux6_solverstatic1d.m` is supplied, so pinning it now means the
    /// policy cannot drift while the solver is absent and untestable.
    #[test]
    fn the_warm_start_policy_admits_only_converged_unchanged_channels() {
        let n = 6;
        let nch = 4;

        let seeded = |relerr: f64, qscale: f64| {
            let (params, geometry, mut th, pwrdens) = core(n);
            let mut ustag = Array2::<f64>::zeros(6 * n, nch);
            for r in 0..6 * n {
                ustag.set(r, 0, 1.0);
            }
            let mut qref = Array2::<f64>::zeros(n, nch);
            for iz in 0..n {
                qref.set(iz, 0, 10.0);
            }
            th.stag6_ustag = ustag;
            th.stag6_qref = qref;
            th.stag6_relerr = vec![relerr; nch];
            th.heatflux = vec![10.0 * qscale; 4 * n];
            let (_, report) = driftflux6_solverstatic3d(&params, &geometry, &th, &pwrdens);
            report.warm_eligible
        };

        // Converged and unchanged -> admitted.
        assert_eq!(seeded(1e-5, 1.0), 1, "converged + unchanged should warm-start");
        // Converged but the flux moved 50% -> rejected.
        assert_eq!(seeded(1e-5, 1.5), 0, "a 50% flux change should reject the seed");
        // Unconverged -> rejected however small the flux change.
        assert_eq!(seeded(1e-1, 1.0), 0, "an unconverged seed is poisoned");
        // A 10% flux change is inside the 20% band.
        assert_eq!(seeded(1e-5, 1.1), 1, "a 10% flux change is admissible");
    }

    /// The named error is available for a caller that wants to fail rather than
    /// continue on stale state.
    #[test]
    fn the_missing_file_error_names_both_files() {
        let e = missing_solver();
        let msg = e.to_string();
        eprintln!("{msg}");
        assert!(msg.contains("driftflux6_solverstatic1d.m"));
        assert!(msg.contains("driftflux6_solverstatic3d.m"));
    }
}
