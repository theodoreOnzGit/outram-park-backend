//! NEACRP case D1 — the inlet cold-water injection transient.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `neacrpd1t.m`, `main_exec_diff3d_standalone` snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.
//!
//! # The transient
//!
//! NEACRP-L-335 (Revision 1) section 6.2 / Fig. 6.1, over 0 to 20 s. The steady
//! state is [`crate::neacrpd1`] unchanged; this file adds only the
//! time-dependent data.
//!
//! The inlet subcooling **doubles** with a 2.5 s time constant:
//!
//! ```text
//! dh(t) = 46.52 * (2 - exp(-0.4 t))   kJ/kg below saturated liquid
//! ```
//!
//! `dh(0) = 46.52 kJ/kg` is exactly the steady inlet of `neacrpd1.m`, so the
//! forcing is continuous at `t = 0`. The inlet mass flow is **constant** and
//! there is **no control-rod motion** — `crodeject` stays `None`.
//!
//! # Why the case forces the HEM thermal-hydraulic model
//!
//! The transient chain ([`crate::th_solvertimexyz`] into
//! [`crate::singleflow1devaptime`]) is the homogeneous-equilibrium enthalpy
//! march, so the **initial steady state must run the same model**. A two-fluid
//! steady state has less void than HEM at the same conditions, and handing that
//! to the transient would be a density mismatch — a spurious reactivity step at
//! `t = 0`. The case therefore sets `th_model = 'hem'` explicitly.
//!
//! That is also the only model this crate can run: the two-fluid path needs
//! `driftflux6_solverstatic1d.m`, which is absent from the snapshot.
//!
//! # The case has to rebuild `fp`, because the steady case zeroes it
//!
//! `neacrpd1.m` leaves `sigmavalues.fp` at zero — the steady solver derives
//! power from the fission source and never reads it. The transient does read
//! it: `P0 = sum(fp * phi)` would be `0/0 = NaN`. So the case builds it from
//! the `nu*Sigma_f` tables using the specification's prompt energy release
//! `E0 = 3.20e-11 J/fission` (Table 5.1):
//!
//! ```text
//! fp = E0 * (nu Sigma_f) / nu,   with nu = 1 as encoded
//! ```
//!
//! Under composition-uniform `nu` the `P/P0` **ratio** is exact, because the
//! `E0` scale cancels. The feedback slopes follow `f`'s, so `fp` stays
//! proportional to `f` under both feedback channels.

use crate::neacrpd1::neacrpd1;
use crate::sigmavalupd3d_handler::FeedbackTables;
use crate::types::{
    Geometry, InletForcing, MassFlux, Params, SigmaValues, Th, ThModel, VolumetricHeatCapacity,
};

/// The specification's prompt energy release per fission, J.
///
/// NEACRP-L-335 Table 5.1. Only the `P/P0` ratio is reported, and this scale
/// cancels out of it.
pub const ENERGY_PER_FISSION: f64 = 3.20e-11;

/// The steady-state inlet subcooling, kJ/kg — Fig. 6.1's `dh(0)`.
pub const SUBCOOLING_0: f64 = 46.52;

/// The approach rate of the cold-water forcing, 1/s (a 2.5 s time constant).
pub const FORCING_RATE: f64 = 0.4;

/// `[params, geometry, th, constants, whichsigma, sigmavalues] = neacrpd1t(params)`.
///
/// Builds [`crate::neacrpd1`] and layers the transient data on top: the time
/// window and grid, the two-group prompt velocities, six-group delayed-neutron
/// data, fuel and cladding volumetric heat capacities, the inlet forcing, and
/// the reconstructed prompt-fission operator.
///
/// # Returns
///
/// `(params, geometry, th, whichsigma, sigmavalues, feedback)`, matching
/// [`crate::neacrpd1::neacrpd1`].
#[allow(clippy::type_complexity)]
pub fn neacrpd1t(
    params: &Params,
) -> (Params, Geometry, Th, crate::matlab::Array3<usize>, SigmaValues, FeedbackTables) {
    let (mut params, mut geometry, mut th, whichsigma, mut sigmavalues, mut feedback) =
        neacrpd1(params);

    // ----- the power (kappa-Sigma_f) operator; see the module docs -----
    let scale_table = |t: &crate::matlab::Array2<f64>| {
        let mut out = crate::matlab::Array2::<f64>::zeros(t.rows(), t.cols());
        for i in 0..t.rows() {
            for j in 0..t.cols() {
                out.set(i, j, t.get(i, j) * ENERGY_PER_FISSION);
            }
        }
        out
    };
    sigmavalues.fp = Some(scale_table(&sigmavalues.f));
    if let Some(tbl) = feedback.fueltemp.as_mut() {
        tbl.fp = scale_table(&tbl.f);
    }
    if let Some(tbl) = feedback.coolden.as_mut() {
        tbl.fp = scale_table(&tbl.f);
    }

    // ----- transient window -----
    // The reference's grid is four overlapping ranges; the driver rounds to
    // 1 us and deduplicates, which removes the repeated join points.
    params.tend = Some(20.0);
    let mut tgrid: Vec<f64> = Vec::new();
    let push_range = |from: f64, step: f64, to: f64, out: &mut Vec<f64>| {
        let n = ((to - from) / step).round() as usize;
        for i in 0..=n {
            out.push(from + i as f64 * step);
        }
    };
    push_range(0.0, 0.025, 2.0, &mut tgrid);
    push_range(2.0, 0.05, 6.0, &mut tgrid);
    push_range(6.0, 0.1, 12.0, &mut tgrid);
    push_range(12.0, 0.2, 20.0, &mut tgrid);
    params.tgrid = Some(tgrid);

    // ----- consistent T-H model for steady and transient; see the module docs -----
    params.th_model = ThModel::Hem;

    // ----- Table 5.1: prompt neutron velocities, cm/s -----
    params.velocities = vec![1.0 / 3.57e-8, 1.0 / 2.27e-6];

    // ----- Table 5.2: delayed neutron data, six groups, total beta 0.76% -----
    params.beta_dnp = vec![0.00026, 0.00152, 0.00139, 0.00307, 0.00110, 0.00026];
    // clippy reads the fourth entry as an approximation of 1/pi. It is a
    // delayed-neutron decay constant from Table 5.2 and has nothing to do
    // with pi; the resemblance is a coincidence at three decimal places.
    #[allow(clippy::approx_constant)]
    {
        params.lambda_dnp = vec![0.013, 0.032, 0.119, 0.318, 1.403, 3.929];
    }

    // ----- section 5.7 volumetric heat capacities -----
    // The steady case leaves these empty; only the transient rod solver reads
    // them. Indexed by `whichk - 1`: fuel then cladding, no gap entry.
    geometry.fuel.rhocp = vec![
        VolumetricHeatCapacity::Uo2Fuel,
        VolumetricHeatCapacity::ZircaloyClad,
    ];

    // ----- no control-rod motion in D1 -----
    geometry.crodeject = None;

    // ----- Fig. 6.1 inlet cold-water forcing -----
    th.inlettemp_t = InletForcing::ExponentialSubcooling {
        pressure: th.coolant.inletpress,
        dh0: SUBCOOLING_0,
        rate: FORCING_RATE,
    };

    // The mass flow is constant through the transient, as it is in the steady
    // case; restated here because the specification calls it out.
    debug_assert!(matches!(th.flowrate, MassFlux::Uniform(_)));

    (params, geometry, th, whichsigma, sigmavalues, feedback)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FreqMode, TimeScheme};

    /// The transient data matches the specification tables, and the steady
    /// state underneath is untouched.
    ///
    /// # Methodology
    ///
    /// `neacrpd1t.m` is a thin layer over `neacrpd1.m`: the cross sections,
    /// geometry and material map must come through **identical**, and only the
    /// transient fields added. Checked against the specification directly —
    /// Table 5.2's total delayed fraction is quoted as 0.76%, the two prompt
    /// velocities are the reciprocals of Table 5.1's inverse velocities, and
    /// the model must be forced to HEM with no rod motion.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// Total delayed fraction **0.00760 (0.760%)**, matching the header's
    /// quoted figure exactly. Velocities `2.8011e7` and `4.4053e5` cm/s.
    /// The material map and the total and fission cross sections come
    /// through byte-identical to [`crate::neacrpd1`]'s.
    ///
    /// **Interpretation.** The transient layer adds data without disturbing
    /// the steady case underneath, which is what makes the `t = 0` state of
    /// the transient the same problem the steady solver converged.
    #[test]
    fn the_transient_data_matches_the_specification() {
        let (params, geometry, th, w, sv, _fb) = neacrpd1t(&Params::default());
        let (_p0, _g0, _t0, w0, sv0, _f0) = crate::neacrpd1::neacrpd1(&Params::default());

        // Total delayed fraction, quoted in the header as 0.76%.
        let betatot: f64 = params.beta_dnp.iter().sum();
        eprintln!("total beta       = {:.5} ({:.3}%)", betatot, betatot * 100.0);
        assert!((betatot - 0.0076).abs() < 1e-12, "beta total is {betatot}");
        assert_eq!(params.beta_dnp.len(), 6);
        assert_eq!(params.lambda_dnp.len(), 6);
        // Decay constants must increase; family 1 is the longest-lived.
        assert!(
            params.lambda_dnp.windows(2).all(|w| w[0] < w[1]),
            "decay constants should be ordered"
        );

        // Table 5.1 inverse velocities.
        assert!((1.0 / params.velocities[0] - 3.57e-8).abs() < 1e-20);
        assert!((1.0 / params.velocities[1] - 2.27e-6).abs() < 1e-18);
        eprintln!(
            "velocities       = {:.4e}, {:.4e} cm/s",
            params.velocities[0], params.velocities[1]
        );

        // The steady case is unchanged underneath.
        assert_eq!(params.tend, Some(20.0));
        assert_eq!(params.th_model, ThModel::Hem, "the transient forces HEM");
        assert_eq!(geometry.crodeject, None, "D1 has no rod motion");
        for ix in 0..17 {
            for iy in 0..17 {
                for iz in 0..14 {
                    assert_eq!(w.get(ix, iy, iz), w0.get(ix, iy, iz));
                }
            }
        }
        for m in 0..crate::neacrpd1::MATERIALS {
            for g in 0..2 {
                assert_eq!(sv.tot.get(m, g), sv0.tot.get(m, g));
                assert_eq!(sv.f.get(m, g), sv0.f.get(m, g));
            }
        }

        // Heat capacities are added; the steady case has none.
        assert_eq!(geometry.fuel.rhocp.len(), 2);
        assert!(matches!(th.flowrate, MassFlux::Uniform(_)));
    }

    /// The prompt-fission operator is rebuilt proportional to `f`.
    ///
    /// # Methodology
    ///
    /// The steady case leaves `fp` at `None`, which the transient would read as
    /// zero and divide by. This case rebuilds it as `E0 * f`. The check is the
    /// proportionality itself: `fp/f` must be exactly `E0` for every material
    /// and group where `f` is non-zero, and `fp` must vanish wherever `f` does
    /// — otherwise a reflector would emit power.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// Proportionality confirmed on all **32** non-zero entries, and both
    /// feedback tables carry the same scaling.
    ///
    /// **Interpretation.** 32 of the 38 material-group entries fission; the
    /// other 6 are the three reflectors, which correctly emit no power. `fp`
    /// tracks `f` exactly under both feedback channels, so the power
    /// normalisation cannot drift away from the fission source as the
    /// transient perturbs the cross sections.
    #[test]
    fn the_prompt_fission_operator_is_proportional_to_the_fission_source() {
        let (_p, _g, _t, _w, sv, fb) = neacrpd1t(&Params::default());
        let fp = sv.fp.as_ref().expect("the transient case must build fp");

        let mut checked = 0;
        for m in 0..crate::neacrpd1::MATERIALS {
            for g in 0..2 {
                let f = sv.f.get(m, g);
                let p = fp.get(m, g);
                if f == 0.0 {
                    assert_eq!(p, 0.0, "material {} group {g} emits power without fission", m + 1);
                } else {
                    let ratio = p / f;
                    assert!(
                        (ratio - ENERGY_PER_FISSION).abs() < 1e-24,
                        "material {} group {g}: fp/f = {ratio:e}, expected {ENERGY_PER_FISSION:e}",
                        m + 1
                    );
                    checked += 1;
                }
            }
        }
        eprintln!("proportionality confirmed on {checked} non-zero entries");

        // Both feedback tables carry the same scaling, so fp tracks f under
        // feedback rather than drifting away from it.
        for (name, tbl) in [
            ("fueltemp", fb.fueltemp.as_ref().unwrap()),
            ("coolden", fb.coolden.as_ref().unwrap()),
        ] {
            for m in 0..crate::neacrpd1::MATERIALS {
                for g in 0..2 {
                    let expect = tbl.f.get(m, g) * ENERGY_PER_FISSION;
                    assert!(
                        (tbl.fp.get(m, g) - expect).abs() < 1e-30,
                        "{name} slope {m}/{g} is not scaled"
                    );
                }
            }
        }
    }

    /// The inlet forcing is continuous at `t = 0` and approaches double the
    /// steady subcooling.
    ///
    /// # Methodology
    ///
    /// Fig. 6.1 gives `dh(t) = 46.52*(2 - exp(-0.4 t))`. Two limits follow
    /// analytically and both are checked through the actual IF97 inversion the
    /// driver will use: at `t = 0` the inlet temperature must equal the steady
    /// case's exactly (else the transient starts with a step), and as
    /// `t -> infinity` the subcooling must approach `2 * 46.52 = 93.04 kJ/kg`.
    /// In between the temperature must fall monotonically.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// `t = 0` gives **547.1528 K**, equal to the steady inlet to within
    /// 1e-9 K. By `t = 20 s` the inlet has fallen to **538.0038 K**, a drop
    /// of **9.149 K**, against an asymptote of **538.0007 K** — within
    /// 0.0031 K of it.
    ///
    /// **Interpretation.** The forcing is continuous at `t = 0`, so the
    /// transient starts with no spurious inlet step, and after 20 s (eight
    /// time constants) it has essentially reached the doubled subcooling
    /// Fig. 6.1 specifies. Note the temperature drop is only 9.1 K for a
    /// *doubling* of the enthalpy subcooling — the water is subcooled
    /// liquid near 6.7 MPa, where `cp` is around 5.1 kJ/(kg K).
    #[test]
    fn the_inlet_forcing_is_continuous_at_zero_and_doubles_the_subcooling() {
        let (_p, _g, th, ..) = neacrpd1t(&Params::default());
        let (_p2, _g2, th0, ..) = crate::neacrpd1::neacrpd1(&Params::default());

        let t0 = th.inlettemp_t.at(0.0).expect("D1 prescribes a forcing");
        eprintln!("steady inlet     = {:.4} K", th0.coolant.inlettemp);
        eprintln!("forcing at t=0   = {t0:.4} K");
        assert!(
            (t0 - th0.coolant.inlettemp).abs() < 1e-9,
            "the forcing must be continuous with the steady inlet"
        );

        // Monotonically falling.
        let mut prev = t0;
        for n in 1..=40 {
            let t = n as f64 * 0.5;
            let temp = th.inlettemp_t.at(t).unwrap();
            assert!(temp < prev, "inlet temperature rose at t = {t}");
            prev = temp;
        }
        eprintln!("forcing at t=20  = {prev:.4} K");
        eprintln!("total drop       = {:.4} K", t0 - prev);

        // The asymptote: subcooling -> 2*dh0 below saturation.
        let tsat = crate::iapws_if97::region4::tsat_p(th.coolant.inletpress);
        let hsat = crate::iapws_if97::basic::h1_pt(th.coolant.inletpress, tsat);
        let t_inf = crate::iapws_if97::backward::t_ph(
            th.coolant.inletpress,
            hsat - 2.0 * SUBCOOLING_0,
        );
        eprintln!("asymptote        = {t_inf:.4} K");
        assert!(prev > t_inf, "t=20 s must not have overshot the asymptote");
        assert!(
            prev - t_inf < 0.05,
            "by t = 20 s (8 time constants) it should be within 0.05 K of the asymptote"
        );
    }

    /// The time grid deduplicates the reference's overlapping ranges.
    ///
    /// # Methodology
    ///
    /// The case writes `[0:0.025:2, 2:0.05:6, 6:0.1:12, 12:0.2:20]`, in which
    /// 2, 6 and 12 each appear twice. The driver rounds to 1 microsecond and
    /// deduplicates precisely so those joins cannot produce a zero-length time
    /// step — a `dt` of 0 would divide by zero in every term of the kinetics
    /// equations. This checks that no step is non-positive.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// The raw grid has **264 points with 3 duplicated joins** (at 2, 6 and
    /// 12 s); after rounding and deduplication **261 steps** are marched,
    /// with `dt` from 0.025 to 0.200 s and no non-positive step.
    ///
    /// **Interpretation.** All three joins were removed, exactly the three
    /// the four overlapping ranges create. Had any survived, the kinetics
    /// equations would have divided by a zero `dt` on that step.
    #[test]
    fn the_time_grid_has_no_zero_length_steps() {
        let (params, ..) = neacrpd1t(&Params::default());
        let raw = params.tgrid.as_ref().unwrap();

        // The raw grid does contain the duplicated joins.
        let dupes = raw.windows(2).filter(|w| w[0] == w[1]).count();
        eprintln!("raw grid points  = {}, duplicated joins = {dupes}", raw.len());
        assert!(dupes > 0, "the reference grid should have overlapping joins");

        let grid = crate::thdiffusion_solvertimexyz::build_time_grid_for_test(&params, 20.0);
        eprintln!("marched steps    = {}", grid.len());
        assert_eq!(grid[0], 0.0);
        assert_eq!(*grid.last().unwrap(), 20.0);
        for w in grid.windows(2) {
            assert!(w[1] > w[0], "non-positive step between {} and {}", w[0], w[1]);
        }
        let dt_min = grid.windows(2).map(|w| w[1] - w[0]).fold(f64::INFINITY, f64::min);
        let dt_max = grid.windows(2).map(|w| w[1] - w[0]).fold(0.0, f64::max);
        eprintln!("dt range         = {dt_min:.4} .. {dt_max:.4} s");
        assert!(dt_min > 0.0);
    }

    /// **The transient runs.** A short window of the D1 cold-water injection.
    ///
    /// # Methodology
    ///
    /// The full transient chain over the first 0.5 s: the coupled steady state,
    /// the phase-2 re-equilibration, then the exponential-transform kinetics
    /// with six precursor families and one transient T-H step per time step.
    /// The window is shortened from the specification's 20 s to keep the test
    /// inside a reasonable runtime — this is a **smoke test of the machinery**,
    /// not a benchmark comparison.
    ///
    /// Pass criteria, all structural: the march completes without tripping the
    /// divergence guard; the initial state really is critical, so `P/P0` starts
    /// at 1 and stays physical; and the precursors stay non-negative, which a
    /// sign error in the analytic integration would break immediately.
    ///
    /// **No published transient result is involved.** The NEACRP specification
    /// is not in `crates/kovan-literature`, so there is no curve to judge
    /// against; see `src/data/PROVENANCE.md`.
    ///
    /// # Results — measured 2026-08-22 (superseding a 2026-08-18 run)
    ///
    /// **Completed** — the divergence guard never tripped. 51 steps.
    ///
    /// | | |
    /// |---|---|
    /// | steady `k_eff` | 0.975285 |
    /// | re-equilibrated `k_eff` | 0.975277, after 1451 power iterations |
    /// | `P/P0` at `t = 0` | 1.000000 |
    /// | `P/P0` at `t = 0.5 s` | 1.015203 |
    /// | core-average fuel T | 787.16 -> 787.17 K |
    /// | maximum fuel T | 2476.31 -> 2476.35 K |
    /// | coolant outlet T | 553.06 -> 552.64 K |
    /// | negative precursors | 0 |
    ///
    /// The 2026-08-18 figures (steady `k_eff` 0.975869, `P/P0` 1.013424, max
    /// fuel T 2440.23 K) predate defect **Z1**'s axial-mesh fix, which is what
    /// moved the steady state this transient starts from. The T9/T13
    /// correction does **not** appear here: `max fuel T` is the pellet
    /// *centreline* from the transient driver's own radial profile, not
    /// `th.fueltempavg`, so it was never the aliased quantity.
    ///
    /// **Interpretation.** The physics runs the right way. Cold water
    /// entering the core makes the moderator **denser**, which in an
    /// under-moderated LWR lattice adds reactivity — and the power rises,
    /// by 1.52% in half a second, while the coolant outlet falls 0.42 K as
    /// the colder water works along the channel. Those two signs agreeing is
    /// the check that the coolant-density feedback is wired the right way
    /// round through the transient path, not just the steady one.
    ///
    /// The fuel temperatures barely move, which is also right: 0.5 s is
    /// short against the fuel-rod time constant, and a 1.3% power change
    /// would not shift a pellet centreline much even given longer.
    ///
    /// Phase 2 re-equilibration moved `k_eff` by 8.2e-6 relative, about
    /// **0.8 pcm** — small, as it should be, since it only removes the
    /// inconsistency between the operator the steady loop converged and the
    /// one the time stepping uses. It took 1451 of its 5000 allowed power
    /// iterations, so the cap was not reached.
    ///
    /// **On the 2440 K peak:** this is the hottest *pellet centreline* over
    /// the whole core, which is a different quantity from the `1714.53 K`
    /// reported by [`crate::neacrpd1`]'s HEM test — that one is the peak
    /// **pellet volume average**. A centreline runs hotter than any average
    /// over the pellet, so the two do not conflict.
    ///
    /// (Before the T9/T13 correction the comparison figure was `1458.73 K`,
    /// the peak **Doppler weight**; the centreline was hotter than that too,
    /// for the same reason and by more.)
    #[test]
    fn the_transient_marches_without_diverging() {
        let (mut params, geometry, th, whichsigma, sigmavalues, feedback) =
            neacrpd1t(&Params::default());

        // A short window: the machinery, not the benchmark.
        params.tend = Some(0.5);
        params.tgrid = None; // the driver's uniform 10 ms default

        let out = crate::thdiffusion_solvertimexyz::thdiffusion_solvertimexyz(
            &geometry,
            &params,
            &th,
            &sigmavalues,
            &feedback,
            &whichsigma,
            None,
            None,
        )
        .expect("the D1 transient should run");

        eprintln!("NEACRP-D1 transient, 0 to 0.5 s, scheme {:?}:", out.timescheme);
        eprintln!("  termination      = {:?}", out.termination);
        eprintln!("  steady k_eff     = {:.6}", out.steady.k_eff);
        eprintln!(
            "  re-equilibrated  = {:.6} after {} power iterations",
            out.k_eff, out.reequilibrate_iterations
        );
        eprintln!("  steps marched    = {}", out.time.len());
        eprintln!("  P/P0 at t=0      = {:.6}", out.relpower[0]);
        eprintln!("  P/P0 final       = {:.6}", out.relpower[out.relpower.len() - 1]);
        eprintln!("  P/P0 peak        = {:.6} at t = {:.3} s", out.prelmax, out.tpmax);
        eprintln!(
            "  avg fuel T       = {:.2} -> {:.2} K",
            out.avgfueltemp[0],
            out.avgfueltemp[out.avgfueltemp.len() - 1]
        );
        eprintln!(
            "  max fuel T       = {:.2} -> {:.2} K",
            out.maxfueltemp[0],
            out.maxfueltemp[out.maxfueltemp.len() - 1]
        );
        eprintln!(
            "  coolant outlet   = {:.2} -> {:.2} K",
            out.coolouttemp[0],
            out.coolouttemp[out.coolouttemp.len() - 1]
        );

        assert_eq!(
            out.termination,
            crate::thdiffusion_solvertimexyz::Termination::Completed,
            "the transient must not trip the divergence guard"
        );
        assert_eq!(out.relpower[0], 1.0, "the transient starts at its steady power");
        assert!(
            out.relpower.iter().all(|p| p.is_finite() && *p > 0.0),
            "power must stay finite and positive"
        );
        // The analytic precursor integration must not produce negative
        // concentrations; a sign error in Eq. (8) shows up here first.
        let mut negatives = 0;
        for r in 0..out.precursors_final.rows() {
            for i in 0..out.precursors_final.cols() {
                if out.precursors_final.get(r, i) < 0.0 {
                    negatives += 1;
                }
            }
        }
        eprintln!("  negative precursors = {negatives}");
        assert_eq!(negatives, 0, "precursor concentrations must stay non-negative");
    }

    /// The two kinetics schemes agree over a short, gently-forced window.
    ///
    /// # Methodology
    ///
    /// [`TimeScheme::ExponentialTransform`] and [`TimeScheme::ImplicitEuler`]
    /// discretise the same equations. They are **not** algebraically identical
    /// — the first integrates the precursors analytically over a linearly
    /// varying transformed source, the second is first-order in everything —
    /// so they cannot be expected to match to round-off. But over a window
    /// where nothing is stiff, they must agree closely, and a transcription
    /// error in either would show up as a gross divergence rather than a small
    /// truncation difference.
    ///
    /// This is the same cross-check technique that caught operator-split
    /// mistakes in the steady solvers: two independently written paths through
    /// the same physics, compared against each other.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// | scheme | `P/P0` at `t = 0.2 s` |
    /// |---|---|
    /// | exponential transform | 0.99908807 |
    /// | implicit Euler | 0.99908485 |
    ///
    /// Relative difference **3.229e-6**.
    ///
    /// **Interpretation.** Two independently transcribed discretisations —
    /// one with analytic precursor integration under an exponential
    /// transform, the other plain first-order implicit Euler with the
    /// precursors eliminated — agree to **3.2 parts per million** on the
    /// same problem. They share the operator assembly and the T-H chain, so
    /// this does not test those; what it does test is the kinetics algebra,
    /// which is written twice and completely differently. A sign error, a
    /// dropped `beta`, or a mis-set precursor coefficient in either path
    /// would show up as a gross divergence, not a ppm-level difference.
    ///
    /// This is the same cross-check that caught operator-split mistakes in
    /// the steady solvers, and the `dt -> infinity` equivalences between the
    /// steady and transient module pairs. It remains the most productive
    /// verification technique found on this port.
    ///
    /// Note `P/P0` sits slightly **below** 1 here where the 0.5 s run above
    /// ends above it: the power dips marginally before rising. Whether that
    /// early dip is physical or a startup artefact of the re-equilibration
    /// is **not established** by these tests.
    #[test]
    fn the_two_kinetics_schemes_agree_on_a_gentle_window() {
        let run = |scheme: TimeScheme| {
            let (mut params, geometry, th, whichsigma, sigmavalues, feedback) =
                neacrpd1t(&Params::default());
            params.tend = Some(0.2);
            params.tgrid = None;
            params.timescheme = scheme;
            params.freqmode = FreqMode::Global;
            crate::thdiffusion_solvertimexyz::thdiffusion_solvertimexyz(
                &geometry,
                &params,
                &th,
                &sigmavalues,
                &feedback,
                &whichsigma,
                None,
                None,
            )
            .expect("both schemes should run")
        };

        let a = run(TimeScheme::ExponentialTransform);
        let b = run(TimeScheme::ImplicitEuler);

        let pa = a.relpower[a.relpower.len() - 1];
        let pb = b.relpower[b.relpower.len() - 1];
        let rel = (pa - pb).abs() / pa;
        eprintln!("exponential transform: P/P0 = {pa:.8}");
        eprintln!("implicit Euler       : P/P0 = {pb:.8}");
        eprintln!("relative difference  : {:.3e}", rel);

        assert_eq!(a.time.len(), b.time.len(), "both marched the same grid");
        assert!(
            rel < 1e-3,
            "the two schemes differ by {rel:e}, which is far more than truncation"
        );
    }
}
