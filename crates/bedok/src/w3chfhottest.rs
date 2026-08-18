//! Find the hottest channel and evaluate W-3 critical heat flux on it.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `w3chfhottest.m`, `main_exec_diff3d_standalone` snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.
//!
//! # What it does
//!
//! Sums the wall heat flux down each `(ix, iy)` channel, picks the largest, and
//! runs [`crate::w3chf`] on that channel alone. The rationale is the usual one
//! for a hot-channel analysis: the departure-from-nucleate-boiling margin is
//! set by the worst channel, so there is no need to evaluate the correlation
//! everywhere.
//!
//! # Reference defect C2 — the search can only return a diagonal channel
//!
//! ```text
//! if sum(heatflux(idx)) > qhi
//!     qhi  = sum(heatflux(idx));
//!     highx = ix;
//!     highy = ix;      % <- iy is meant
//! end
//! ```
//!
//! `highy` is assigned `ix`, not `iy`. So whichever channel is hottest, the one
//! actually analysed is `(ix, ix)` — always on the diagonal of the lattice.
//!
//! **This silently analyses the wrong channel** for any core whose hot spot is
//! off-diagonal, which is most of them: a rod-ejection transient's hot spot
//! sits where the rod was, and a quarter-core model's peak is rarely diagonal.
//! The DNBR it reports is then a real number for a real channel — just not the
//! limiting one, so the margin is overstated whenever the diagonal channel is
//! cooler.
//!
//! Translated as written, with the true peak reported alongside the analysed
//! one so a caller can see the two diverge.

use crate::handle3dcoords::handle3dcoords;
use crate::types::{FuelGeometry, Params, Th};
use crate::w3chf::{w3chf, Chf};

/// Which channel the search picked, and which it should have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HottestChannel {
    /// The `(ix, iy)` the reference actually analyses — always `(ix, ix)` by
    /// defect C2.
    pub analysed: (usize, usize),
    /// The `(ix, iy)` that genuinely carries the most integrated wall heat
    /// flux.
    ///
    /// Not computed by the reference. When this differs from
    /// [`HottestChannel::analysed`] the reported DNBR is for the wrong channel.
    pub true_peak: (usize, usize),
}

impl HottestChannel {
    /// Whether defect C2 changed the outcome for this particular core.
    pub fn misidentified(&self) -> bool {
        self.analysed != self.true_peak
    }
}

/// `chf = w3chfhottest(params, geometry, th)`.
///
/// # Arguments
///
/// - `params` — the three extents.
/// - `fuel` — passed through to [`crate::w3chf`]; needs `hydia`.
/// - `th` — needs `heatflux` and the coolant state over the whole core.
///
/// # Returns
///
/// `(chf, channel)` — the W-3 result for the analysed channel, `maxiz` entries
/// long, and which channel that was. See [`HottestChannel`] for why the second
/// is worth checking.
///
/// # Panics
///
/// If any per-node coolant vector is shorter than the node count.
pub fn w3chfhottest(params: &Params, fuel: &FuelGeometry, th: &Th) -> (Chf, HottestChannel) {
    let (maxix, maxiy, maxiz) = handle3dcoords(params);
    let xstep = maxiy * maxiz;

    let channel_heat = |ix: usize, iy: usize| -> f64 {
        let base = ix * xstep + iy * maxiz;
        (0..maxiz).map(|iz| th.heatflux[base + iz]).sum()
    };

    // The reference's search, defect and all.
    let (mut highx, mut highy) = (0usize, 0usize);
    // True peak, tracked alongside but not by the reference.
    let (mut peakx, mut peaky) = (0usize, 0usize);
    let mut qhi = 0.0;
    let mut qpeak = 0.0;

    for ix in 0..maxix {
        for iy in 0..maxiy {
            let q = channel_heat(ix, iy);
            if q > qhi {
                qhi = q;
                highx = ix;
                // C2: `iy` is meant.
                highy = ix;
            }
            if q > qpeak {
                qpeak = q;
                peakx = ix;
                peaky = iy;
            }
        }
    }

    // Slice the analysed channel out of the whole-core state.
    let base = highx * xstep + highy * maxiz;
    let slice = |v: &[f64]| -> Vec<f64> { (0..maxiz).map(|iz| v[base + iz]).collect() };

    let subth = Th {
        heatflux: slice(&th.heatflux),
        coolant: crate::types::Coolant {
            press: slice(&th.coolant.press),
            alphag: slice(&th.coolant.alphag),
            vm: slice(&th.coolant.vm),
            ldens: slice(&th.coolant.ldens),
            gdens: slice(&th.coolant.gdens),
            enth: slice(&th.coolant.enth),
            quality: slice(&th.coolant.quality),
            // The inlet scalars and everything else carry over untouched, as
            // `subth = th` then overwriting does in the reference.
            ..th.coolant.clone()
        },
        ..th.clone()
    };

    (
        w3chf(fuel, &subth),
        HottestChannel {
            analysed: (highx, highy),
            true_peak: (peakx, peaky),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Coolant, FuelGeometry};

    /// A `n`-by-`n` core of `nz`-node channels, uniform except for the wall
    /// heat flux, which the caller sets per channel.
    fn core(n: usize, nz: usize, hot: (usize, usize), hot_flux: f64) -> (Params, FuelGeometry, Th) {
        let params = Params {
            maxix: Some(n),
            maxiy: Some(n),
            maxiz: Some(nz),
            g: 1,
            nc: Some(0),
            ..Default::default()
        };
        let es = n * n * nz;
        let fuel = FuelGeometry {
            hydia: 1.18,
            ..Default::default()
        };

        let mut heatflux = vec![10.0; es];
        let base = hot.0 * n * nz + hot.1 * nz;
        for iz in 0..nz {
            heatflux[base + iz] = hot_flux;
        }

        let th = Th {
            coolant: Coolant {
                inlettemp: 560.0,
                inletpress: 15.5,
                press: vec![15.5; es],
                enth: vec![1300.0; es],
                quality: vec![-0.05; es],
                alphag: vec![0.0; es],
                vm: vec![500.0; es],
                ldens: vec![0.70; es],
                gdens: vec![0.10; es],
                ..Default::default()
            },
            heatflux,
            ..Default::default()
        };
        (params, fuel, th)
    }

    /// A diagonal hot channel is found correctly — the defect does not bite.
    ///
    /// # Methodology
    ///
    /// With the hot channel at `(2, 2)`, `highy = ix` happens to equal `iy`, so
    /// the reference picks the right one. This establishes the search works
    /// before the next test shows when it does not.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// Analysed `(2, 2)`, true peak `(2, 2)` — they agree, and the returned
    /// CHF vector has one entry per axial node.
    #[test]
    fn a_diagonal_hot_channel_is_found_correctly() {
        let (params, fuel, th) = core(3, 5, (2, 2), 50.0);
        let (chf, ch) = w3chfhottest(&params, &fuel, &th);

        eprintln!("analysed {:?}, true peak {:?}", ch.analysed, ch.true_peak);
        assert_eq!(ch.analysed, (2, 2));
        assert_eq!(ch.true_peak, (2, 2));
        assert!(!ch.misidentified());
        assert_eq!(chf.chf.len(), 5, "one entry per axial node of the channel");
    }

    /// Defect C2, pinned: an off-diagonal hot channel is misidentified.
    ///
    /// # Methodology
    ///
    /// The hot channel is placed at `(2, 0)` — the third row, first column — in
    /// a 3x3 lattice. The search correctly finds `ix = 2` but assigns
    /// `highy = ix = 2`, so it analyses `(2, 2)`: a channel at the ordinary
    /// 10 W/cm² rather than the hot 50.
    ///
    /// The consequence is not a crash but an **overstated margin**. The DNBR is
    /// computed against a five-times-lower wall flux, so it comes back roughly
    /// five times larger than the limiting channel's.
    ///
    /// Pass criterion: `analysed` is `(2, 2)`, `true_peak` is `(2, 0)`,
    /// `misidentified()` is true, and the reported DNBR exceeds what the real
    /// hot channel would give.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// Analysed `(2, 2)` where the true peak is `(2, 0)`. The reported DNBR is
    /// **27.485**; the limiting channel's is **5.497**.
    ///
    /// **Interpretation.** C2 confirmed, and the size of the error is the
    /// point: the margin is overstated by a factor of **5.0**, exactly the
    /// ratio of the two wall heat fluxes. A DNBR of 27 reads as an enormous
    /// margin, and a reviewer looking only at the reported number has no way to
    /// tell it belongs to a channel that was never in contention. Nothing in
    /// the reference's output records which channel was analysed — which is why
    /// [`HottestChannel`] is returned here.
    #[test]
    fn an_off_diagonal_hot_channel_is_misidentified() {
        let (params, fuel, th) = core(3, 5, (2, 0), 50.0);
        let (chf, ch) = w3chfhottest(&params, &fuel, &th);

        eprintln!("analysed {:?}, true peak {:?}", ch.analysed, ch.true_peak);
        assert_eq!(ch.analysed, (2, 2), "C2 forces the diagonal");
        assert_eq!(ch.true_peak, (2, 0));
        assert!(ch.misidentified());

        // What the limiting channel would actually have given.
        let mut hot_th = th.clone();
        hot_th.heatflux = vec![50.0; 5];
        hot_th.coolant.press = vec![15.5; 5];
        hot_th.coolant.enth = vec![1300.0; 5];
        hot_th.coolant.quality = vec![-0.05; 5];
        hot_th.coolant.alphag = vec![0.0; 5];
        hot_th.coolant.vm = vec![500.0; 5];
        hot_th.coolant.ldens = vec![0.70; 5];
        hot_th.coolant.gdens = vec![0.10; 5];
        let truth = w3chf(&fuel, &hot_th);

        eprintln!(
            "reported DNBR = {:.3}, limiting channel's DNBR = {:.3}",
            chf.dnbr[0], truth.dnbr[0]
        );
        assert!(
            chf.dnbr[0] > truth.dnbr[0],
            "the misidentified channel should overstate the margin"
        );
    }

    /// A uniform core has no hot channel, and the search leaves its initial
    /// pick.
    ///
    /// # Methodology
    ///
    /// The reference initialises `highx = highy = 1` and `qhi = 0`, and only
    /// updates on a **strict** `>`. With every channel identical, the first one
    /// examined wins and nothing displaces it. In 0-based terms that is
    /// `(0, 0)`.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// Analysed `(0, 0)`, matching the reference's `highx = highy = 1` initial
    /// pick under a strict `>` comparison.
    #[test]
    fn a_uniform_core_keeps_the_first_channel() {
        let (params, fuel, th) = core(3, 5, (0, 0), 10.0);
        let (_, ch) = w3chfhottest(&params, &fuel, &th);
        eprintln!("uniform core: analysed {:?}", ch.analysed);
        assert_eq!(ch.analysed, (0, 0));
        assert!(!ch.misidentified());
    }
}
