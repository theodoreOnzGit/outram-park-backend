//! Build the ACE cross-section blocks from a [`ReconrResult`].
//!
//! Ports the cross-section portion of `acelod` in NJOY2016 `acefc.f90`:
//! constructing the union energy grid, the ESZ block (energy, total,
//! disappearance, elastic, heating), and the MTR/LQR/TYR/LSIG/SIG reaction
//! blocks. ~~The secondary-distribution blocks (NU/AND/DLW) and heating
//! (KERMA) are out of scope for this increment~~ — **CORRECTED 2026-09-20**:
//! AND and DLW *are* built here (see `jxs::LAND`/`AND`/`LDLW`/`DLW` below).
//! **NU also landed 2026-09-20** (`acer::nu`), bit-identical to NJOY2016 on
//! U-235. ~~Still absent: photon production (`NXS(6)` written as 0).~~
//! **CORRECTED 2026-09-20** — photon production landed the same day
//! (`acer::photon_blocks` + `append_photon_blocks` below): MTRP/LSIGP/SIGP/
//! LANDP/ANDP/LDLWP/DLWP are all written, and `NXS(6)` matches NJOY2016 on
//! U-234 (6), U-235 (583) and U-238 (358). What is still refused outright is
//! *anisotropic* photon emission (MF=14 with `LI=0`).
//! See the [module docs](super).
//!
//! ## Reaction bookkeeping (faithful to `acelod`, incident neutron)
//!
//! Reconstructed sections fall into three groups:
//!
//! - **Elastic (MT=2)** — stored in its own ESZ column, never in MTR.
//! - **Redundant sums** — MT=1 (total), MT=3 (nonelastic), MT=4 (total inelastic,
//!   ~~*only* when discrete levels MT=51–91 are present~~ — **CORRECTED
//!   2026-09-20**: upstream drops MT=4 unless the evaluation carries MF=12/MT=4
//!   (`acefc.f90:1713-1731`); the discrete-level test was this port's own
//!   invention and disagreed with NJOY on U-235 ENDF/B-VII.0), MT=27/101 (absorption
//!   sums), MT=19/20/21/38 (partial fission, when total fission MT=18 is present),
//!   and the derived quantities MT≥251. These are dropped: the ACE total is
//!   rebuilt as `elastic + Σ partials` on the union grid so it is self-consistent.
//! - **Stored partials** — everything else (MT=18 fission, MT=16 (n,2n),
//!   MT=102 capture, MT=51–91 inelastic levels, charged-particle reactions, …).
//!   Each becomes one MTR/LQR/TYR/LSIG/SIG entry.
//!
//! **Stored is not the same as summed.** A discrete charged-particle level
//! (MT=600–849) is stored so it can be tallied, but is left OUT of the ESZ
//! total and disappearance whenever its lumped total (MT=103–107) is also
//! present, because the lumped section already sums those levels. Upstream
//! spells this out as the `mt103.eq.0 .and. mth.ge.mpmin …` guards in
//! `acefc.f90`; omitting it double-counts the whole (n,p)/(n,α) channel into
//! the total, silently.
//!
//! The **disappearance** cross section (ESZ column 3) sums the stored partials
//! that remove the neutron without re-emitting one: capture and charged-particle
//! absorption (MT=102–150, plus MT=155/182/191/192/193/197, plus discrete
//! charged-particle levels MT=600–849). Fission is *not* disappearance.

use crate::reconr::{eval_lin_lin, ReconrResult, ReconrSection};

use super::angular::ElasticAngular;
use super::energy::Emission;
use super::{jxs, nxs, AceTable};

/// Convert eV → MeV (NJOY `emev`).
const EMEV: f64 = 1.0e6;

/// How closely MT=19+20+21+38 must reproduce MT=18 before the partial fission
/// channels may replace it as the stored representation.
///
/// They are the same quantity by definition, so this is a consistency check,
/// not a tolerance on physics: anything above it means one representation is
/// carrying structure the other lacks.
const FISSION_SUM_TOL: f64 = 1.0e-3;

/// Round `x` to `n` significant figures, matching NJOY's `sigfig(x,n,0)`.
///
/// NJOY writes ACE values to 7 significant figures so that independently
/// processed libraries compare cleanly; reproducing it keeps our output aligned
/// with the upstream oracle.
pub(crate) fn sigfig(x: f64, n: i32) -> f64 {
    if x == 0.0 || !x.is_finite() {
        return x;
    }
    let d = (n - 1) - x.abs().log10().floor() as i32;
    let f = 10f64.powi(d);
    (x * f).round() / f
}

/// Evaluate a reconstructed section at energy `e` \[eV\], treating energies below
/// the section's first tabulated point as **zero** (threshold reactions) and
/// energies above the last point as the last value (clamped, as the evaluations
/// extend to the table top). This is the behaviour the union grid needs: a
/// threshold partial must read zero below its threshold, not the clamped endpoint
/// value that [`eval_lin_lin`] would return.
fn eval_partial(sec: &ReconrSection, e: f64) -> f64 {
    match sec.pairs.first() {
        Some(&(e0, _)) if e < e0 => 0.0,
        _ => eval_lin_lin(&sec.pairs, e),
    }
}

/// Classify a reaction by its raw MT number.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    /// Elastic scattering (MT=2): its own ESZ column.
    Elastic,
    /// Redundant sum or derived quantity: dropped.
    Redundant,
    /// A real partial cross section: stored in MTR and summed into the total.
    Partial,
}

/// The five lumped charged-particle channels and the discrete MF=3 level
/// ranges each one sums over, as `acelod` names them (`mpmin`/`mpmax` …
/// `m4min`/`m4max`, `acefc.f90` lines 1141-1150).
const LUMPED_LEVEL_RANGES: [(i32, i32, i32); 5] = [
    (103, 600, 649), // (n,p)
    (104, 650, 699), // (n,d)
    (105, 700, 749), // (n,t)
    (106, 750, 799), // (n,³He)
    (107, 800, 849), // (n,α)
];

/// True when `mt` is a discrete charged-particle level whose lumped total is
/// also present, so the level is **stored but not summed**.
///
/// This is upstream's `mt103.eq.0 .and. mth.ge.mpmin .and. mth.le.mpmax`
/// family of guards (`acefc.f90` ~5652 for disappearance, ~5670 for the
/// total). The lumped MT=103/107 already carries the sum of its levels, so
/// adding both would double-count the channel. Getting this wrong is silent:
/// the table still builds, the total is just too big.
fn covered_by_lumped(mt: i32, present: &[i32]) -> bool {
    LUMPED_LEVEL_RANGES
        .iter()
        .any(|&(lumped, lo, hi)| (lo..=hi).contains(&mt) && present.contains(&lumped))
}

/// True for MTs whose cross section removes the neutron without producing one
/// (the ESZ disappearance column), per `acelod` for an incident neutron.
///
/// `present` is every MT on the tape, needed for the lumped-level guard above.
fn is_disappearance(mt: i32, present: &[i32]) -> bool {
    if (600..=849).contains(&mt) {
        return !covered_by_lumped(mt, present);
    }
    (102..=150).contains(&mt) || matches!(mt, 155 | 182 | 191 | 192 | 193 | 197)
}

/// True when the lumped MT=4 is stored *and* its own discrete levels are too,
/// so MT=4 must be kept out of the ESZ sums.
///
/// This is the same double-count hazard as [`covered_by_lumped`] but in the
/// other direction: there a *level* is covered by its lumped total, here the
/// *lumped* MT=4 is covered by the MT=51–91 levels that sum to it. Upstream
/// never hits it because `acelod` takes the ESZ total straight off MF=3 MT=1
/// rather than rebuilding it; this port rebuilds, so it needs the guard
/// explicitly. Getting it wrong is silent — the table still builds, the total
/// is just inelastic-scattering too big.
fn covered_by_levels(mt: i32, present: &[i32]) -> bool {
    mt == 4 && present.iter().any(|&m| (51..=91).contains(&m))
}

/// True when a stored partial contributes to the rebuilt ESZ **total**.
///
/// Every stored partial contributes except a discrete charged-particle level
/// whose lumped total is present — the same guard as [`covered_by_lumped`],
/// and the reason MT=649 and MT=800-849 can appear in MTR without inflating
/// the total.
fn contributes_to_total(mt: i32, present: &[i32]) -> bool {
    !covered_by_lumped(mt, present) && !covered_by_levels(mt, present)
}

/// Assign a [`Role`] to a reaction, given the set of MT numbers present.
///
/// # When the lumped MT=4 is kept
///
/// `mt4_has_mf12` is whether the evaluation carries an **MF=12 section keyed on
/// MT=4**, and it is the whole of upstream's rule.
///
/// `convr`'s redundant-reaction pass (`acefc.f90:1713-1731`) eliminates MT=3
/// and MT=4 *unless* the MT appears in `mf12s`, in `mf16s`, or is the
/// unresolved-resonance competition reaction. `mf12s` is built from the tape's
/// own dictionary for `mfd.eq.12.and.(mtd.lt.5.or.mtd.gt.600)` (`:390`), and
/// `mf16s` only ever receives MTs `.ge.600` (`:4418`) — so for MT=4 the test
/// reduces to "does MF=12/MT=4 exist".
///
/// **This port previously keyed it on whether MT=51–91 were present**, which is
/// not upstream's rule and is not even correlated with it: U-235 ENDF/B-VII.0
/// and ENDF/B-VIII.0 both carry ~40 discrete levels, and NJOY keeps MT=4 on the
/// first and drops it on the second — because only VII.0 carries MF=12/MT=4.
/// Found by the 57-tape sweep, which reported 46 reactions against NJOY's 47.
///
/// **Not ported:** the `mtcomp.eq.4` clause (`:1731`), which keeps MT=4 when the
/// unresolved-resonance data names it as the competition reaction. No tape in
/// `reference-data/endf/` exercises it against this port's PURR-free path, so it
/// is left unimplemented rather than written untested.
///
/// # Which fission representation is stored
///
/// An evaluation may give total fission (MT=18), the partial chances
/// (MT=19/20/21/38), or both. Exactly one set may be stored — MT=18 is their
/// sum, so keeping both double-counts fission in the total.
///
/// **The switch is not "is MT=18 present".** Upstream keys it on `mt19`
/// (`acefc.f90:388`): whether the evaluation supplies **MF=4/5/6 secondary
/// distributions for MT=19**. The skip test at `acefc.f90:1734` reads
///
/// ```text
/// (mt19==1 .and. mth==18) .or. (mt19==0 .and. mth in {19,20,21,38})
/// ```
///
/// so a first-chance-fission distribution means the partials are the better
/// representation and MT=18 is dropped; without one, MT=18 is kept and the
/// partials are dropped. Checked against the tapes on 2026-09-20: U-234 has
/// MF=4/MT=19 and NJOY stores 19/20/21/38 without 18; U-235 and U-238 have
/// none and NJOY stores 18 without the partials.
///
/// `has_partial_fission` guards the MT=18 drop so that an evaluation with
/// `mt19` set but no partial sections in MF=3 keeps its fission rather than
/// losing it entirely. On a real tape `mt19` implies the partials exist, so
/// this never fires; it is here because the failure it prevents is silent.
fn role_of(mt: i32, mt4_has_mf12: bool, mt19: bool, has_partial_fission: bool) -> Role {
    match mt {
        2 => Role::Elastic,
        1 | 3 | 27 | 101 => Role::Redundant,
        4 if !mt4_has_mf12 => Role::Redundant,
        18 if mt19 && has_partial_fission => Role::Redundant,
        19 | 20 | 21 | 38 if !mt19 => Role::Redundant,
        // Discrete charged-particle levels and (n,2n) levels are REAL partials.
        // Upstream stores them in a second pass over MF=3 (`acefc.f90` ~5536:
        // pass 1 defers `mt.gt.200.and.mt.le.849`, pass 2 picks exactly that
        // range back up), so they carry an MTR/LQR/TYR/LSIG/SIG entry and can
        // be tallied. They are kept OUT of the ESZ sums by
        // `covered_by_lumped` when their lumped total is present.
        //
        // MT=600-849 is verified against NJOY2016's own U-235 table (MT=649
        // and MT=800-835). MT=875-891 follows from the same upstream passes
        // but **no tape in `reference-data/` carries it**, so that half is
        // unverified and is marked as such rather than claimed.
        m if (600..=849).contains(&m) => Role::Partial,
        m if (875..=891).contains(&m) => Role::Partial,
        m if m >= 251 => Role::Redundant, // mu-bar, ξ, heating, KERMA, photon data
        _ => Role::Partial,
    }
}

impl AceTable {
    /// Assemble a continuous-energy ACE table from RECONR/BROADR output.
    ///
    /// # Parameters
    /// - `result` — reconstructed (and optionally Doppler-broadened) cross
    ///   sections for one material.
    /// - `kt_mev` — table temperature as kT \[MeV\] (`0.0` for a 0 K table). Use
    ///   `k_B[eV/K] · T[K] / 1e6`.
    /// - `suffix` — the ZAID identifier suffix as hundredths (e.g. `0` → `.00c`,
    ///   `3` → `.03c`), the conventional MCNP temperature/evaluation tag.
    ///
    /// The build is faithful to `acelod`: the ESZ total is recomputed as
    /// `elastic + Σ partials` on the union grid (not copied from the ENDF MT=1),
    /// so it is exactly consistent with the stored partials. This constructor
    /// leaves heating (ESZ column 5) zero; supply a HEATR KERMA via
    /// [`from_reconr_full`][Self::from_reconr_full] to fill it.
    ///
    /// This form writes **no** angular distributions (the AND block is absent).
    /// To include the elastic angular distribution, use
    /// [`from_reconr_with_angular`][Self::from_reconr_with_angular].
    pub fn from_reconr(result: &ReconrResult, kt_mev: f64, suffix: u32) -> Self {
        Self::build(result, kt_mev, suffix, None, &[], None, None, false, None)
    }

    /// Assemble an ACE table including the **elastic** angular distribution.
    ///
    /// Same as [`from_reconr`][Self::from_reconr] but also writes the LAND/AND
    /// blocks from `angular` (parse MF=4/MT=2 with
    /// [`parse_elastic_angular`][super::angular::parse_elastic_angular]). If the
    /// distribution is isotropic at every incident energy, the AND block is
    /// omitted (elastic stays isotropic, the reader's default).
    pub fn from_reconr_with_angular(
        result: &ReconrResult,
        kt_mev: f64,
        suffix: u32,
        angular: &ElasticAngular,
    ) -> Self {
        Self::build(result, kt_mev, suffix, Some(angular), &[], None, None, false, None)
    }

    /// Assemble a full ACE table: cross sections, the elastic angular
    /// distribution, **and** the secondary-neutron energy distributions (TYR /
    /// LDLW / DLW) for the producing reactions in `emissions` (build them with
    /// [`build_emissions`][super::energy::build_emissions]).
    ///
    /// This is the loadable-transport path: reactions listed in `emissions` get a
    /// TYR yield and a DLW law (Law 3 for discrete levels, Law 4 for continuum /
    /// (n,xn)); their secondary angular distribution is left isotropic (an
    /// AND-block upgrade is future work). NXS(5)=NR is set to the producer count.
    /// `heating` is the MT=301 KERMA cross section (build it with
    /// [`Kerma::from_reconr`][crate::heatr::Kerma::from_reconr]); when supplied,
    /// the ESZ heating column is filled with the ACE heating number
    /// `KERMA(E) / σ_total(E)` \[MeV\] (`acefc`'s `xss(ih+j)`). `None` leaves it
    /// zero.
    pub fn from_reconr_full(
        result: &ReconrResult,
        kt_mev: f64,
        suffix: u32,
        angular: Option<&ElasticAngular>,
        emissions: &[Emission],
        heating: Option<&crate::heatr::Kerma>,
        nu: Option<&[f64]>,
        mt19: bool,
        photons: Option<&[super::photon_blocks::PhotonEntry]>,
    ) -> Self {
        Self::build(
            result, kt_mev, suffix, angular, emissions, heating, nu, mt19, photons,
        )
    }

    /// Shared assembly for the `from_reconr*` constructors.
    fn build(
        result: &ReconrResult,
        kt_mev: f64,
        suffix: u32,
        angular: Option<&ElasticAngular>,
        emissions: &[Emission],
        heating: Option<&crate::heatr::Kerma>,
        nu: Option<&[f64]>,
        mt19: bool,
        photons: Option<&[super::photon_blocks::PhotonEntry]>,
    ) -> Self {
        let za = result.material.za.round() as i32;
        let awr = result.material.awr;

        // Partition the reconstructed sections by role.
        let present: Vec<i32> = result.sections.iter().map(|s| i32::from(s.mt)).collect();
        // Upstream reads this off the tape dictionary (`mf12s`, `acefc.f90:390`).
        // The photon block is built from the same MF=12 sections and numbers its
        // entries `MT*1000 + k`, so an MT=4 section shows up here as entries
        // 4001.., which is the same evidence by a different route.
        //
        // LIMITATION: when the photon block was refused outright (MF=14 `LI=0`)
        // this reads `false` and MT=4 is dropped, where upstream would still have
        // consulted the dictionary. No tape in `reference-data/endf/` hits that
        // combination -- U-235 ENDF/B-VII.0 is the only one carrying MF=12/MT=4
        // and its photon block builds -- so the fallback is unexercised rather
        // than verified, and is recorded here instead of being claimed correct.
        let mt4_has_mf12 = photons
            .map(|p| p.iter().any(|e| e.mtrp / 1000 == 4))
            .unwrap_or(false);
        let has_partial_fission = present.iter().any(|&m| matches!(m, 19 | 20 | 21 | 38));

        // CAN the partial fission channels actually replace MT=18 here?
        //
        // Upstream's `mt19` rule says which representation to STORE, and the
        // ESZ total follows from what is stored, because this port rebuilds
        // the total as `elastic + Σ partials` rather than copying MF=3 MT=1
        // the way `acelod` does.
        //
        // That makes storage and completeness the same question for us and not
        // for NJOY. Measured on U-234, 2026-09-20: RECONR adds the resonance
        // reconstruction to **MT=18 only** — its MT=19 comes back as the smooth
        // 355-point background — so swapping MT=18 for the partials silently
        // dropped resonance fission and put the total 18 % low at 516 eV while
        // every individual section stayed self-consistent.
        //
        // So the rule is applied only when the partials genuinely sum to
        // MT=18. This is measured, not assumed, and it self-corrects: if
        // RECONR later reconstructs the partials too, the check passes and the
        // inventory matches NJOY without anyone revisiting this.
        //
        // Evaluating a higher-chance channel below its threshold correctly
        // yields zero, so the comparison runs over MT=18's own grid.
        let fission_partials_complete = if has_partial_fission {
            match result.sections.iter().find(|s| i32::from(s.mt) == 18) {
                None => true,
                Some(m18) => {
                    let parts: Vec<&ReconrSection> = result
                        .sections
                        .iter()
                        .filter(|s| matches!(i32::from(s.mt), 19 | 20 | 21 | 38))
                        .collect();
                    m18.pairs.iter().all(|&(e, x18)| {
                        if x18 <= 0.0 {
                            return true;
                        }
                        let sum: f64 = parts.iter().map(|p| eval_partial(p, e)).sum();
                        ((sum - x18) / x18).abs() <= FISSION_SUM_TOL
                    })
                }
            }
        } else {
            true
        };
        let mt19 = mt19 && fission_partials_complete;

        let elastic = result.sections.iter().find(|s| i32::from(s.mt) == 2);
        let partials: Vec<&ReconrSection> = result
            .sections
            .iter()
            .filter(|s| {
                role_of(i32::from(s.mt), mt4_has_mf12, mt19, has_partial_fission)
                    == Role::Partial
            })
            .collect();

        // ── Union energy grid [eV] ──────────────────────────────────────────
        // The ACE grid is the union of the elastic grid and every stored
        // partial's grid (acelod builds this with `unionx`).
        let mut egrid: Vec<f64> = Vec::new();
        if let Some(e) = elastic {
            egrid.extend(e.pairs.iter().map(|&(x, _)| x));
        }
        for sec in &partials {
            egrid.extend(sec.pairs.iter().map(|&(x, _)| x));
        }
        egrid.retain(|&x| x > 0.0);
        egrid.sort_by(|a, b| a.partial_cmp(b).unwrap());
        egrid.dedup_by(|a, b| (*a - *b).abs() <= 1e-10 * b.abs().max(1.0));
        let nes = egrid.len();

        // ── ESZ block: energy, total, disappearance, elastic, heating ───────
        // Evaluate elastic and each partial once per grid energy.
        let elastic_xs: Vec<f64> = egrid
            .iter()
            .map(|&e| elastic.map_or(0.0, |s| sigfig(eval_partial(s, e), 7)))
            .collect();

        // Pre-evaluate every partial on the grid (reused for total/disappearance
        // and again for the SIG block).
        let partial_xs: Vec<Vec<f64>> = partials
            .iter()
            .map(|sec| {
                egrid
                    .iter()
                    .map(|&e| sigfig(eval_partial(sec, e), 7))
                    .collect()
            })
            .collect();

        let mut total = elastic_xs.clone();
        let mut disappear = vec![0.0f64; nes];
        for (k, sec) in partials.iter().enumerate() {
            let mt = i32::from(sec.mt);
            let disap = is_disappearance(mt, &present);
            let in_total = contributes_to_total(mt, &present);
            for j in 0..nes {
                if in_total {
                    total[j] += partial_xs[k][j];
                }
                if disap {
                    disappear[j] += partial_xs[k][j];
                }
            }
        }
        // Re-round the accumulated sums to 7 sig figs, as acelod does.
        for j in 0..nes {
            total[j] = sigfig(total[j], 7);
            disappear[j] = sigfig(disappear[j], 7);
        }

        // Lay the XSS block out, recording the integer/real type of each word.
        let mut b = XssBuilder::new();

        // ESZ: five contiguous arrays of length NES (all real).
        for &e in &egrid {
            b.real(e / EMEV); // ACE energies are in MeV
        }
        total.iter().for_each(|&v| b.real(v));
        disappear.iter().for_each(|&v| b.real(v));
        elastic_xs.iter().for_each(|&v| b.real(v));
        // Heating: the ACE "heating number" H(E) = KERMA(E)/σ_total(E) in MeV
        // (acefc's `xss(ih+j) = s/emev/xss(it+j)`), where KERMA(E) is the MT=301
        // heating cross section [eV·barn]. Zero where no HEATR result is supplied
        // or the total vanishes.
        for (j, &e) in egrid.iter().enumerate() {
            let h = match heating {
                Some(k) if total[j] != 0.0 => sigfig(k.eval(e) / EMEV / total[j], 7),
                _ => 0.0,
            };
            b.real(h);
        }

        let ntr = partials.len();
        let mut jxs = [0i32; 32];
        jxs[jxs::ESZ] = 1;

        // NU: fission ν̄, between ESZ and MTR (`acefc.f90` ~5369: `nu=next;
        // next=nu+nnu; … mtr=next`). Absent for a non-fissile nuclide, where
        // JXS(2)=0 is correct rather than a gap — see `acer::nu`.
        if let Some(block) = nu {
            if !block.is_empty() {
                jxs[jxs::NU] = b.next_locator();
                for &v in block {
                    b.real(v);
                }
            }
        }

        if ntr > 0 {
            // MTR: reaction MT numbers (integers).
            jxs[jxs::MTR] = b.next_locator();
            for sec in &partials {
                b.int(i32::from(sec.mt));
            }

            // LQR: reaction Q-values [MeV] (reals).
            jxs[jxs::LQR] = b.next_locator();
            for sec in &partials {
                b.real(sigfig(sec.qi / EMEV, 7));
            }

            // TYR: neutron yield with frame sign. Zero for reactions with no
            // secondary neutron; the producer value comes from `emissions`.
            jxs[jxs::TYR] = b.next_locator();
            for sec in &partials {
                let mt = i32::from(sec.mt);
                let tyr = emissions.iter().find(|e| e.mt == mt).map_or(0, |e| e.tyr);
                b.int(tyr);
            }

            // LSIG: per-reaction locator into the SIG block (1-based, integers).
            // Each reaction's SIG entry is [IE, NE, σ(1..NE)] → 2 + NE words; we
            // store every reaction on the full grid, so NE = NES and IE = 1.
            jxs[jxs::LSIG] = b.next_locator();
            for i in 0..ntr {
                b.int((1 + i * (2 + nes)) as i32);
            }

            // SIG: [IE, NE, σ values] for each reaction.
            jxs[jxs::SIG] = b.next_locator();
            for xs in &partial_xs {
                b.int(1); // IE — first grid index (1-based)
                b.int(nes as i32); // NE — number of points
                xs.iter().for_each(|&v| b.real(v));
            }
        }

        // Producers: the neutron-emitting reactions, in MTR order. NXS(5)=NR.
        let producers: Vec<&Emission> = partials
            .iter()
            .filter_map(|sec| {
                let mt = i32::from(sec.mt);
                emissions.iter().find(|e| e.mt == mt)
            })
            .collect();
        let nr = producers.len();

        // ── LAND / AND (angular) and LDLW / DLW (energy) secondary blocks ────
        // Reactions in LAND order: elastic first, then each producer. Discrete
        // levels carry an MF=4 angular distribution; continuum producers are
        // isotropic (correlated angle → future Law 61/44).
        let mut angulars: Vec<Option<&ElasticAngular>> = Vec::with_capacity(nr + 1);
        angulars.push(angular);
        for e in &producers {
            angulars.push(e.angular.as_ref());
        }
        let any_aniso = angulars
            .iter()
            .any(|a| a.is_some_and(|x| !x.is_all_isotropic()));
        if any_aniso || nr > 0 {
            let (land, and) = append_angular_blocks(&mut b, &angulars);
            jxs[jxs::LAND] = land;
            jxs[jxs::AND] = and;
        }
        if nr > 0 {
            let (ldlw, dlw) =
                append_dlw(&mut b, &producers, egrid[0] / EMEV, egrid[nes - 1] / EMEV);
            jxs[jxs::LDLW] = ldlw;
            jxs[jxs::DLW] = dlw;
        }

        // Photon production. Absent is the normal case and a legal table; the
        // builder returns None rather than a partial block for a form it
        // cannot write (see `photon_blocks::build`).
        let mut ntrp = 0i32;
        if let Some(entries) = photons {
            if !entries.is_empty() {
                let (mtrp, lsigp, sigp, landp, andp, ldlwp, dlwp) =
                    append_photon_blocks(&mut b, entries, &egrid);
                jxs[jxs::MTRP] = mtrp;
                jxs[jxs::LSIGP] = lsigp;
                jxs[jxs::SIGP] = sigp;
                jxs[jxs::LANDP] = landp;
                jxs[jxs::ANDP] = andp;
                jxs[jxs::LDLWP] = ldlwp;
                jxs[jxs::DLWP] = dlwp;
                ntrp = entries.len() as i32;
            }
        }

        let (xss, is_int) = b.finish();
        jxs[jxs::END] = xss.len() as i32;

        // ── NXS array ───────────────────────────────────────────────────────
        let mut nxs_arr = [0i32; 16];
        nxs_arr[nxs::LEN_XSS] = xss.len() as i32;
        nxs_arr[nxs::ZA] = za;
        nxs_arr[nxs::NES] = nes as i32;
        nxs_arr[nxs::NTR] = ntr as i32;
        nxs_arr[nxs::NR] = nr as i32;
        // NTRP was hard-zeroed here while photon production was unwritten.
        nxs_arr[nxs::NTRP] = ntrp;
        nxs_arr[nxs::S] = 0;
        nxs_arr[nxs::Z] = za / 1000;
        nxs_arr[nxs::A] = za % 1000;

        // ── Header strings ──────────────────────────────────────────────────
        // ZAID: ZA + suffix/100 in an f9.2 field, then the class letter 'c'
        // (continuous-energy neutron), e.g. "92235.00c".
        let zaid_num = za as f64 + suffix as f64 / 100.0;
        let zaid = format!("{:9.2}c", zaid_num);
        // `hm` is a cosmetic 10-char material tag. We do not retain the ENDF MAT
        // in `ReconrResult`, so tag with ZA (what a reader displays anyway).
        let mat_id = format!("{:>10}", format!("mat{za}"));

        AceTable {
            zaid,
            awr,
            kt_mev,
            date: "  njoy-rust".to_string(),
            // Provenance, written into the ACE `hk` field (70 chars) so it
            // travels with the table rather than beside it: which crate
            // version, which commit (`+` = built from a dirty tree), and when.
            //
            // AWR was dropped from this string to make room. It is not lost --
            // it is field 2 of line 1 of every ACE file, at full precision,
            // two columns from this comment. Repeating it here bought nothing
            // and cost the characters the provenance needs.
            comment: format!(
                "ZA={za} njoy-outram-park-fork v{} {} {}",
                env!("CARGO_PKG_VERSION"),
                env!("NJOY_OP_GIT_SHA"),
                env!("NJOY_OP_BUILD_DATE"),
            ),
            mat_id,
            nxs: nxs_arr,
            jxs,
            xss,
            xss_is_int: is_int,
        }
    }
}

/// Accumulates the ACE [`xss`](super::AceTable::xss) data block together with the
/// parallel integer/real type mask, so blocks can be appended in order while the
/// 1-based locators that JXS needs are read off with [`next_locator`][Self::next_locator].
struct XssBuilder {
    xss: Vec<f64>,
    is_int: Vec<bool>,
}

impl XssBuilder {
    fn new() -> Self {
        XssBuilder {
            xss: Vec::new(),
            is_int: Vec::new(),
        }
    }

    /// Append a real-valued word (written `1pE20.11`).
    fn real(&mut self, v: f64) {
        self.xss.push(v);
        self.is_int.push(false);
    }

    /// Append an integer-valued word (written `i20`).
    fn int(&mut self, v: i32) {
        self.xss.push(v as f64);
        self.is_int.push(true);
    }

    /// Append a pre-serialised `(value, is_integer)` word.
    fn word(&mut self, v: f64, is_int: bool) {
        self.xss.push(v);
        self.is_int.push(is_int);
    }

    /// The 1-based XSS index the *next* appended word will occupy — i.e. the JXS
    /// locator for a block about to be written.
    fn next_locator(&self) -> i32 {
        self.xss.len() as i32 + 1
    }

    /// Consume the builder, returning the data block and its type mask.
    fn finish(self) -> (Vec<f64>, Vec<bool>) {
        (self.xss, self.is_int)
    }
}

/// Append the **LAND** and **AND** blocks for a set of reactions in LAND order
/// (elastic first, then the neutron producers), returning the 1-based
/// `(LAND, AND)` locators for JXS(8)/JXS(9).
///
/// Each reaction's angular distribution, if anisotropic, is written to the AND
/// block and referenced by its LAND locator (AND-relative, 1-based); isotropic or
/// absent reactions get LAND locator `0`. Layout per `change` in `acefc.f90`.
fn append_angular_blocks(b: &mut XssBuilder, reactions: &[Option<&ElasticAngular>]) -> (i32, i32) {
    // Segment length (AND words) of each reaction's data; 0 if isotropic/absent.
    let seg_len: Vec<i32> = reactions
        .iter()
        .map(|r| match r {
            Some(a) if !a.is_all_isotropic() => {
                let ne = a.energies.len() as i32;
                let dists: i32 = a
                    .energies
                    .iter()
                    .filter(|e| !e.is_isotropic())
                    .map(|e| 2 + 3 * e.cosines.len() as i32)
                    .sum();
                1 + ne + ne + dists // NE + E(NE) + L(NE) + dists
            }
            _ => 0,
        })
        .collect();

    // AND-relative start offset (1-based) of each present segment.
    let mut starts = vec![0i32; reactions.len()];
    let mut off = 1i32;
    for (i, &len) in seg_len.iter().enumerate() {
        if len > 0 {
            starts[i] = off;
            off += len;
        }
    }

    // LAND: one locator per reaction.
    let land = b.next_locator();
    for &s in &starts {
        b.int(s);
    }

    // AND: each anisotropic reaction's angular data.
    let and = b.next_locator();
    for (i, r) in reactions.iter().enumerate() {
        if seg_len[i] > 0 {
            write_angular_segment(b, r.unwrap(), starts[i]);
        }
    }
    (land, and)
}

/// Write one reaction's angular segment starting at AND-relative word `s`
/// (1-based): `NE`, `E(1..NE)` \[MeV\], `L(1..NE)` (AND-relative locators;
/// negative ⇒ tabulated, `0` ⇒ isotropic), then `[JJ, NP, μ, pdf, cdf]` for each
/// anisotropic energy.
fn write_angular_segment(b: &mut XssBuilder, a: &ElasticAngular, s: i32) {
    let energies = &a.energies;
    let ne = energies.len() as i32;

    // First distribution block sits after the [NE, E(NE), L(NE)] preamble.
    let mut dist_base = s + 2 * ne + 1;
    let mut locs = Vec::with_capacity(energies.len());
    for e in energies {
        if e.is_isotropic() {
            locs.push(0);
        } else {
            locs.push(-dist_base);
            dist_base += 2 + 3 * e.cosines.len() as i32;
        }
    }

    b.int(ne); // NE
    for e in energies {
        b.real(e.e_mev); // E [MeV]
    }
    for &l in &locs {
        b.int(l); // L locator
    }
    for e in energies {
        if e.is_isotropic() {
            continue;
        }
        b.int(2); // JJ = 2 (lin-lin)
        b.int(e.cosines.len() as i32); // NP
        e.cosines.iter().for_each(|&m| b.real(m));
        e.pdf.iter().for_each(|&p| b.real(p));
        e.cdf.iter().for_each(|&c| b.real(c));
    }
}

/// Append the **LDLW** and **DLW** blocks for the `producers`, returning their
/// 1-based `(LDLW, DLW)` locators for JXS(10)/JXS(11).
///
/// LDLW holds one locator per producer (DLW-relative, 1-based). Each DLW entry is
/// the 9-word law-validity header `[LNW=0, LAW, IDAT, NR=0, NE=2, E_lo, E_hi,
/// P=1, P=1]` (applies with probability 1 over `[e_lo, e_hi]` \[MeV\]) followed
/// at `IDAT` by the law data. Ports the DLW layout of `acelod`/`acelf5`.
fn append_dlw(b: &mut XssBuilder, producers: &[&Emission], e_lo: f64, e_hi: f64) -> (i32, i32) {
    let nr = producers.len();

    // Header is a fixed 9 words; law data follows. Compute each producer's
    // DLW-relative header offset (1-based) so LDLW and the IDAT locators agree.
    const HEADER: i32 = 9;
    let mut header_off = Vec::with_capacity(nr);
    let mut off = 1i32; // DLW-relative 1-based (DLW starts right after LDLW)
    for e in producers {
        header_off.push(off);
        off += HEADER + e.law.data_len();
    }

    // LDLW block: one DLW-relative locator per producer.
    let ldlw = b.next_locator();
    for &o in &header_off {
        b.int(o);
    }

    // DLW block.
    let dlw = b.next_locator();
    for (i, e) in producers.iter().enumerate() {
        let idat_rel = header_off[i] + HEADER; // DLW-relative 1-based data start
        b.int(0); // LNW — single law
        b.int(e.law.law_number()); // LAW
        b.int(idat_rel); // IDAT
        b.int(0); // NR (law applicability interp)
        b.int(2); // NE
        b.real(e_lo); // E_lo [MeV]
        b.real(e_hi); // E_hi [MeV]
        b.real(1.0); // P_lo
        b.real(1.0); // P_hi
        for (v, is_int) in e.law.serialize(idat_rel) {
            b.word(v, is_int);
        }
    }

    (ldlw, dlw)
}

/// Append the seven **photon-production** blocks, returning the 1-based
/// locators for `JXS(13..19)` in order `(MTRP, LSIGP, SIGP, LANDP, ANDP,
/// LDLWP, DLWP)`.
///
/// Mirrors [`append_dlw`] for the photon side (`acelpp`, `acefc.f90`
/// 8214-9014). `ANDP` is returned equal to `LDLWP` when every photon is
/// isotropic — which is what NJOY writes, and what makes an all-zero `LANDP`
/// unambiguous.
fn append_photon_blocks(
    b: &mut XssBuilder,
    entries: &[super::photon_blocks::PhotonEntry],
    egrid: &[f64],
) -> (i32, i32, i32, i32, i32, i32, i32) {
    use super::photon_blocks::SigP;
    let n = entries.len();

    // MTRP.
    let mtrp = b.next_locator();
    for e in entries {
        b.int(e.mtrp);
    }

    // LSIGP: SIGP-relative 1-based offsets, so size each SIGP entry first.
    // MFTYPE=13 entries are tabulated on the ACE grid HERE, because this is
    // where that grid exists. Each runs from the first grid point at or above
    // the section's own first energy to the last at or below its last -- NOT
    // to the end of the grid, which is what NJOY writes (U-234's MT=3
    // subsections stop early).
    let xs_window = |interp: &[(u32, u32)], pairs: &[(f64, f64)]| -> (i32, Vec<f64>) {
        let (e_first, e_last) = match (pairs.first(), pairs.last()) {
            (Some(&(a, _)), Some(&(b, _))) => (a, b),
            _ => return (1, Vec::new()),
        };
        let ie = egrid.iter().position(|&e| e >= e_first).unwrap_or(0);
        let last = egrid
            .iter()
            .rposition(|&e| e <= e_last)
            .unwrap_or(egrid.len().saturating_sub(1));
        if last < ie {
            return (1, Vec::new());
        }
        let sig = egrid[ie..=last]
            .iter()
            .map(|&e| crate::endf::interp::eval_tab1(e, interp, pairs).unwrap_or(0.0))
            .collect();
        (ie as i32 + 1, sig)
    };

    let sigp_len = |e: &super::photon_blocks::PhotonEntry| -> i32 {
        match &e.sigp {
            // [12, MTMULT, NR=0, NE, E(NE), y(NE)]
            SigP::Yield { e_mev, .. } => 4 + 2 * e_mev.len() as i32,
            // [13, IE, NE, sigma(NE)]
            SigP::Xs { interp, pairs } => 3 + xs_window(interp, pairs).1.len() as i32,
        }
    };
    let mut sig_off = Vec::with_capacity(n);
    let mut off = 1i32;
    for e in entries {
        sig_off.push(off);
        off += sigp_len(e);
    }
    let lsigp = b.next_locator();
    for &o in &sig_off {
        b.int(o);
    }

    // SIGP.
    let sigp = b.next_locator();
    for e in entries {
        match &e.sigp {
            SigP::Yield { mftype, mtmult, e_mev, y } => {
                b.int(*mftype);
                b.int(*mtmult);
                b.int(0); // NR — single lin-lin range
                b.int(e_mev.len() as i32);
                for &v in e_mev {
                    b.real(v);
                }
                for &v in y {
                    b.real(v);
                }
            }
            SigP::Xs { interp, pairs } => {
                let (ie, sig) = xs_window(interp, pairs);
                b.int(13);
                b.int(ie);
                b.int(sig.len() as i32);
                for &v in &sig {
                    b.real(v);
                }
            }
        }
    }

    // LANDP: 0 ⇒ isotropic. Every photon this port writes is isotropic, which
    // `photon_blocks::build` guarantees by refusing MF=14 with LI=0 outright.
    let landp = b.next_locator();
    for _ in entries {
        b.int(0);
    }

    // LDLWP / DLWP, laid out exactly as the neutron LDLW/DLW.
    const HEADER: i32 = 9;
    let mut header_off = Vec::with_capacity(n);
    let mut off = 1i32;
    for e in entries {
        header_off.push(off);
        off += HEADER + e.law.data_len();
    }
    let ldlwp = b.next_locator();
    for &o in &header_off {
        b.int(o);
    }
    // ANDP carries nothing, so it starts where DLWP does — the same
    // convention NJOY writes.
    let andp = ldlwp;
    let dlwp = b.next_locator();
    for (i, e) in entries.iter().enumerate() {
        let idat_rel = header_off[i] + HEADER;
        b.int(0); // LNW
        b.int(e.law.law_number());
        b.int(idat_rel);
        b.int(0); // NR
        b.int(2); // NE
        b.real(e.e_lo_mev);
        b.real(e.e_hi_mev);
        b.real(1.0);
        b.real(1.0);
        for (v, is_int) in e.law.serialize(idat_rel) {
            b.word(v, is_int);
        }
    }

    (mtrp, lsigp, sigp, landp, andp, ldlwp, dlwp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endf::MtReaction;

    #[test]
    fn sigfig_rounds_to_n_significant_figures() {
        assert_eq!(sigfig(0.0, 7), 0.0);
        assert!((sigfig(1.234_567_89, 7) - 1.234_568).abs() < 1e-12);
        assert!((sigfig(92_235.678_9, 7) - 92_235.68).abs() < 1e-9);
    }

    #[test]
    fn role_drops_redundant_sums_keeps_partials() {
        // MT=1/3 are always redundant; MT=2 is elastic.
        assert!(matches!(role_of(1, true, false, false), Role::Redundant));
        assert!(matches!(role_of(3, false, false, false), Role::Redundant));
        assert!(matches!(role_of(2, false, false, false), Role::Elastic));
        // MT=4 is kept ONLY when the evaluation carries MF=12/MT=4
        // (`acefc.f90:1713-1731` via `mf12s`, built at `:390`). It is NOT keyed
        // on whether discrete inelastic levels exist -- U-235 ENDF/B-VII.0 and
        // ENDF/B-VIII.0 both have ~40 levels and NJOY keeps MT=4 only on the
        // first, which is exactly the tape carrying MF=12/MT=4.
        assert!(matches!(role_of(4, false, false, false), Role::Redundant));
        assert!(matches!(role_of(4, true, false, false), Role::Partial));
        // Real partials are kept.
        assert!(matches!(role_of(102, false, false, false), Role::Partial));
        assert!(matches!(role_of(16, false, false, false), Role::Partial));
    }

    /// Exactly ONE fission representation is stored, and which one is decided
    /// by `mt19` — whether the evaluation gives MF=4/5/6 for MT=19 — not by
    /// whether MT=18 happens to be present.
    ///
    /// This is the rule that had U-234 wrong: NJOY2016 stores MT=19/20/21/38
    /// and no MT=18 for it, and this port stored MT=18 and dropped the
    /// partials. Both are internally consistent, so nothing failed — the
    /// tables simply disagreed about which reactions exist, which is why it
    /// took a cross-nuclide comparison to surface.
    #[test]
    fn fission_representation_follows_mt19_not_mt18() {
        // U-235 / U-238 shape: no MF=4/5/6 for MT=19, evaluation gives MT=18
        // and the partials. Keep MT=18, drop the partials.
        assert!(matches!(role_of(18, false, false, true), Role::Partial));
        for mt in [19, 20, 21, 38] {
            assert!(
                matches!(role_of(mt, false, false, true), Role::Redundant),
                "MT={mt} should be dropped when mt19 is unset"
            );
        }

        // U-234 shape: MF=4/MT=19 present. Keep the partials, drop MT=18.
        assert!(matches!(role_of(18, false, true, true), Role::Redundant));
        for mt in [19, 20, 21, 38] {
            assert!(
                matches!(role_of(mt, false, true, true), Role::Partial),
                "MT={mt} should be stored when mt19 is set"
            );
        }

        // Guard: mt19 set but no partial sections on the tape. Dropping MT=18
        // would leave the table with NO fission at all, so it is kept.
        assert!(matches!(role_of(18, false, true, false), Role::Partial));
    }

    #[test]
    fn disappearance_excludes_fission_and_scatter() {
        let none: [i32; 0] = [];
        assert!(is_disappearance(102, &none)); // capture
        assert!(is_disappearance(103, &none)); // (n,p)
        assert!(is_disappearance(107, &none)); // (n,α)
        assert!(!is_disappearance(18, &none)); // fission is NOT disappearance
        assert!(!is_disappearance(2, &none)); // elastic
        assert!(!is_disappearance(16, &none)); // (n,2n)
    }

    /// The discrete charged-particle levels are STORED but only SUMMED when
    /// their lumped total is absent — upstream's `mt103.eq.0 .and. …` guard.
    ///
    /// This is the test that would have caught the double-count: without the
    /// guard, MT=649 and MT=800-835 are added to the ESZ total on top of
    /// MT=103/107, which already sum them. The table still builds; the total
    /// is just silently wrong, which is why this is asserted rather than
    /// eyeballed.
    #[test]
    fn charged_particle_levels_are_stored_but_not_double_counted() {
        // Stored either way: they carry an MTR entry so they can be tallied.
        assert!(matches!(role_of(649, true, false, false), Role::Partial));
        assert!(matches!(role_of(800, true, false, false), Role::Partial));
        assert!(matches!(role_of(835, true, false, false), Role::Partial));

        // U-235's case: the lumped totals ARE present, so the levels must not
        // reach the total or the disappearance column.
        let with_lumped = [2, 18, 102, 103, 107, 649, 800, 835];
        assert!(covered_by_lumped(649, &with_lumped));
        assert!(covered_by_lumped(835, &with_lumped));
        assert!(!contributes_to_total(649, &with_lumped));
        assert!(!contributes_to_total(835, &with_lumped));
        assert!(!is_disappearance(649, &with_lumped));
        assert!(!is_disappearance(835, &with_lumped));

        // An evaluation carrying ONLY the levels: now they are the channel, so
        // they must be summed. Upstream's guard is `mt103.eq.0`, not "never".
        let levels_only = [2, 102, 649, 800, 835];
        assert!(!covered_by_lumped(649, &levels_only));
        assert!(contributes_to_total(649, &levels_only));
        assert!(is_disappearance(649, &levels_only));
        assert!(is_disappearance(835, &levels_only));

        // The ranges are per-channel: an (n,α) level is not covered by MT=103.
        let only_103 = [2, 103, 800];
        assert!(!covered_by_lumped(800, &only_103));
        assert!(contributes_to_total(800, &only_103));
    }

    #[test]
    fn eval_partial_is_zero_below_threshold() {
        let sec = ReconrSection {
            lr: 0,
            mt: MtReaction::Mt16N2n,
            qi: -1.0e7,
            pairs: vec![(1.0e6, 0.0), (2.0e6, 3.0)],
        };
        assert_eq!(eval_partial(&sec, 5.0e5), 0.0, "below threshold → 0");
        assert_eq!(eval_partial(&sec, 1.0e6), 0.0, "at threshold");
        assert!(
            (eval_partial(&sec, 1.5e6) - 1.5).abs() < 1e-12,
            "interpolated"
        );
        assert_eq!(eval_partial(&sec, 9.9e6), 3.0, "above range → clamp high");
    }
}
