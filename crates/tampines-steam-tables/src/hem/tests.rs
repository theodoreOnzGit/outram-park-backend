//! Unit tests of the HEM closures and solver plumbing. The V&V cases (Sod,
//! positivity, conservation, double rarefaction, Edwards) are integration
//! tests: `tests/hem_pipe_verification.rs`, `tests/hem_pipe_edwards.rs`.

use petir::mathf::RealMath;
use super::pipe::test_access::muscl_1d;
use super::*;
use uom::si::area::square_meter;
use uom::si::available_energy::{joule_per_kilogram, kilojoule_per_kilogram};
use uom::si::f64::*;
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_flux::kilogram_per_square_meter_second;
use uom::si::pressure::{megapascal, pascal};
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

fn pa(v: f64) -> Pressure {
    Pressure::new::<pascal>(v)
}
fn kg_m3(v: f64) -> MassDensity {
    MassDensity::new::<kilogram_per_cubic_meter>(v)
}
fn j_kg(v: f64) -> AvailableEnergy {
    AvailableEnergy::new::<joule_per_kilogram>(v)
}
fn ms(v: f64) -> Velocity {
    Velocity::new::<meter_per_second>(v)
}

// ───────────────────────── closures ─────────────────────────

/// Ideal gas: `(p, ρ) → (ρ, e) → p` is exact algebra, and every constructor
/// agrees with the closed form.
#[test]
fn ideal_gas_closure_round_trips() {
    let g = IdealGasHem::air();
    let s = g.state_p_rho(pa(1.0e5), kg_m3(1.0)).unwrap();
    assert!((s.specific_internal_energy.get::<joule_per_kilogram>() - 2.5e5).abs() < 1e-9);
    assert!((s.equilibrium_sound_speed.get::<meter_per_second>() - (1.4e5f64).sqrt()).abs() < 1e-9);
    let back = g
        .state_rho_e(s.density, s.specific_internal_energy, None)
        .unwrap();
    assert!((back.pressure.get::<pascal>() - 1.0e5).abs() < 1e-9);
    let t = g.state_p_t(pa(1.0e5), s.temperature).unwrap();
    assert!((t.density.get::<kilogram_per_cubic_meter>() - 1.0).abs() < 1e-12);
    let h = g.state_p_h(pa(1.0e5), s.specific_enthalpy).unwrap();
    assert!((h.density.get::<kilogram_per_cubic_meter>() - 1.0).abs() < 1e-12);
    assert_eq!(s.quality, None);
    assert_eq!(g.name(), "ideal gas");
}

/// Ideal gas input validation, and the trait's default `break_flow`.
#[test]
fn ideal_gas_rejects_bad_input_and_has_no_break_model() {
    let r = SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(287.0);
    assert!(IdealGasHem::new(Ratio::new::<ratio>(1.0), r).is_err());
    assert!(IdealGasHem::new(
        Ratio::new::<ratio>(1.4),
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(-1.0)
    )
    .is_err());
    let g = IdealGasHem::new(Ratio::new::<ratio>(1.4), r).unwrap();
    assert!(matches!(
        g.state_rho_e(kg_m3(1.0), j_kg(-1.0), None),
        Err(HemError::EquationOfState { .. })
    ));
    assert!(matches!(
        g.break_flow(pa(2e5), j_kg(3e5), pa(1e5)),
        Err(HemError::Unsupported { .. })
    ));
}

/// **IF97 `(ρ, e)` inversion round trip.** A state built from `(p, T)` or
/// `(p, h)` is closed again from its own `(ρ, e)`, with and without a
/// pressure hint, in subcooled liquid, inside the dome, and in vapour. The
/// pressure must come back to `1e-9` relative: the inversion tolerance is
/// `1e-12` on the bracket, and the liquid's stiffness (`|d ln p/d ln v|` up to
/// ~1e4) is not in play because the density fed back is exact.
#[test]
fn iapws97_rho_e_inversion_round_trips_in_every_phase() {
    let w = Iapws97Hem;
    let states = [
        w.state_p_t(
            Pressure::new::<megapascal>(7.0),
            ThermodynamicTemperature::new::<kelvin>(500.0),
        )
        .unwrap(),
        w.state_p_h(
            Pressure::new::<megapascal>(1.0),
            AvailableEnergy::new::<kilojoule_per_kilogram>(1500.0),
        )
        .unwrap(),
        w.state_p_t(
            Pressure::new::<megapascal>(0.5),
            ThermodynamicTemperature::new::<kelvin>(600.0),
        )
        .unwrap(),
    ];
    assert!(
        states[1].quality.is_some(),
        "1 MPa, 1500 kJ/kg is inside the dome"
    );
    for s in states {
        for hint in [None, Some(s.pressure * 1.3)] {
            let back = w
                .state_rho_e(s.density, s.specific_internal_energy, hint)
                .unwrap();
            let rel = (back.pressure.get::<pascal>() / s.pressure.get::<pascal>() - 1.0).abs();
            assert!(rel < 1e-9, "p round trip {rel:e} for {s:?}");
        }
    }
}

/// IF97 refuses what it cannot close, with an error rather than a panic.
#[test]
fn iapws97_errors_outside_its_envelope() {
    let w = Iapws97Hem;
    assert!(matches!(
        w.state_rho_e(kg_m3(-1.0), j_kg(1e6), None),
        Err(HemError::EquationOfState { .. })
    ));
    assert!(matches!(
        w.state_rho_e(kg_m3(1000.0), j_kg(f64::NAN), None),
        Err(HemError::EquationOfState { .. })
    ));
    // Far colder than 273.15 K liquid: no pressure brackets it.
    assert!(w.state_rho_e(kg_m3(1000.0), j_kg(-5.0e5), None).is_err());
    assert!(w.state_p_h(pa(1e5), j_kg(1e8)).is_err());
    assert!(w
        .state_p_t(pa(1e5), ThermodynamicTemperature::new::<kelvin>(200.0))
        .is_err());
    assert!(matches!(
        w.state_p_rho(pa(1e5), kg_m3(1.0)),
        Err(HemError::Unsupported { .. })
    ));
    assert_eq!(w.name(), "IAPWS-IF97");
}

/// IF97 break flow: no flow at or below the back pressure; choked from a
/// 7 MPa, 500 K subcooled stagnation state to 0.1 MPa (the Edwards break) at
/// the critical flux of the dispatcher it wraps; subcritical (not choked) for
/// a 1 % pressure drop, with a flux below the critical one.
#[test]
fn iapws97_break_flow_branches() {
    let w = Iapws97Hem;
    let s = w
        .state_p_t(
            Pressure::new::<megapascal>(7.0),
            ThermodynamicTemperature::new::<kelvin>(500.0),
        )
        .unwrap();
    let none = w
        .break_flow(
            s.pressure,
            s.specific_enthalpy,
            Pressure::new::<megapascal>(8.0),
        )
        .unwrap();
    assert_eq!(
        none.mass_flux.get::<kilogram_per_square_meter_second>(),
        0.0
    );
    let choked = w
        .break_flow(
            s.pressure,
            s.specific_enthalpy,
            Pressure::new::<megapascal>(0.1),
        )
        .unwrap();
    assert!(choked.choked);
    let (_, g_star) = crate::steam_turbine_equations::converging_diverging_nozzles::choked_flow::get_critical_pressure_and_mass_flux_multiphase_ph(s.pressure, s.specific_enthalpy);
    assert_eq!(choked.mass_flux, g_star);
    let sub = w
        .break_flow(
            s.pressure,
            s.specific_enthalpy,
            Pressure::new::<megapascal>(6.93),
        )
        .unwrap();
    assert!(!sub.choked);
    let g_sub = sub.mass_flux.get::<kilogram_per_square_meter_second>();
    assert!(g_sub > 0.0 && g_sub < g_star.get::<kilogram_per_square_meter_second>());
}

// ───────────────────────── reconstruction ─────────────────────────

/// **The uniform-1-D MUSCL used by `HemPipe` equals basic-lib's
/// `reconstruct_pos_neg`.** Methodology: a 6-cell 1-D `FvMesh` with fixed-value
/// patches, a non-monotone field, van Leer and minmod; the internal-face
/// `pos`/`neg` values of both must agree to `1e-14` (they differ only by the
/// rounding of a Gauss gradient against a difference quotient). This is what
/// lets the solver claim it uses basic-lib's reconstruction rather than a
/// second one.
#[test]
fn reconstruction_matches_basic_lib_reconstruct_pos_neg() {
    use outram_foam_basic_lib::fields::boundary::bc::BoundaryCondition;
    use outram_foam_basic_lib::fields::field::Field;
    use outram_foam_basic_lib::fields::vol_field::VolScalarField;
    use outram_foam_basic_lib::fv_operators::fvc::{reconstruct_pos_neg, Limiter};
    use outram_foam_basic_lib::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use outram_foam_basic_lib::primitives::vector::Vector3;
    use std::sync::Arc;

    let n = 6usize;
    let dx = 0.25;
    let mut owner: Vec<usize> = (0..n - 1).collect();
    owner.push(0);
    owner.push(n - 1);
    let neighbour: Vec<usize> = (1..n).collect();
    let mut sf = vec![Vector3::new(1.0, 0.0, 0.0); n - 1];
    sf.push(Vector3::new(-1.0, 0.0, 0.0));
    sf.push(Vector3::new(1.0, 0.0, 0.0));
    let mut fc: Vec<Vector3> = (1..n)
        .map(|i| Vector3::new(i as f64 * dx, 0.0, 0.0))
        .collect();
    fc.push(Vector3::new(0.0, 0.0, 0.0));
    fc.push(Vector3::new(n as f64 * dx, 0.0, 0.0));
    let mesh = Arc::new(
        FvMeshBuilder::new()
            .n_cells(n)
            .n_internal_faces(n - 1)
            .owner(owner)
            .neighbour(neighbour)
            .patches(vec![
                BoundaryPatch::new("left", n - 1, 1, PatchKind::Patch),
                BoundaryPatch::new("right", n, 1, PatchKind::Patch),
            ])
            .cell_volumes(vec![dx; n])
            .cell_centres(
                (0..n)
                    .map(|i| Vector3::new((i as f64 + 0.5) * dx, 0.0, 0.0))
                    .collect(),
            )
            .face_area_vectors(sf)
            .face_centres(fc)
            .build()
            .unwrap(),
    );
    let phi = [1.0, 1.2, 3.0, 2.5, 2.6, 0.4];
    let (b_l, b_r) = (0.7, 0.1);
    for (lim_b, lim_h) in [
        (Limiter::VanLeer, FluxLimiter::VanLeer),
        (Limiter::Minmod, FluxLimiter::Minmod),
    ] {
        let mut f = VolScalarField::zeros("phi", mesh.clone());
        f.internal = Field::new(phi.to_vec());
        f.boundary[0].bc = BoundaryCondition::FixedValue(b_l);
        f.boundary[0].values = Field::new(vec![b_l]);
        f.boundary[1].bc = BoundaryCondition::FixedValue(b_r);
        f.boundary[1].values = Field::new(vec![b_r]);
        let (pos, neg) = reconstruct_pos_neg(&f, lim_b);
        let mut ext = vec![2.0 * b_l - phi[0]];
        ext.extend_from_slice(&phi);
        ext.push(2.0 * b_r - phi[n - 1]);
        let (p2, n2) = muscl_1d(&ext, lim_h);
        for fi in 0..n - 1 {
            assert!((pos.internal[fi] - p2[fi]).abs() < 1e-14, "pos face {fi}");
            assert!((neg.internal[fi] - n2[fi]).abs() < 1e-14, "neg face {fi}");
        }
    }
}

// ───────────────────────── solver plumbing ─────────────────────────

fn gas_pipe(n: usize, initial: HemInitialCondition, numerics: HemNumerics) -> HemPipe<IdealGasHem> {
    let geo = HemPipeGeometry::new(
        Length::new::<meter>(1.0),
        Area::new::<square_meter>(0.01),
        n,
    )
    .unwrap();
    HemPipe::new(geo, IdealGasHem::air(), initial, numerics).unwrap()
}

fn gas(p: f64, rho: f64, u: f64) -> PrimitiveState {
    PrimitiveState::PressureDensity {
        pressure: pa(p),
        density: kg_m3(rho),
        velocity: ms(u),
    }
}

/// Bad geometry, numerics, initial conditions, boundaries and calls are
/// `InvalidInput`, before any physics runs.
#[test]
fn invalid_inputs_are_rejected() {
    let l = Length::new::<meter>(1.0);
    let a = Area::new::<square_meter>(1.0);
    assert!(HemPipeGeometry::new(Length::new::<meter>(0.0), a, 3).is_err());
    assert!(HemPipeGeometry::new(l, Area::new::<square_meter>(-1.0), 3).is_err());
    assert!(HemPipeGeometry::new(l, a, 0).is_err());
    let geo = HemPipeGeometry::new(l, a, 3).unwrap();
    assert_eq!(geo.length(), l);
    assert_eq!(geo.flow_area(), a);
    assert!((geo.cell_width().get::<meter>() - 1.0 / 3.0).abs() < 1e-15);
    assert!((geo.cell_centre(2).get::<meter>() - 2.5 / 3.0).abs() < 1e-15);
    let bad_ic = HemInitialCondition::PerCell(vec![gas(1e5, 1.0, 0.0); 2]);
    assert!(matches!(
        HemPipe::new(geo, IdealGasHem::air(), bad_ic, HemNumerics::default()),
        Err(HemError::InvalidInput(_))
    ));
    let mut nm = HemNumerics::default();
    nm.time_step = TimeStepControl::Fixed(Time::new::<second>(0.0));
    assert!(HemPipe::new(
        geo,
        IdealGasHem::air(),
        HemInitialCondition::Uniform(gas(1e5, 1.0, 0.0)),
        nm
    )
    .is_err());
    nm.time_step = TimeStepControl::AdaptiveCourant {
        courant: Ratio::new::<ratio>(0.0),
        max_dt: Time::new::<second>(1.0),
    };
    assert!(HemPipe::new(
        geo,
        IdealGasHem::air(),
        HemInitialCondition::Uniform(gas(1e5, 1.0, 0.0)),
        nm
    )
    .is_err());
    // IF97 cannot take a (p, rho) initial state: error names the cell.
    assert!(matches!(
        HemPipe::new(
            geo,
            Iapws97Hem,
            HemInitialCondition::Uniform(gas(1e5, 1.0, 0.0)),
            HemNumerics::default()
        ),
        Err(HemError::Unsupported { .. })
    ));

    let mut p = gas_pipe(
        4,
        HemInitialCondition::Uniform(gas(1e5, 1.0, 0.0)),
        HemNumerics::default(),
    );
    assert!(p
        .set_boundary(PipeEnd::Outlet, HemBoundary::FixedPressure(pa(-1.0)))
        .is_err());
    assert!(p
        .set_boundary(
            PipeEnd::Outlet,
            HemBoundary::ChokedBreak {
                area_fraction: Ratio::new::<ratio>(1.5),
                back_pressure: pa(1e5)
            }
        )
        .is_err());
    assert!(p
        .set_boundary(
            PipeEnd::Inlet,
            HemBoundary::Reservoir {
                stagnation_pressure: pa(0.0),
                stagnation_enthalpy: j_kg(3e5)
            }
        )
        .is_err());
    assert!(p.cell_state(4).is_err());
    p.run_until(Time::new::<second>(1e-5)).unwrap();
    assert!(p.run_until(Time::new::<second>(0.0)).is_err());
    assert_eq!(p.boundary(PipeEnd::Inlet), HemBoundary::Wall);
    assert_eq!(p.geometry().n_cells(), 4);
    assert_eq!(*p.numerics(), HemNumerics::default());
    assert_eq!(p.equation_of_state().name(), "ideal gas");
}

/// A choked break on a closure with no break model is `Unsupported`, raised
/// by the step, and the state is left untouched.
#[test]
fn choked_break_needs_a_break_model_and_failed_steps_roll_back() {
    let mut p = gas_pipe(
        4,
        HemInitialCondition::Uniform(gas(2e5, 1.0, 0.0)),
        HemNumerics::default(),
    );
    p.set_boundary(
        PipeEnd::Outlet,
        HemBoundary::ChokedBreak {
            area_fraction: Ratio::new::<ratio>(1.0),
            back_pressure: pa(1e5),
        },
    )
    .unwrap();
    let before = p.snapshot();
    assert!(matches!(p.step(), Err(HemError::Unsupported { .. })));
    assert_eq!(p.snapshot(), before);
}

/// **Galilean invariance and symmetry.** A uniform gas moving at 50 m/s
/// between transmissive ends stays uniform (to `1e-12` relative: the boundary
/// physical flux and the internal KNP flux of equal states differ only by the
/// rounding of `(a⁺ − a⁻)/(a⁺ − a⁻)`); a mirror-symmetric state between two
/// walls stays mirror-symmetric to round-off.
#[test]
fn uniform_flow_is_stationary_and_mirror_symmetry_is_kept() {
    let mut p = gas_pipe(
        20,
        HemInitialCondition::Uniform(gas(1e5, 1.2, 50.0)),
        HemNumerics::default(),
    );
    p.set_boundary(PipeEnd::Inlet, HemBoundary::Transmissive)
        .unwrap();
    p.set_boundary(PipeEnd::Outlet, HemBoundary::Transmissive)
        .unwrap();
    p.run_until(Time::new::<second>(1e-3)).unwrap();
    for c in p.snapshot().cells {
        assert!((c.pressure.get::<pascal>() / 1e5 - 1.0).abs() < 1e-12);
        assert!((c.velocity.get::<meter_per_second>() / 50.0 - 1.0).abs() < 1e-12);
    }

    let n = 16;
    let cells: Vec<PrimitiveState> = (0..n)
        .map(|i| {
            let x = (i as f64 + 0.5) / n as f64 - 0.5; // symmetric about 0
            gas(1e5 * (1.0 + 0.5 * (-20.0 * x * x).r_exp()), 1.0, 0.0)
        })
        .collect();
    let mut s = gas_pipe(
        n,
        HemInitialCondition::PerCell(cells),
        HemNumerics::default(),
    );
    s.run_until(Time::new::<second>(5e-4)).unwrap();
    let snap = s.snapshot();
    for i in 0..n / 2 {
        let (a, b) = (&snap.cells[i], &snap.cells[n - 1 - i]);
        assert!((a.pressure.get::<pascal>() - b.pressure.get::<pascal>()).abs() < 1e-9);
        assert!(
            (a.velocity.get::<meter_per_second>() + b.velocity.get::<meter_per_second>()).abs()
                < 1e-9
        );
    }
}

/// **Open boundaries keep the books.** Reservoir inflow at the inlet and a
/// fixed lower pressure at the outlet drive a gas through the pipe under
/// forward Euler, SSP33 and SSP104. Pass: the run completes, mass flows in at
/// the inlet and out at the outlet, and `mass in pipe + Σ mass out` is
/// constant to `1e-12` relative (the four boundary integrals ride in the
/// state vector, so this is round-off).
#[test]
fn reservoir_and_fixed_pressure_ends_conserve_with_bookkeeping() {
    for integ in [
        TimeIntegrator::ForwardEuler,
        TimeIntegrator::Ssp33,
        TimeIntegrator::Ssp104,
    ] {
        let nm = HemNumerics {
            time_integrator: integ,
            ..HemNumerics::default()
        };
        let mut p = gas_pipe(20, HemInitialCondition::Uniform(gas(1e5, 1.2, 0.0)), nm);
        let g = IdealGasHem::air();
        let h0 = g
            .state_p_rho(pa(1.2e5), kg_m3(1.3))
            .unwrap()
            .specific_enthalpy;
        p.set_boundary(
            PipeEnd::Inlet,
            HemBoundary::Reservoir {
                stagnation_pressure: pa(1.2e5),
                stagnation_enthalpy: h0,
            },
        )
        .unwrap();
        p.set_boundary(PipeEnd::Outlet, HemBoundary::FixedPressure(pa(0.9e5)))
            .unwrap();
        let m0 = p.total_mass().get::<kilogram>();
        let e0 = p.total_energy().value;
        p.run_until(Time::new::<second>(5e-3)).unwrap();
        let m_in = -p.cumulative_mass_outflow(PipeEnd::Inlet).get::<kilogram>();
        let m_out = p.cumulative_mass_outflow(PipeEnd::Outlet).get::<kilogram>();
        assert!(
            m_in > 0.0 && m_out > 0.0,
            "{integ:?}: in {m_in}, out {m_out}"
        );
        let m1 = p.total_mass().get::<kilogram>();
        assert!(
            ((m1 - m_in + m_out) - m0).abs() < 1e-12 * m0,
            "{integ:?} mass books"
        );
        let e1 = p.total_energy().value
            + p.cumulative_energy_outflow(PipeEnd::Inlet).value
            + p.cumulative_energy_outflow(PipeEnd::Outlet).value;
        assert!((e1 - e0).abs() < 1e-12 * e0, "{integ:?} energy books");
        assert!(p.courant_rate().unwrap().value > 0.0);
    }
}

/// Error display strings carry the numbers a reader needs.
#[test]
fn errors_display_their_values() {
    let e = HemError::CourantViolation {
        courant: 0.75,
        limit: 0.5,
    };
    assert!(e.to_string().contains("0.7500"));
    let e = HemError::EquationOfState {
        cell: None,
        detail: "x".into(),
    }
    .in_cell(3);
    assert!(e.to_string().contains("cell 3"));
    let e = HemError::Unphysical {
        cell: 2,
        quantity: "density",
        value: -1.0,
    };
    assert_eq!(e.clone().in_cell(9), e);
}

// ───────────────────── gh:#1102 cold-liquid break flow ─────────────────────

/// `(p0, h0, ρ0)` of IF97 liquid at `(p0 [Pa], T0 [K])`.
///
/// The density is `ρ_1(T0, p0)` from the forward Region 1 equation at the
/// temperature the state was built from. **CORRECTED 2026-11-02 (gh:#1111):**
/// it used to be `state_p_t(..).density`, which `Iapws97Hem` forms from
/// `(p, h)` through the backward `T(p, h)`. That density belongs to
/// `T_ph(p0, h0)` (300.022 K for the 300 K state, −10 mK at 500 K), off by
/// 6e-6 / 1.6e-5 relative. The Bernoulli references shared that error with
/// the old solver; once `break_flow` used the forward-consistent `T0`, the
/// Δp → 0 check measured the reference's 3e-6 / −8e-6 instead of the
/// solver. No tolerance was changed.
fn liquid_stagnation(p0_pa: f64, t0_k: f64) -> (Pressure, AvailableEnergy, f64) {
    let t0 = ThermodynamicTemperature::new::<kelvin>(t0_k);
    let s = Iapws97Hem.state_p_t(pa(p0_pa), t0).unwrap();
    let rho0 = 1.0 / crate::region_1_subcooled_liquid::v_tp_1(t0, s.pressure).value;
    (s.pressure, s.specific_enthalpy, rho0)
}

/// Hand Bernoulli flux `√(2 ρ0 Δp)`, kg/(m² s).
fn bernoulli(rho0: f64, dp_pa: f64) -> f64 {
    (2.0 * rho0 * dp_pa).sqrt()
}

/// **gh:#1102 reproduction, and the hand Bernoulli check.** 300 K liquid at
/// 2.31 → 1.50, 2.0 → 1.5, 1.2 → 1.0 and 3 → 1 bar. Before the fix the first
/// three returned `Err` ("h0 − h_b < 0", e.g. −6.8 J/kg at 2.31 → 1.50 bar)
/// and the fourth returned 14 987 kg/(m² s), 25 % below Bernoulli, because
/// `h0 − h(p_b, s0)` passed through two IF97 backward equations whose
/// consistency error (tens of J/kg) is the size of `v Δp`.
///
/// Pass criterion: `Ok`, not choked (p* ≈ p_sat(300 K) = 3.5 kPa is below
/// every back pressure), and `|G/√(2 ρ0 Δp) − 1| ≤ 2 κ_T Δp`, the
/// compressibility correction to incompressible Bernoulli (κ_T(300 K) =
/// 4.5e-10 1/Pa, IAPWS-95 / IF97 Region 1), doubled for margin. Measured 2026-11-02:
/// worst |G/G_B − 1| = 6.7e-5 at 3 → 1 bar, against a bound of 1.8e-4.
#[test]
fn gh1102_cold_liquid_low_dp_break_flow_is_unchoked_bernoulli() {
    const KAPPA_T_300K: f64 = 4.5e-10;
    for (p0_bar, pb_bar) in [(2.31, 1.50), (2.0, 1.5), (1.2, 1.0), (3.0, 1.0)] {
        let (p0, h0, rho0) = liquid_stagnation(p0_bar * 1e5, 300.0);
        let dp = (p0_bar - pb_bar) * 1e5;
        let bf = Iapws97Hem
            .break_flow(p0, h0, pa(pb_bar * 1e5))
            .unwrap_or_else(|e| panic!("{p0_bar} -> {pb_bar} bar: {e:?}"));
        assert!(!bf.choked, "{p0_bar} -> {pb_bar} bar choked");
        let rel = bf.mass_flux.get::<kilogram_per_square_meter_second>() / bernoulli(rho0, dp) - 1.0;
        assert!(
            rel.abs() <= 2.0 * KAPPA_T_300K * dp,
            "{p0_bar} -> {pb_bar} bar: G/G_Bernoulli - 1 = {rel:e}"
        );
    }
}

/// **Continuity as Δp → 0.** For 300 K / 2.31 bar, 370 K / 1.2 bar and the
/// Edwards stagnation 500 K / 7 MPa, `G(Δp)/√(2 ρ0 Δp) → 1` for Δp = 1, 10,
/// 100 Pa, and `G` is zero at Δp = 0. Bound 1e-6: the round-off of `h0 − h_b`
/// at Δp = 1 Pa (`Δh` ≈ 1e-3 J/kg against `h` ≈ 1e5–1e6 J/kg at 1e-16
/// relative, ≈ 1e-7 in `Δh`) plus `κ_T Δp` ≤ 1e-7 at 100 Pa. Measured
/// 2026-11-02: worst 7.1e-8 (300 K, 1 Pa). **Re-measured 2026-11-02
/// (gh:#1111)**, with the forward-consistent `T0` in `break_flow` and the
/// reference density corrected (see [`liquid_stagnation`]): worst 4.4e-7
/// (370 K, 1 Pa), the round-off floor of `T Δs` at 1 Pa; bound unchanged.
#[test]
fn gh1102_break_flow_tends_to_bernoulli_as_dp_vanishes() {
    for (p0_pa, t0) in [(2.31e5, 300.0), (1.2e5, 370.0), (7.0e6, 500.0)] {
        let (p0, h0, rho0) = liquid_stagnation(p0_pa, t0);
        let zero = Iapws97Hem.break_flow(p0, h0, p0).unwrap();
        assert_eq!(zero.mass_flux.get::<kilogram_per_square_meter_second>(), 0.0);
        for dp in [1.0, 10.0, 100.0] {
            let bf = Iapws97Hem.break_flow(p0, h0, pa(p0_pa - dp)).unwrap();
            assert!(!bf.choked);
            let rel = bf.mass_flux.get::<kilogram_per_square_meter_second>() / bernoulli(rho0, dp) - 1.0;
            eprintln!("T0 = {t0} K, dp = {dp} Pa: rel = {rel:e}");
            assert!(rel.abs() <= 1e-6, "T0 = {t0} K, dp = {dp} Pa: {rel:e}");
        }
    }
}

/// Unchoked flux just above, and the choked flux just below, the critical
/// pressure `p*` of the dispatcher, with `G` non-decreasing as `p_b` falls
/// over a sweep from `p0` through `p*` to `p*/2`.
fn choked_transition(p0_pa: f64, t0: f64) -> (f64, f64, f64, bool) {
    let (p0, h0, rho0) = liquid_stagnation(p0_pa, t0);
    let (p_star, _) = crate::steam_turbine_equations::converging_diverging_nozzles::choked_flow::get_critical_pressure_and_mass_flux_multiphase_ph(p0, h0);
    let ps = p_star.get::<pascal>();
    let g = |pb: f64| {
        let b = Iapws97Hem.break_flow(p0, h0, pa(pb)).unwrap();
        (b.mass_flux.get::<kilogram_per_square_meter_second>(), b.choked)
    };
    let (g_above, c_above) = g(ps * (1.0 + 1e-9));
    let (g_below, c_below) = g(ps * (1.0 - 1e-9));
    assert!(!c_above && c_below, "T0 = {t0} K: choked flags {c_above} / {c_below}");
    let mut last = 0.0;
    let mut monotone = true;
    for k in 0..=200 {
        let pb = p0_pa - (p0_pa - 0.5 * ps) * f64::from(k) / 200.0;
        let (gk, _) = g(pb);
        monotone &= gk >= last * (1.0 - 1e-12);
        last = gk;
    }
    // Available enthalpy to the choke, for the tolerance.
    let dh = (p0_pa - ps) / rho0;
    (g_above, g_below, dh, monotone)
}

/// **Continuity across the choked/unchoked transition, Edwards stagnation
/// (7 MPa, 500 K).** Pass criterion, fixed before measuring:
/// `|G_unchoked(p*⁺)/G*(p*⁻) − 1| ≤ c_p · 25 mK / Δh`, where 25 mK is
/// IF97's permitted backward-equation consistency `|ΔT|` (the choked G* is
/// still computed through `h_ps_eqm`, gh:#1111), `c_p` = 4.65 kJ/(kg K) at
/// 500 K, and `Δh = (p0 − p*)/ρ0`. That bound is 2.2e-2 (on `Δh`, so
/// conservative by 2× on `G ∝ √Δh`). Measured 2026-11-02: `G(p*⁺)` =
/// 85 583, `G*` = 86 283 kg/(m² s), `rel` = −8.1e-3, a step up at the choke
/// inside the bound. The sweep from `p0` to `p*/2` is non-decreasing.
/// **Re-measured 2026-11-02 (gh:#1111)** after both branches moved to the
/// forward-consistent `T0`: `G(p*⁺)` = 85 577.866, `G*` = 85 577.867,
/// `rel` = −5.5e-9.
#[test]
fn gh1102_choked_transition_is_continuous_for_edwards_liquid() {
    let (g_above, g_below, dh, monotone) = choked_transition(7.0e6, 500.0);
    let rel = g_above / g_below - 1.0;
    let bound = 4.65e3 * 0.025 / dh;
    eprintln!("Edwards: G(p*+) = {g_above}, G*(p*-) = {g_below}, rel = {rel:e}, bound = {bound:e}");
    assert!(rel.abs() <= bound, "rel = {rel:e} > {bound:e}");
    assert!(monotone, "G(p_b) decreases somewhere as p_b falls");
}

/// **The same continuity check for cold liquid (300 K, 2.31 bar) — PASSES
/// since 2026-11-02, gh:#1111 (un-ignored).** Measured 2026-11-02 (release)
/// with the global-maximum subcooled solver on the forward-consistent
/// stagnation state: `G(p*⁺)` = 21 291.327, `G*` = 21 291.326 kg/(m² s),
/// `rel` = +4.2e-8, sweep monotone. The record below is how it ~~FAILS~~
/// failed before. Same criterion (c_p = 4.18 kJ/(kg K), Δh ≈ 228 J/kg, bound
/// 0.46). Measured 2026-11-02: `G(p*⁺)` = 21 291, `G*` = 16 387 kg/(m² s),
/// `rel` = +0.30. That is inside the bound, which at 300 K cannot see the
/// defect (25 mK of `c_p` is 105 J/kg, half the available `Δh`). The test
/// fails on the monotonicity assertion: G drops 23 % as `p_b` crosses `p*`,
/// which no back-pressure sweep may do. The unchoked side is now
/// Bernoulli to 1e-4 (tests above). The choked side is low because
/// `get_critical_pressure_and_mass_flux_subcooled_liquid_ph` still forms its
/// energy objective through IF97 backward equations, the defect this issue
/// fixed in the unchoked branch. Fixing it there re-measures the Zaloudek,
/// Moody and Marviken V&V, so it is gh:#1111 and not done here.
///
/// **Diagnosed 2026-11-02 (gh:#1111).** More precisely, `s0` is the entropy
/// at `T0 = T_ph(p0, h0)`, but the drop uses the given `h0`, which is 93 J/kg
/// below `h_1(T0, p0)` here. With `h_1(T0, p0)` substituted, this test passes
/// as written (rel = +4.8e-7, monotone). ~~That patch is **not applied**,
/// because it moves the Moody 0.50 isobar point h/h_ref = 2.2549 from err
/// 0.005 to 0.120 against its 0.08 tolerance.~~ **Applied 2026-11-02** in the
/// forward-Newton form, with the branch switches replaced. The Moody point is
/// a named known miss. See
/// `verification_and_validation/hem_pipe_vs_exact_riemann_and_edwards.md`
/// §10–§12.
#[test]
fn gh1102_choked_transition_is_continuous_for_cold_liquid() {
    let (g_above, g_below, dh, monotone) = choked_transition(2.31e5, 300.0);
    let rel = g_above / g_below - 1.0;
    let bound = 4.18e3 * 0.025 / dh;
    eprintln!("300 K: G(p*+) = {g_above}, G*(p*-) = {g_below}, rel = {rel:e}, bound = {bound:e}");
    assert!(rel.abs() <= bound, "rel = {rel:e} > {bound:e}");
    assert!(monotone, "G(p_b) decreases somewhere as p_b falls");
}

