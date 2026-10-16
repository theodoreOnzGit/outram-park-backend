//! Edwards–O'Brien (1970) pipe-blowdown case data, for the `HemPipe` run.
//!
//! **COPIED** (values unchanged) from `tests/edwards_blowdown.rs` at commit
//! 79b63cdbe (2026-10-16), the PIMPLE (`TampinesSteamArray`) Edwards case, so
//! the HEM run reads the same geometry, Hendrie profile and digitised GS-1
//! data without editing that file (another change is in flight there). If
//! one copy is corrected, correct the other. Provenance as recorded there:
//!
//! - Geometry, break and Hendrie (1973) non-isothermal initial temperature
//!   profile: Tomlinson & Aumiller, B-T-3271, Table 1 (the modified RELAP5-3D
//!   nodalisation of the Edwards & O'Brien experiment).
//! - GS-1 "Data" pressure: digitised from B-T-3271 Figure 3 (graphreader.com),
//!   subsampled every 0.02 s; `benchmark_digitised.csv`, series "Data".

/// Total pipe length \[m\] (13.44 ft).
pub const PIPE_LENGTH_M: f64 = 4.096;
/// Inside diameter \[m\] (2.88 in).
pub const PIPE_ID_M: f64 = 0.073;
/// Pipe flow area \[m²\], `π D²/4`.
pub const PIPE_AREA_M2: f64 = std::f64::consts::PI * PIPE_ID_M * PIPE_ID_M / 4.0;
/// Break flow area fraction (13 % glass-disc residue).
pub const BREAK_AREA_FRACTION: f64 = 0.87;
/// Rupture-disc opening time \[s\] (1.0 ms linear ramp).
pub const DISC_OPEN_TIME_S: f64 = 1.0e-3;
/// Initial pipe pressure \[Pa abs\], 1000 psig.
pub const P_INIT_PA: f64 = 7.0e6;
/// Containment back-pressure \[Pa abs\].
pub const P_AMBIENT_PA: f64 = 1.0e5;

/// ft → m.
pub const FT_TO_M: f64 = 0.3048;
/// Pa → psi.
pub const PA_TO_PSI: f64 = 1.0 / 6894.757_293;

/// Volume-centre position \[ft from the closed end\], RELAP volumes 1–24.
pub const NODE_CENTRE_FT: [f64; 24] = [
    0.260, 0.780, 1.300, 1.820, 2.385, 3.000, 3.615, 4.215, 4.820, 5.425, 6.025, 6.640, 7.255,
    7.820, 8.340, 8.940, 9.630, 10.320, 10.920, 11.440, 11.905, 12.370, 12.890, 13.295,
];
/// Hendrie initial temperature \[°F\] at those volumes.
pub const NODE_T_DEGF: [f64; 24] = [
    447.5, 448.4, 449.4, 450.3, 451.4, 452.5, 451.7, 450.8, 450.0, 449.2, 448.3, 447.5, 448.0,
    448.5, 448.9, 449.4, 450.0, 446.5, 443.4, 440.8, 438.4, 436.0, 437.0, 437.8,
];

/// GS-1 position \[ft from the closed end\] (nearest the break).
pub const GS1_POS_FT: f64 = 12.890;

/// Digitised Edwards experimental GS-1 pressure, `(t [s], p [psia])`.
pub const GS1_DATA_PSIA: [(f64, f64); 16] = [
    (0.000, 985.0),
    (0.020, 350.777),
    (0.040, 364.038),
    (0.060, 367.358),
    (0.080, 343.024),
    (0.100, 312.239),
    (0.120, 298.234),
    (0.140, 296.0),
    (0.160, 288.68),
    (0.180, 288.394),
    (0.200, 289.0),
    (0.220, 283.112),
    (0.240, 271.433),
    (0.260, 252.96),
    (0.280, 228.0),
    (0.300, 190.093),
];

/// °F → K.
pub fn degf_to_k(f: f64) -> f64 {
    (f - 32.0) * 5.0 / 9.0 + 273.15
}

/// Piecewise-linear interpolation, flat outside the table.
pub fn lerp(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    if x <= xs[0] {
        return ys[0];
    }
    if x >= xs[xs.len() - 1] {
        return ys[ys.len() - 1];
    }
    for i in 0..xs.len() - 1 {
        if x >= xs[i] && x <= xs[i + 1] {
            let w = (x - xs[i]) / (xs[i + 1] - xs[i]);
            return ys[i] * (1.0 - w) + ys[i + 1] * w;
        }
    }
    ys[ys.len() - 1]
}

/// Hendrie temperature \[K\] at axial position `x_m` \[m from the closed end\].
pub fn hendrie_temperature_k(x_m: f64) -> f64 {
    let xs: Vec<f64> = NODE_CENTRE_FT.iter().map(|f| f * FT_TO_M).collect();
    let ts: Vec<f64> = NODE_T_DEGF.iter().map(|&f| degf_to_k(f)).collect();
    lerp(&xs, &ts, x_m)
}

/// Break area fraction at time `t` \[s\]: linear 1 ms ramp to 0.87.
pub fn break_area_fraction(t: f64) -> f64 {
    (t / DISC_OPEN_TIME_S).clamp(0.0, 1.0) * BREAK_AREA_FRACTION
}
