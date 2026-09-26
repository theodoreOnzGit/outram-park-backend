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
pub(crate) struct UrrPoint {
    /// Cumulative bin probability, ascending, last entry 1.0.
    pub(crate) cum: Vec<f64>,
    /// Per-bin `[total, elastic, fission, capture]` — factors or barns per
    /// the parent's `lssf`.
    pub(crate) value: Vec<[f64; 4]>,
    /// Per-bin **heating**: eV per collision when `lssf = 0`, a dimensionless
    /// factor on the smooth heating when `lssf = 1` — the same split as
    /// `value`. It is the sixth column of an ACE UNR block and of PURR's
    /// MT=153, carried so a table read from one can be written back out
    /// (GitHub #325). All zeros when no HEATR heating was available to the
    /// generator — see [`UrrProbabilityTables::from_endf`].
    pub(crate) heating: Vec<f64>,
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
    /// Interpolation between tabulated energies, as the ACE block's third word
    /// states it: `2` lin-lin, `5` log-log. **NJOY's ACER always writes `2`**
    /// (`acefc.f90:5966`, `xss(next+2)=2`) whatever PURR's own `intunr` was, so
    /// a table generated here is `2` and a table read from a file is whatever
    /// the file says.
    pub interpolation: i32,
    /// The **inelastic competition flag** (the ACE block's `ILF`): `-1` for
    /// none, an MT in 51..=91 when exactly one discrete level competes inside
    /// the unresolved range, `4` when more than one does.
    ///
    /// ~~Not stored — "the flags describe how the *generator* treated
    /// competition, which a consumer of finished tables cannot act on".~~
    /// **CORRECTED 2026-09-26 (GitHub #325).** A consumer does act on it:
    /// OpenMC's `Nuclide::calculate_urr_xs` (`src/nuclide.cpp:949-990`) adds
    /// the flagged reaction's **smooth** cross section into the URR total,
    /// `total = elastic + inelastic + capture + fission`. And a writer cannot
    /// reproduce the block without it. Measured: NJOY's U-238 table carries
    /// `51`.
    pub inelastic_competition: i32,
    /// The **other-absorption competition flag** (the ACE block's `IOA`): `-1`
    /// for none, the MT when exactly one non-inelastic reaction competes, `0`
    /// when more than one does. Measured: NJOY's U-238 table carries `0`.
    pub absorption_competition: i32,
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

    /// The per-energy bands, for `acer::unr` to serialise. Crate-private: a
    /// consumer samples through [`Self::sample`], which forces it to see
    /// whether the numbers are factors or barns.
    pub(crate) fn points(&self) -> &[UrrPoint] {
        &self.points
    }

    /// The number of probability bands per energy (the ACE block's `M`).
    pub fn n_bands(&self) -> usize {
        self.points.first().map_or(0, |p| p.cum.len())
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

        // `rdf3un` settles the competition flags and `ecomp` BEFORE it reduces
        // the LSSF>0 background, and the reduction depends on both.
        let (inelastic_competition, absorption_competition) =
            competition_flags(tape, mat, &energy, &bkg_all);
        let competes = inelastic_competition >= 0 || absorption_competition >= 0;
        // `icx`: the first energy whose remainder exceeds `small = 1e-5`
        // (`purr.f90:1150-1153`).
        let ecomp = bkg_all
            .iter()
            .position(|b| b[0] - b[1] - b[2] - b[3] > 1.0e-5)
            .map(|i| energy[i]);

        let mut points = Vec::with_capacity(energy.len());
        for (k, &e) in energy.iter().enumerate() {
            let inf = infinite_dilution_reference(&ranges, e)?;
            let bkg = if range.lssf > 0 {
                lssf_reduced_background(bkg_all[k], e, competes, ecomp)
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

            // PURR hands its tables to ACER through the PENDF tape: MT=153
            // stores `sigfig(tabl, 7, 0)` (`purr.f90:505`, and `:522` after
            // the LSSF=1 division) in 11-column text, and ACER sums the
            // probabilities it reads back (`acefc.f90:5978-5979`). So the
            // cumulative is built from the rounded bin probabilities.
            // ~~Summing the unrounded ones~~ differed from NJOY2016's U-234
            // table in the 7th figure (CORRECTED 2026-09-26).
            let mt153 = |x: f64| {
                use crate::endf::parse::{format_endf_float, parse_endf_float};
                let r = crate::acer::build::sigfig(x, 7);
                parse_endf_float(&format_endf_float(r)).unwrap_or(r)
            };
            let mut cum = Vec::with_capacity(nbin);
            let mut acc = 0.0;
            for p in &t.bin_probability {
                acc += mt153(*p);
                cum.push(acc);
            }
            if let Some(last) = cum.last_mut() {
                *last = 1.0;
            }

            let value: Vec<[f64; 4]> = (0..t.bin_xs.len())
                .map(|j| {
                    let mut v = [0.0f64; 4];
                    for i in 0..4 {
                        let x = crate::acer::build::sigfig(t.bin_xs[j][i], 7);
                        v[i] = mt153(if range.lssf == 1 {
                            let sigu = t.bondarenko[0][i];
                            if sigu != 0.0 {
                                x / sigu
                            } else {
                                1.0
                            }
                        } else {
                            x
                        });
                    }
                    v
                })
                .collect();

            // Heating: upstream PURR fills this column from HEATR's MT=301
            // (and, for full fluctuations, MT=302/318/402) on the PENDF tape it
            // reads (`rdheat`, `purr.f90:267-279`). This builds from an ENDF
            // **evaluation**, which carries no MT=301, so it is upstream's
            // `ihave = 0` case exactly: "no heating found on pendf / ur heating
            // set to zero". The reference `RECONR+BROADR+PURR+ACER` deck has no
            // HEATR either, and NJOY's own U-238 table's heating column is
            // all zeros — measured, not assumed.
            let heating = vec![0.0; value.len()];
            points.push(UrrPoint { cum, value, heating });
        }

        Ok(Some(UrrProbabilityTables {
            lssf: range.lssf,
            e_low: range.el,
            e_high: range.eh,
            temperature_k,
            interpolation: 2,
            inelastic_competition,
            absorption_competition,
            energy,
            points,
        }))
    }

    /// Read the probability tables straight out of an **ACE** table's UNR
    /// block — GitHub #307.
    ///
    /// Ported from `openmc/data/urr.py::ProbabilityTables.from_ace` at OpenMC
    /// `afa7a14`. Where [`Self::from_endf`] *generates* the tables by sampling
    /// ladders (PURR), this **deserialises** tables somebody already generated,
    /// which is what an ACE library carries.
    ///
    /// # Why this exists
    ///
    /// `outram_mc_libs::Nuclide::from_ace` set `urr: None`, so the ACE route
    /// carried no unresolved-resonance self-shielding while the ENDF route
    /// applied it by default. Two routes through one workspace with different
    /// physics is the shape the root `CLAUDE.md`'s "correct physics is the
    /// DEFAULT SETTING" rule exists to stop, and here it was worse than a
    /// flag: structurally absent, so the ablation machinery could not express
    /// it either.
    ///
    /// # The block, word for word
    ///
    /// `JXS(23)` ([`crate::acer::jxs::LUNR`]) locates it, `0` meaning the
    /// evaluation has no unresolved range:
    ///
    /// | words | meaning |
    /// |---|---|
    /// | 0 | `N`, number of incident energies |
    /// | 1 | `M`, number of probability bands |
    /// | 2 | interpolation: 2 lin-lin, 5 log-log |
    /// | 3 | inelastic competition flag |
    /// | 4 | other-absorption flag |
    /// | 5 | `IFF`: 1 ⇒ the values multiply the smooth cross section |
    /// | 6..6+N | the incident energies \[MeV\] |
    /// | then | `N × 6 × M`, C-order `(energy, column, band)` |
    ///
    /// The six columns are `[cumulative probability, total, elastic, fission,
    /// capture, heating]`. **Only the heating column is in MeV** and needs
    /// scaling; the other four are barns (or dimensionless factors) and must
    /// not be touched — upstream scales exactly `table[:, 5, :]`.
    ///
    /// # `IFF` is ACE's `LSSF`
    ///
    /// `IFF = 1` means the tabulated values **multiply** the smooth cross
    /// section, which is precisely `LSSF = 1`'s
    /// [`UrrSample::SelfShieldingFactors`]; `IFF = 0` gives cross sections to
    /// use in place of the smooth ones, i.e. [`UrrSample::CrossSections`].
    /// Mapping them the wrong way round multiplies barns by barns and is
    /// invisible to any check that only looks at shapes, so it is asserted in
    /// the tests rather than trusted.
    ///
    /// # Errors
    ///
    /// A block whose declared extent runs past `XSS`, a non-ascending energy
    /// grid, or a non-positive band count. Returns `Ok(None)` when the table
    /// simply has no UNR block, which is not an error — most light nuclides
    /// have none.
    pub fn from_ace(
        table: &crate::acer::read::RawAceTable,
        temperature_k: f64,
    ) -> Result<Option<Self>, NjoyError> {
        const EV_PER_MEV: f64 = 1.0e6;

        let loc = table.jxs[crate::acer::jxs::LUNR];
        if loc <= 0 {
            return Ok(None);
        }
        let base = (loc - 1) as usize;
        let need = |at: usize, n: usize, what: &str| -> Result<(), NjoyError> {
            if at + n > table.xss.len() {
                return Err(NjoyError::EndfParse(format!(
                    "ACE UNR block: {what} needs words {at}..{} but XSS has {}",
                    at + n,
                    table.xss.len()
                )));
            }
            Ok(())
        };
        need(base, 6, "the header")?;

        let n_energy = table.xss[base] as usize;
        let n_bands = table.xss[base + 1] as usize;
        let interpolation = table.xss[base + 2] as i32;
        let inelastic_flag = table.xss[base + 3] as i32;
        let absorption_flag = table.xss[base + 4] as i32;
        let multiply_smooth = table.xss[base + 5] as i32 == 1;

        if n_energy == 0 || n_bands == 0 {
            return Err(NjoyError::EndfParse(format!(
                "ACE UNR block declares {n_energy} energies and {n_bands} bands; \
                 a table with either at zero cannot be sampled, and silently \
                 returning None here would hide a corrupt block behind the same \
                 answer as a nuclide that legitimately has no unresolved range"
            )));
        }

        let e_at = base + 6;
        need(e_at, n_energy, "the energy grid")?;
        let energy: Vec<f64> = table.xss[e_at..e_at + n_energy]
            .iter()
            .map(|&e| e * EV_PER_MEV)
            .collect();
        if !energy.windows(2).all(|w| w[1] > w[0]) {
            return Err(NjoyError::EndfParse(
                "ACE UNR block: the energy grid is not strictly ascending, so a \
                 binary search over it would return an arbitrary band"
                    .into(),
            ));
        }

        let t_at = e_at + n_energy;
        need(t_at, n_energy * 6 * n_bands, "the probability table")?;
        let raw = &table.xss[t_at..t_at + n_energy * 6 * n_bands];

        // C-order `(energy, column, band)`, matching upstream's
        // `reshape(N, 6, M)`.
        let at = |i: usize, col: usize, b: usize| raw[(i * 6 + col) * n_bands + b];

        // Heating is the one column whose UNIT depends on IFF. NJOY's ACER
        // divides it by 1e6 only when `lssf == 0` (`acefc.f90:5979-5983`),
        // because with `lssf == 1` it is a dimensionless factor, not an
        // energy. So it is scaled back only in that case.
        //
        // This DIVERGES from OpenMC's reader, which multiplies column 5 by 1e6
        // unconditionally (`openmc/data/urr.py:211`) — correct for IFF = 0 and
        // a factor-of-a-million error on an IFF = 1 table with non-zero
        // heating. It is moot on every table in `reference-data/ace`, whose
        // heating column is all zeros (no HEATR in the deck), and it is the
        // reason this follows NJOY's writer rather than OpenMC's reader: the
        // writer defines the unit.
        let heat_scale = if multiply_smooth { 1.0 } else { EV_PER_MEV };

        let mut points = Vec::with_capacity(n_energy);
        for i in 0..n_energy {
            let mut cum = Vec::with_capacity(n_bands);
            let mut value = Vec::with_capacity(n_bands);
            let mut heating = Vec::with_capacity(n_bands);
            for b in 0..n_bands {
                cum.push(at(i, 0, b));
                // `[total, elastic, fission, capture]` — columns 1..=4, barns
                // or factors, never scaled.
                value.push([at(i, 1, b), at(i, 2, b), at(i, 3, b), at(i, 4, b)]);
                heating.push(at(i, 5, b) * heat_scale);
            }
            points.push(UrrPoint { cum, value, heating });
        }

        Ok(Some(Self {
            // IFF = 1 ⇒ the values multiply the smooth cross section, which is
            // LSSF = 1's meaning.
            lssf: i32::from(multiply_smooth),
            e_low: energy[0],
            e_high: energy[n_energy - 1],
            temperature_k,
            // ~~`interpolation` and the two competition flags are read and
            // checked but not stored ... the flags describe how the generator
            // treated competition, which a consumer of finished tables cannot
            // act on.~~ CORRECTED 2026-09-26 (GitHub #325): stored. A writer
            // needs all three to reproduce the block, and OpenMC's transport
            // acts on the inelastic flag (see the field's docs).
            interpolation,
            inelastic_competition: inelastic_flag,
            absorption_competition: absorption_flag,
            energy,
            points,
        }))
    }

}

/// PURR's **competition flags** for an unresolved range — a port of the
/// `icx`/`iinel`/`iabso` block of `purr.f90` (`:1110-1192`, `rdf3un`).
///
/// Two steps, both as upstream does them:
///
/// 1. **Is there competition at all?** At each unresolved energy take the
///    background `σ_x = σ_total − σ_el − σ_f − σ_γ` from MF=3; if it never
///    exceeds `1e-5` b, there is none and both flags are `-1`. `sb` is the
///    same four-column background [`super::super::unresr::mf2::background_cross_sections`]
///    already builds for the band tables, with upstream's `UP`/`DN` edge nudges.
/// 2. **Which reactions?** Walk MF=3 in MT order up to MT=891, keeping the MTs
///    upstream's filter keeps, and take each one's threshold the way upstream
///    does — [`crate::endf::gety1`]'s `x = 0` call. A reaction with
///    `MT > 4`, not 18/19/102, whose threshold (times `1.00001`) lies below the
///    top of the unresolved range competes: 51..=91 as **inelastic**, anything
///    else as **absorption**. One of a kind gives that MT; a second collapses
///    the flag to `4` (inelastic) or `0` (absorption).
///
/// Upstream reads MF=3 off RECONR's PENDF where this reads the evaluation. The
/// two agree here because RECONR preserves every reaction's threshold, and `σ_x`
/// is the non-resonant remainder on both — RECONR adds the unresolved average to
/// MT=1/2/18/102 together, so it cancels out of the difference.
fn competition_flags(
    tape: &Tape,
    mat: i32,
    eunr: &[f64],
    sb: &[[f64; 4]],
) -> (i32, i32) {
    const UP: f64 = 1.000_01; // purr.f90:1097
    const SMALL: f64 = 1.0e-5; // purr.f90:1099
    const MT_MAX: i32 = 891; // purr.f90:1118, "continuum n,2n"

    let competes = sb.iter().any(|b| b[0] - b[1] - b[2] - b[3] > SMALL);
    if !competes {
        return (-1, -1);
    }
    let Some(e_top) = eunr.last().map(|e| e.abs()) else {
        return (-1, -1);
    };

    // `purr.f90:1120-1127`: the MF=3 sections whose thresholds are recorded.
    let kept = |mt: i32| {
        mt < 6
            || (mt > 10 && mt < 12)
            || (mt > 15 && mt < 43)
            || (mt > 43 && mt < 46)
            || (mt > 49 && mt < 92)
            || (mt > 100 && mt < 110)
            || (mt > 110 && mt < 118)
            || (mt > 150 && mt <= 200)
            || mt >= 600
    };

    let mut mts: Vec<i32> = tape
        .sections()
        .iter()
        .filter(|s| s.key.mat == mat && s.key.mf == 3)
        .map(|s| s.key.mt)
        .filter(|&mt| mt >= 4 && mt <= MT_MAX && kept(mt))
        .collect();
    mts.sort_unstable();

    let (mut iinel, mut iabso) = (-1i32, -1i32);
    for mtc in mts {
        if mtc <= 4 || mtc == 18 || mtc == 19 || mtc == 102 {
            continue;
        }
        let Some(sec) = tape.section(mat, 3, mtc) else {
            continue;
        };
        let mut cur = crate::endf::records::SectionCursor::new(&sec.rows);
        if cur.read_cont().is_err() {
            continue;
        }
        let Ok(tab) = cur.read_tab1() else {
            continue;
        };
        let threshold = crate::endf::gety1::Gety1::new(&tab).get(0.0).xnext;
        if !(UP * threshold < e_top) {
            continue;
        }
        if (51..=91).contains(&mtc) {
            iinel = if iinel < 0 { mtc } else { 4 };
        } else {
            iabso = if iabso < 0 { mtc } else { 0 };
        }
    }
    (iinel, iabso)
}

/// PURR's `LSSF>0` background rule (`purr.f90:1195-1230`): the partial
/// backgrounds are zeroed and the total keeps only the competition remainder
/// `total - elastic - fission - capture`. That remainder is kept only when it
/// exceeds `tol * total` with `tol = 1e-6`, there is competition, and the
/// energy is at or above `ecomp`. Otherwise it is zeroed.
///
/// ~~`TOL = 1e-3`, and no competition or `ecomp` test~~ (CORRECTED
/// 2026-09-26). Just above U-238's first inelastic level (45 keV) the
/// remainder is under 0.1 % of the total, so it was dropped where NJOY keeps
/// it. That changed the band values at 13 of U-238's 83 energies
/// (45.1-45.8 keV) against NJOY2016's own table.
fn lssf_reduced_background(bkg: [f64; 4], e: f64, competes: bool, ecomp: Option<f64>) -> [f64; 4] {
    const TOL: f64 = 1.0e-6; // purr.f90:1100
    let [tot, el, fis, cap] = bkg;
    let remainder = tot - el - fis - cap;
    let keep = if remainder > TOL * tot {
        let below_ecomp = ecomp.is_none_or(|ec| e < ec);
        if !competes || below_ecomp {
            0.0
        } else {
            remainder
        }
    } else {
        0.0
    };
    [keep, 0.0, 0.0, 0.0]
}

/// The unresolved-range energy grid, built the way NJOY builds it — a port of
/// the node logic of `rdunf2` (`unresr.f90:426-748`).
///
/// ~~For **Case C** this is the union of every J-state's tabulated parameter
/// energies — the grid NJOY builds `eunr` from, and the one the verified
/// comparison reproduces point for point (83 points on ENDF/B-VIII.0 U-238).
/// Cases A and B carry no per-energy parameter table, so a log-spaced grid
/// across the range is used instead.~~ **CORRECTED 2026-09-26 (GitHub #325).**
/// Three statements there were wrong, found by comparing generated tables
/// against NJOY's own UNR blocks:
///
/// - NJOY takes Case C's nodes from the **first** `(l, j)` state only, and
///   leaves out its first and last points (`:676-679`), not the union of every
///   state;
/// - NJOY then **refines** any interval wider than `1.26x` with a fixed ladder
///   of 78 "round" energies (`egridu`, `:698-722`). That pass was never ported,
///   so U-234 came out on **10** points where NJOY has **26**, and U-235 on
///   **14** against **19**. U-238's evaluation grid is already fine enough
///   that nothing is inserted, which is why the old comparison passed there
///   and the omission went unseen;
/// - Case B **does** carry a per-energy table — the fission-width grid
///   (`:585-594`) — and Case A is refined across its whole range (`indep = 1`)
///   rather than filled with 40 log-spaced points, which was a stand-in with
///   no upstream counterpart.
///
/// And one that was invisible at the precision it was checked to: NJOY
/// **shades the endpoints** one unit in the 7th figure inward
/// (`sigfig(el,7,+1)`, `sigfig(eh,7,-1)`, `:506-513`), so U-238's grid runs
/// `20000.01 .. 149008.6` eV, not `20000 .. 149008.7`. "Point for point" was
/// true at four printed figures.
///
/// # The algorithm, in upstream's order
///
/// 1. Prime the list with `1 MeV` (`ilist` requires one node above every other).
/// 2. Add the four shaded range endpoints `sigfig(el|eh, 7, ∓1 / ±1)`.
/// 3. Add the evaluation's own nodes: Case B's fission-width energies from the
///    second on; Case C's first `(l, j)` state without its end points; Case A
///    none, and mark it energy-independent.
/// 4. Walk adjacent pairs below 1 MeV. Where `next >= 1.26 * last`, or always
///    for Case A, insert every `egridu` energy above `1.01 * (previous inserted)`
///    and below `next`.
/// 5. Drop the first node (the lower outer shade) and the last two (the upper
///    outer shade and the 1 MeV primer), and drop any node within
///    `sigfig(·, 7, 2)` of the one kept before it.
///
/// **Not ported:** upstream merges the nodes of *every* LRU=2 range of every
/// isotope into one list, and marks nodes below the resolved range's upper
/// bound negative (`:733-734`) to flag a resolved-unresolved overlap. This
/// crate builds tables from the first range only (see
/// [`UrrProbabilityTables::from_endf`]), and no held evaluation has an
/// overlap; both are stated rather than silently assumed.
fn urr_energy_grid(range: &UnresolvedRange) -> Vec<f64> {
    use crate::mixr::mix::sigfig;

    const ONEMEV: f64 = 1.0e6; // unresr.f90:420
    const WIDE: f64 = 1.26; // :418
    const STEP: f64 = 1.01; // :422
    // `egridu`, unresr.f90:405-417 — ten-ish "round" energies per decade.
    const EGRIDU: [f64; 78] = [
        1.0e1, 1.25e1, 1.5e1, 1.7e1, 2.0e1, 2.5e1, 3.0e1, 3.5e1, 4.0e1, 5.0e1, 6.0e1, 7.2e1,
        8.5e1, 1.0e2, 1.25e2, 1.5e2, 1.7e2, 2.0e2, 2.5e2, 3.0e2, 3.5e2, 4.0e2, 5.0e2, 6.0e2,
        7.2e2, 8.5e2, 1.0e3, 1.25e3, 1.5e3, 1.7e3, 2.0e3, 2.5e3, 3.0e3, 3.5e3, 4.0e3, 5.0e3,
        6.0e3, 7.2e3, 8.5e3, 1.0e4, 1.25e4, 1.5e4, 1.7e4, 2.0e4, 2.5e4, 3.0e4, 3.5e4, 4.0e4,
        5.0e4, 6.0e4, 7.2e4, 8.5e4, 1.0e5, 1.25e5, 1.5e5, 1.7e5, 2.0e5, 2.5e5, 3.0e5, 3.5e5,
        4.0e5, 5.0e5, 6.0e5, 7.2e5, 8.5e5, 1.0e6, 1.25e6, 1.5e6, 1.7e6, 2.0e6, 2.5e6, 3.0e6,
        3.5e6, 4.0e6, 5.0e6, 6.0e6, 7.2e6, 8.5e6,
    ];

    // `ilist` (`:751-778`): ordered insert, omitting an exact duplicate.
    fn ilist(e: f64, list: &mut Vec<f64>) {
        match list.iter().position(|&x| e <= x) {
            Some(i) if list[i] == e => {}
            Some(i) => list.insert(i, e),
            None => list.push(e),
        }
    }

    if !(range.el > 0.0 && range.eh > range.el) {
        return Vec::new();
    }

    // 1-2. Primer, then the shaded endpoints.
    let mut eunr = vec![ONEMEV];
    ilist(sigfig(range.el, 7, -1), &mut eunr);
    ilist(sigfig(range.el, 7, 1), &mut eunr);
    ilist(sigfig(range.eh, 7, -1), &mut eunr);
    ilist(sigfig(range.eh, 7, 1), &mut eunr);

    // 3. The evaluation's own nodes.
    let indep = match &range.case_ {
        UnresolvedCase::CaseA { .. } => true,
        UnresolvedCase::CaseB {
            fission_energies, ..
        } => {
            for &e in fission_energies.iter().skip(1) {
                ilist(sigfig(e, 7, 0), &mut eunr);
            }
            false
        }
        UnresolvedCase::CaseC { l_states, .. } => {
            if let Some(j) = l_states.first().and_then(|l| l.j_states.first()) {
                let ne = j.points.len();
                for (n, pt) in j.points.iter().enumerate() {
                    if n != 0 && n + 1 != ne {
                        ilist(sigfig(pt.e, 7, 0), &mut eunr);
                    }
                }
            }
            false
        }
    };

    // 4. Refinement (`:698-722`), in upstream's 1-based indexing translated to
    //    0-based: Fortran's `eunr(k)` is `eunr[k - 1]`.
    let mut i = 1usize; // Fortran i = 1
    let mut elast = eunr[1]; // eunr(2)
    loop {
        i += 1;
        let Some(&enext) = eunr.get(i) else { break }; // eunr(i+1)
        if enext >= ONEMEV {
            break;
        }
        if enext >= WIDE * elast || indep {
            let mut et = elast;
            loop {
                let enut = EGRIDU
                    .iter()
                    .copied()
                    .find(|&g| g > STEP * et)
                    .unwrap_or(enext);
                et = enut;
                if et >= enext {
                    break;
                }
                ilist(et, &mut eunr);
                i += 1;
            }
        }
        elast = eunr[i]; // eunr(i+1)
    }

    // 5. Drop the lower outer shade and the primer, de-duplicate at
    //    sigfig(., 7, 2), then drop the upper outer shade.
    let lim = eunr.len().saturating_sub(1);
    let mut out: Vec<f64> = Vec::with_capacity(lim);
    let mut en = 0.0f64;
    for &et in eunr.iter().take(lim).skip(1) {
        if et >= en {
            out.push(et);
            en = sigfig(et.abs(), 7, 2);
        }
    }
    out.pop();
    out
}
