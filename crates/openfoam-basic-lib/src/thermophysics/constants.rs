/// Universal gas constant in J/(mol·K).
/// Using this value with `MolarMass` in kg/mol gives `r = R_UNIVERSAL / W` in J/(kg·K).
pub const R_UNIVERSAL: f64 = 8.314_462_618_153_24;

/// Standard thermodynamic temperature (used as entropy reference in S = Cp·ln(T/Tstd)).
pub const T_STD: f64 = 298.15; // K

/// Minimum temperature floor used in Newton T-iteration to prevent log(0).
pub const T_MIN: f64 = 100.0; // K

/// Upper JANAF coefficient range limit.
pub const T_MAX: f64 = 6000.0; // K

/// Standard-state reference pressure for entropy calculations.
pub const P_REF: f64 = 101_325.0; // Pa
