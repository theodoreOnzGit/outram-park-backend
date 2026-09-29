//! # The HTR-10 reactor building (vented confinement) as one lumped control volume
//!
//! Added 2026-09-29 (gh:#400, source-term stage 2 of gh:#398): the first real
//! code in BISHAN, at the maintainer's direction in the source-term plan.
//!
//! ## What the building is, from the published design
//!
//! HTR-10 has a **vented confinement**, not a pressurised containment
//! (Jiang, Ye & Yang 2002, *Nucl. Eng. Des.* 218, 209-214; proprietary, cited
//! not redistributed):
//!
//! - five cavities (reactor, steam generator, fuel handling, operation gas
//!   valves, helium purification), the first four interconnected (section 2);
//! - a safety-class **negative-pressure exhaust** holds the cavities at
//!   **-150 Pa** in normal operation; "the allowable leakage rate of the
//!   confinement is **100% (of the volume) per day** for a negative pressure of
//!   150 Pa" (section 2); the exhaust "continuously takes out the leaking air"
//!   through prefilters, HEPA filters (and adsorbers on the accident train) to
//!   a **40 m chimney** (sections 3-4);
//! - above **10 kPa** gauge the rupture disks burst and the helium goes out of
//!   a **22.6 m chimney without filtering** (sections 2 and 4); maximum design
//!   pressure 30 kPa gauge.
//!
//! ## What this CV carries
//!
//! One well-mixed airborne inventory per nuclide, in **atoms**, with a source
//! from the primary circuit and three sinks:
//!
//! ```text
//! dB/dt = I - (lambda + k_deposition + k_exhaust) B
//! stack release rate = (1 - capture) k_exhaust B      [atoms/s]
//! ```
//!
//! stepped **exactly** for an inflow `I` held constant over the step (the same
//! closed form as the primary pools), so a step of any length conserves
//! atoms.
//!
//! ## The parameters and where each comes from
//!
//! | Parameter | Value | Source |
//! |---|---|---|
//! | exhaust turnover `k_exhaust` | 1 volume/day | Jiang et al. (2002) s.2: allowable in-leakage 100 %/day at -150 Pa, which the exhaust must remove to hold the negative pressure. **The actual exhaust flow is not published**; this is the design in-leakage the fan at least matches, so it is a lower bound on the turnover (a faster turnover gives less decay in the building, i.e. more release). |
//! | filter capture | 0 (no credit) | Liu & Cao (2002) sections 3.1 and 4.1.1: the filtering is "not taken into account in the conservative calculation". The published HTR-10 filter decontamination factors are not available; HEPA and adsorbers exist (Jiang s.4), so this over-states the release. |
//! | building deposition `k_deposition` | 0 (no credit) | Liu & Cao (2002) s.4.1.1: "the filtering and plate-out effects in the reactor building are not taken into account". |
//!
//! No volume is needed: every rate is per volume of the building.
//!
//! ## What is NOT modelled
//!
//! - The pressure of the building, the rupture-disk path and the 22.6 m
//!   chimney: a blowdown fast enough to exceed 10 kPa (source-term stage 4,
//!   gh:#402) needs them; normal leakage and a small break do not.
//! - Separate cavities; aerosol physics; resuspension.
//! - **Not dose.** BISHAN ends at what is released.

use uom::si::f64::{Frequency, Time};
use uom::si::frequency::hertz;
use uom::si::time::second;

/// Jiang et al. (2002) section 2: the confinement's allowable leakage,
/// **100 % of its volume per day** at -150 Pa, as a rate \[1/s\].
pub const HTR10_CONFINEMENT_TURNOVER_PER_DAY: f64 = 1.0;

/// The building's parameters. See the module doc for each value's source.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BuildingParameters {
    /// Exhaust turnover `k_exhaust`, volumes per second.
    pub exhaust_turnover: Frequency,
    /// Fraction of the exhausted activity the filters capture, in `[0, 1]`.
    pub filter_capture: f64,
    /// Deposition on building surfaces `k_deposition`.
    pub deposition: Frequency,
}

impl BuildingParameters {
    /// HTR-10's vented confinement on the published basis: turnover 1/day
    /// (Jiang et al. 2002), filters and deposition not credited (Liu & Cao
    /// 2002, as in their own release calculations).
    #[must_use]
    pub fn htr10() -> Self {
        Self {
            exhaust_turnover: Frequency::new::<hertz>(
                HTR10_CONFINEMENT_TURNOVER_PER_DAY / 86_400.0,
            ),
            filter_capture: 0.0,
            deposition: Frequency::new::<hertz>(0.0),
        }
    }
}

/// One nuclide's building state \[atoms\] and the cumulative amounts that left.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BuildingInventory {
    /// Airborne in the building.
    pub airborne: f64,
    /// Cumulative atoms released up the stack (undecayed count at release).
    pub released: f64,
    /// Cumulative atoms captured by the filters.
    pub filtered: f64,
    /// Cumulative atoms deposited on building surfaces.
    pub deposited: f64,
}

/// What one step moved \[atoms\] and the release rate at the step's end.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BuildingStep {
    /// Atoms that entered from the primary circuit.
    pub entered: f64,
    /// Atoms that decayed in the building.
    pub decayed: f64,
    /// Atoms released up the stack during the step.
    pub released: f64,
    /// Stack release rate at the end of the step \[atoms/s\].
    pub release_rate_end: f64,
}

/// Step one nuclide's building inventory by `dt` with the inflow `inflow`
/// \[atoms/s\] held constant, **exactly**. `decay` is the nuclide's decay
/// constant.
///
/// # Panics
///
/// If `dt` is negative, a rate is negative, or the filter capture is outside
/// `[0, 1]` -- caller errors.
#[must_use]
pub fn step(
    state: BuildingInventory,
    inflow: f64,
    decay: Frequency,
    parameters: BuildingParameters,
    dt: Time,
) -> (BuildingInventory, BuildingStep) {
    let dt = dt.get::<second>();
    let lambda = decay.get::<hertz>();
    let k_ex = parameters.exhaust_turnover.get::<hertz>();
    let k_dep = parameters.deposition.get::<hertz>();
    let capture = parameters.filter_capture;
    assert!(dt >= 0.0 && dt.is_finite(), "negative or non-finite step");
    assert!(
        lambda >= 0.0 && k_ex >= 0.0 && k_dep >= 0.0,
        "negative rate"
    );
    assert!(
        (0.0..=1.0).contains(&capture),
        "filter capture outside [0, 1]"
    );

    let beta = lambda + k_ex + k_dep;
    let b0 = state.airborne;
    let (b1, integral) = if beta * dt > 1.0e-12 {
        let e = (-beta * dt).exp();
        let b_eq = inflow / beta;
        (
            b0 * e + b_eq * (1.0 - e),
            b_eq * dt + (b0 - b_eq) * (1.0 - e) / beta,
        )
    } else {
        (b0 + (inflow - beta * b0) * dt, b0 * dt)
    };
    let exhausted = k_ex * integral;
    let released = (1.0 - capture) * exhausted;
    let filtered = capture * exhausted;
    let deposited = k_dep * integral;
    let decayed = lambda * integral;

    (
        BuildingInventory {
            airborne: b1,
            released: state.released + released,
            filtered: state.filtered + filtered,
            deposited: state.deposited + deposited,
        },
        BuildingStep {
            entered: inflow * dt,
            decayed,
            released,
            release_rate_end: (1.0 - capture) * k_ex * b1,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(v: f64) -> Frequency {
        Frequency::new::<hertz>(v)
    }

    /// **Atoms are conserved and the step is exact.**
    ///
    /// Methodology: I-131-like decay (1.0e-6 /s), the HTR-10 parameters but
    /// with a 30 % filter capture and a 1e-6 /s deposition so every sink is
    /// exercised; from a non-zero inventory, 3600 steps of 1 s against one of
    /// 3600 s. Require `entered = dB + released + filtered + deposited +
    /// decayed` to 1e-12 of the gross (entered + initial inventory -- the
    /// rounding floor of 3600 steps on 1e12 atoms), and the two paths to 1e-9.
    /// **Results (2026-09-29):** residual 8.4e-12 of `entered` = 1.5e-13 of
    /// the gross; many-vs-one 2.5e-13.
    #[test]
    fn atoms_are_conserved_and_small_steps_equal_one_big_step() {
        let mut p = BuildingParameters::htr10();
        p.filter_capture = 0.3;
        p.deposition = f(1.0e-6);
        let lam = f(1.0e-6);
        let start = BuildingInventory {
            airborne: 1.0e12,
            ..Default::default()
        };
        let inflow = 5.0e6;
        let (mut many, mut entered, mut decayed) = (start, 0.0, 0.0);
        for _ in 0..3600 {
            let (s, st) = step(many, inflow, lam, p, Time::new::<second>(1.0));
            many = s;
            entered += st.entered;
            decayed += st.decayed;
        }
        let (one, _) = step(start, inflow, lam, p, Time::new::<second>(3600.0));
        let residual = entered
            - (many.airborne - start.airborne)
            - many.released
            - many.filtered
            - many.deposited
            - decayed;
        let path = (many.airborne - one.airborne).abs() / one.airborne;
        println!(
            "building atom residual {:.3e} of entered; many-vs-one {path:.3e}",
            residual / entered
        );
        // Rounding scales with the largest term (the 1e12-atom inventory
        // stepped 3600 times), not with the inflow.
        let gross = entered + start.airborne;
        assert!((residual / gross).abs() < 1e-12);
        assert!(path < 1e-9);
    }

    /// **The HTR-10 building releases, at steady state, what enters it less
    /// what decays inside**, with the published turnover of one volume a day:
    /// `release / inflow = k_ex / (lambda + k_ex)`. For a stable species the
    /// building passes everything; for I-131 (8.02 d) it holds back
    /// `lambda / (lambda + k_ex)` = 11.1 %.
    #[test]
    fn the_steady_building_passes_inflow_less_decay() {
        let p = BuildingParameters::htr10();
        let k_ex = 1.0 / 86_400.0;
        for lam in [0.0, (2.0f64).ln() / (8.02 * 86_400.0)] {
            let (s, st) = step(
                BuildingInventory::default(),
                1.0e6,
                f(lam),
                p,
                Time::new::<second>(1.0e9),
            );
            let ratio = st.release_rate_end / 1.0e6;
            let expected = k_ex / (lam + k_ex);
            assert!((ratio - expected).abs() < 1e-9, "{ratio} vs {expected}");
            assert!(s.airborne > 0.0);
        }
    }
}
