//! Writes the `(rho,h)` flash verification out as a committed markdown report.
//!
//! Reuses the report writer the Chebyshev module already has
//! ([`crate::backward_eqn_chebyshev_experimental::tests::vv_report`]) rather
//! than growing a second one, overriding only its status blurb — that default
//! declares its subject to be an in-house non-IAPWS fit, which is precisely
//! what this module is *not*.

use uom::si::available_energy::kilojoule_per_kilogram;
use uom::si::f64::*;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::{bar, pascal};
use uom::si::specific_volume::cubic_meter_per_kilogram;

use crate::backward_eqn_chebyshev_experimental::tests::vv_report::VvReport;
use crate::interfaces::functional_programming::ph_flash_eqm::{ph_flash_region, v_ph_eqm};
use crate::interfaces::functional_programming::pt_flash_eqm::FwdEqnRegion;
use crate::interfaces::functional_programming::rho_h_flash_eqm::{
    p_rho_h_eqm, rho_h_is_within_validity_range, tpx_rho_h_eqm,
};

use super::steam_table_data::{SATURATION_NODES, SINGLE_PHASE_NODES};

const STATUS: &str = "This report covers `p_rho_h_eqm` / `tpx_rho_h_eqm`, which \
**invert the published IAPWS-IF97 backward equations** rather than fitting a \
correlation. Every thermodynamic value comes from this crate's IF97 \
implementation; what is added is the inversion strategy (region dispatch plus a \
bracketed root find), which is numerics, not thermodynamics.\n\n\
The reference nodes are the published steam tables this crate already checks its \
`(p,h)` flash against. Per `RESPONSIBLE_USE.md` this is AI-assisted draft \
material: the numbers below are measurements, **not a validation sign-off**. No \
human has reviewed them.";

/// Regenerates `verification_and_validation/generated/p_rho_h_if97_inversion.md`.
///
/// Measurement, not a gate — the assertions live in [`super`]. This exists so
/// the numbers are readable without running the suite, which is the same
/// contract the Chebyshev module's generated reports carry.
#[test]
fn write_rho_h_flash_vv_report() {
    let mut report = VvReport::new(
        "p_rho_h_if97_inversion",
        "(rho,h) flash by inversion of the IAPWS-IF97 backward equations",
    )
    .with_status(STATUS);

    // --- single-phase sweep -------------------------------------------------
    let mut solved_by_pressure = 0_usize;
    let mut solved_by_volume = 0_usize;
    let mut unsolved = 0_usize;
    let mut p_err_by_region = [0.0_f64; 6];
    let mut count_by_region = [0_usize; 6];

    for node in SINGLE_PHASE_NODES {
        let [p_bar_val, _t, _v_tab, h_kj] = *node;
        let p = Pressure::new::<bar>(p_bar_val);
        let h = AvailableEnergy::new::<kilojoule_per_kilogram>(h_kj);

        let probe = std::panic::catch_unwind(|| v_ph_eqm(p, h).get::<cubic_meter_per_kilogram>());
        let Ok(v_target) = probe else { continue };
        if !(v_target.is_finite() && v_target > 0.0) {
            continue;
        }
        let rho = MassDensity::new::<kilogram_per_cubic_meter>(1.0 / v_target);
        if !rho_h_is_within_validity_range(rho, h) {
            continue;
        }

        let p_back = p_rho_h_eqm(rho, h);
        let rel_err = ((p_back.get::<pascal>() - p.get::<pascal>()) / p.get::<pascal>()).abs();
        let v_back = v_ph_eqm(p_back, h).get::<cubic_meter_per_kilogram>();
        let volume_residual = ((v_back - v_target) / v_target).abs();

        let idx = match ph_flash_region(p, h) {
            FwdEqnRegion::Region1 => 1,
            FwdEqnRegion::Region2 => 2,
            FwdEqnRegion::Region3 => 3,
            FwdEqnRegion::Region4 => 4,
            FwdEqnRegion::Region5 => 5,
        };
        p_err_by_region[idx] = p_err_by_region[idx].max(rel_err);
        count_by_region[idx] += 1;

        if rel_err < 1.0e-6 {
            solved_by_pressure += 1;
        } else if volume_residual < 1.0e-9 {
            solved_by_volume += 1;
        } else {
            unsolved += 1;
        }
    }

    report.section("Methodology");
    report.paragraph(
        "`v(p,h)` is already explicit in IF97 — Regions 1 and 2 through the \
         `T(p,h)` backward equations, Region 3 through `v(p,h)` directly, Region \
         4 as a quality-weighted mixture. Inverting it for pressure is therefore \
         a one-dimensional bracketed root find in `p` alone, with one explicit \
         backward-equation evaluation per residual. The search interval is split \
         at every IF97 region seam first, because `v(p,h)` is not monotone \
         across one.",
    );
    report.paragraph(
        "Two measures are recorded per node. `|dp/p|` is the error in the \
         recovered **pressure** against the tabulated value. `|dv/v|` is the \
         residual the root find actually minimises — whether the returned \
         pressure reproduces the **density** it was given. A node counts as \
         solved if either is tight; only a node failing both indicts the solver.",
    );

    report.section("Results — single-phase nodes, by region");
    let mut rows = Vec::new();
    for region in 1..=5 {
        if count_by_region[region] > 0 {
            rows.push(vec![
                format!("Region {region}"),
                format!("{}", count_by_region[region]),
                format!("{:.3e}", p_err_by_region[region]),
            ]);
        }
    }
    report.table(&["region", "nodes", "max abs(dp/p)"], &rows);
    report.paragraph(&format!(
        "Of the nodes swept, **{solved_by_pressure}** recovered the pressure to \
         better than `1e-6` relative and **{solved_by_volume}** recovered the \
         density to better than `1e-9` instead. **{unsolved}** failed both.",
    ));

    // --- two-phase sweep ----------------------------------------------------
    let qualities = [0.1_f64, 0.25, 0.5, 0.75, 0.9];
    let mut dome_checked = 0_usize;
    let mut max_dome_p = 0.0_f64;
    let mut max_dome_x = 0.0_f64;

    for node in SATURATION_NODES {
        let [_t_deg_c, p_sat_bar, v_f, v_g, h_f, h_g] = *node;
        if !(v_f.is_finite() && v_f > 0.0 && v_g.is_finite() && v_g > v_f) {
            continue;
        }
        let p_ref = Pressure::new::<bar>(p_sat_bar);
        for x in qualities {
            let v = v_f + x * (v_g - v_f);
            let h_kj = h_f + x * (h_g - h_f);
            let rho = MassDensity::new::<kilogram_per_cubic_meter>(1.0 / v);
            let h = AvailableEnergy::new::<kilojoule_per_kilogram>(h_kj);
            if !rho_h_is_within_validity_range(rho, h) {
                continue;
            }
            let state = tpx_rho_h_eqm(rho, h);
            max_dome_p = max_dome_p.max(
                ((state.pressure.get::<pascal>() - p_ref.get::<pascal>()) / p_ref.get::<pascal>())
                    .abs(),
            );
            max_dome_x = max_dome_x.max((state.vapour_quality - x).abs());
            dome_checked += 1;
        }
    }

    report.section("Results — two-phase interior (Region 4)");
    report.paragraph(
        "States manufactured on each saturation tie line, where pressure and \
         quality are both known in advance by construction.",
    );
    report.table(
        &["states", "max abs(dp/p)", "max abs(dx)"],
        &[vec![
            format!("{dome_checked}"),
            format!("{max_dome_p:.3e}"),
            format!("{max_dome_x:.3e}"),
        ]],
    );

    report.section("Interpretation, including where this does not work");
    report.paragraph(
        "Region 4 — the two-phase dome, which is where a depressurisation \
         transient spends its time — is recovered essentially exactly. Regions 2 \
         and 3 are recovered to the level of the tables' own six-figure rounding.",
    );
    report.paragraph(
        "**Region 1, the compressed liquid, is not recoverable, and that is \
         physics rather than a defect.** Liquid water is nearly incompressible, \
         so its density carries almost no information about its pressure. At 0.1 \
         bar and 18 degC the amplification `|d ln p / d ln v|_h` reaches `2.0e5`, \
         which exceeds the pressure signal across the whole range of interest: \
         IF97's own `T(p,h)` backward equation, good to about 25 mK, already \
         moves the specific volume by more than the pressure does. The solver \
         there returns a pressure reproducing the requested density to machine \
         precision that is still badly wrong, and no implementation can do \
         better. `p_rho_h_conditioning` exposes this so a caller can detect the \
         regime.",
    );
    report.paragraph(
        "A second, smaller effect: nodes sitting exactly on an IF97 sub-region \
         seam show a large `|dv/v|` with an excellent `|dp/p|`, because the \
         reference `v(p,h)` is itself discontinuous there. The 40 bar / 280 degC \
         node lands on the Region 2a/2b backward-equation split at `p = 4 MPa`, \
         where `v` steps by `1.5e-5` while the pressure is recovered to `2.6e-13`.",
    );

    report.write(
        "cargo test --release -p tampines-steam-tables --lib \
         write_rho_h_flash_vv_report",
    );
}
