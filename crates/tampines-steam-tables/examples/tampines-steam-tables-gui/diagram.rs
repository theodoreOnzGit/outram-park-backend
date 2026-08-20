//! The four diagram types, and how a [`ThermoPoint`] projects onto each.
//!
//! # The four tabs
//!
//! | Tab | x | y | Issue #26 status |
//! |---|---|---|---|
//! | T-p | temperature, °C | pressure, bar | added at the maintainer's request |
//! | p-h | specific enthalpy, kJ/kg | pressure, bar | required |
//! | T-s | specific entropy, kJ/(kg K) | temperature, °C | listed as optional |
//! | h-s (Mollier) | specific entropy, kJ/(kg K) | specific enthalpy, kJ/kg | required |
//!
//! The issue asked for p-h and h-s, with T-s optional. The maintainer asked for
//! all four including T-p, so all four are here and all four have equal export
//! support.
//!
//! # The T-p diagram is not like the other three
//!
//! In T-p coordinates the two-phase region **collapses onto a line**: every
//! two-phase state at pressure `p` sits at `T = T_sat(p)`, whatever its quality.
//! So on that tab the saturated-liquid line, the saturated-vapour line and all
//! five quality lines are the *same curve* — the vapour-pressure curve. Drawing
//! five distinct "quality lines" there would be drawing five copies of one
//! curve and implying a structure that does not exist.
//!
//! That is why [`crate::layers::LayerId::availability_on`] disables the quality
//! lines on the T-p tab with an explicit reason rather than plotting them. It is
//! the same rule the reference-data layers follow: **say a layer does not apply,
//! never fill it with something that looks like data.**

use uom::si::available_energy::kilojoule_per_kilogram;
use uom::si::pressure::bar;
use uom::si::specific_heat_capacity::kilojoule_per_kilogram_kelvin;
use uom::si::thermodynamic_temperature::degree_celsius;

use crate::data::ThermoPoint;
use crate::figure::AxisScale;

/// Which thermodynamic diagram is being drawn.
///
/// An enum, dispatched by `match`, rather than a trait object — the set of
/// diagrams is closed and adding a fifth should be a compile error at every
/// site that handles them, per the workspace Rust design rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DiagramKind {
    /// Temperature-pressure: the phase diagram proper.
    TemperaturePressure,
    /// Pressure-enthalpy.
    PressureEnthalpy,
    /// Temperature-entropy.
    TemperatureEntropy,
    /// Enthalpy-entropy, the Mollier diagram.
    EnthalpyEntropy,
}

impl DiagramKind {
    /// All four, in tab order.
    pub const ALL: [DiagramKind; 4] = [
        DiagramKind::TemperaturePressure,
        DiagramKind::PressureEnthalpy,
        DiagramKind::TemperatureEntropy,
        DiagramKind::EnthalpyEntropy,
    ];

    /// Short tab label.
    pub fn tab_label(self) -> &'static str {
        match self {
            Self::TemperaturePressure => "T-p",
            Self::PressureEnthalpy => "p-h",
            Self::TemperatureEntropy => "T-s",
            Self::EnthalpyEntropy => "h-s (Mollier)",
        }
    }

    /// Full figure title.
    pub fn title(self) -> &'static str {
        match self {
            Self::TemperaturePressure => {
                "Water / steam temperature-pressure diagram (IAPWS-IF97, tampines-steam-tables)"
            }
            Self::PressureEnthalpy => {
                "Water / steam pressure-enthalpy diagram (IAPWS-IF97, tampines-steam-tables)"
            }
            Self::TemperatureEntropy => {
                "Water / steam temperature-entropy diagram (IAPWS-IF97, tampines-steam-tables)"
            }
            Self::EnthalpyEntropy => {
                "Water / steam Mollier h-s diagram (IAPWS-IF97, tampines-steam-tables)"
            }
        }
    }

    /// File-name stem used for exports, following the directory layout issue
    /// #26 suggests (`figures/property_validation/`).
    pub fn file_stem(self) -> &'static str {
        match self {
            Self::TemperaturePressure => "tp_validation_coverage",
            Self::PressureEnthalpy => "ph_validation_coverage",
            Self::TemperatureEntropy => "ts_validation_coverage",
            Self::EnthalpyEntropy => "mollier_validation_coverage",
        }
    }

    /// x-axis label including units.
    pub fn x_label(self) -> &'static str {
        match self {
            Self::TemperaturePressure => "Temperature T (\u{00B0}C)",
            Self::PressureEnthalpy => "Specific enthalpy h (kJ/kg)",
            Self::TemperatureEntropy | Self::EnthalpyEntropy => "Specific entropy s (kJ/(kg K))",
        }
    }

    /// y-axis label including units.
    pub fn y_label(self) -> &'static str {
        match self {
            Self::TemperaturePressure | Self::PressureEnthalpy => "Pressure p (bar)",
            Self::TemperatureEntropy => "Temperature T (\u{00B0}C)",
            Self::EnthalpyEntropy => "Specific enthalpy h (kJ/kg)",
        }
    }

    /// Whether the y axis is a pressure axis, and therefore a candidate for the
    /// log scale issue #26 asks for.
    pub fn y_is_pressure(self) -> bool {
        matches!(self, Self::TemperaturePressure | Self::PressureEnthalpy)
    }

    /// The y-axis scale this diagram defaults to.
    ///
    /// Pressure axes default to log: they span the triple point (0.0061 bar) to
    /// 1000 bar, five and a half decades, and a linear axis would flatten
    /// everything below about 100 bar onto the frame. The other two default to
    /// linear.
    pub fn default_y_scale(self) -> AxisScale {
        if self.y_is_pressure() {
            AxisScale::Log10
        } else {
            AxisScale::Linear
        }
    }

    /// Formats a hover-readout x-coordinate with its unit, e.g. `"h = 2432.10
    /// kJ/kg"`, using the same symbol/unit convention as [`DiagramKind::x_label`].
    pub fn x_hover(self, x: f64) -> String {
        match self {
            Self::TemperaturePressure => format!("T = {x:.2} \u{00B0}C"),
            Self::PressureEnthalpy => format!("h = {x:.2} kJ/kg"),
            Self::TemperatureEntropy | Self::EnthalpyEntropy => {
                format!("s = {x:.4} kJ/(kg\u{00B7}K)")
            }
        }
    }

    /// Formats a hover-readout y-coordinate with its unit, e.g. `"p = 12.345
    /// bar"`, using the same symbol/unit convention as [`DiagramKind::y_label`].
    ///
    /// `y` is in the *canvas's own space*: when `log` is true (the live
    /// canvas's log-pressure toggle), `y` is `log10(p / bar)` rather than `p`
    /// itself, and this converts it back to bar before formatting — the
    /// reader should never see a bare log10 value in a hover readout.
    pub fn y_hover(self, y: f64, log: bool) -> String {
        match self {
            Self::TemperaturePressure | Self::PressureEnthalpy => {
                let p_bar = if log { 10.0_f64.powf(y) } else { y };
                format!("p = {p_bar:.4} bar")
            }
            Self::TemperatureEntropy => format!("T = {y:.2} \u{00B0}C"),
            Self::EnthalpyEntropy => format!("h = {y:.2} kJ/kg"),
        }
    }

    /// Projects a state onto this diagram's axes, in the plot units named by
    /// [`DiagramKind::x_label`] and [`DiagramKind::y_label`].
    pub fn project(self, point: &ThermoPoint) -> [f64; 2] {
        let t_degc = point.temperature.get::<degree_celsius>();
        let p_bar = point.pressure.get::<bar>();
        let h_kj = point.specific_enthalpy.get::<kilojoule_per_kilogram>();
        let s_kj = point
            .specific_entropy
            .get::<kilojoule_per_kilogram_kelvin>();
        match self {
            Self::TemperaturePressure => [t_degc, p_bar],
            Self::PressureEnthalpy => [h_kj, p_bar],
            Self::TemperatureEntropy => [s_kj, t_degc],
            Self::EnthalpyEntropy => [s_kj, h_kj],
        }
    }
}

/// Checks that projection picks the coordinates each diagram claims to.
///
/// # Methodology
///
/// Builds one state with four deliberately distinct values — 10 bar, 200 °C,
/// 2 000 kJ/kg, 6 kJ/(kg K) — projects it onto all four diagrams, and asserts
/// each pair equals the axis pair named in the label strings. A transposed
/// projection is otherwise very hard to spot by eye on a dome-shaped plot,
/// because the dome looks plausible either way up.
///
/// # Result
///
/// Passes as of 2026-08-20: T-p gives (200, 10); p-h gives (2000, 10); T-s
/// gives (6, 200); h-s gives (6, 2000).
#[cfg(test)]
#[test]
fn each_diagram_projects_the_axes_it_advertises() {
    use uom::si::f64::*;
    let point = ThermoPoint::new(
        Pressure::new::<bar>(10.0),
        ThermodynamicTemperature::new::<degree_celsius>(200.0),
        AvailableEnergy::new::<kilojoule_per_kilogram>(2000.0),
        SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(6.0),
        None,
    );
    let close = |a: f64, b: f64| assert!((a - b).abs() < 1e-9, "{a} != {b}");
    let tp = DiagramKind::TemperaturePressure.project(&point);
    close(tp[0], 200.0);
    close(tp[1], 10.0);
    let ph = DiagramKind::PressureEnthalpy.project(&point);
    close(ph[0], 2000.0);
    close(ph[1], 10.0);
    let ts = DiagramKind::TemperatureEntropy.project(&point);
    close(ts[0], 6.0);
    close(ts[1], 200.0);
    let hs = DiagramKind::EnthalpyEntropy.project(&point);
    close(hs[0], 6.0);
    close(hs[1], 2000.0);
}

/// Checks that the hover-readout formatters name the right symbol/unit and,
/// on a pressure axis, correctly invert the live canvas's log10 transform
/// back to bar before printing it.
///
/// # Methodology
///
/// For each diagram, formats a distinct x and y value (chosen so a
/// mismatched axis pairing would be visible) and asserts the formatted
/// string starts with the expected symbol and ends with the expected unit.
/// For a pressure axis, additionally checks the non-log and `log=true` paths
/// agree on the same underlying pressure — `y_hover(1.0, true)` (i.e.
/// `log10(p/bar) = 1.0`) must report the same `10.0 bar` as
/// `y_hover(10.0, false)`.
///
/// # Result
///
/// Passes as of 2026-08-20 on all four diagrams.
#[cfg(test)]
#[test]
fn hover_formatters_name_the_right_symbol_and_invert_the_log_axis() {
    let tp_x = DiagramKind::TemperaturePressure.x_hover(123.4);
    assert!(
        tp_x.starts_with("T = ") && tp_x.ends_with("\u{00B0}C"),
        "{tp_x}"
    );
    let tp_y = DiagramKind::TemperaturePressure.y_hover(10.0, false);
    assert!(tp_y.starts_with("p = ") && tp_y.ends_with("bar"), "{tp_y}");

    let ph_x = DiagramKind::PressureEnthalpy.x_hover(2500.0);
    assert!(
        ph_x.starts_with("h = ") && ph_x.ends_with("kJ/kg"),
        "{ph_x}"
    );

    let ts_x = DiagramKind::TemperatureEntropy.x_hover(6.5);
    assert!(
        ts_x.starts_with("s = ") && ts_x.ends_with("kJ/(kg\u{00B7}K)"),
        "{ts_x}"
    );
    let ts_y = DiagramKind::TemperatureEntropy.y_hover(300.0, false);
    assert!(
        ts_y.starts_with("T = ") && ts_y.ends_with("\u{00B0}C"),
        "{ts_y}"
    );

    let hs_y = DiagramKind::EnthalpyEntropy.y_hover(2800.0, false);
    assert!(
        hs_y.starts_with("h = ") && hs_y.ends_with("kJ/kg"),
        "{hs_y}"
    );

    // Log-axis inversion: log10(10.0) = 1.0, so y_hover(1.0, true) must read
    // back the same 10 bar as y_hover(10.0, false).
    let logged = DiagramKind::PressureEnthalpy.y_hover(1.0, true);
    let linear = DiagramKind::PressureEnthalpy.y_hover(10.0, false);
    assert_eq!(logged, linear, "log10(p/bar)=1.0 must read back as 10 bar");
}
