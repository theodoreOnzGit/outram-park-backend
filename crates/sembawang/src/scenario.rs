// SPDX-License-Identifier: GPL-3.0

//! The prescribed temperature transient.
//!
//! # This crate does not solve for a temperature history
//!
//! It consumes one. Nothing here is a thermal-hydraulic model, and the shape
//! of the transient is the single largest determinant of how much is released
//! — so a run's credibility is the credibility of whatever produced its
//! transient, not of this code.
//!
//! Two constructors, deliberately:
//!
//! - [`TemperatureTransient::from_ramp`] — an analytic ramp and hold. For
//!   shakedown, sensitivity sweeps and demonstrating the chain. Not a plant
//!   calculation.
//! - [`TemperatureTransient::from_nodes`] — an arbitrary per-node history, so a
//!   trace from `outram-park-digital-twin-engine`'s `htgr_sim_v1` headless mode
//!   (or any other source) drops in unchanged.
//!
//! The second exists now rather than later on purpose: adding it while the
//! shape of the type is still being decided costs nothing, and retrofitting it
//! after the API has consumers costs a lot.
//!
//! # Index order
//!
//! `temperatures[ring][time][axial]`, matching the layout `boon-lay`'s
//! code-to-code fixture uses (`sc.temps[r][t][k]`), so a caller moving between
//! the two does not have to transpose.

use uom::si::f64::{ThermodynamicTemperature, Time};
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::second;

use crate::error::{Error, Result};

/// A prescribed temperature history over the core nodes.
#[derive(Debug, Clone, PartialEq)]
pub struct TemperatureTransient {
    /// Sample times, ascending, from the start of the accident.
    pub times: Vec<Time>,
    /// `[ring][time][axial]`, degrees Celsius carried as
    /// [`ThermodynamicTemperature`].
    pub temperatures: Vec<Vec<Vec<ThermodynamicTemperature>>>,
    /// Number of radial rings.
    pub n_radial: usize,
    /// Number of axial nodes.
    pub n_axial: usize,
}

impl TemperatureTransient {
    /// From an arbitrary per-node history.
    ///
    /// This is the hook for a real thermal-hydraulic trace.
    ///
    /// # Errors
    /// [`Error::TransientTooShort`] if fewer than two samples;
    /// [`Error::TransientLengthMismatch`] if any ring's history is not one
    /// entry per time.
    ///
    /// # Panics
    /// Panics if `temperatures` is empty, if the rings disagree on `n_axial`,
    /// or if `times` is not strictly ascending.
    pub fn from_nodes(
        times: Vec<Time>,
        temperatures: Vec<Vec<Vec<ThermodynamicTemperature>>>,
    ) -> Result<Self> {
        if times.len() < 2 {
            return Err(Error::TransientTooShort(times.len()));
        }
        assert!(!temperatures.is_empty(), "need at least one radial ring");
        assert!(
            times.windows(2).all(|w| w[1].get::<second>() > w[0].get::<second>()),
            "transient times must be strictly ascending"
        );
        let n_radial = temperatures.len();
        let n_axial = temperatures[0]
            .first()
            .map(Vec::len)
            .expect("each ring needs at least one time sample");
        for ring in &temperatures {
            if ring.len() != times.len() {
                return Err(Error::TransientLengthMismatch {
                    times: times.len(),
                    temperatures: ring.len(),
                });
            }
            for slice in ring {
                assert_eq!(
                    slice.len(),
                    n_axial,
                    "every ring and time must carry the same number of axial nodes"
                );
            }
        }
        Ok(Self {
            times,
            temperatures,
            n_radial,
            n_axial,
        })
    }

    /// A linear ramp from `start` to `peak` over `ramp`, then a hold at `peak`
    /// to `total`, uniform across every node.
    ///
    /// **A demonstration shape, not a plant transient.** Every node sees the
    /// same history, so there is no radial or axial gradient and therefore no
    /// hot node in any meaningful sense — which is exactly the thing a real
    /// calculation turns on. Use it to exercise the chain, never to produce a
    /// number anyone acts on.
    ///
    /// The sample spacing is `total / (samples - 1)`.
    ///
    /// # Errors
    /// [`Error::TransientTooShort`] if `samples` is below 2.
    ///
    /// # Panics
    /// Panics if `ramp` is not positive or exceeds `total`, or if either node
    /// count is zero.
    pub fn from_ramp(
        start: ThermodynamicTemperature,
        peak: ThermodynamicTemperature,
        ramp: Time,
        total: Time,
        samples: usize,
        n_radial: usize,
        n_axial: usize,
    ) -> Result<Self> {
        if samples < 2 {
            return Err(Error::TransientTooShort(samples));
        }
        let ramp_s = ramp.get::<second>();
        let total_s = total.get::<second>();
        assert!(ramp_s > 0.0, "the ramp must have positive duration");
        assert!(
            ramp_s <= total_s,
            "the ramp ({ramp_s} s) cannot be longer than the transient ({total_s} s)"
        );
        assert!(n_radial > 0 && n_axial > 0, "need at least one node in each direction");

        let t0 = start.get::<degree_celsius>();
        let t1 = peak.get::<degree_celsius>();
        let dt = total_s / (samples - 1) as f64;

        let times: Vec<Time> = (0..samples).map(|i| Time::new::<second>(i as f64 * dt)).collect();
        let profile: Vec<ThermodynamicTemperature> = times
            .iter()
            .map(|t| {
                let s = t.get::<second>();
                let c = if s >= ramp_s { t1 } else { t0 + (t1 - t0) * s / ramp_s };
                ThermodynamicTemperature::new::<degree_celsius>(c)
            })
            .collect();

        let temperatures = vec![
            profile
                .iter()
                .map(|t| vec![*t; n_axial])
                .collect::<Vec<_>>();
            n_radial
        ];
        Self::from_nodes(times, temperatures)
    }

    /// Number of time samples.
    #[must_use]
    pub fn len(&self) -> usize {
        self.times.len()
    }

    /// Whether there are no samples. Never true for a constructed transient,
    /// which needs at least two.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.times.is_empty()
    }

    /// When the transient ends.
    #[must_use]
    pub fn end(&self) -> Time {
        self.times[self.times.len() - 1]
    }

    /// One node's history, `[time]`.
    ///
    /// # Panics
    /// Panics if either index is out of range.
    #[must_use]
    pub fn node_history(&self, ring: usize, axial: usize) -> Vec<ThermodynamicTemperature> {
        (0..self.times.len())
            .map(|t| self.temperatures[ring][t][axial])
            .collect()
    }

    /// Every node's history, flattened ring-major, as
    /// `mean_temperature_rate` wants it.
    #[must_use]
    pub fn all_node_histories(&self) -> Vec<Vec<ThermodynamicTemperature>> {
        let mut out = Vec::with_capacity(self.n_radial * self.n_axial);
        for r in 0..self.n_radial {
            for k in 0..self.n_axial {
                out.push(self.node_history(r, k));
            }
        }
        out
    }

    /// The history of the node taken as hottest: innermost ring, mid-height.
    ///
    /// **`n_axial / 2` is upstream's choice of hot node, reproduced.** It is a
    /// geometric guess, not a search — on a transient with a genuine axial peak
    /// elsewhere it picks the wrong node, and this crate keeps it only because
    /// deviating would make a comparison against upstream meaningless.
    #[must_use]
    pub fn hot_node_history(&self) -> Vec<ThermodynamicTemperature> {
        self.node_history(0, self.n_axial / 2)
    }

    /// The peak temperature anywhere in the transient, for checking it against
    /// the fitted range of the diffusion correlation.
    #[must_use]
    pub fn peak_celsius(&self) -> f64 {
        self.temperatures
            .iter()
            .flat_map(|ring| ring.iter())
            .flat_map(|slice| slice.iter())
            .map(|t| t.get::<degree_celsius>())
            .fold(f64::NEG_INFINITY, f64::max)
    }

    /// The minimum temperature anywhere in the transient.
    #[must_use]
    pub fn min_celsius(&self) -> f64 {
        self.temperatures
            .iter()
            .flat_map(|ring| ring.iter())
            .flat_map(|slice| slice.iter())
            .map(|t| t.get::<degree_celsius>())
            .fold(f64::INFINITY, f64::min)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(x: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<degree_celsius>(x)
    }

    #[test]
    fn a_ramp_reaches_its_peak_and_holds() {
        let t =
            TemperatureTransient::from_ramp(c(600.0), c(1600.0), Time::new::<second>(1000.0),
                Time::new::<second>(2000.0), 21, 2, 3)
                .unwrap();
        assert_eq!(t.len(), 21);
        assert_eq!(t.n_radial, 2);
        assert_eq!(t.n_axial, 3);
        assert!((t.min_celsius() - 600.0).abs() < 1e-9);
        assert!((t.peak_celsius() - 1600.0).abs() < 1e-9);
        // Half way up the ramp.
        let h = t.node_history(0, 0);
        assert!((h[5].get::<degree_celsius>() - 1100.0).abs() < 1e-9);
        // Held at the peak after the ramp.
        assert!((h[20].get::<degree_celsius>() - 1600.0).abs() < 1e-9);
    }

    #[test]
    fn the_hot_node_is_upstreams_geometric_guess_not_a_search() {
        // Put the real peak at axial 0, where upstream's n_axial/2 will miss it.
        let times = vec![Time::new::<second>(0.0), Time::new::<second>(100.0)];
        let temps = vec![vec![vec![c(2000.0), c(700.0), c(700.0)]; 2]];
        let t = TemperatureTransient::from_nodes(times, temps).unwrap();
        let hot = t.hot_node_history();
        assert!(
            (hot[0].get::<degree_celsius>() - 700.0).abs() < 1e-9,
            "upstream picks n_axial/2 = 1, which is NOT the hottest node here -- \
             this is the documented behaviour, reproduced"
        );
        assert!((t.peak_celsius() - 2000.0).abs() < 1e-9);
    }

    #[test]
    fn all_node_histories_is_ring_major_and_complete() {
        let t = TemperatureTransient::from_ramp(c(600.0), c(1200.0), Time::new::<second>(100.0),
            Time::new::<second>(200.0), 5, 3, 4).unwrap();
        let all = t.all_node_histories();
        assert_eq!(all.len(), 12);
        assert!(all.iter().all(|h| h.len() == 5));
    }

    #[test]
    fn a_one_sample_transient_is_rejected() {
        let e = TemperatureTransient::from_ramp(c(600.0), c(1200.0), Time::new::<second>(100.0),
            Time::new::<second>(200.0), 1, 1, 1).unwrap_err();
        assert!(matches!(e, Error::TransientTooShort(1)));
    }

    #[test]
    fn a_ring_with_the_wrong_number_of_time_slices_is_an_error() {
        let times = vec![
            Time::new::<second>(0.0),
            Time::new::<second>(100.0),
            Time::new::<second>(200.0),
        ];
        let temps = vec![vec![vec![c(700.0)]; 2]];
        let e = TemperatureTransient::from_nodes(times, temps).unwrap_err();
        assert!(matches!(
            e,
            Error::TransientLengthMismatch { times: 3, temperatures: 2 }
        ));
    }
}
