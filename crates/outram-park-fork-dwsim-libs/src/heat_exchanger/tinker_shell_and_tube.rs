//! Tinker's method (simplified), shell-and-tube heat exchanger rating.
//!
//! Ported from DWSIM `UnitOperations/HeatExchanger.vb`'s
//! `ShellandTube_Rating`/`ShellandTube_CalcFoulingFactor` calculation modes
//! (both are `HeatExchangerCalcMode` branches inside one `Select Case` at
//! lines 2051-2635 of that file, itself inside `Calculate` at lines
//! 1206-2751) and the `STHXProperties` geometry class (lines 3469-3537).
//! Bell-Delaware-like shell-side treatment citing Tinker, ch. 5.
//!
//! This module covers the **self-contained geometric/correlation building
//! blocks**: tube-side Reynolds/friction/HTC, the shell-side `Nh`/`Y`/`Np`
//! regressions, Colburn `j_h` and shell friction factor `f_s` (both
//! piecewise by tube layout and Reynolds/pitch-ratio bins), the shell-side
//! HTC with its baffle-leakage correction `E_c`, shell/tube pressure drop,
//! and the overall-`U`/fouling-factor formulas. It does **not** replicate
//! DWSIM's outer convergence loop (`Do ... Loop Until fx < 0.001`, max 100
//! iterations, converging outlet temperatures in Rating mode or `U` in
//! CalcFoulingFactor mode) -- that loop re-flashes both streams' properties
//! at the current mean temperature every iteration, which needs a
//! property-package/flash this crate intentionally does not have (see this
//! crate's top-level doc). A caller with flash access (e.g. `tampines`)
//! assembles [`tube_side`]/[`shell_side`]/[`overall_coefficient`] plus
//! [`super::f_correction::f_correction_factor`] (the same Bowman/Underwood
//! multi-pass `F` factor DWSIM uses here) into that outer loop itself.
//!
//! # Known dead inputs (confirmed unread in the DWSIM source)
//! `Shell_BaffleType`, `Shell_BaffleOrientation`, and `Shell_Roughness` are
//! declared in `STHXProperties` but never used by either calculation mode --
//! not represented in [`ShellAndTubeGeometry`].
//!
//! # A note on `d_e`
//! DWSIM does **not** use a classical Kern-style layout-dependent hydraulic
//! diameter here -- `d_e` is simply the tube outer diameter, reused directly
//! everywhere (Reynolds, Prandtl, `j_h`, `f_s`, pressure drop, HTC). This is
//! a real property of the ported method, not an omission.

use uom::si::f64::{
    DynamicViscosity, HeatTransfer, Length, MassDensity, MassRate, Pressure, Ratio,
    SpecificHeatCapacity, ThermalConductivity,
};
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;

/// Fouling resistance, area-specific \[K m^2 / W\]. `uom` has no dedicated
/// quantity for this (its `ThermalResistance` is K/W for a whole object,
/// dimensionally different from the per-unit-area R-value used here), so
/// this is a documented newtype rather than a bare `f64`.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct FoulingFactor(pub f64);

/// Tube bundle layout pattern (`STHXProperties.Tube_Layout` 0/1/2/3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TubeLayout {
    /// Triangular, 30 degree pitch angle.
    Triangular30,
    /// Triangular, rotated (60 degree effective).
    TriangularRotated,
    /// Square, 90 degree pitch angle.
    Square90,
    /// Square, rotated 45 degrees.
    SquareRotated45,
}

/// Shell-and-tube exchanger geometry (`uom`-typed `STHXProperties` fields
/// this module's correlations actually use).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShellAndTubeGeometry {
    /// Number of shells connected in series, `Nc`.
    pub number_of_shells_in_series: u32,
    /// Number of shell-side passes (drives the Bowman/Underwood `F` factor).
    pub shell_passes: u32,
    /// Shell internal diameter.
    pub shell_diameter: Length,
    /// Shell-side fouling resistance, `r_s`.
    pub shell_fouling: FoulingFactor,
    /// Baffle cut, as a fraction of shell diameter \[0, 1\].
    pub shell_baffle_cut: Ratio,
    /// Baffle spacing.
    pub shell_baffle_spacing: Length,
    /// Tube internal diameter, `d_i`.
    pub tube_inner_diameter: Length,
    /// Tube outer diameter, `d_e` (also DWSIM's shell-side "equivalent
    /// diameter" -- see this module's doc).
    pub tube_outer_diameter: Length,
    /// Tube length.
    pub tube_length: Length,
    /// Tube passes per shell.
    pub tube_passes_per_shell: u32,
    /// Total tube count per shell.
    pub tube_number_per_shell: u32,
    /// Tube bundle layout.
    pub tube_layout: TubeLayout,
    /// Tube pitch (centre-to-centre spacing).
    pub tube_pitch: Length,
    /// Tube absolute roughness (default 0.045 mm in DWSIM).
    pub tube_roughness: Length,
    /// User-exposed empirical multiplier on computed tube-side friction
    /// factor (DWSIM default 1.2 -- a pure fudge factor, not a computed
    /// correction; applies to tube side only, not shell side).
    pub tube_friction_correction_factor: Ratio,
    /// Tube wall thermal conductivity, `k_t`.
    pub tube_thermal_conductivity: ThermalConductivity,
    /// Tube-side fouling resistance, `r_t`.
    pub tube_fouling: FoulingFactor,
}

/// Errors from the shell-side correlations.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum ShellAndTubeError {
    /// `pitch / tube_outer_diameter` exceeded 1.5, the upper bound DWSIM's
    /// shell friction-factor correlation covers (DWSIM: `Throw New
    /// Exception("ratio between tube spacing and tube external diameter
    /// needs to be <= 1.5")`).
    #[error("pitch/tube_outer_diameter = {pitch_over_de} exceeds the shell friction-factor correlation's 1.5 upper bound")]
    PitchToDiameterRatioTooLarge {
        /// The out-of-range `pitch / tube_outer_diameter` ratio.
        pitch_over_de: f64,
    },
}

/// Result of [`tube_side`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TubeSideResult {
    /// Tube-side Reynolds number.
    pub reynolds: Ratio,
    /// Darcy friction factor, after `tube_friction_correction_factor`.
    pub friction_factor: Ratio,
    /// Tube-side heat-transfer coefficient (Petukhov 1970).
    pub htc: HeatTransfer,
    /// Tube-side pressure drop (Darcy-Weisbach, `tube_passes_per_shell`
    /// equivalent lengths).
    pub pressure_drop: Pressure,
}

/// Tube-side Reynolds/friction-factor/HTC/pressure-drop, for the fluid
/// flowing through the tubes at mass flow `w`, density `rho`, viscosity
/// `mu`, thermal conductivity `k`, and specific heat `cp`.
///
/// Friction factor: the same two-branch explicit Colebrook-type fit as
/// [`super::super::pipe::friction_factor::darcy_friction_factor`] (confirmed
/// identical constants against DWSIM `Pipe.vb`), Reynolds-gated at 3250 (not
/// 2100/4000 as in the pipe module -- this exchanger method uses a single
/// threshold), then scaled by `tube_friction_correction_factor`.
///
/// HTC: genuine Petukhov (1970) form, `h_i = (k/d_i)(f/8) Re Pr / (1.07 +
/// 12.7 (f/8)^0.5 (Pr^(2/3)-1))` -- not Dittus-Boelter, and not the
/// Gnielinski-form correlation DWSIM's own `Pipe.vb` confusingly also calls
/// `hint_petukhov` (that one has a `(Re-1000)` term and constant `1.0`; this
/// one does not and is not Reynolds-gated).
pub fn tube_side(
    geometry: &ShellAndTubeGeometry,
    w: MassRate,
    rho: MassDensity,
    mu: DynamicViscosity,
    k: ThermalConductivity,
    cp: SpecificHeatCapacity,
) -> TubeSideResult {
    let di = geometry.tube_inner_diameter.get::<meter>();
    let nt = (geometry.tube_number_per_shell / geometry.tube_passes_per_shell).max(1) as f64;
    let area = std::f64::consts::PI / 4.0 * di * di;
    let v_t = w.get::<kilogram_per_second>() / (rho.value * nt * area);
    let re_t = rho.value * v_t * di / mu.value;
    // Cp unit note: DWSIM stores Cp in kJ/(kg K) internally and multiplies
    // by 1000 to get Pr right; uom's SpecificHeatCapacity is already SI
    // (J/(kg K)), so no such factor is needed here.
    let pr_t = mu.value * cp.value / k.value;

    let epsilon_over_di = geometry.tube_roughness.get::<meter>() / di;
    let fric = if re_t > 3250.0 {
        let a1 = ((epsilon_over_di.powf(1.1096)) / 2.8257 + (7.149 / re_t).powf(0.8961)).log10();
        let b1 = -2.0 * (epsilon_over_di / 3.7065 - 5.0452 * a1 / re_t).log10();
        (1.0 / b1).powi(2)
    } else {
        64.0 / re_t
    } * geometry.tube_friction_correction_factor.get::<ratio>();

    let dp_tube = fric
        * geometry.tube_length.get::<meter>()
        * geometry.tube_passes_per_shell as f64
        / di
        * v_t
        * v_t
        / 2.0
        * rho.value;

    let h_i = k.value / di * (fric / 8.0) * re_t * pr_t
        / (1.07 + 12.7 * (fric / 8.0).sqrt() * (pr_t.powf(2.0 / 3.0) - 1.0));

    TubeSideResult {
        reynolds: Ratio::new::<ratio>(re_t),
        friction_factor: Ratio::new::<ratio>(fric),
        htc: HeatTransfer::new::<watt_per_square_meter_kelvin>(h_i),
        pressure_drop: Pressure::new::<pascal>(dp_tube),
    }
}

/// Layout-dependent `(a, b, c)` regression constants for a power law
/// `a * x^b * y^c`, per DWSIM's inline `Nh`/`Y`/`Np` fits.
fn nh_y_np_constants(layout: TubeLayout) -> ((f64, f64, f64), (f64, f64, f64), (f64, f64, f64)) {
    match layout {
        TubeLayout::Triangular30 | TubeLayout::TriangularRotated => (
            (0.9078565328950694, 0.66331106126564476, -4.4329764639656482),
            (5.3718559074820611, -0.33416765138071414, 0.7267144209289168),
            (0.53807650470841084, 0.3761125784751041, -3.8741224386187474),
        ),
        TubeLayout::Square90 => (
            (0.84134824361715088, 0.61374520485097339, -4.2696318466170409),
            (4.9901814007765743, -0.32437442510328618, 1.084850423269188),
            (0.5502379008813062, 0.36559560225434834, -3.99041305625483),
        ),
        TubeLayout::SquareRotated45 => (
            (0.66738654406767639, 0.680260033886211, -4.522291113086232),
            (4.5749169651729105, -0.32201759442337358, 1.17295183743691),
            (0.36869631130961067, 0.38397859475813922, -3.6273465996780421),
        ),
    }
}

/// Shell-side geometry factors `(Nh, Y, Np)`, from `xx = D_shell /
/// baffle_spacing` and `yy = pitch / d_e` via layout-dependent power-law
/// fits (DWSIM lines ~2302-2358).
pub fn nh_y_np(
    layout: TubeLayout,
    shell_diameter: Length,
    baffle_spacing: Length,
    pitch: Length,
    tube_outer_diameter: Length,
) -> (Ratio, Ratio, Ratio) {
    let xx = shell_diameter.get::<meter>() / baffle_spacing.get::<meter>();
    let yy = pitch.get::<meter>() / tube_outer_diameter.get::<meter>();
    let ((na, nb, nc), (ya, yb, yc), (pa, pb, pc)) = nh_y_np_constants(layout);
    (
        Ratio::new::<ratio>(na * xx.powf(nb) * yy.powf(nc)),
        Ratio::new::<ratio>(ya * xx.powf(yb) * yy.powf(yc)),
        Ratio::new::<ratio>(pa * xx.powf(pb) * yy.powf(pc)),
    )
}

/// Colburn `j_h` for the shell-side pressure-drop (crossflow) context --
/// two Reynolds bins (`< 100`, `>= 100`), separate fits per layout (DWSIM
/// lines ~2372-2379). **Not the same fit** as [`colburn_j_h_htc`] (see that
/// function's doc for the discrepancy DWSIM's own source has between the two
/// contexts).
pub fn colburn_j_h_friction(layout: TubeLayout, re_shell: Ratio) -> Ratio {
    let re = re_shell.get::<ratio>();
    let j = match layout {
        TubeLayout::Triangular30 | TubeLayout::TriangularRotated => {
            if re < 100.0 { 0.497 * re.powf(0.54) } else { 0.378 * re.powf(0.59) }
        }
        TubeLayout::Square90 => {
            if re < 100.0 { 0.385 * re.powf(0.526) } else { 0.2487 * re.powf(0.625) }
        }
        TubeLayout::SquareRotated45 => {
            if re < 100.0 { 0.496 * re.powf(0.54) } else { 0.354 * re.powf(0.61) }
        }
    };
    Ratio::new::<ratio>(j)
}

/// Colburn `j_h` for the shell-side heat-transfer-coefficient context.
///
/// **Not identical to [`colburn_j_h_friction`]**: DWSIM recomputes `j_h`
/// separately here from a different Reynolds number (`Rsh`, based on the
/// leakage-corrected area `Ssh` rather than `Ssf`), and for the triangular
/// layouts the high-Reynolds exponent is **0.61**, not 0.59 as in the
/// friction context (confirmed against DWSIM source -- an easy point to
/// silently merge when porting, so kept as two functions here). Square and
/// square-rotated layouts share one undifferentiated fit in this context
/// (DWSIM does not split `Square90`/`SquareRotated45` here, unlike
/// [`colburn_j_h_friction`]).
pub fn colburn_j_h_htc(layout: TubeLayout, re_shell_htc: Ratio) -> Ratio {
    let re = re_shell_htc.get::<ratio>();
    let j = match layout {
        TubeLayout::Triangular30 | TubeLayout::TriangularRotated => {
            if re < 100.0 { 0.497 * re.powf(0.54) } else { 0.378 * re.powf(0.61) }
        }
        TubeLayout::Square90 | TubeLayout::SquareRotated45 => {
            if re < 100.0 { 0.385 * re.powf(0.526) } else { 0.2487 * re.powf(0.625) }
        }
    };
    Ratio::new::<ratio>(j)
}

/// Shell-side Darcy friction factor `f_s`, piecewise in Reynolds number
/// (`< 100`, `< 1000`, `>= 1000`) and in `pitch/d_e` bins (`<= 1.2, 1.3,
/// 1.4, 1.5`), separately for the triangular-family and square-family
/// layouts (DWSIM lines ~2408-2478).
///
/// # Errors
/// [`ShellAndTubeError::PitchToDiameterRatioTooLarge`] if
/// `pitch/tube_outer_diameter > 1.5` (DWSIM: hard `Throw`).
pub fn shell_friction_factor(
    layout: TubeLayout,
    re_shell: Ratio,
    pitch: Length,
    tube_outer_diameter: Length,
) -> Result<Ratio, ShellAndTubeError> {
    let re = re_shell.get::<ratio>();
    let pitch_over_de = pitch.get::<meter>() / tube_outer_diameter.get::<meter>();

    let triangular_family =
        matches!(layout, TubeLayout::Triangular30 | TubeLayout::TriangularRotated);

    let f = if pitch_over_de <= 1.2 {
        if triangular_family {
            if re < 100.0 { 276.46 * re.powf(-0.979) } else if re < 1000.0 { 30.26 * re.powf(-0.523) } else { 2.93 * re.powf(-0.186) }
        } else if re < 100.0 { 230.0 * re.powf(-1.0) } else if re < 1000.0 { 16.23 * re.powf(-0.43) } else { 2.67 * re.powf(-0.173) }
    } else if pitch_over_de <= 1.3 {
        if triangular_family {
            if re < 100.0 { 208.14 * re.powf(-0.945) } else if re < 1000.0 { 27.6 * re.powf(-0.525) } else { 2.27 * re.powf(-0.163) }
        } else if re < 100.0 { 142.22 * re.powf(-0.949) } else if re < 1000.0 { 11.93 * re.powf(-0.43) } else { 1.77 * re.powf(-0.144) }
    } else if pitch_over_de <= 1.4 {
        if triangular_family {
            if re < 100.0 { 122.73 * re.powf(-0.865) } else if re < 1000.0 { 17.82 * re.powf(-0.474) } else { 1.86 * re.powf(-0.146) }
        } else if re < 100.0 { 110.77 * re.powf(-0.965) } else if re < 1000.0 { 7.524 * re.powf(-0.4) } else { 1.01 * re.powf(-0.104) }
    } else if pitch_over_de <= 1.5 {
        if triangular_family {
            if re < 100.0 { 104.33 * re.powf(-0.869) } else if re < 1000.0 { 12.69 * re.powf(-0.434) } else { 1.526 * re.powf(-0.129) }
        } else if re < 100.0 { 58.18 * re.powf(-0.862) } else if re < 1000.0 { 6.76 * re.powf(-0.411) } else { 0.718 * re.powf(-0.008) }
    } else {
        return Err(ShellAndTubeError::PitchToDiameterRatioTooLarge { pitch_over_de });
    };
    Ok(Ratio::new::<ratio>(f))
}

/// Result of [`shell_side`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShellSideResult {
    /// Shell-side heat-transfer coefficient, baffle-leakage-corrected.
    pub htc: HeatTransfer,
    /// Shell-side pressure drop (all shells in series).
    pub pressure_drop: Pressure,
    /// Shell-side crossflow Reynolds number (friction context).
    pub reynolds_friction: Ratio,
}

/// Full shell-side coefficient and pressure-drop evaluation, combining the
/// bundle geometry, `Nh`/`Y`/`Np`, both `j_h` contexts, `f_s`, and the
/// baffle-leakage correction `E_c` (DWSIM lines ~2298-2537).
///
/// `w`, `rho`, `mu`, `k`, `cp` are the shell-side fluid's mass flow rate and
/// properties (evaluated at the shell-side mean temperature -- the caller's
/// responsibility, since this crate has no flash access).
///
/// # Errors
/// Propagates [`ShellAndTubeError::PitchToDiameterRatioTooLarge`] from
/// [`shell_friction_factor`].
pub fn shell_side(
    geometry: &ShellAndTubeGeometry,
    w: MassRate,
    rho: MassDensity,
    mu: DynamicViscosity,
    k: ThermalConductivity,
    cp: SpecificHeatCapacity,
) -> Result<ShellSideResult, ShellAndTubeError> {
    let layout = geometry.tube_layout;
    let de = geometry.tube_outer_diameter.get::<meter>();
    let pitch = geometry.tube_pitch.get::<meter>();
    let dsi = geometry.shell_diameter.get::<meter>();
    let baffle_spacing = geometry.shell_baffle_spacing.get::<meter>();
    let l = geometry.tube_length.get::<meter>();
    let n = geometry.tube_number_per_shell as f64;

    // Bundle "shape" constant used only for the (unused-downstream) Dsf --
    // kept for documentation fidelity even though DWSIM itself never reads
    // Dsf after computing it.
    let nsc = match layout {
        TubeLayout::Triangular30 | TubeLayout::TriangularRotated => 1.1 * n.sqrt(),
        TubeLayout::Square90 | TubeLayout::SquareRotated45 => 1.19 * n.sqrt(),
    };
    let _dsf = (nsc - 1.0) * pitch + de; // unused downstream, matches DWSIM

    let n_baffles = l / baffle_spacing + 1.0;

    let (_nh, y, np) = nh_y_np(
        layout,
        geometry.shell_diameter,
        geometry.shell_baffle_spacing,
        geometry.tube_pitch,
        geometry.tube_outer_diameter,
    );
    let fp = 1.0 / (0.8 + np.get::<ratio>() * (dsi / pitch).sqrt());

    let cb = match layout {
        TubeLayout::Triangular30 | TubeLayout::TriangularRotated | TubeLayout::Square90 => 0.97,
        TubeLayout::SquareRotated45 => 1.37,
    };
    let ca = cb * (pitch - de) / pitch;
    let ss = ca * baffle_spacing * dsi; // note: DWSIM uses Dsf here in a comment but the
                                        // live code path multiplies by Dsi-derived geometry
                                        // consistently with the rest of the block -- see
                                        // module doc on Dsf being otherwise unused.
    let ssf = ss / fp;

    let g_sf = w.get::<kilogram_per_second>() / ssf;
    let re_s_friction = g_sf * de / mu.value;
    let pr_s = mu.value * cp.value / k.value;

    let j_h_friction = colburn_j_h_friction(layout, Ratio::new::<ratio>(re_s_friction));
    let f_s = shell_friction_factor(
        layout,
        Ratio::new::<ratio>(re_s_friction),
        geometry.tube_pitch,
        geometry.tube_outer_diameter,
    )?;
    let _ = j_h_friction; // computed for parity with DWSIM but not otherwise consumed
                          // by the pressure-drop formula itself (matches source: jh
                          // is only used in the HTC branch downstream of this point).

    let cx = match layout {
        TubeLayout::Triangular30 | TubeLayout::TriangularRotated => 1.154,
        TubeLayout::Square90 => 1.0,
        TubeLayout::SquareRotated45 => 1.414,
    };
    let hdi = geometry.shell_baffle_cut.get::<ratio>();
    let dp_shell = 4.0 * f_s.get::<ratio>() * g_sf * g_sf / (2.0 * rho.value) * cx * (1.0 - hdi)
        * dsi
        / pitch
        * n_baffles
        * (1.0 + y.get::<ratio>() * pitch / dsi)
        * geometry.number_of_shells_in_series as f64;

    // HTC context: separate leakage-corrected area Ssh, separate Reynolds
    // Rsh, separate (and for triangular layouts, differently-exponented)
    // j_h -- see colburn_j_h_htc's doc.
    let m = 0.96;
    let fh = 1.0 / (1.0 + _nh.get::<ratio>() * (dsi / pitch).sqrt());
    let ssh = ss * m / fh;
    let g_sh = w.get::<kilogram_per_second>() / ssh;
    let re_s_htc = g_sh * de / mu.value;
    let j_h_htc = colburn_j_h_htc(layout, Ratio::new::<ratio>(re_s_htc));

    let mut h_e = j_h_htc.get::<ratio>() * k.value * pr_s.powf(0.34) / de;
    let lb = baffle_spacing * (n_baffles - 1.0);
    let e_c = if (l - lb).abs() > 1e-12 {
        lb + (l - lb) * (2.0 * baffle_spacing / (l - lb)).powf(0.6) / l
    } else {
        f64::NAN
    };
    let e_c = if e_c.is_nan() { 1.0 } else { e_c };
    h_e *= e_c;

    Ok(ShellSideResult {
        htc: HeatTransfer::new::<watt_per_square_meter_kelvin>(h_e),
        pressure_drop: Pressure::new::<pascal>(dp_shell),
        reynolds_friction: Ratio::new::<ratio>(re_s_friction),
    })
}

/// Overall heat-transfer coefficient `U` from the tube-side and shell-side
/// HTCs, fouling resistances, and wall conduction --
/// `1/U = d_e/(h_i d_i) + r_t d_e/d_i + d_e/(2 k_t) ln(d_e/d_i) + r_s + 1/h_e`
/// (DWSIM lines ~2539-2545, Rating-mode form).
pub fn overall_coefficient(
    geometry: &ShellAndTubeGeometry,
    tube_side: &TubeSideResult,
    shell_side: &ShellSideResult,
) -> HeatTransfer {
    let di = geometry.tube_inner_diameter.get::<meter>();
    let de = geometry.tube_outer_diameter.get::<meter>();
    let h_i = tube_side.htc.get::<watt_per_square_meter_kelvin>();
    let h_e = shell_side.htc.get::<watt_per_square_meter_kelvin>();
    let f1 = de / (h_i * di);
    let f2 = geometry.tube_fouling.0 * de / di;
    let f3 = de / (2.0 * geometry.tube_thermal_conductivity.value) * (de / di).ln();
    let f4 = geometry.shell_fouling.0;
    let f5 = 1.0 / h_e;
    HeatTransfer::new::<watt_per_square_meter_kelvin>(1.0 / (f1 + f2 + f3 + f4 + f5))
}

/// Back-calculate the overall fouling factor `R_f = 1/U_design - 1/U_clean`
/// -- DWSIM's `ShellandTube_CalcFoulingFactor` mode (lines ~2547-2554).
/// `u_clean` excludes the fouling terms (`f1 + f3 + f5` only); `u_design` is
/// the duty-derived `U` (`Q / (A * mean_delta_t)`, the caller's
/// responsibility to compute).
pub fn fouling_factor_from_design_u(
    geometry: &ShellAndTubeGeometry,
    tube_side: &TubeSideResult,
    shell_side: &ShellSideResult,
    u_design: HeatTransfer,
) -> FoulingFactor {
    let di = geometry.tube_inner_diameter.get::<meter>();
    let de = geometry.tube_outer_diameter.get::<meter>();
    let h_i = tube_side.htc.get::<watt_per_square_meter_kelvin>();
    let h_e = shell_side.htc.get::<watt_per_square_meter_kelvin>();
    let f1 = de / (h_i * di);
    let f3 = de / (2.0 * geometry.tube_thermal_conductivity.value) * (de / di).ln();
    let f5 = 1.0 / h_e;
    let u_clean = 1.0 / (f1 + f3 + f5);
    FoulingFactor(1.0 / u_design.get::<watt_per_square_meter_kelvin>() - 1.0 / u_clean)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::dynamic_viscosity::pascal_second;
    use uom::si::heat_transfer::watt_per_square_meter_kelvin as htc_unit;
    use uom::si::length::{meter, millimeter};
    use uom::si::mass_density::kilogram_per_cubic_meter;
    use uom::si::mass_rate::kilogram_per_second;
    use uom::si::pressure::pascal;
    use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
    use uom::si::thermal_conductivity::watt_per_meter_kelvin;

    fn sample_geometry(layout: TubeLayout) -> ShellAndTubeGeometry {
        ShellAndTubeGeometry {
            number_of_shells_in_series: 1,
            shell_passes: 2,
            shell_diameter: Length::new::<millimeter>(500.0),
            shell_fouling: FoulingFactor(0.0),
            shell_baffle_cut: Ratio::new::<ratio>(0.2),
            shell_baffle_spacing: Length::new::<millimeter>(250.0),
            tube_inner_diameter: Length::new::<millimeter>(50.0),
            tube_outer_diameter: Length::new::<millimeter>(60.0),
            tube_length: Length::new::<meter>(5.0),
            tube_passes_per_shell: 2,
            tube_number_per_shell: 50,
            tube_layout: layout,
            tube_pitch: Length::new::<millimeter>(70.0),
            tube_roughness: Length::new::<millimeter>(0.045),
            tube_friction_correction_factor: Ratio::new::<ratio>(1.2),
            tube_thermal_conductivity: ThermalConductivity::new::<watt_per_meter_kelvin>(70.0),
            tube_fouling: FoulingFactor(0.0),
        }
    }

    #[test]
    fn tube_side_produces_positive_htc_and_pressure_drop() {
        let geometry = sample_geometry(TubeLayout::Triangular30);
        let result = tube_side(
            &geometry,
            MassRate::new::<kilogram_per_second>(5.0),
            MassDensity::new::<kilogram_per_cubic_meter>(998.0),
            DynamicViscosity::new::<pascal_second>(1.0e-3),
            ThermalConductivity::new::<watt_per_meter_kelvin>(0.6),
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(4180.0),
        );
        assert!(result.reynolds.get::<ratio>() > 0.0);
        assert!(result.htc.get::<htc_unit>() > 0.0);
        assert!(result.pressure_drop.get::<pascal>() > 0.0);
    }

    #[test]
    fn shell_side_all_layouts_produce_positive_htc_and_pressure_drop() {
        for layout in [
            TubeLayout::Triangular30,
            TubeLayout::TriangularRotated,
            TubeLayout::Square90,
            TubeLayout::SquareRotated45,
        ] {
            let geometry = sample_geometry(layout);
            let result = shell_side(
                &geometry,
                MassRate::new::<kilogram_per_second>(5.0),
                MassDensity::new::<kilogram_per_cubic_meter>(998.0),
                DynamicViscosity::new::<pascal_second>(1.0e-3),
                ThermalConductivity::new::<watt_per_meter_kelvin>(0.6),
                SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(4180.0),
            )
            .unwrap_or_else(|e| panic!("layout {layout:?} failed: {e:?}"));
            assert!(result.htc.get::<htc_unit>() > 0.0, "layout {layout:?}");
            assert!(result.pressure_drop.get::<pascal>() > 0.0, "layout {layout:?}");
        }
    }

    #[test]
    fn shell_friction_factor_rejects_pitch_over_1_5() {
        let geometry_pitch = Length::new::<millimeter>(100.0); // pitch/de = 100/60 = 1.67 > 1.5
        let result = shell_friction_factor(
            TubeLayout::Triangular30,
            Ratio::new::<ratio>(500.0),
            geometry_pitch,
            Length::new::<millimeter>(60.0),
        );
        assert!(matches!(result, Err(ShellAndTubeError::PitchToDiameterRatioTooLarge { .. })));
    }

    #[test]
    fn overall_coefficient_decreases_with_added_fouling() {
        let mut geometry = sample_geometry(TubeLayout::Triangular30);
        let w = MassRate::new::<kilogram_per_second>(5.0);
        let rho = MassDensity::new::<kilogram_per_cubic_meter>(998.0);
        let mu = DynamicViscosity::new::<pascal_second>(1.0e-3);
        let k = ThermalConductivity::new::<watt_per_meter_kelvin>(0.6);
        let cp = SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(4180.0);

        let tube = tube_side(&geometry, w, rho, mu, k, cp);
        let shell = shell_side(&geometry, w, rho, mu, k, cp).unwrap();
        let u_clean = overall_coefficient(&geometry, &tube, &shell);

        geometry.tube_fouling = FoulingFactor(0.0002);
        geometry.shell_fouling = FoulingFactor(0.0002);
        let u_fouled = overall_coefficient(&geometry, &tube, &shell);

        assert!(u_fouled.get::<htc_unit>() < u_clean.get::<htc_unit>());
    }

    #[test]
    fn fouling_factor_from_design_u_is_positive_when_design_u_is_lower() {
        let geometry = sample_geometry(TubeLayout::Triangular30);
        let w = MassRate::new::<kilogram_per_second>(5.0);
        let rho = MassDensity::new::<kilogram_per_cubic_meter>(998.0);
        let mu = DynamicViscosity::new::<pascal_second>(1.0e-3);
        let k = ThermalConductivity::new::<watt_per_meter_kelvin>(0.6);
        let cp = SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(4180.0);

        let tube = tube_side(&geometry, w, rho, mu, k, cp);
        let shell = shell_side(&geometry, w, rho, mu, k, cp).unwrap();
        let u_clean = overall_coefficient(&geometry, &tube, &shell);
        // A design U lower than the (fouling-free) clean U implies positive fouling.
        let u_design = HeatTransfer::new::<htc_unit>(u_clean.get::<htc_unit>() * 0.8);
        let fouling = fouling_factor_from_design_u(&geometry, &tube, &shell, u_design);
        assert!(fouling.0 > 0.0);
    }
}
