//! Build the ACE cross-section blocks from a [`ReconrResult`].
//!
//! Ports the cross-section portion of `acelod` in NJOY2016 `acefc.f90`:
//! constructing the union energy grid, the ESZ block (energy, total,
//! disappearance, elastic, heating), and the MTR/LQR/TYR/LSIG/SIG reaction
//! blocks. The secondary-distribution blocks (NU/AND/DLW) and heating (KERMA)
//! are out of scope for this increment — see the [module docs](super).
//!
//! ## Reaction bookkeeping (faithful to `acelod`, incident neutron)
//!
//! Reconstructed sections fall into three groups:
//!
//! - **Elastic (MT=2)** — stored in its own ESZ column, never in MTR.
//! - **Redundant sums** — MT=1 (total), MT=3 (nonelastic), MT=4 (total inelastic,
//!   *only* when discrete levels MT=51–91 are present), MT=27/101 (absorption
//!   sums), MT=19/20/21/38 (partial fission, when total fission MT=18 is present),
//!   and the derived quantities MT≥251. These are dropped: the ACE total is
//!   rebuilt as `elastic + Σ partials` on the union grid so it is self-consistent.
//! - **Stored partials** — everything else (MT=18 fission, MT=16 (n,2n),
//!   MT=102 capture, MT=51–91 inelastic levels, charged-particle reactions, …).
//!   Each becomes one MTR/LQR/TYR/LSIG/SIG entry and contributes to the total.
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

/// Round `x` to `n` significant figures, matching NJOY's `sigfig(x,n,0)`.
///
/// NJOY writes ACE values to 7 significant figures so that independently
/// processed libraries compare cleanly; reproducing it keeps our output aligned
/// with the upstream oracle.
fn sigfig(x: f64, n: i32) -> f64 {
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

/// True for MTs whose cross section removes the neutron without producing one
/// (the ESZ disappearance column), per `acelod` for an incident neutron.
fn is_disappearance(mt: i32) -> bool {
    (102..=150).contains(&mt)
        || matches!(mt, 155 | 182 | 191 | 192 | 193 | 197)
        || (600..=849).contains(&mt)
}

/// Assign a [`Role`] to a reaction, given the set of MT numbers present.
///
/// `has_discrete_inelastic` is whether any of MT=51–91 are present (which makes
/// the lumped MT=4 redundant); `has_total_fission` is whether MT=18 is present
/// (which makes the partial-fission MTs 19/20/21/38 redundant).
fn role_of(mt: i32, has_discrete_inelastic: bool, has_total_fission: bool) -> Role {
    match mt {
        2 => Role::Elastic,
        1 | 3 | 27 | 101 => Role::Redundant,
        4 if has_discrete_inelastic => Role::Redundant,
        19 | 20 | 21 | 38 if has_total_fission => Role::Redundant,
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
    /// so it is exactly consistent with the stored partials. Heating (ESZ column
    /// 5) is left zero pending a HEATR port.
    ///
    /// This form writes **no** angular distributions (the AND block is absent).
    /// To include the elastic angular distribution, use
    /// [`from_reconr_with_angular`][Self::from_reconr_with_angular].
    pub fn from_reconr(result: &ReconrResult, kt_mev: f64, suffix: u32) -> Self {
        Self::build(result, kt_mev, suffix, None, &[])
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
        Self::build(result, kt_mev, suffix, Some(angular), &[])
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
    pub fn from_reconr_full(
        result: &ReconrResult,
        kt_mev: f64,
        suffix: u32,
        angular: Option<&ElasticAngular>,
        emissions: &[Emission],
    ) -> Self {
        Self::build(result, kt_mev, suffix, angular, emissions)
    }

    /// Shared assembly for the `from_reconr*` constructors.
    fn build(
        result: &ReconrResult,
        kt_mev: f64,
        suffix: u32,
        angular: Option<&ElasticAngular>,
        emissions: &[Emission],
    ) -> Self {
        let za = result.material.za.round() as i32;
        let awr = result.material.awr;

        // Partition the reconstructed sections by role.
        let present: Vec<i32> = result.sections.iter().map(|s| i32::from(s.mt)).collect();
        let has_discrete_inelastic = present.iter().any(|&m| (51..=91).contains(&m));
        let has_total_fission = present.contains(&18);

        let elastic = result.sections.iter().find(|s| i32::from(s.mt) == 2);
        let partials: Vec<&ReconrSection> = result
            .sections
            .iter()
            .filter(|s| {
                role_of(i32::from(s.mt), has_discrete_inelastic, has_total_fission)
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
            .map(|sec| egrid.iter().map(|&e| sigfig(eval_partial(sec, e), 7)).collect())
            .collect();

        let mut total = elastic_xs.clone();
        let mut disappear = vec![0.0f64; nes];
        for (k, sec) in partials.iter().enumerate() {
            let mt = i32::from(sec.mt);
            let disap = is_disappearance(mt);
            for j in 0..nes {
                total[j] += partial_xs[k][j];
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
        for _ in 0..nes {
            b.real(0.0); // heating (KERMA) — deferred (needs HEATR)
        }

        let ntr = partials.len();
        let mut jxs = [0i32; 32];
        jxs[jxs::ESZ] = 1;

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
        let any_aniso = angulars.iter().any(|a| a.is_some_and(|x| !x.is_all_isotropic()));
        if any_aniso || nr > 0 {
            let (land, and) = append_angular_blocks(&mut b, &angulars);
            jxs[jxs::LAND] = land;
            jxs[jxs::AND] = and;
        }
        if nr > 0 {
            let (ldlw, dlw) = append_dlw(&mut b, &producers, egrid[0] / EMEV, egrid[nes - 1] / EMEV);
            jxs[jxs::LDLW] = ldlw;
            jxs[jxs::DLW] = dlw;
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
        nxs_arr[nxs::NTRP] = 0;
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
            comment: format!("ZA={za} AWR={awr:.6} processed by njoy-outram-park-fork"),
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
        XssBuilder { xss: Vec::new(), is_int: Vec::new() }
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
        assert!(matches!(role_of(1, true, true), Role::Redundant));
        assert!(matches!(role_of(3, false, false), Role::Redundant));
        assert!(matches!(role_of(2, false, false), Role::Elastic));
        // MT=4 is redundant only when discrete inelastic levels exist.
        assert!(matches!(role_of(4, true, false), Role::Redundant));
        assert!(matches!(role_of(4, false, false), Role::Partial));
        // Partial fission MTs are redundant only when total fission MT=18 exists.
        assert!(matches!(role_of(19, false, true), Role::Redundant));
        assert!(matches!(role_of(19, false, false), Role::Partial));
        // Real partials are kept.
        assert!(matches!(role_of(18, false, false), Role::Partial));
        assert!(matches!(role_of(102, false, false), Role::Partial));
        assert!(matches!(role_of(16, false, false), Role::Partial));
    }

    #[test]
    fn disappearance_excludes_fission_and_scatter() {
        assert!(is_disappearance(102)); // capture
        assert!(is_disappearance(103)); // (n,p)
        assert!(is_disappearance(107)); // (n,α)
        assert!(!is_disappearance(18)); // fission is NOT disappearance
        assert!(!is_disappearance(2)); // elastic
        assert!(!is_disappearance(16)); // (n,2n)
    }

    #[test]
    fn eval_partial_is_zero_below_threshold() {
        let sec = ReconrSection {
            mt: MtReaction::Mt16N2n,
            qi: -1.0e7,
            pairs: vec![(1.0e6, 0.0), (2.0e6, 3.0)],
        };
        assert_eq!(eval_partial(&sec, 5.0e5), 0.0, "below threshold → 0");
        assert_eq!(eval_partial(&sec, 1.0e6), 0.0, "at threshold");
        assert!((eval_partial(&sec, 1.5e6) - 1.5).abs() < 1e-12, "interpolated");
        assert_eq!(eval_partial(&sec, 9.9e6), 3.0, "above range → clamp high");
    }
}
