//! Per-nuclide cross-section data source for the plot screen (op-1lj, op-dzl).
//!
//! # Data path — why the embedded CORE set, not a tape reconstruction
//!
//! `njoy-outram-park-fork` can reconstruct pointwise cross sections from a raw
//! ENDF tape via [`njoy_outram_park_fork::interface::NuclearDataLibrary`]
//! (`from_file` -> `reconstruct` -> `broaden`), but that path reads a tape
//! file from disk and runs RECONR's adaptive grid refinement — not something
//! to redo on every keystroke of an interactive touchscreen session, and it
//! needs an ENDF tape the TUI has no bundled copy of. Per the task brief,
//! this TUI instead uses the crate's **representative in-crate data path**:
//! the embedded windowed-multipole CORE library
//! ([`WmpLibrary::core`], `njoy-outram-park-fork/src/data/wmp_core.wmpl`,
//! 125 reactor-grade + LFTR nuclides, ENDF/B-VII.1) for the resonance/thermal
//! range, plus its fast-range MGXS companion ([`MgxsLibrary::core`]) above
//! each nuclide's WMP `e_max` up to 20 MeV. Both are `include_bytes!`-embedded
//! — no file I/O, no network, and WMP's analytic Doppler broadening
//! ([`WindowedMultipole::evaluate`]) is fast enough to resample on every
//! temperature change. This is a genuine trade: the MGXS fast range is a
//! coarse, non-self-shielded 10-group fallback (see [`Mgxs`] docs), not a
//! pointwise reconstruction — acceptable for a terminal-resolution plot, but
//! it is not the ENDF pointwise fidelity the full RECONR/BROADR path gives.
//!
//! `ν̄`/`χ` (fission secondary data) are intentionally left at their
//! `Default` (all-zero) here: the CORE set carries no ENDF MF=1/452 tape to
//! derive a real `ν̄(E)` from, so [`MtChannel`] does not expose a
//! ν·σ_f channel — showing an always-zero one would be actively misleading.

use njoy_outram_park_fork::endf::MtReaction;
use njoy_outram_park_fork::nuclear_data::{Mgxs, MgxsLibrary, MicroXs, XsProvider};
use njoy_outram_park_fork::wmp::{WindowedMultipole, WmpLibrary};

/// The four base reaction channels this TUI plots, mapped onto
/// [`MicroXs`]'s fields. JANIS-style MT numbers are attached via
/// [`MtChannel::mt_reaction`] purely for the on-screen label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MtChannel {
    Total,
    Elastic,
    Fission,
    Capture,
}

impl MtChannel {
    /// All channels, in the order shown on the MT selector row.
    pub const ALL: [MtChannel; 4] = [
        MtChannel::Total,
        MtChannel::Elastic,
        MtChannel::Fission,
        MtChannel::Capture,
    ];

    /// Pull this channel's value \[barn\] out of a [`MicroXs`].
    pub fn value(self, m: &MicroXs) -> f64 {
        match self {
            MtChannel::Total => m.total,
            MtChannel::Elastic => m.elastic,
            MtChannel::Fission => m.fission,
            MtChannel::Capture => m.capture,
        }
    }

    /// Short button label, e.g. `"total"`.
    pub fn label(self) -> &'static str {
        match self {
            MtChannel::Total => "total",
            MtChannel::Elastic => "elastic",
            MtChannel::Fission => "fission",
            MtChannel::Capture => "capture",
        }
    }

    /// The standard ENDF MT this channel corresponds to, for the JANIS-style
    /// `"MT=102"` annotation on the plot title.
    pub fn mt_reaction(self) -> MtReaction {
        match self {
            MtChannel::Total => MtReaction::Mt1Total,
            MtChannel::Elastic => MtReaction::Mt2Elastic,
            MtChannel::Fission => MtReaction::Mt18Fission,
            MtChannel::Capture => MtReaction::Mt102Capture,
        }
    }
}

/// One nuclide's cross-section source: WMP resonance/thermal range plus its
/// fast-range MGXS fallback, combined into a single `micro(E, T)` call. See
/// the module docs for why this pair (not a tape reconstruction) is used.
#[derive(Debug, Clone)]
pub struct NuclideXs {
    name: String,
    wmp: WindowedMultipole,
    /// Fast-range (above `wmp.e_max`) multigroup fallback, if the CORE fast
    /// library has one for this nuclide (absent for a nuclide like H-1 whose
    /// WMP form already spans the full 20 MeV, so there is no fast gap).
    mgxs: Option<Mgxs>,
}

impl NuclideXs {
    /// Look up a nuclide by its WMP name (e.g. `"U238"`, from
    /// [`crate::nuclides::NuclideId::name`]) in the embedded CORE libraries.
    ///
    /// # Errors
    /// A human-readable message if the name is not in the embedded WMP CORE
    /// set (should not happen for a name the finder itself produced).
    pub fn load(name: &str) -> Result<Self, String> {
        let wmp = WmpLibrary::core().get(name).map_err(|e| e.to_string())?;
        let mgxs = MgxsLibrary::core().get(name).cloned();
        Ok(NuclideXs {
            name: name.to_string(),
            wmp,
            mgxs,
        })
    }

    /// This nuclide's WMP name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Lower bound of the plottable energy range \[eV\]. Floored just above
    /// zero so `log10` stays finite even for a WMP form whose `e_min` is
    /// (numerically) exactly `0`.
    pub fn e_min(&self) -> f64 {
        self.wmp.e_min.max(1.0e-5)
    }

    /// Upper bound of the plottable energy range \[eV\] — the fast-MGXS top
    /// bound (20 MeV) when a fast range exists, otherwise the WMP form's own
    /// `e_max`.
    pub fn e_max(&self) -> f64 {
        self.mgxs
            .as_ref()
            .and_then(|m| m.group_bounds.last().copied())
            .unwrap_or(self.wmp.e_max)
    }

    /// Microscopic cross sections at incident energy `e` \[eV\] and
    /// temperature `temp_k` \[K\].
    ///
    /// Dispatches by energy: below/at the WMP form's `e_max` this is the
    /// analytically Doppler-broadened multipole evaluation (temperature has a
    /// real, physical effect here); above it, the fast MGXS fallback, which
    /// is **temperature-independent by construction** (see the module docs).
    /// `e` is clamped into `[e_min, e_max]` so a caller sampling a fixed grid
    /// never has to special-case the endpoints.
    pub fn micro(&self, e: f64, temp_k: f64) -> MicroXs {
        let e = e.clamp(self.e_min(), self.e_max());
        if e > self.wmp.e_max {
            if let Some(mg) = &self.mgxs {
                return mg.micro(e);
            }
        }
        let provider = XsProvider::Multipole {
            wmp: self.wmp.clone(),
            nu: Default::default(),
            chi: Default::default(),
        };
        provider.micro(e, temp_k)
    }
}

/// One log-log-scaled point: `(log10(E / eV), log10(sigma / barn))`.
pub type LogPoint = (f64, f64);

/// Sample `mt`'s cross section for `nx` at temperature `temp_k` \[K\] over
/// `n_points` log-spaced energies spanning `nx.e_min()..=nx.e_max()`,
/// returning `(log10 E, log10 sigma)` pairs ready for a log-log [`ratatui`]
/// `Chart`.
///
/// # Sampling choice
/// This is a **fixed log-spaced grid**, not RECONR's adaptive
/// resonance-aware refinement — appropriate for a terminal plot whose
/// horizontal resolution (braille dot columns) tops out in the low hundreds,
/// but it can visually under-resolve a very narrow resonance at `n_points`
/// this small. Points where sigma is exactly `0` (e.g. the fission channel
/// for a non-fissionable nuclide, or capture above the last MGXS group) are
/// dropped — `log10(0)` is undefined — so an empty return means "this
/// channel is zero everywhere in range", not a sampling failure.
pub fn sample_log_log(
    nx: &NuclideXs,
    mt: MtChannel,
    temp_k: f64,
    n_points: usize,
) -> Vec<LogPoint> {
    let n_points = n_points.max(2);
    let e_lo = nx.e_min();
    let e_hi = nx.e_max();
    let ratio = e_hi / e_lo;
    (0..n_points)
        .filter_map(|i| {
            let e = e_lo * ratio.powf(i as f64 / (n_points - 1) as f64);
            let sigma = mt.value(&nx.micro(e, temp_k));
            (sigma > 0.0).then(|| (e.log10(), sigma.log10()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::temperature::{to_kelvin, TempUnit};

    #[test]
    fn u238_loads_and_has_a_sane_energy_range() {
        let nx = NuclideXs::load("U238").expect("U238 is in the CORE set");
        assert!(nx.e_min() > 0.0);
        assert!(nx.e_max() > nx.e_min());
        assert!(
            nx.e_max() >= 1.0e7,
            "expected the fast range to reach ~20 MeV, got {}",
            nx.e_max()
        );
    }

    #[test]
    fn unknown_name_is_a_readable_error() {
        assert!(NuclideXs::load("Xx999").is_err());
    }

    #[test]
    fn total_channel_is_populated_across_the_range() {
        let nx = NuclideXs::load("U238").unwrap();
        let pts = sample_log_log(&nx, MtChannel::Total, 300.0, 200);
        assert!(
            pts.len() > 150,
            "expected most of 200 samples to be positive, got {}",
            pts.len()
        );
        // Ascending in log10(E) since the grid is monotonically increasing.
        for w in pts.windows(2) {
            assert!(w[0].0 < w[1].0);
        }
    }

    #[test]
    fn fission_channel_is_nonzero_for_a_fissile_nuclide() {
        let nx = NuclideXs::load("U235").unwrap();
        let pts = sample_log_log(&nx, MtChannel::Fission, 300.0, 200);
        assert!(
            !pts.is_empty(),
            "U235 must have a nonzero fission cross section somewhere"
        );
    }

    #[test]
    fn fission_channel_is_empty_for_a_non_fissionable_nuclide() {
        let nx = NuclideXs::load("H1").unwrap();
        let pts = sample_log_log(&nx, MtChannel::Fission, 300.0, 200);
        assert!(pts.is_empty(), "H1 has no fission channel");
    }

    /// Doppler broadening should visibly change the cross section somewhere
    /// in the resonance range (this is the physical effect op-dzl exists to
    /// show); a 0 K vs. ~1200 K U-238 capture comparison at a known resonance
    /// energy is the standard smoke check for "temperature does something".
    #[test]
    fn raising_temperature_changes_the_resonance_region_xs() {
        let nx = NuclideXs::load("U238").unwrap();
        // U-238's well-known ~6.67 eV capture resonance.
        let e = 6.67_f64;
        let cold = nx.micro(e, to_kelvin(0.0, TempUnit::Kelvin)).capture;
        let hot = nx.micro(e, to_kelvin(1200.0, TempUnit::Celsius)).capture;
        assert!(
            (cold - hot).abs() > 1e-6,
            "expected Doppler broadening to change sigma at the resonance peak (cold={cold}, hot={hot})"
        );
    }
}
