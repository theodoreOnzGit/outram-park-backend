//! Unresolved-resonance probability tables in the form a **Monte Carlo
//! transport code** consumes them.
//!
//! [`super::probability_table`] is a faithful port of NJOY's `unrest` and
//! returns everything that subroutine computes, including Bondarenko moments
//! and convergence diagnostics that only a data-processing driver wants. This
//! module is the thin layer above it: run PURR across a nuclide's whole
//! unresolved range once, keep only what a transport kernel needs, and expose a
//! single `sample` call.
//!
//! # What a transport code actually needs
//!
//! On entering the unresolved range a Monte Carlo code draws one uniform and
//! uses it to pick a **band** of the cross-section probability distribution at
//! that energy, then uses that band's cross sections for the collision. That
//! captures resonance self-shielding statistically without resolved
//! resonances. It needs the cumulative probabilities and the per-band values —
//! nothing else in [`super::ProbabilityTable`].
//!
//! # `LSSF` decides what the bands *mean*, and getting it wrong is silent
//!
//! - **`LSSF = 1`** (ENDF/B-VIII.0 U-238, and the common actinide case): MF=3
//!   **already carries the infinitely-dilute unresolved cross sections**, so the
//!   table supplies a **self-shielding factor** to multiply them by. The bands
//!   are dimensionless ratios.
//! - **`LSSF = 0`**: MF=3 carries only a background, and the table supplies the
//!   unresolved cross sections themselves, in barns.
//!
//! This type stores [`Self::lssf`] and returns [`UrrSample`], which names which
//! of the two it is holding, so a consumer cannot quietly multiply barns by
//! barns. That distinction is the same one `op-mzvp.2.12` was derailed by:
//! reading `LSSF=1` as `LSSF=0` makes the reconstruction look catastrophically
//! wrong when it is correct.

use crate::endf::tape::Tape;
use crate::unresr::mf2::{self, UnresolvedCase, UnresolvedRange};
use crate::NjoyError;

use super::wfun::DopplerTable;
use super::{infinite_dilution_reference, probability_table, Rng};

/// NJOY's PURR seed (`purr.f90:166`), reproduced so a table built here matches
/// one built by the reference path.
pub const PURR_SEED: i32 = -101;

/// What one sampled band of an unresolved-resonance probability table holds.
///
/// The variant names the physical meaning, which depends on the evaluation's
/// `LSSF` flag — see the module docs. A consumer must match on it; there is
/// deliberately no way to get the four numbers without seeing which they are.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UrrSample {
    /// `LSSF = 1` — dimensionless **self-shielding factors** to multiply the
    /// MF=3 cross sections by, in the order `[total, elastic, fission,
    /// capture]`.
    SelfShieldingFactors([f64; 4]),
    /// `LSSF = 0` — unresolved **cross sections** \[b\] in the order
    /// `[total, elastic, fission, capture]`, to be used in place of the
    /// evaluation's smooth values (plus any MF=3 background).
    CrossSections([f64; 4]),
}

/// One energy point's bands.
#[derive(Debug, Clone)]
struct UrrPoint {
    /// Cumulative bin probability, ascending, last entry 1.0.
    cum: Vec<f64>,
    /// Per-bin `[total, elastic, fission, capture]` — factors or barns per
    /// the parent's `lssf`.
    value: Vec<[f64; 4]>,
}

/// A nuclide's unresolved-resonance probability tables over its whole
/// unresolved range, at one temperature.
#[derive(Debug, Clone)]
pub struct UrrProbabilityTables {
    /// The evaluation's `LSSF` flag — see [`UrrSample`].
    pub lssf: i32,
    /// Lower bound of the unresolved range \[eV\].
    pub e_low: f64,
    /// Upper bound of the unresolved range \[eV\].
    pub e_high: f64,
    /// Temperature the tables were generated at \[K\].
    pub temperature_k: f64,
    /// Ascending energy grid \[eV\] the tables are tabulated on.
    energy: Vec<f64>,
    points: Vec<UrrPoint>,
}

impl UrrProbabilityTables {
    /// Number of tabulated energy points.
    pub fn len(&self) -> usize {
        self.energy.len()
    }

    /// Whether the table is empty (never true for a value returned by
    /// [`Self::from_endf`], which returns `None` instead).
    pub fn is_empty(&self) -> bool {
        self.energy.is_empty()
    }

    /// The tabulated energy grid \[eV\].
    pub fn energies(&self) -> &[f64] {
        &self.energy
    }

    /// Whether energy `e` \[eV\] lies inside the unresolved range these tables
    /// cover.
    pub fn covers(&self, e: f64) -> bool {
        e >= self.e_low && e <= self.e_high
    }

    /// Sample one band at energy `e` \[eV\] with the uniform `xi` in `[0, 1)`.
    ///
    /// Returns `None` when `e` is outside the unresolved range, so a caller can
    /// use the result to decide whether to consume a random number at all —
    /// which matters, because a transport kernel that draws unconditionally
    /// would shift every RNG stream in the code whether or not a nuclide has
    /// tables.
    ///
    /// The energy grid is searched for the bracketing point and the **nearer**
    /// one is used, matching how a band table is a discrete sampling of
    /// `P(sigma | E)` at tabulated energies rather than a continuous function
    /// to interpolate. (OpenMC interpolates between adjacent tables; this port
    /// does not yet — recorded as a known difference rather than hidden.)
    pub fn sample(&self, e: f64, xi: f64) -> Option<UrrSample> {
        if !self.covers(e) || self.energy.is_empty() {
            return None;
        }
        let i = match self
            .energy
            .binary_search_by(|p| p.partial_cmp(&e).unwrap_or(std::cmp::Ordering::Less))
        {
            Ok(i) => i,
            Err(0) => 0,
            Err(k) if k >= self.energy.len() => self.energy.len() - 1,
            Err(k) => {
                if (e - self.energy[k - 1]).abs() <= (self.energy[k] - e).abs() {
                    k - 1
                } else {
                    k
                }
            }
        };
        let p = &self.points[i];
        let xi = xi.clamp(0.0, 1.0);
        let b = p.cum.partition_point(|&c| c < xi).min(p.value.len() - 1);
        let v = p.value[b];
        Some(if self.lssf == 1 {
            UrrSample::SelfShieldingFactors(v)
        } else {
            UrrSample::CrossSections(v)
        })
    }

    /// Build probability tables for material `mat` from an ENDF `tape`, by
    /// running PURR across the evaluation's unresolved range.
    ///
    /// Returns `Ok(None)` when the evaluation has no `LRU=2` range — most
    /// nuclides — so a caller can treat "no unresolved region" as ordinary
    /// rather than an error.
    ///
    /// `nbin` / `nladr` / `nsamp` are PURR's own controls; NJOY's production
    /// defaults are `20 / 64 / 10000` and reproduce this crate's verified
    /// comparison (`tests/purr_u238_ptables_vs_njoy.rs`). **Cost is roughly
    /// linear in `nladr * nsamp` and in the number of energy points**: the
    /// verified settings take ~45 s for U-238's 83 points on one core, so a
    /// caller that wants tables cheaply should lower `nladr` and say so.
    ///
    /// # Errors
    ///
    /// [`NjoyError`] if MF=2/MT=151 is absent or unparsable, or if PURR
    /// rejects the evaluation's parameters at some energy.
    pub fn from_endf(
        tape: &Tape,
        mat: i32,
        temperature_k: f64,
        nbin: usize,
        nladr: usize,
        nsamp: usize,
    ) -> Result<Option<Self>, NjoyError> {
        let Some(sec) = tape.section(mat, 2, 151) else {
            return Ok(None);
        };
        if sec.rows.len() < 2 {
            return Ok(None);
        }
        // `parse_lru2_ranges` starts at the per-isotope CONT (`rdunf2:439`),
        // not the section's material CONT.
        let ranges = mf2::parse_lru2_ranges(&sec.rows[1..])?;
        let Some(range) = ranges.into_iter().next() else {
            return Ok(None);
        };

        let energy = urr_energy_grid(&range);
        if energy.is_empty() {
            return Ok(None);
        }

        // MF=3 background on the same grid, reduced by PURR's own LSSF rule.
        let mt_data: Vec<(i32, Vec<(u32, u32)>, Vec<(f64, f64)>)> = [1i32, 2, 18, 102]
            .iter()
            .filter_map(|&mt| {
                let s = tape.section(mat, 3, mt)?;
                let mut c = crate::endf::records::SectionCursor::new(&s.rows);
                let _ = c.read_cont().ok()?;
                let t = c.read_tab1().ok()?;
                Some((mt, t.interp, t.pairs))
            })
            .collect();
        let bkg_all = mf2::background_cross_sections(&energy, &mt_data)?;

        let ranges = [range.clone()];
        let mut rng = Rng::new(PURR_SEED);
        let _warmup = rng.next(); // purr.f90:167
        let dop = DopplerTable::new();

        let mut points = Vec::with_capacity(energy.len());
        for (k, &e) in energy.iter().enumerate() {
            let inf = infinite_dilution_reference(&ranges, e)?;
            let bkg = if range.lssf > 0 {
                lssf_reduced_background(bkg_all[k])
            } else {
                bkg_all[k]
            };
            let res = probability_table(
                &inf.sequences,
                &inf,
                bkg,
                &[1.0e10],
                &[temperature_k],
                nbin,
                nladr,
                nsamp,
                &mut rng,
                &dop,
            )?;
            let t = &res.tables[0];

            let mut cum = Vec::with_capacity(nbin);
            let mut acc = 0.0;
            for p in &t.bin_probability {
                acc += *p;
                cum.push(acc);
            }
            if let Some(last) = cum.last_mut() {
                *last = 1.0;
            }

            let value = (0..t.bin_xs.len())
                .map(|j| {
                    let mut v = [0.0f64; 4];
                    for i in 0..4 {
                        v[i] = if range.lssf == 1 {
                            let sigu = t.bondarenko[0][i];
                            if sigu != 0.0 {
                                t.bin_xs[j][i] / sigu
                            } else {
                                1.0
                            }
                        } else {
                            t.bin_xs[j][i]
                        };
                    }
                    v
                })
                .collect();

            points.push(UrrPoint { cum, value });
        }

        Ok(Some(UrrProbabilityTables {
            lssf: range.lssf,
            e_low: range.el,
            e_high: range.eh,
            temperature_k,
            energy,
            points,
        }))
    }
}

/// PURR's `LSSF>0` background rule (`purr.f90:1195-1230`): the partial
/// backgrounds are zeroed and the total keeps only the competition remainder,
/// itself dropped when it is within round-off of zero.
fn lssf_reduced_background(bkg: [f64; 4]) -> [f64; 4] {
    const TOL: f64 = 1.0e-3;
    let [tot, el, fis, cap] = bkg;
    let remainder = tot - el - fis - cap;
    let keep = if remainder > TOL * tot {
        remainder
    } else {
        0.0
    };
    [keep, 0.0, 0.0, 0.0]
}

/// The evaluation's own unresolved energy grid.
///
/// For **Case C** (`LRF=2`, energy-dependent parameters) this is the union of
/// every J-state's tabulated parameter energies — the grid NJOY builds `eunr`
/// from, and the one the verified comparison reproduces point for point (83
/// points on ENDF/B-VIII.0 U-238). Cases A and B carry no per-energy parameter
/// table, so a log-spaced grid across the range is used instead.
fn urr_energy_grid(range: &UnresolvedRange) -> Vec<f64> {
    let mut grid: Vec<f64> = Vec::new();
    if let UnresolvedCase::CaseC { l_states, .. } = &range.case_ {
        for l in l_states {
            for j in &l.j_states {
                for p in &j.points {
                    grid.push(p.e);
                }
            }
        }
    }
    if grid.is_empty() {
        // Cases A/B: no tabulated grid in the evaluation. 40 log-spaced points
        // is a deliberate stand-in, not an upstream behaviour -- NJOY derives
        // its grid differently there, and a caller comparing against NJOY on
        // such an evaluation should expect the grids to differ.
        const N: usize = 40;
        if range.el > 0.0 && range.eh > range.el {
            let ratio = range.eh / range.el;
            for i in 0..=N {
                grid.push(range.el * ratio.powf(i as f64 / N as f64));
            }
        }
    }
    grid.retain(|e| *e >= range.el && *e <= range.eh);
    grid.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    grid.dedup_by(|a, b| (*a - *b).abs() <= 1.0e-9 * b.abs());
    grid
}
