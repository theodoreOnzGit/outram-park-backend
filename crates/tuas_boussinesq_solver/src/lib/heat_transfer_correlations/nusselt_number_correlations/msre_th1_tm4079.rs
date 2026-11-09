//! MSRE TH-1 (gh:#1134, epic gh:#1125): TUAS's default pipe Nusselt
//! correlation against measured forced-convection heat transfer of a molten
//! fluoride salt, ORNL-TM-4079. Validation; research, education and V&V only.
//!
//! # Reference (read from the scan, 2026-11-09)
//!
//! J. W. Cooke and B. Cox, *Forced-Convection Heat-Transfer Measurements
//! with a Molten Fluoride Salt Mixture Flowing in a Smooth Tube*,
//! ORNL-TM-4079 (1973).
//!
//! - **Salt:** MSBR fuel salt LiF-BeF₂-ThF₄-UF₄ 67.5-20-12-0.5 mol %.
//! - **Test section:** Hastelloy N tube, 24.5 in. long, 0.25 in. OD, 0.035 in.
//!   wall (ID 0.18 in.), uniform heat flux by direct electrical heating;
//!   horizontal, with runs 201-208 vertical.
//! - **Data:** Table B-1, report pp. 39-41 (PDF 45-47), 70 runs. The rows below
//!   come from the MSRE repository's AI transcription, checked against the
//!   scan on every row for run, T_in, T_out, q/A, h, Re, Pr and Nu, with no
//!   discrepancies.
//! - **Properties:** Table D.2, report p. 58 (PDF 64): μ = 0.187 exp(8000/T[°R])
//!   lb/ft·hr (±25 %, estimated); k = 0.69 Btu/hr·ft·°F (±12 %, measured);
//!   c_p = 0.324 Btu/lb·°F (±4 %).
//! - **Report p. 18:** laminar h are length-integrated means; transition and
//!   turbulent h are the limiting (developed) values.
//!
//! # Methodology (pre-registered on gh:#1134, 2026-11-09)
//!
//! - **Model:** TUAS's pipe default, `PipeGnielinskiGeneric`, i.e.
//!   `gnielinski_correlation_interpolated_uniform_heat_flux_liquids_developing_bulk_fluid_prandtl`.
//! - **Inputs per run:**
//!   - Re and Pr as tabulated; smooth-tube Darcy f from `darcy(Re, 0)`;
//!   - L/D = 24.5/0.18 = 136.1, assuming the heated length is the tube length;
//!   - Pr_w = Pr · μ(T_w)/μ(T_b), with T_b = (T_in + T_out)/2 and
//!     T_w = T_b + (q/A)/h. This holds because c_p and k are constant in
//!     Table D.2.
//! - **Reported:** the residual TUAS/measured − 1 by regime (laminar Re < 2300,
//!   transition 2300-12,000, turbulent > 12,000), alongside Dittus-Boelter
//!   and two explicit ablations of the default:
//!   - without the entrance factor, for Re > 4000;
//!   - without the Pr-ratio correction.
//! - No constant is adjusted, and nothing is gated on the residual.
//!
//! # Results (2026-11-09, develop 6c973ec4f + this change, Rust 1.99.1)
//!
//! Residual TUAS/measured − 1, 70 runs, none excluded:
//!
//! | Regime | n | TUAS default: mean (range) | Dittus-Boelter mean | No entrance factor | No Pr-ratio |
//! |---|---|---|---|---|---|
//! | Laminar, Re < 2300 | 7 | +2.3 % (−5.3 to +14.5) | +41.0 % | — | +2.3 % |
//! | Transition, 2300-12,000 | 41 | **+24.4 %** (−4.6 to +48.8) | +28.8 % | +18.0 % (n = 37, Re > 4000) | +21.3 % |
//! | Turbulent, > 12,000 | 22 | **+21.8 %** (+11.0 to +43.1) | +10.1 % | +17.3 % | +20.1 % |
//!
//! **Against the predictions (gh:#1134):**
//! - Laminar: predicted within ±10 %; measured +2.3 %. As predicted.
//! - Transition: predicted +25 to +40 %; measured +24.4 %, just below the
//!   predicted range.
//! - Turbulent: predicted +15 to +25 %; measured +21.8 %. As predicted.
//!
//! **Interpretation.**
//! - Laminar: TUAS's uniform-flux developing correlation reproduces the
//!   data, mean absolute 5.6 %.
//! - Transition and turbulent: TUAS's default overpredicts this salt's heat
//!   transfer by about 22-24 %.
//!   - Neither term carries the excess. Removing the entrance factor leaves
//!     +17 to +18 %; removing the wall-Prandtl correction leaves +20 to
//!     +21 %. The excess is in the Gnielinski form itself, against these data.
//!   - The entrance factor applies a length-mean correction to data that
//!     are limiting (developed) values (report p. 18). For this
//!     comparison, the no-entrance-factor row is the like-for-like one.
//!   - Dittus-Boelter is closer in turbulent flow (+10.1 %).
//! - Where the residual is largest:
//!   - the low-transition runs, Re 3300-4500 at Pr about 16, up to +49 %;
//!   - low-heat-flux runs with poor heat balance (158: 0.89, 160: 0.93).
//!   - Vertical runs 201-208 sit closer: mean +10.3 %.
//! - The property uncertainties (μ ±25 % estimated, k ±12 %) are large
//!   enough to move Re, Pr and Nu by comparable amounts. This is a
//!   residual, not a calibration target. No constant was adjusted.

#[cfg(test)]
use petir::mathf::RealMath;

/// One run of TM-4079 Table B-1, as printed (US customary units).
#[cfg(test)]
#[derive(Clone, Copy, Debug)]
struct Tm4079Run {
    run: u32,
    t_in_f: f64,
    t_out_f: f64,
    /// q/A as printed, in units of 1e5 Btu/hr·ft².
    q_over_a_e5: f64,
    /// Mean h, Btu/hr·ft²·°F.
    h: f64,
    re: f64,
    pr: f64,
    nu: f64,
    vertical: bool,
}

/// TM-4079 Table B-1, all 70 runs.
#[cfg(test)]
const TM4079_TABLE_B1: [Tm4079Run; 70] = [
    Tm4079Run { run: 107, t_in_f: 1388.3, t_out_f: 1436.0, q_over_a_e5: 4.07, h: 4882.0, re: 15993.0, pr: 6.3, nu: 106.1, vertical: false },
    Tm4079Run { run: 115, t_in_f: 1362.7, t_out_f: 1415.8, q_over_a_e5: 2.48, h: 2831.0, re: 8345.0, pr: 6.6, nu: 61.5, vertical: false },
    Tm4079Run { run: 117, t_in_f: 1383.7, t_out_f: 1438.4, q_over_a_e5: 3.33, h: 3617.0, re: 11419.0, pr: 6.3, nu: 78.6, vertical: false },
    Tm4079Run { run: 119, t_in_f: 1379.1, t_out_f: 1436.6, q_over_a_e5: 2.30, h: 2302.0, re: 7495.0, pr: 6.3, nu: 50.0, vertical: false },
    Tm4079Run { run: 121, t_in_f: 1418.0, t_out_f: 1474.4, q_over_a_e5: 4.16, h: 4590.0, re: 14917.0, pr: 5.8, nu: 99.8, vertical: false },
    Tm4079Run { run: 122, t_in_f: 1456.2, t_out_f: 1507.9, q_over_a_e5: 5.17, h: 6192.0, re: 21647.0, pr: 5.5, nu: 134.6, vertical: false },
    Tm4079Run { run: 123, t_in_f: 1488.2, t_out_f: 1537.8, q_over_a_e5: 5.36, h: 6462.0, re: 25119.0, pr: 5.1, nu: 140.4, vertical: false },
    Tm4079Run { run: 127, t_in_f: 1097.9, t_out_f: 1156.4, q_over_a_e5: 3.23, h: 1936.0, re: 5005.0, pr: 13.1, nu: 42.1, vertical: false },
    Tm4079Run { run: 129, t_in_f: 1082.7, t_out_f: 1163.3, q_over_a_e5: 0.68, h: 396.0, re: 738.0, pr: 13.5, nu: 8.6, vertical: false },
    Tm4079Run { run: 130, t_in_f: 1081.5, t_out_f: 1177.1, q_over_a_e5: 0.90, h: 427.0, re: 839.0, pr: 13.3, nu: 9.2, vertical: false },
    Tm4079Run { run: 131, t_in_f: 1089.8, t_out_f: 1159.9, q_over_a_e5: 0.54, h: 407.0, re: 673.0, pr: 13.5, nu: 8.8, vertical: false },
    Tm4079Run { run: 132, t_in_f: 1090.6, t_out_f: 1160.2, q_over_a_e5: 0.60, h: 381.0, re: 760.0, pr: 13.4, nu: 8.3, vertical: false },
    Tm4079Run { run: 133, t_in_f: 1029.6, t_out_f: 1118.7, q_over_a_e5: 0.66, h: 357.0, re: 550.0, pr: 15.9, nu: 7.7, vertical: false },
    Tm4079Run { run: 134, t_in_f: 1036.3, t_out_f: 1134.8, q_over_a_e5: 0.52, h: 358.0, re: 405.0, pr: 15.3, nu: 7.8, vertical: false },
    Tm4079Run { run: 143, t_in_f: 1062.8, t_out_f: 1117.7, q_over_a_e5: 3.30, h: 1940.0, re: 4940.0, pr: 14.5, nu: 42.2, vertical: false },
    Tm4079Run { run: 144, t_in_f: 1075.4, t_out_f: 1135.7, q_over_a_e5: 3.87, h: 2200.0, re: 5523.0, pr: 13.8, nu: 47.8, vertical: false },
    Tm4079Run { run: 145, t_in_f: 1048.2, t_out_f: 1103.4, q_over_a_e5: 3.73, h: 2129.0, re: 5304.0, pr: 15.0, nu: 46.3, vertical: false },
    Tm4079Run { run: 146, t_in_f: 1064.0, t_out_f: 1115.9, q_over_a_e5: 3.85, h: 2513.0, re: 6026.0, pr: 14.6, nu: 54.7, vertical: false },
    Tm4079Run { run: 147, t_in_f: 1076.9, t_out_f: 1131.0, q_over_a_e5: 4.20, h: 2727.0, re: 6573.0, pr: 14.0, nu: 59.2, vertical: false },
    Tm4079Run { run: 148, t_in_f: 1093.5, t_out_f: 1148.3, q_over_a_e5: 4.63, h: 3068.0, re: 7583.0, pr: 13.2, nu: 66.7, vertical: false },
    Tm4079Run { run: 149, t_in_f: 1460.4, t_out_f: 1482.9, q_over_a_e5: 0.89, h: 2230.0, re: 8393.0, pr: 5.6, nu: 48.5, vertical: false },
    Tm4079Run { run: 150, t_in_f: 1460.6, t_out_f: 1489.5, q_over_a_e5: 1.66, h: 3434.0, re: 12260.0, pr: 5.5, nu: 74.7, vertical: false },
    Tm4079Run { run: 151, t_in_f: 1469.4, t_out_f: 1488.4, q_over_a_e5: 1.34, h: 3977.0, re: 15282.0, pr: 5.5, nu: 86.4, vertical: false },
    Tm4079Run { run: 152, t_in_f: 1477.0, t_out_f: 1501.8, q_over_a_e5: 2.05, h: 4573.0, re: 18300.0, pr: 5.4, nu: 99.5, vertical: false },
    Tm4079Run { run: 153, t_in_f: 1486.7, t_out_f: 1513.4, q_over_a_e5: 2.51, h: 5426.0, re: 21235.0, pr: 5.6, nu: 117.9, vertical: false },
    Tm4079Run { run: 154, t_in_f: 1496.1, t_out_f: 1523.3, q_over_a_e5: 2.80, h: 5740.0, re: 23719.0, pr: 5.1, nu: 124.8, vertical: false },
    Tm4079Run { run: 155, t_in_f: 1466.3, t_out_f: 1484.7, q_over_a_e5: 1.99, h: 5551.0, re: 23214.0, pr: 5.5, nu: 120.7, vertical: false },
    Tm4079Run { run: 156, t_in_f: 1475.0, t_out_f: 1488.2, q_over_a_e5: 1.45, h: 5229.0, re: 23861.0, pr: 5.5, nu: 113.7, vertical: false },
    Tm4079Run { run: 157, t_in_f: 1479.7, t_out_f: 1502.7, q_over_a_e5: 2.72, h: 6627.0, re: 26192.0, pr: 5.4, nu: 144.1, vertical: false },
    Tm4079Run { run: 158, t_in_f: 1466.3, t_out_f: 1483.8, q_over_a_e5: 0.76, h: 2236.0, re: 9284.0, pr: 5.5, nu: 48.6, vertical: false },
    Tm4079Run { run: 159, t_in_f: 1466.7, t_out_f: 1492.6, q_over_a_e5: 1.22, h: 2708.0, re: 10171.0, pr: 5.5, nu: 58.9, vertical: false },
    Tm4079Run { run: 160, t_in_f: 1471.3, t_out_f: 1492.2, q_over_a_e5: 1.06, h: 2741.0, re: 11107.0, pr: 5.5, nu: 59.6, vertical: false },
    Tm4079Run { run: 161, t_in_f: 1472.4, t_out_f: 1494.4, q_over_a_e5: 1.20, h: 3033.0, re: 11853.0, pr: 5.5, nu: 66.0, vertical: false },
    Tm4079Run { run: 162, t_in_f: 1463.7, t_out_f: 1522.0, q_over_a_e5: 2.46, h: 2675.0, re: 9464.0, pr: 5.2, nu: 58.2, vertical: false },
    Tm4079Run { run: 163, t_in_f: 1470.0, t_out_f: 1527.5, q_over_a_e5: 2.76, h: 3071.0, re: 10884.0, pr: 5.2, nu: 66.7, vertical: false },
    Tm4079Run { run: 164, t_in_f: 1480.0, t_out_f: 1535.8, q_over_a_e5: 2.85, h: 3334.0, re: 11770.0, pr: 5.1, nu: 72.5, vertical: false },
    Tm4079Run { run: 165, t_in_f: 1486.9, t_out_f: 1539.2, q_over_a_e5: 3.71, h: 4385.0, re: 16497.0, pr: 5.1, nu: 95.3, vertical: false },
    Tm4079Run { run: 166, t_in_f: 1501.6, t_out_f: 1546.2, q_over_a_e5: 4.21, h: 5852.0, re: 22512.0, pr: 5.0, nu: 127.2, vertical: false },
    Tm4079Run { run: 167, t_in_f: 1513.9, t_out_f: 1561.7, q_over_a_e5: 5.60, h: 6892.0, re: 28499.0, pr: 4.9, nu: 149.8, vertical: false },
    Tm4079Run { run: 170, t_in_f: 1064.6, t_out_f: 1080.1, q_over_a_e5: 0.94, h: 1737.0, re: 4524.0, pr: 15.9, nu: 37.7, vertical: false },
    Tm4079Run { run: 171, t_in_f: 1066.3, t_out_f: 1082.2, q_over_a_e5: 1.26, h: 2340.0, re: 5938.0, pr: 15.7, nu: 50.9, vertical: false },
    Tm4079Run { run: 172, t_in_f: 1069.8, t_out_f: 1084.0, q_over_a_e5: 1.31, h: 2905.0, re: 6939.0, pr: 15.6, nu: 63.2, vertical: false },
    Tm4079Run { run: 173, t_in_f: 1049.0, t_out_f: 1081.2, q_over_a_e5: 0.22, h: 320.0, re: 493.0, pr: 16.3, nu: 7.0, vertical: false },
    Tm4079Run { run: 186, t_in_f: 1061.6, t_out_f: 1076.2, q_over_a_e5: 0.79, h: 1445.0, re: 3986.0, pr: 16.0, nu: 31.4, vertical: false },
    Tm4079Run { run: 191, t_in_f: 1062.0, t_out_f: 1078.5, q_over_a_e5: 0.74, h: 1041.0, re: 3322.0, pr: 15.9, nu: 22.6, vertical: false },
    Tm4079Run { run: 192, t_in_f: 1064.6, t_out_f: 1080.3, q_over_a_e5: 0.77, h: 1337.0, re: 3690.0, pr: 15.7, nu: 29.0, vertical: false },
    Tm4079Run { run: 193, t_in_f: 1235.2, t_out_f: 1262.7, q_over_a_e5: 1.03, h: 1591.0, re: 4731.0, pr: 9.3, nu: 34.6, vertical: false },
    Tm4079Run { run: 195, t_in_f: 1247.0, t_out_f: 1270.6, q_over_a_e5: 1.86, h: 3560.0, re: 10200.0, pr: 9.1, nu: 77.4, vertical: false },
    Tm4079Run { run: 198, t_in_f: 1255.8, t_out_f: 1296.0, q_over_a_e5: 4.07, h: 4645.0, re: 13693.0, pr: 8.7, nu: 101.0, vertical: false },
    Tm4079Run { run: 199, t_in_f: 1273.2, t_out_f: 1294.4, q_over_a_e5: 2.30, h: 4752.0, re: 14944.0, pr: 8.6, nu: 103.3, vertical: false },
    Tm4079Run { run: 200, t_in_f: 1279.6, t_out_f: 1303.7, q_over_a_e5: 2.76, h: 5050.0, re: 16088.0, pr: 8.4, nu: 109.8, vertical: false },
    Tm4079Run { run: 201, t_in_f: 1237.8, t_out_f: 1263.8, q_over_a_e5: 1.18, h: 2406.0, re: 5892.4, pr: 9.14, nu: 52.3, vertical: true },
    Tm4079Run { run: 202, t_in_f: 1250.7, t_out_f: 1272.6, q_over_a_e5: 1.27, h: 2762.0, re: 7660.9, pr: 8.94, nu: 60.0, vertical: true },
    Tm4079Run { run: 203, t_in_f: 1252.2, t_out_f: 1276.2, q_over_a_e5: 1.66, h: 4029.0, re: 9200.0, pr: 8.89, nu: 87.6, vertical: true },
    Tm4079Run { run: 204, t_in_f: 1258.4, t_out_f: 1282.4, q_over_a_e5: 1.86, h: 3625.0, re: 10435.5, pr: 8.79, nu: 78.8, vertical: true },
    Tm4079Run { run: 205, t_in_f: 1229.9, t_out_f: 1255.0, q_over_a_e5: 1.18, h: 2119.0, re: 5910.9, pr: 9.41, nu: 46.1, vertical: true },
    Tm4079Run { run: 206, t_in_f: 1231.7, t_out_f: 1255.4, q_over_a_e5: 1.36, h: 3004.0, re: 7238.9, pr: 9.39, nu: 65.3, vertical: true },
    Tm4079Run { run: 207, t_in_f: 1243.6, t_out_f: 1268.1, q_over_a_e5: 1.68, h: 3122.0, re: 8947.4, pr: 9.08, nu: 67.9, vertical: true },
    Tm4079Run { run: 208, t_in_f: 1247.2, t_out_f: 1270.6, q_over_a_e5: 1.84, h: 4337.0, re: 10359.7, pr: 9.00, nu: 94.2, vertical: true },
    Tm4079Run { run: 210, t_in_f: 1132.4, t_out_f: 1156.1, q_over_a_e5: 1.69, h: 2416.0, re: 6512.0, pr: 12.5, nu: 52.5, vertical: false },
    Tm4079Run { run: 211, t_in_f: 1168.4, t_out_f: 1215.4, q_over_a_e5: 3.38, h: 3049.0, re: 7696.0, pr: 10.8, nu: 66.3, vertical: false },
    Tm4079Run { run: 212, t_in_f: 1203.4, t_out_f: 1226.7, q_over_a_e5: 1.69, h: 2823.0, re: 8230.0, pr: 10.2, nu: 61.4, vertical: false },
    Tm4079Run { run: 213, t_in_f: 1138.1, t_out_f: 1168.7, q_over_a_e5: 2.76, h: 3477.0, re: 8566.0, pr: 12.2, nu: 75.6, vertical: false },
    Tm4079Run { run: 214, t_in_f: 1097.9, t_out_f: 1149.6, q_over_a_e5: 4.92, h: 3573.0, re: 8297.0, pr: 13.3, nu: 77.7, vertical: false },
    Tm4079Run { run: 215, t_in_f: 1248.1, t_out_f: 1256.1, q_over_a_e5: 0.79, h: 4335.0, re: 12374.0, pr: 9.2, nu: 94.2, vertical: false },
    Tm4079Run { run: 216, t_in_f: 1258.3, t_out_f: 1276.2, q_over_a_e5: 1.77, h: 4490.0, re: 12957.0, pr: 8.8, nu: 97.6, vertical: false },
    Tm4079Run { run: 217, t_in_f: 1280.6, t_out_f: 1310.6, q_over_a_e5: 2.85, h: 4616.0, re: 13454.0, pr: 8.2, nu: 100.3, vertical: false },
    Tm4079Run { run: 218, t_in_f: 1288.1, t_out_f: 1341.7, q_over_a_e5: 5.03, h: 4915.0, re: 14035.0, pr: 7.7, nu: 106.9, vertical: false },
    Tm4079Run { run: 219, t_in_f: 1293.8, t_out_f: 1383.2, q_over_a_e5: 5.04, h: 2999.0, re: 8620.0, pr: 7.3, nu: 65.2, vertical: false },
    Tm4079Run { run: 220, t_in_f: 1118.5, t_out_f: 1155.7, q_over_a_e5: 1.51, h: 1082.0, re: 3554.0, pr: 12.8, nu: 23.5, vertical: false },
];

/// Heated length over inside diameter, 24.5 in. / 0.18 in.
#[cfg(test)]
const L_OVER_D: f64 = 24.5 / 0.18;

/// TM-4079 Table D.2 viscosity, lb/ft·hr, at a temperature in °F.
#[cfg(test)]
fn tm4079_viscosity(t_f: f64) -> f64 {
    0.187 * (8000.0 / (t_f + 459.67)).r_exp()
}

/// Wall Prandtl number of a run: Pr · μ(T_w)/μ(T_b).
#[cfg(test)]
fn wall_prandtl(run: &Tm4079Run) -> f64 {
    let t_b = 0.5 * (run.t_in_f + run.t_out_f);
    let t_w = t_b + run.q_over_a_e5 * 1.0e5 / run.h;
    run.pr * tm4079_viscosity(t_w) / tm4079_viscosity(t_b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fluid_mechanics_correlations::darcy;
    use crate::heat_transfer_correlations::nusselt_number_correlations::pipe_correlations;

    /// Regime of a run, by TM-4079's own boundaries.
    fn regime(re: f64) -> usize {
        if re < 2300.0 {
            0
        } else if re <= 12_000.0 {
            1
        } else {
            2
        }
    }

    /// Count, mean, mean absolute, min and max of a set of residuals.
    fn stats(r: &[f64]) -> (usize, f64, f64, f64, f64) {
        let n = r.len();
        let mean = r.iter().sum::<f64>() / n as f64;
        let mean_abs = r.iter().map(|x| x.abs()).sum::<f64>() / n as f64;
        let min = r.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = r.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        (n, mean, mean_abs, min, max)
    }

    /// TH-1: the residual of TUAS's default pipe correlation against the 70
    /// TM-4079 runs, by regime, with Dittus-Boelter and the two ablations.
    /// Reported, not gated; see the module doc and gh:#1134.
    #[test]
    fn th1_tuas_default_against_tm4079_by_regime() {
        let names = ["laminar (Re < 2300)", "transition (2300-12000)", "turbulent (> 12000)"];
        // [regime][model]: model 0 TUAS default, 1 Dittus-Boelter,
        // 2 default without the entrance factor (Re > 4000),
        // 3 default without the Pr-ratio correction.
        let mut res: [[Vec<f64>; 4]; 3] = Default::default();
        println!("TH-1: run, V, Re, Pr, Pr_w, Nu_meas, TUAS, TUAS/meas-1 [%], DB/meas-1 [%]");
        for run in TM4079_TABLE_B1.iter() {
            let pr_w = wall_prandtl(run);
            let f = darcy(run.re, 0.0).unwrap();
            let tuas = pipe_correlations::gnielinski_correlation_interpolated_uniform_heat_flux_liquids_developing_bulk_fluid_prandtl(
                run.re, run.pr, pr_w, f, L_OVER_D,
            );
            let no_pr_ratio = pipe_correlations::gnielinski_correlation_interpolated_uniform_heat_flux_liquids_developing_bulk_fluid_prandtl(
                run.re, run.pr, run.pr, f, L_OVER_D,
            );
            let db = pipe_correlations::dittus_boelter_correlation(run.re, run.pr, true);
            let g = regime(run.re);
            res[g][0].push(tuas / run.nu - 1.0);
            res[g][1].push(db / run.nu - 1.0);
            if run.re > 4000.0 {
                let developed = pipe_correlations::gnielinski_correlation_liquids_fully_developed(
                    run.re, run.pr, pr_w, f,
                );
                res[g][2].push(developed / run.nu - 1.0);
            }
            res[g][3].push(no_pr_ratio / run.nu - 1.0);
            println!(
                "TH-1: {:4} {} {:7.0} {:5.2} {:5.2} {:6.1} {:6.1} {:+7.1} {:+7.1}",
                run.run,
                if run.vertical { "V" } else { "H" },
                run.re,
                run.pr,
                pr_w,
                run.nu,
                tuas,
                100.0 * (tuas / run.nu - 1.0),
                100.0 * (db / run.nu - 1.0)
            );
            assert!(tuas.is_finite() && pr_w > 0.0);
        }
        let models = ["TUAS default", "Dittus-Boelter", "no entrance factor (Re>4000)", "no Pr-ratio"];
        for (g, name) in names.iter().enumerate() {
            for (m, model) in models.iter().enumerate() {
                if res[g][m].is_empty() {
                    continue;
                }
                let (n, mean, mean_abs, min, max) = stats(&res[g][m]);
                println!(
                    "TH-1 {name:24} {model:30} n = {n:2}: mean {:+6.1} %, mean |.| {:5.1} %, range {:+6.1} to {:+6.1} %",
                    100.0 * mean,
                    100.0 * mean_abs,
                    100.0 * min,
                    100.0 * max
                );
            }
        }
        assert_eq!(res.iter().map(|g| g[0].len()).sum::<usize>(), 70);
    }
}
