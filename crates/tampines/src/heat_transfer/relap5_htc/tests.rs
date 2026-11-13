//! V&V for the RELAP5 geometry-101/110 single-phase heat-transfer
//! selection (GitHub #1063).
//!
//! **Methodology.** Each correlation is pinned at points computed by hand
//! from its source paper's equation (calculator arithmetic in each test's
//! doc comment, independently re-evaluated in IEEE double with the
//! platform `pow`), at `max_relative = 1e-10` (PETIR's `r_powf` against the
//! platform `pow` differs by round-off only). The dispatcher is tested at
//! each regime boundary from both sides: the laminar/turbulent crossover
//! `Re* = (4.36/0.023/Pr^n)^1.25`, natural-convection dominance at low `Re`
//! and high `Gr`, the `Pr = 0.1` liquid-metal switch, and the `P/D` clips
//! of both bundle correlations. The `uom` wrapper is pinned against
//! hand-formed `Re`, `Pr`, `Gr` and `h = Nu k / D_h`.
//!
//! **Predictions (written 2026-11-01, before the first run).** Every pin
//! agrees to better than 1e-12 relative. Crossover: `Re* = 703.39` at
//! `Pr = 1` (heated) and `265.86` at `Pr = 7`, so Re = 700 / 710 and
//! 260 / 270 straddle them. `Pr = 0.1` selects Dittus–Boelter (14.512 at
//! `Re = 1e4`), `Pr = 0.0999` selects Seban–Shimazaki (11.2747): a 22 %
//! drop across the switch.
//!
//! **Results (2026-11-01, `cargo test --release -p tampines --lib
//! relap5_htc`, toolchain 1.99.1).** 9 of 9 pass on the first run. With
//! every `1e-10` tolerance temporarily tightened to `1e-15` they still
//! pass: each pin agrees with its IEEE re-evaluation to a few ulp. All
//! switches fall where predicted. Table:
//! `crates/tampines/docs/relap5-htc-dispatcher-vv.md`.

use super::*;
use uom::si::acceleration::meter_per_second_squared;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::velocity::meter_per_second;

/// Relative-closeness assertion with `approx`'s spelling; tampines has no
/// `approx` dev-dependency, and this keeps it that way.
fn assert_relative_eq(a: f64, b: f64, max_relative: f64) {
    let scale = a.abs().max(b.abs());
    assert!(
        (a - b).abs() <= max_relative * scale,
        "{a} vs {b}: relative difference {} > {}",
        (a - b).abs() / scale,
        max_relative
    );
}

fn r(x: f64) -> Ratio {
    Ratio::new::<ratio>(x)
}

fn groups(re: f64, pr: f64, gr: f64) -> Relap5DimensionlessGroups {
    Relap5DimensionlessGroups {
        reynolds: r(re),
        prandtl: r(pr),
        grashof: r(gr),
        free_convection_length_over_hydraulic_diameter: r(1.0),
        heating_direction: FluidHeatingDirection::FluidHeated,
    }
}

fn pipe(g: Relap5DimensionlessGroups) -> Relap5NusseltSelection {
    relap5_nusselt(Relap5HeatTransferGeometry::InternalPipe, g).unwrap()
}

fn bundle(pd: Option<f64>, g: Relap5DimensionlessGroups) -> Relap5NusseltSelection {
    relap5_nusselt(
        Relap5HeatTransferGeometry::VerticalRodBundleParallelFlow {
            pitch_to_diameter: pd.map(r),
        },
        g,
    )
    .unwrap()
}

/// Seban–Shimazaki, by hand: `Pe = 100`: `100^0.8 = 39.81072`,
/// `Nu = 5 + 0.99527 = 5.99527`; `Pe = 1000`: `1000^0.8 = 251.1886`,
/// `Nu = 5 + 6.27972 = 11.27972`. IEEE: `5.995267926383743`,
/// `11.279716078773951`. `Pe = 0` gives exactly 5.
#[test]
fn seban_shimazaki_matches_hand_calculation() {
    assert_relative_eq(
        seban_shimazaki_liquid_metal_nusselt(100.0),
        5.995267926383743,
        1e-10
    );
    assert_relative_eq(
        seban_shimazaki_liquid_metal_nusselt(1000.0),
        11.279716078773951,
        1e-10
    );
    assert_eq!(seban_shimazaki_liquid_metal_nusselt(0.0), 5.0);
}

/// Kazimi–Carelli, by hand: `P/D = 1.2, Pe = 100`: `1.2^3.8 = 1.99938`,
/// `1.2^5 = 2.48832`, `Nu = 4 + 0.33·1.99938 + 0.16·2.48832 = 5.05793`
/// (IEEE `5.05791656017895`). `P/D = 1.3, Pe = 500`: IEEE
/// `8.163613972978698`. Clip: `P/D = 1.0 → 1.1` (IEEE `4.731711944565632`
/// at `Pe = 100`), `1.5 → 1.4` (`6.045742504749947`), `None → 1.1`.
#[test]
fn kazimi_carelli_matches_hand_calculation_and_clips() {
    let kc = kazimi_carelli_rod_bundle_liquid_metal_nusselt;
    assert_relative_eq(kc(1.2, 100.0), 5.05791656017895, 1e-10);
    assert_relative_eq(kc(1.2, 100.0), 5.05793, 1e-5);
    assert_relative_eq(kc(1.3, 500.0), 8.163613972978698, 1e-10);
    assert_eq!(kazimi_carelli_pitch_to_diameter(Some(1.0)), 1.1);
    assert_eq!(kazimi_carelli_pitch_to_diameter(Some(1.5)), 1.4);
    assert_eq!(kazimi_carelli_pitch_to_diameter(Some(1.25)), 1.25);
    assert_eq!(kazimi_carelli_pitch_to_diameter(None), 1.1);
    assert_relative_eq(
        kc(kazimi_carelli_pitch_to_diameter(Some(1.0)), 100.0),
        4.731711944565632,
        1e-10
    );
    assert_relative_eq(
        kc(kazimi_carelli_pitch_to_diameter(Some(1.5)), 100.0),
        6.045742504749947,
        1e-10
    );
}

/// Inayatov clip: `None` and `< 1.1` give 1.1, `> 1.6` gives 1.6; the
/// coefficient is `0.023 P/D`.
#[test]
fn inayatov_multiplier_clips() {
    assert_eq!(inayatov_pitch_to_diameter(None), 1.1);
    assert_eq!(inayatov_pitch_to_diameter(Some(0.9)), 1.1);
    assert_eq!(inayatov_pitch_to_diameter(Some(1.3)), 1.3);
    assert_eq!(inayatov_pitch_to_diameter(Some(2.0)), 1.6);
    assert_relative_eq(inayatov_bundle_coefficient(Some(1.3)), 0.0299, 1e-14);
}

/// Geometry 101, laminar/turbulent crossover. `Pr = 1`, heated, `Gr = 0`:
/// `Re* = (4.36/0.023)^1.25 = 703.39`. At `Re = 700` Dittus–Boelter is
/// `4.3432 < 4.36` → laminar 4.36; at `Re = 710` it is `4.392730049578013`
/// → turbulent. `Pr = 7`: `Re* = 265.86`; `Re = 260` → laminar, `270` →
/// `4.414261563439074`. Cooling at `Re = 1e4, Pr = 7`: `n = 0.3`,
/// `65.35175396357931`.
#[test]
fn pipe_switches_laminar_to_turbulent_at_the_crossover() {
    let s = pipe(groups(700.0, 1.0, 0.0));
    assert_eq!(s.regime, Relap5HeatTransferRegime::ForcedLaminar);
    assert_eq!(s.nusselt.get::<ratio>(), 4.36);
    let s = pipe(groups(710.0, 1.0, 0.0));
    assert_eq!(s.regime, Relap5HeatTransferRegime::ForcedTurbulent);
    assert_relative_eq(s.nusselt.get::<ratio>(), 4.392730049578013, 1e-10);

    assert_eq!(pipe(groups(260.0, 7.0, 0.0)).regime, Relap5HeatTransferRegime::ForcedLaminar);
    let s = pipe(groups(270.0, 7.0, 0.0));
    assert_eq!(s.regime, Relap5HeatTransferRegime::ForcedTurbulent);
    assert_relative_eq(s.nusselt.get::<ratio>(), 4.414261563439074, 1e-10);

    let cooled = Relap5DimensionlessGroups {
        heating_direction: FluidHeatingDirection::FluidCooled,
        ..groups(1e4, 7.0, 0.0)
    };
    assert_relative_eq(
        pipe(cooled).nusselt.get::<ratio>(),
        65.35175396357931,
        1e-10
    );
}

/// Natural-convection dominance: `Re = 100, Pr = 7` (Dittus–Boelter 1.99,
/// laminar 4.36), `Gr = 1e6/7` so `Ra = 1e6`: Churchill–Chu
/// `19.976669857906355` wins. At `Gr = 0` the free-convection leg is
/// 0.680625 and laminar wins. With `L = 2 D_h` the value on `L` is
/// rescaled by `D_h/L = 0.5`.
#[test]
fn pipe_selects_free_convection_at_low_re_high_gr() {
    let s = pipe(groups(100.0, 7.0, 1e6 / 7.0));
    assert_eq!(s.regime, Relap5HeatTransferRegime::FreeConvection);
    assert_relative_eq(s.nusselt.get::<ratio>(), 19.976669857906355, 1e-10);
    assert_eq!(pipe(groups(100.0, 7.0, 0.0)).regime, Relap5HeatTransferRegime::ForcedLaminar);
    let long = Relap5DimensionlessGroups {
        free_convection_length_over_hydraulic_diameter: r(2.0),
        ..groups(100.0, 7.0, 1e6 / 7.0)
    };
    assert_relative_eq(
        pipe(long).nusselt.get::<ratio>(),
        19.976669857906355 / 2.0,
        1e-10
    );
}

/// `Pr = 0.1` is an ordinary fluid (Dittus–Boelter, `Re = 1e4`:
/// `0.023·1584.893·0.1^0.4 = 14.512`, IEEE `14.51201892304445`);
/// `Pr = 0.0999` is a liquid metal (`Pe = 999`, Seban–Shimazaki
/// `11.274691803332587`), even with a large Grashof number (no
/// max-selection for liquid metals).
#[test]
fn pipe_switches_to_liquid_metal_below_prandtl_0_1() {
    let s = pipe(groups(1e4, 0.1, 0.0));
    assert_eq!(s.regime, Relap5HeatTransferRegime::ForcedTurbulent);
    assert_relative_eq(s.nusselt.get::<ratio>(), 14.51201892304445, 1e-10);
    let s = pipe(groups(1e4, 0.0999, 1e12));
    assert_eq!(s.regime, Relap5HeatTransferRegime::LiquidMetal);
    assert_relative_eq(s.nusselt.get::<ratio>(), 11.274691803332587, 1e-10);
}

/// Geometry 110. Ordinary fluid, `P/D = 1.3, Re = 5e4, Pr = 1.5`, heated:
/// `0.023·1.3·(5e4)^0.8·1.5^0.4 = 201.9685` (IEEE `201.9685257588318`).
/// `P/D` absent → 1.1 (`170.8964448728577`); `2.0` → 1.6
/// (`248.576647087793`). Liquid metal, `Re = 1e4, Pr = 0.05` (`Pe = 500`):
/// `P/D = 1.3` → `8.163613972978698`; absent → 1.1
/// (`6.149678180777782`); `1.5` → clipped to 1.4, equal to `P/D = 1.4`.
/// The laminar and free-convection legs are shared with geometry 101.
#[test]
fn rod_bundle_applies_inayatov_and_liquid_metal_forms_with_clipping() {
    let g = groups(5e4, 1.5, 0.0);
    assert_relative_eq(
        bundle(Some(1.3), g).nusselt.get::<ratio>(),
        201.9685257588318,
        1e-10
    );
    assert_relative_eq(
        bundle(None, g).nusselt.get::<ratio>(),
        170.8964448728577,
        1e-10
    );
    assert_relative_eq(
        bundle(Some(2.0), g).nusselt.get::<ratio>(),
        248.576647087793,
        1e-10
    );
    let lm = groups(1e4, 0.05, 0.0);
    let s = bundle(Some(1.3), lm);
    assert_eq!(s.regime, Relap5HeatTransferRegime::LiquidMetal);
    assert_relative_eq(s.nusselt.get::<ratio>(), 8.163613972978698, 1e-10);
    assert_relative_eq(
        bundle(None, lm).nusselt.get::<ratio>(),
        6.149678180777782,
        1e-10
    );
    assert_eq!(bundle(Some(1.5), lm).nusselt, bundle(Some(1.4), lm).nusselt);
    // shared legs
    assert_eq!(bundle(Some(1.3), groups(10.0, 1.0, 0.0)).nusselt.get::<ratio>(), 4.36);
    assert_eq!(
        bundle(Some(1.3), groups(100.0, 7.0, 1e6 / 7.0)).nusselt,
        pipe(groups(100.0, 7.0, 1e6 / 7.0)).nusselt
    );
    assert_eq!(
        Relap5HeatTransferGeometry::VerticalRodBundleParallelFlow { pitch_to_diameter: None }
            .relap5_geometry_number(),
        110
    );
    assert_eq!(Relap5HeatTransferGeometry::InternalPipe.relap5_geometry_number(), 101);
}

fn water_like(speed: f64, wall_k: f64) -> Relap5WallConvectionInputs {
    Relap5WallConvectionInputs {
        density: MassDensity::new::<kilogram_per_cubic_meter>(1000.0),
        dynamic_viscosity: DynamicViscosity::new::<pascal_second>(1e-3),
        thermal_conductivity: ThermalConductivity::new::<watt_per_meter_kelvin>(0.6),
        specific_heat_capacity: SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(4180.0),
        thermal_expansion_coefficient: TemperatureCoefficient::new::<per_kelvin>(2e-4),
        bulk_temperature: ThermodynamicTemperature::new::<kelvin>(300.0),
        wall_temperature: ThermodynamicTemperature::new::<kelvin>(wall_k),
        flow_speed: Velocity::new::<meter_per_second>(speed),
        hydraulic_diameter: Length::new::<meter>(0.02),
        free_convection_length: None,
        gravity: Acceleration::new::<meter_per_second_squared>(9.81),
    }
}

/// The `uom` wrapper, by hand. `ρ = 1000, μ = 1e-3, k = 0.6, c_p = 4180,
/// β = 2e-4, T = 300 K, T_w = 310 K, D_h = 0.02 m, g = 9.81`:
/// `Pr = 1e-3·4180/0.6 = 6.96667`,
/// `Gr = 1e6·9.81·2e-4·10·(0.02)³/1e-6 = 156960`.
/// `u = 1 m/s`: `Re = 20000`, Dittus–Boelter `137.9627508157737` wins,
/// `h = 137.96·0.6/0.02 = 4138.88 W/m²K` (IEEE `4138.882524473211`).
/// `u = 0.001 m/s`: `Re = 20`, Churchill–Chu at `Ra = 1.0935e6`
/// `20.463677824860387` wins, `h = 613.91`. With `L = 0.04 m`:
/// `Gr ×8`, `Nu_L = 36.671780602556275`, `Nu_Dh = 18.33589`,
/// `h = 550.0767090383441`. `T_w < T` selects the cooling exponent.
#[test]
fn wall_heat_transfer_forms_groups_and_h() {
    let out =
        relap5_wall_heat_transfer(Relap5HeatTransferGeometry::InternalPipe, &water_like(1.0, 310.0))
            .unwrap();
    assert_relative_eq(out.groups.reynolds.get::<ratio>(), 20000.0, 1e-12);
    assert_relative_eq(out.groups.prandtl.get::<ratio>(), 6.966666666666667, 1e-12);
    assert_relative_eq(out.groups.grashof.get::<ratio>(), 156960.0, 1e-12);
    assert_eq!(out.selection.regime, Relap5HeatTransferRegime::ForcedTurbulent);
    assert_relative_eq(
        out.heat_transfer_coefficient.get::<watt_per_square_meter_kelvin>(),
        4138.882524473211,
        1e-10
    );

    let slow =
        relap5_wall_heat_transfer(Relap5HeatTransferGeometry::InternalPipe, &water_like(0.001, 310.0))
            .unwrap();
    assert_eq!(slow.selection.regime, Relap5HeatTransferRegime::FreeConvection);
    assert_relative_eq(
        slow.heat_transfer_coefficient.get::<watt_per_square_meter_kelvin>(),
        20.463677824860387 * 30.0,
        1e-10
    );

    let long = Relap5WallConvectionInputs {
        free_convection_length: Some(Length::new::<meter>(0.04)),
        ..water_like(0.001, 310.0)
    };
    let long = relap5_wall_heat_transfer(Relap5HeatTransferGeometry::InternalPipe, &long).unwrap();
    assert_relative_eq(
        long.heat_transfer_coefficient.get::<watt_per_square_meter_kelvin>(),
        550.0767090383441,
        1e-10
    );

    let cooled = water_like(1.0, 290.0).dimensionless_groups().unwrap();
    assert_eq!(cooled.heating_direction, FluidHeatingDirection::FluidCooled);
    assert_relative_eq(cooled.grashof.get::<ratio>(), 156960.0, 1e-12);
}

/// Unphysical inputs are rejected rather than producing NaN.
#[test]
fn invalid_inputs_are_rejected() {
    let geo = Relap5HeatTransferGeometry::InternalPipe;
    assert!(relap5_nusselt(geo, groups(-1.0, 1.0, 0.0)).is_err());
    assert!(relap5_nusselt(geo, groups(1e4, 0.0, 0.0)).is_err());
    assert!(relap5_nusselt(geo, groups(1e4, 1.0, -1.0)).is_err());
    assert!(relap5_nusselt(geo, groups(f64::NAN, 1.0, 0.0)).is_err());
    let bad_l = Relap5DimensionlessGroups {
        free_convection_length_over_hydraulic_diameter: r(0.0),
        ..groups(1e4, 1.0, 0.0)
    };
    assert!(relap5_nusselt(geo, bad_l).is_err());
    let bad_pd = Relap5HeatTransferGeometry::VerticalRodBundleParallelFlow {
        pitch_to_diameter: Some(r(f64::NAN)),
    };
    assert!(relap5_nusselt(bad_pd, groups(1e4, 1.0, 0.0)).is_err());
    let bad_d = Relap5WallConvectionInputs {
        hydraulic_diameter: Length::new::<meter>(0.0),
        ..water_like(1.0, 310.0)
    };
    assert!(relap5_wall_heat_transfer(geo, &bad_d).is_err());
    let bad_len = Relap5WallConvectionInputs {
        free_convection_length: Some(Length::new::<meter>(-1.0)),
        ..water_like(1.0, 310.0)
    };
    assert!(bad_len.dimensionless_groups().is_err());
    let bad_g = Relap5WallConvectionInputs {
        gravity: Acceleration::new::<meter_per_second_squared>(f64::NAN),
        ..water_like(1.0, 310.0)
    };
    assert!(bad_g.dimensionless_groups().is_err());
}
