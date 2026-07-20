//! Delayed-neutron data: precursor decay constants, delayed ν̄(E), and the
//! delayed fission-neutron energy spectrum χ_delayed.
//!
//! The prompt-neutron data in [`super::secondary`] (total ν̄ from MF=1/452 and
//! the prompt χ from MF=5/MT=18) is not enough for a *delayed*-critical or a
//! point-kinetics calculation: those need the fraction of fission neutrons that
//! are emitted late (through a β-decaying precursor), grouped into a handful of
//! precursor families each with its own decay constant λ and its own emission
//! spectrum. ENDF stores that as two sections, both read here:
//!
//! - **MF=1 / MT=455** — [`DelayedNuBar`]: the `NNF` (usually 6) precursor-group
//!   decay constants λ \[s⁻¹\] plus the total delayed ν̄_d(E) (average delayed
//!   neutrons per fission vs incident energy).
//! - **MF=5 / MT=455** — [`DelayedChi`]: the delayed fission-neutron energy
//!   spectrum χ_delayed(E→E'), one subsection per precursor group.
//!
//! # Provenance
//!
//! The record layout mirrors the **ENDF-6 Formats Manual** (Trkov, Herman,
//! Brown, eds., *ENDF-6 Formats Manual*, CSEWG document ENDF-102 / BNL report
//! BNL-203218-2018-INRE, 2018), §1.6 (MF=1/MT=455) and §5 (MF=5). ENDF is an
//! open, published format; this reader is written from that specification, not
//! ported from NJOY. It reuses the crate's generic ENDF record parsers
//! ([`crate::endf::records`]). The NJOY2016 delayed-ν̄ handling in `groupr`/`acer`
//! (commit `ac5adf5`) was consulted only to confirm the LDG/LNU/LF branch
//! semantics; no NJOY code is reproduced here.
//!
//! Data source for all measured numbers in the tests: ENDF/B-VIII.0 neutron
//! sublibrary (open-source, IAEA/NNDC), U-235 evaluation `n-092_U_235-ENDF8.0`.
//!
//! # Design
//!
//! Storage is plain `f64` in documented units (matching [`super::secondary`]),
//! with a named [`DecayConstant`] `uom` alias exposed on the accessor boundary
//! so a caller hovering in their editor sees a dimensioned rate, not a bare
//! float. No trait objects, no `Box`, no lifetimes (workspace rules).

use uom::si::f64::Frequency;
use uom::si::frequency::hertz;

use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::NjoyError;

/// A radioactive-decay constant λ \[s⁻¹\] — the rate at which a delayed-neutron
/// precursor family β-decays (and thereby emits its delayed neutron).
///
/// Dimensionally a frequency (inverse time), so it is exposed as [`Frequency`];
/// read the numeric value back in s⁻¹ with `.get::<uom::si::frequency::hertz>()`
/// (1 Hz ≡ 1 s⁻¹). Physical range for fission precursors is ~0.01–3 s⁻¹
/// (half-lives ~0.2–55 s).
pub type DecayConstant = Frequency;

/// Delayed ν̄ and the precursor decay constants, from ENDF **MF=1 / MT=455**.
///
/// Holds the `NNF` (commonly 6) precursor-group decay constants λ \[s⁻¹\] and the
/// total delayed neutron yield ν̄_d(E) as a lin-lin table in incident energy
/// \[eV\]. The groups are ordered fastest-decaying *last* in ENDF convention for
/// most major actinides, but this reader preserves the tape order exactly and
/// does not re-sort — [`Self::lambdas`] returns them as stored.
///
/// # Assumptions / valid range
///
/// - Only the **energy-independent decay-constant** form (`LDG=0`) is fully
///   supported; that is what ENDF/B-VII/VIII use for the major actinides
///   (U-235/238, Pu-239/241, …). For the rare energy-dependent form (`LDG=1`)
///   the decay constants at the *lowest* tabulated incident energy are returned
///   and [`Self::ldg1_energy_dependent`] is set — see its note; this path is
///   parsed from the spec but has no fixture in-repo to verify against.
/// - Both ν̄_d representations are handled: `LNU=2` (tabulated, stored directly)
///   and `LNU=1` (polynomial in E, sampled onto a 60-point log grid over
///   `[1e-3, 2e7]` eV, matching [`super::secondary::NuBar`]).
#[derive(Debug, Clone, Default)]
pub struct DelayedNuBar {
    /// Precursor-group decay constants λ \[s⁻¹\], in ENDF tape order.
    pub lambda: Vec<f64>,
    /// Incident-energy grid \[eV\] for delayed ν̄_d, ascending.
    pub energy: Vec<f64>,
    /// Total delayed ν̄_d aligned with [`Self::energy`].
    pub nu_delayed: Vec<f64>,
    /// `true` if the tape used the energy-dependent decay-constant form
    /// (`LDG=1`) and [`Self::lambda`] holds the lowest-energy set (see the
    /// type-level note); `false` for the standard `LDG=0` form.
    pub ldg1_energy_dependent: bool,
}

impl DelayedNuBar {
    /// Parse delayed ν̄ + precursor decay constants from ENDF **MF=1 / MT=455**
    /// for material `mat`.
    ///
    /// Returns `Ok(None)` when the material has no MF=1/455 section (a nuclide
    /// with no delayed-neutron data), or [`NjoyError::EndfParse`] on a malformed
    /// record or an unrecognised `LNU`.
    pub fn from_endf(tape: &Tape, mat: i32) -> Result<Option<DelayedNuBar>, NjoyError> {
        let sec = match tape.section(mat, 1, 455) {
            Some(s) => s,
            None => return Ok(None),
        };
        Self::from_rows(&sec.rows).map(Some)
    }

    /// Row-level MF=1/455 parser — the body of [`Self::from_endf`], factored out
    /// so it can be unit-tested against hand-built `[f64; 6]` rows without a full
    /// [`Tape`].
    pub(crate) fn from_rows(rows: &[[f64; 6]]) -> Result<DelayedNuBar, NjoyError> {
        let mut cur = SectionCursor::new(rows);
        // HEAD: ZA, AWR, LDG (=L1), LNU (=L2), 0, 0.
        let head = cur.read_cont()?;
        let ldg = head.l1;
        let lnu = head.l2;

        let (lambda, ldg1) = if ldg == 0 {
            // LDG=0: a single LIST of NNF decay constants (energy-independent).
            let list = cur.read_list()?;
            (list.data, false)
        } else {
            // LDG=1: TAB2 over NE incident energies, then per energy a LIST of
            // 2*NNF interleaved (λ_i, α_i) pairs. Take the lowest-energy λ set as
            // the representative energy-independent constants (documented on the
            // type). No LDG=1 fixture is in-repo, so this branch is spec-faithful
            // but unverified against a golden tape.
            let _tab2 = cur.read_tab2()?;
            let list = cur.read_list()?;
            let lambda = list.data.iter().step_by(2).copied().collect();
            (lambda, true)
        };

        // Delayed ν̄_d(E): TAB1 for LNU=2, polynomial LIST for LNU=1.
        let (energy, nu_delayed) = match lnu {
            2 => {
                let tab1 = cur.read_tab1()?;
                tab1.pairs.iter().copied().unzip()
            }
            1 => {
                let list = cur.read_list()?;
                Self::sample_polynomial(&list.data)
            }
            other => {
                return Err(NjoyError::EndfParse(format!(
                    "MF=1/455 unrecognised LNU={other} (expected 1 or 2)"
                )));
            }
        };

        Ok(DelayedNuBar {
            lambda,
            energy,
            nu_delayed,
            ldg1_energy_dependent: ldg1,
        })
    }

    /// Sample an `LNU=1` polynomial ν̄_d(E) = Σ Cₖ Eᵏ onto a 60-point log grid
    /// over `[1e-3, 2e7]` eV (same grid as [`super::secondary::NuBar`]).
    fn sample_polynomial(coeffs: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let (e_lo, e_hi, n) = (1.0e-3_f64, 2.0e7_f64, 60usize);
        let mut energy = Vec::with_capacity(n + 1);
        let mut nu = Vec::with_capacity(n + 1);
        for i in 0..=n {
            let e = e_lo * (e_hi / e_lo).powf(i as f64 / n as f64);
            // Horner over reversed coefficients: C1 + C2·E + C3·E² + …
            let v = coeffs.iter().rev().fold(0.0, |acc, &c| acc * e + c);
            energy.push(e);
            nu.push(v);
        }
        (energy, nu)
    }

    /// Number of precursor groups (`NNF`), e.g. 6 for the standard actinides.
    pub fn n_groups(&self) -> usize {
        self.lambda.len()
    }

    /// The precursor decay constants λ \[s⁻¹\] as raw floats, in tape order.
    pub fn lambdas(&self) -> &[f64] {
        &self.lambda
    }

    /// The decay constant of precursor group `g` as a dimensioned
    /// [`DecayConstant`] (a [`Frequency`]; 1 Hz ≡ 1 s⁻¹). Returns `None` if `g`
    /// is out of range.
    pub fn decay_constant(&self, g: usize) -> Option<DecayConstant> {
        self.lambda.get(g).map(|&l| Frequency::new::<hertz>(l))
    }

    /// Interpolate total delayed ν̄_d at incident energy `e` \[eV\] (lin-lin,
    /// clamped at the ends). Returns `0.0` if no ν̄_d table was parsed.
    pub fn nu_delayed_at(&self, e: f64) -> f64 {
        interp_clamped(&self.energy, &self.nu_delayed, e)
    }
}

/// The delayed fission-neutron energy spectrum χ_delayed, from ENDF
/// **MF=5 / MT=455**.
///
/// One [`DelayedChiGroup`] per precursor family (`NK` subsections, matching the
/// `NNF` groups of [`DelayedNuBar`]). Each group carries its energy-dependent
/// probability fraction p_k(E) (the group's share of the delayed emission — the
/// "β_k" shape) and its outgoing-energy spectrum.
///
/// # Assumptions / valid range
///
/// Two ENDF secondary-energy laws (`LF`) are supported — the two that appear for
/// delayed spectra in ENDF/B-VII/VIII:
/// - **LF=5** (general evaporation): the spectrum is g(E'/θ(E)) with a tabulated
///   shape g and temperature θ(E). ENDF/B-VIII.0 U-235/Pu-239 use this with
///   θ(E) ≡ 1, so g is directly the outgoing spectrum in eV.
/// - **LF=1** (arbitrary tabulated): the outgoing spectrum is tabulated per
///   incident energy; the lowest-incident-energy table is stored as the
///   representative shape.
///
/// Any other `LF` makes [`Self::from_endf`] return `Ok(None)` (documented
/// unsupported), never a fabricated spectrum.
#[derive(Debug, Clone, Default)]
pub struct DelayedChi {
    /// One entry per precursor group (`NK` subsections), in tape order.
    pub groups: Vec<DelayedChiGroup>,
}

/// One precursor group's delayed-emission data within [`DelayedChi`].
///
/// `spectrum` is the outgoing-neutron energy distribution as `(E' [eV], density
/// [eV⁻¹])` samples: for `LF=5` this is the tabulated shape g (in eV, since
/// θ ≡ 1 for the actinide delayed spectra); for `LF=1` it is g(E→E') at the
/// lowest incident energy. It is defined normalized in ENDF (∫ over E' = 1); the
/// parser stores the tape values verbatim so a verification test can *measure*
/// the normalization rather than assume it.
#[derive(Debug, Clone, Default)]
pub struct DelayedChiGroup {
    /// The group's probability fraction p_k(E) as `(E [eV], fraction)` — the
    /// per-group share of the total delayed emission vs incident energy.
    pub fraction: Vec<(f64, f64)>,
    /// ENDF `LF` law code for this group's outgoing spectrum (5 or 1 here).
    pub lf: i32,
    /// Outgoing-energy spectrum samples `(E' [eV], density [eV⁻¹])`, ascending
    /// in E'.
    pub spectrum: Vec<(f64, f64)>,
}

impl DelayedChiGroup {
    /// Trapezoidal integral of the outgoing spectrum over E' — the spectrum's
    /// normalization ∫χ_delayed(E') dE'. Physically ≈ 1 for a normalized ENDF
    /// spectrum. (For `LF=5` with θ ≡ 1 the change of variable x = E'/θ is the
    /// identity, so ∫g(x)dx = ∫χ dE' directly.)
    pub fn normalization(&self) -> f64 {
        let s = &self.spectrum;
        if s.len() < 2 {
            return 0.0;
        }
        let mut sum = 0.0;
        for k in 1..s.len() {
            let dx = s[k].0 - s[k - 1].0;
            sum += 0.5 * (s[k].1 + s[k - 1].1) * dx;
        }
        sum
    }

    /// The smallest outgoing-spectrum density value — a non-negativity probe
    /// (a physical spectrum has no negative density). Returns `0.0` for an empty
    /// spectrum.
    pub fn min_density(&self) -> f64 {
        if self.spectrum.is_empty() {
            return 0.0;
        }
        self.spectrum.iter().map(|&(_, y)| y).fold(f64::INFINITY, f64::min)
    }
}

impl DelayedChi {
    /// Parse the delayed fission-neutron spectrum from ENDF **MF=5 / MT=455** for
    /// material `mat`.
    ///
    /// Returns `Ok(None)` when the material has no MF=5/455 section, or when any
    /// precursor subsection uses an `LF` law this reader does not support (see
    /// the [`DelayedChi`] note) — the whole parse aborts rather than return a
    /// partially-filled, physically-inconsistent spectrum.
    pub fn from_endf(tape: &Tape, mat: i32) -> Result<Option<DelayedChi>, NjoyError> {
        let sec = match tape.section(mat, 5, 455) {
            Some(s) => s,
            None => return Ok(None),
        };
        Self::from_rows(&sec.rows)
    }

    /// Row-level MF=5/455 parser — the body of [`Self::from_endf`], factored out
    /// for direct unit testing against hand-built rows.
    pub(crate) fn from_rows(rows: &[[f64; 6]]) -> Result<Option<DelayedChi>, NjoyError> {
        let mut cur = SectionCursor::new(rows);
        // HEAD: ZA, AWR, 0, 0, NK, 0.
        let head = cur.read_cont()?;
        let nk = head.n1.max(0) as usize;
        if nk == 0 {
            return Ok(None);
        }

        let mut groups = Vec::with_capacity(nk);
        for _ in 0..nk {
            // Subsection header TAB1: C1=U, C2=0, L2=LF, then p_k(E).
            let p_tab = cur.read_tab1()?;
            let lf = p_tab.head.l2;
            let fraction = p_tab.pairs.clone();

            let spectrum = match lf {
                5 => {
                    // General evaporation: θ(E) TAB1, then g(x) TAB1.
                    let _theta = cur.read_tab1()?;
                    let g = cur.read_tab1()?;
                    g.pairs
                }
                1 => {
                    // Arbitrary tabulated: TAB2 over NE incident energies, each
                    // an inner TAB1 g(E→E'). Store the first (lowest incident E).
                    let tab2 = cur.read_tab2()?;
                    let ne = tab2.head.n2.max(0) as usize;
                    let mut first = Vec::new();
                    for i in 0..ne {
                        let g = cur.read_tab1()?;
                        if i == 0 {
                            first = g.pairs;
                        }
                    }
                    first
                }
                // LF=7/9/11 (parametric) and LF=12 (Madland-Nix) are not stored
                // as a directly-tabulated χ; unsupported here (do not fabricate).
                _ => return Ok(None),
            };

            groups.push(DelayedChiGroup { fraction, lf, spectrum });
        }

        Ok(Some(DelayedChi { groups }))
    }

    /// Number of precursor groups (`NK`).
    pub fn n_groups(&self) -> usize {
        self.groups.len()
    }
}

/// Lin-lin interpolate `y` on an ascending `x` grid \[eV\] at `e`, clamped to the
/// endpoint values outside `[x[0], x[last]]`. Returns `0.0` for an empty table.
fn interp_clamped(x: &[f64], y: &[f64], e: f64) -> f64 {
    match x.first() {
        None => 0.0,
        Some(&x0) if e <= x0 => y[0],
        Some(_) => {
            let &xn = x.last().unwrap();
            if e >= xn {
                return *y.last().unwrap();
            }
            let hi = x.iter().position(|&v| v >= e).unwrap();
            let (a, b) = (x[hi - 1], x[hi]);
            let (ya, yb) = (y[hi - 1], y[hi]);
            ya + (yb - ya) * (e - a) / (b - a)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build the rows of an MF=1/455 LDG=0, LNU=2 section: NNF decay constants in
    /// a LIST, then a 2-point lin-lin ν̄_d(E) TAB1.
    fn mf1_455_ldg0_rows(lambdas: &[f64], nu_lo: f64, nu_hi: f64) -> Vec<[f64; 6]> {
        let nnf = lambdas.len();
        let mut rows = vec![[92235.0, 233.0, 0.0, 2.0, 0.0, 0.0]]; // HEAD LDG=0 LNU=2
        // LIST head: N1 = NNF.
        rows.push([0.0, 0.0, 0.0, 0.0, nnf as f64, 0.0]);
        // LIST data packed 6 per row.
        for chunk in lambdas.chunks(6) {
            let mut row = [0.0; 6];
            row[..chunk.len()].copy_from_slice(chunk);
            rows.push(row);
        }
        // TAB1 ν̄_d: NR=1, NP=2.
        rows.push([0.0, 0.0, 0.0, 0.0, 1.0, 2.0]);
        rows.push([2.0, 2.0, 0.0, 0.0, 0.0, 0.0]); // interp (NBT=2, INT=2 lin-lin)
        rows.push([1.0e-5, nu_lo, 2.0e7, nu_hi, 0.0, 0.0]); // (E, nu) pairs
        rows
    }

    /// LDG=0/LNU=2 round-trip: the six decay constants and the ν̄_d table parse
    /// back exactly, and interpolation clamps at the ends.
    #[test]
    fn parses_mf1_455_ldg0_six_groups() {
        let lam = [0.0133, 0.0327, 0.1208, 0.3028, 0.8495, 2.853];
        let rows = mf1_455_ldg0_rows(&lam, 0.0165, 0.009);
        let d = DelayedNuBar::from_rows(&rows).unwrap();
        assert_eq!(d.n_groups(), 6);
        assert!(!d.ldg1_energy_dependent);
        for (g, &l) in lam.iter().enumerate() {
            assert!((d.lambdas()[g] - l).abs() < 1e-9, "λ[{g}]");
            let uom_l = d.decay_constant(g).unwrap().get::<hertz>();
            assert!((uom_l - l).abs() < 1e-9, "uom λ[{g}]");
        }
        // Clamped ends + a midpoint interpolation of ν̄_d.
        assert!((d.nu_delayed_at(1.0e-6) - 0.0165).abs() < 1e-9);
        assert!((d.nu_delayed_at(1.0e8) - 0.009).abs() < 1e-9);
        assert_eq!(d.decay_constant(6), None);
    }

    /// An MF=5/455 LF=5 subsection: p_k(E) TAB1 (L2=LF=5), θ(E) TAB1, g(x) TAB1.
    fn mf5_455_lf5_group(frac: f64, g_pts: &[(f64, f64)]) -> Vec<[f64; 6]> {
        let mut rows = Vec::new();
        // p_k(E) TAB1: C1=U=-3e7, L2=LF=5, NR=1, NP=2.
        rows.push([-3.0e7, 0.0, 0.0, 5.0, 1.0, 2.0]);
        rows.push([2.0, 2.0, 0.0, 0.0, 0.0, 0.0]);
        rows.push([1.0e-5, frac, 3.0e7, frac, 0.0, 0.0]);
        // θ(E) TAB1 ≡ 1: NR=1, NP=2.
        rows.push([0.0, 0.0, 0.0, 0.0, 1.0, 2.0]);
        rows.push([2.0, 2.0, 0.0, 0.0, 0.0, 0.0]);
        rows.push([1.0e-5, 1.0, 3.0e7, 1.0, 0.0, 0.0]);
        // g(x) TAB1: NR=1, NP=g_pts.len().
        rows.push([0.0, 0.0, 0.0, 0.0, 1.0, g_pts.len() as f64]);
        rows.push([g_pts.len() as f64, 1.0, 0.0, 0.0, 0.0, 0.0]); // INT=1 hist
        for chunk in g_pts.chunks(3) {
            let mut row = [0.0; 6];
            for (i, &(x, y)) in chunk.iter().enumerate() {
                row[2 * i] = x;
                row[2 * i + 1] = y;
            }
            rows.push(row);
        }
        rows
    }

    /// LF=5 delayed χ: two groups parse, each keeps its LF, fraction, and a
    /// non-negative spectrum whose trapezoidal normalization is finite/positive.
    #[test]
    fn parses_mf5_455_lf5_two_groups() {
        // A little triangular spectrum on [0, 2 MeV] that integrates (trapezoid)
        // to exactly 1: peak 1e-6 /eV at 1 MeV, zero at the ends.
        let g = [(0.0, 0.0), (1.0e6, 1.0e-6), (2.0e6, 0.0)];
        let mut rows = vec![[92235.0, 233.0, 0.0, 0.0, 2.0, 0.0]]; // HEAD NK=2
        rows.extend(mf5_455_lf5_group(0.035, &g));
        rows.extend(mf5_455_lf5_group(0.18, &g));

        let chi = DelayedChi::from_rows(&rows).unwrap().unwrap();
        assert_eq!(chi.n_groups(), 2);
        for grp in &chi.groups {
            assert_eq!(grp.lf, 5);
            assert!(grp.spectrum.iter().all(|&(_, y)| y >= 0.0));
            // Triangle area = 0.5 * base(2e6) * height(1e-6) = 1.0.
            assert!((grp.normalization() - 1.0).abs() < 1e-6, "norm {}", grp.normalization());
        }
        assert!((chi.groups[0].fraction[0].1 - 0.035).abs() < 1e-9);
        assert!((chi.groups[1].fraction[0].1 - 0.18).abs() < 1e-9);
    }

    /// An unsupported LF (here LF=7) aborts the whole parse to `None` rather than
    /// return a fabricated spectrum.
    #[test]
    fn unsupported_lf_returns_none() {
        let mut rows = vec![[92235.0, 233.0, 0.0, 0.0, 1.0, 0.0]]; // NK=1
        // p_k TAB1 with LF=7.
        rows.push([0.0, 0.0, 0.0, 7.0, 1.0, 2.0]);
        rows.push([2.0, 2.0, 0.0, 0.0, 0.0, 0.0]);
        rows.push([1.0e-5, 1.0, 3.0e7, 1.0, 0.0, 0.0]);
        // θ(E) TAB1 that a LF=7 reader would consume (irrelevant; we bail first).
        rows.push([0.0, 0.0, 0.0, 0.0, 1.0, 2.0]);
        rows.push([2.0, 2.0, 0.0, 0.0, 0.0, 0.0]);
        rows.push([1.0e-5, 1.0e6, 3.0e7, 1.5e6, 0.0, 0.0]);
        assert!(DelayedChi::from_rows(&rows).unwrap().is_none());
    }
}
