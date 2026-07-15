// Ported from NJOY2016 `src/covr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! COVR user-input card model.
//!
//! This module is a *faithful, line-traceable* translation of the **input-card
//! reading** of NJOY2016's `subroutine covr` (`covr.f90:49-284`). COVR reads a
//! free-format card deck that selects between two mutually exclusive output
//! modes and lists the reaction (MAT/MT vs MAT1/MT1) pairs to process:
//!
//! - **Plot option** (`nout <= 0`) — produce VIEWR PostScript plots of the
//!   correlation matrix and the relative-standard-deviation vectors
//!   (`covr.f90:76-111`, cards 2, 2', 2a, 3a).
//! - **Library option** (`nout > 0`) — produce a condensed BOXER-format
//!   covariance/correlation library (`covr.f90:113-125`, cards 2b, 3b, 3c).
//!
//! Card 4 (the reaction selector) is common to both options (`covr.f90:127-147`).
//!
//! The card layout is modelled by [`CovrInput`] plus the [`CovrMode`] enum
//! (plot vs library) and its selector enums. Field names keep the upstream
//! Fortran identifiers in their doc comments so a discrepancy can be localised
//! to a specific `read(nsysi,*)` statement.
//!
//! # Units
//!
//! - Energies (`epmin`) are in **electron-volts (eV)** — COVR's group
//!   boundaries are eV, matching the ERRORR output tape.
//! - Unit numbers (`nin`, `nout`, `nplot`) are logical I/O handles, not
//!   physical quantities. A negative unit denotes a binary (unformatted) tape
//!   in NJOY, positive a formatted (BCD) tape, zero "absent".
//! - MAT/MT numbers are ENDF integer identifiers (dimensionless).

use crate::NjoyError;

// ---------------------------------------------------------------------------
// Selector enums
// ---------------------------------------------------------------------------

/// Correlation-matrix plot colour style (`icolor`, card 2, `covr.f90:79-94`).
///
/// Selects how the shaded contour plot of the correlation matrix is rendered.
/// Default [`ColorStyle::Monochrome`] (`icolor==0`, `covr.f90:197`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorStyle {
    /// `0`: monochrome — regions distinguished by cross-hatching.
    Monochrome,
    /// `1`: colour background and contours (default level set applies).
    Color,
    /// `2`: colour background and contours, plus a user-supplied level array
    /// on card 2' (`tlev`). See [`PlotOptions::shade_levels`].
    ColorWithLevels,
}

impl ColorStyle {
    /// Upstream integer code (`covr.f90:79-84`).
    pub fn to_icolor(self) -> i32 {
        match self {
            ColorStyle::Monochrome => 0,
            ColorStyle::Color => 1,
            ColorStyle::ColorWithLevels => 2,
        }
    }

    /// Parse the upstream `icolor` code. Returns `None` for out-of-range input.
    pub fn from_icolor(icolor: i32) -> Option<Self> {
        match icolor {
            0 => Some(ColorStyle::Monochrome),
            1 => Some(ColorStyle::Color),
            2 => Some(ColorStyle::ColorWithLevels),
            _ => None,
        }
    }
}

/// Whether the covariances on the input tape are absolute or relative
/// (`irelco`, card 3a, `covr.f90:98-99`).
///
/// In the plot option COVR converts absolute covariances to relative before
/// forming standard deviations; in the library option it does **not**
/// (`covr.f90:240-243`), so standard deviations track the ERRORR output.
/// Default [`CovarianceForm::Relative`] (`irelco==1`, `covr.f90:223`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CovarianceForm {
    /// `0`: absolute covariances (units of the observable squared).
    Absolute,
    /// `1`: relative covariances (dimensionless). Default.
    Relative,
}

impl CovarianceForm {
    /// Upstream integer code (`covr.f90:98-99`).
    pub fn to_irelco(self) -> i32 {
        match self {
            CovarianceForm::Absolute => 0,
            CovarianceForm::Relative => 1,
        }
    }

    /// Parse the upstream `irelco` code (anything != 0 is relative, matching
    /// `if (irelco.ne.1)` at `covr.f90:929`).
    pub fn from_irelco(irelco: i32) -> Self {
        if irelco == 0 {
            CovarianceForm::Absolute
        } else {
            CovarianceForm::Relative
        }
    }
}

/// Plot-legend option (`noleg`, card 3a, `covr.f90:103-106`).
///
/// Default [`LegendOption::AllPlots`] (`noleg==0`, `covr.f90:225`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegendOption {
    /// `-1`: legend for the first subcase only.
    FirstSubcaseOnly,
    /// `0`: legend for all plots. Default.
    AllPlots,
    /// `1`: no legends.
    None,
}

impl LegendOption {
    /// Upstream integer code (`covr.f90:104-106`).
    pub fn to_noleg(self) -> i32 {
        match self {
            LegendOption::FirstSubcaseOnly => -1,
            LegendOption::AllPlots => 0,
            LegendOption::None => 1,
        }
    }

    /// Parse the upstream `noleg` code. Returns `None` for out-of-range input.
    pub fn from_noleg(noleg: i32) -> Option<Self> {
        match noleg {
            -1 => Some(LegendOption::FirstSubcaseOnly),
            0 => Some(LegendOption::AllPlots),
            1 => Some(LegendOption::None),
            _ => None,
        }
    }
}

/// Output-library matrix option (`matype`, card 2b, `covr.f90:116-118`).
///
/// Selects whether the BOXER-format library holds covariances or correlations.
/// Default [`MatrixOutputType::Covariances`] (`matype==3`, `covr.f90:244-247`).
/// Any value other than 4 is coerced to 3 upstream (`covr.f90:247`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatrixOutputType {
    /// `3`: relative covariance matrices. Default.
    Covariances,
    /// `4`: correlation matrices.
    Correlations,
}

impl MatrixOutputType {
    /// Upstream integer code (`covr.f90:116-118`).
    pub fn to_matype(self) -> i32 {
        match self {
            MatrixOutputType::Covariances => 3,
            MatrixOutputType::Correlations => 4,
        }
    }

    /// Parse the upstream `matype` code (`if (matype.ne.4) matype=3`,
    /// `covr.f90:247`).
    pub fn from_matype(matype: i32) -> Self {
        if matype == 4 {
            MatrixOutputType::Correlations
        } else {
            MatrixOutputType::Covariances
        }
    }
}

// ---------------------------------------------------------------------------
// Card 4 — reaction selector
// ---------------------------------------------------------------------------

/// Card 4: a single reaction-pair selector (`covr.f90:127-147`, read at :259).
///
/// Fortran: `read(nsysi,*) imat(i),imt(i),imat1(i),imt1(i)`, repeated `ncase`
/// times. All four fields are ENDF integer identifiers (dimensionless).
///
/// Defaults (`covr.f90:132-135`): `mt`, `mat1`, `mt1` default to 0, meaning
/// "process all MTs for this MAT with `mat1==mat`".
///
/// Negative `mt`/`mat1`/`mt1` are *strip* directives, not literal MTs: `-n`
/// strips both `mt=1` and `mt=n` from the expanded MT list; `-4` strips
/// `mt=1`, `mt=3`, and `mt=4`; and e.g. `-62` strips `mt=1` and every MT from
/// 62 up to and including 90 (`covr.f90:137-143`). See [`is_mt_stripped`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReactionSelector {
    /// `imat` — desired MAT number. Ascending order required across cases when
    /// several materials appear on the input tape (`covr.f90:146-147`).
    pub mat: i32,
    /// `imt` — desired MT number (0 = all MTs; negative = strip directive).
    pub mt: i32,
    /// `imat1` — desired MAT1 number (0 or negative = same MAT as `mat`).
    pub mat1: i32,
    /// `imt1` — desired MT1 number (0 = same MT as `mt`; negative = strip).
    pub mt1: i32,
}

impl ReactionSelector {
    /// Construct a same-reaction (auto-covariance) selector: MAT1=MAT, MT1=MT.
    pub fn auto(mat: i32, mt: i32) -> Self {
        Self {
            mat,
            mt,
            mat1: mat,
            mt1: mt,
        }
    }

    /// True when this selector requests the MT-list *expansion* path
    /// (`if (imt(n).le.0) call expndo`, `covr.f90:369`): `mt <= 0`.
    pub fn expands_mt_list(&self) -> bool {
        self.mt <= 0
    }
}

/// MT-stripping predicate from `subroutine expndo` (`covr.f90:546-553`).
///
/// When card-4 `mt`/`mat1`/`mt1` are negative, COVR negates them into a
/// three-element strip list `mstrip = [-mt, -mat1, -mt1]` (`covr.f90:529-531`)
/// and, while enumerating the MTs actually present on the tape, drops any MT
/// that matches a stripper. This function reproduces that per-MT test.
///
/// Rules (for each active — nonzero — stripper `s` in `mstrip`):
/// - `mtn == 1` is always stripped (once any stripper is active).
/// - `mtn == s` is stripped.
/// - `mtn == 3` is stripped when `s == 4` (total-inelastic special case).
/// - for `s` in the discrete-level band `51..=90`, every `mtn` with
///   `s < mtn <= 90` is stripped.
///
/// Returns `true` if `mtn` should be removed from the expanded MT list.
///
/// `mtn` and `mstrip` entries are ENDF MT numbers (dimensionless).
pub fn is_mt_stripped(mtn: i32, mstrip: [i32; 3]) -> bool {
    for &s in &mstrip {
        // covr.f90:547 — inactive stripper, skip.
        if s == 0 {
            continue;
        }
        // covr.f90:548 — mt=1 always stripped once a stripper is active.
        if mtn == 1 {
            return true;
        }
        // covr.f90:549 — exact match.
        if mtn == s {
            return true;
        }
        // covr.f90:550 — total inelastic (mt=3) stripped with mt=4.
        if mtn == 3 && s == 4 {
            return true;
        }
        // covr.f90:551 — only discrete-level strippers do range stripping.
        if s < 51 || s > 90 {
            continue;
        }
        // covr.f90:552 — strip the level band above s up to 90.
        if mtn > s && mtn <= 90 {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Plot-option parameters (nout <= 0)
// ---------------------------------------------------------------------------

/// Parameters for the **plot option** (`nout <= 0`, `covr.f90:76-111`).
///
/// Groups cards 2, 2', 2a, and 3a. Used to drive the VIEWR PostScript plots
/// (which are **not** ported — see the module `run()` skeleton).
#[derive(Debug, Clone, PartialEq)]
pub struct PlotOptions {
    /// `icolor` — colour style (card 2, `covr.f90:79-84`).
    pub color: ColorStyle,
    /// `tlev(1..nlev)` — correlation-matrix contour level boundaries
    /// (card 2', `covr.f90:85-94`). Dimensionless correlation thresholds in
    /// `(0, 1]`, strictly increasing, with the last equal to 1.0. When
    /// [`ColorStyle::ColorWithLevels`] is *not* selected the default set
    /// `[0.001, 0.1, 0.2, 0.3, 0.6, 1.0]` applies (`covr.f90:161-162`).
    pub tlev: Vec<f64>,
    /// `epmin` — lowest plotted energy, in **eV** (card 2a, `covr.f90:95-96`).
    /// Stored as the user value; upstream multiplies by `rdn=0.999998` at read
    /// time (`covr.f90:222`) — see [`PlotOptions::epmin_scaled`].
    pub epmin: f64,
    /// `irelco` — absolute/relative covariance form (card 3a, `covr.f90:98`).
    pub covariance_form: CovarianceForm,
    /// `noleg` — legend option (card 3a, `covr.f90:103`).
    pub legend: LegendOption,
    /// `nstart` — sequential figure number; 0 omits figure numbers
    /// (card 3a, `covr.f90:107-109`).
    pub nstart: i32,
    /// `ndiv` — number of subdivisions of each grey shade; coerced to >= 1
    /// (card 3a, `covr.f90:110-111`, `:229`).
    pub ndiv: i32,
}

impl Default for PlotOptions {
    /// NJOY plot-option defaults (`covr.f90:197,220-227`).
    fn default() -> Self {
        Self {
            color: ColorStyle::Monochrome,
            tlev: default_tlev(),
            epmin: 0.0,
            covariance_form: CovarianceForm::Relative,
            legend: LegendOption::AllPlots,
            nstart: 1,
            ndiv: 1,
        }
    }
}

/// Default correlation contour boundaries `tlev` (`covr.f90:161-162`):
/// `[0.001, 0.1, 0.2, 0.3, 0.6, 1.0]` (nlev = 6). Dimensionless.
pub fn default_tlev() -> Vec<f64> {
    vec![0.001, 0.1, 0.2, 0.3, 0.6, 1.0]
}

/// Reduction factor applied to `epmin` at read time (`rdn`, `covr.f90:164`).
pub const EPMIN_READ_FACTOR: f64 = 0.999_998;

impl PlotOptions {
    /// `epmin` after the upstream read-time scaling by `rdn` (`covr.f90:222`).
    /// In eV.
    pub fn epmin_scaled(&self) -> f64 {
        self.epmin * EPMIN_READ_FACTOR
    }

    /// Build the expanded shade-level array `xlev` (`covr.f90:289-305`).
    ///
    /// Each of the `nlev = tlev.len()` contour intervals is subdivided into
    /// `ndiv` equal sub-levels between the previous boundary and `tlev[i]`,
    /// giving `nlev * ndiv` monotonically increasing thresholds; a tiny
    /// `da = 0.001` bump is added to the topmost level so a correlation of
    /// exactly 1.0 falls inside the last band (`covr.f90:302-304`).
    ///
    /// Returned values are dimensionless correlation thresholds. The
    /// significance test in the plot path uses `xlev[0]` (`covr.f90:683`).
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] wrapping a descriptive message when `tlev` is
    /// empty or not strictly increasing — mirroring the upstream validation at
    /// `covr.f90:206-212`. A non-positive `ndiv` is coerced to 1
    /// (`covr.f90:229`) rather than rejected.
    pub fn shade_levels(&self) -> Result<Vec<f64>, NjoyError> {
        if self.tlev.is_empty() {
            return Err(NjoyError::EndfParse("covr: tlev array is empty".into()));
        }
        let ndiv = if self.ndiv <= 0 { 1 } else { self.ndiv };
        // covr.f90:206-212 — tlev must strictly increase.
        for w in self.tlev.windows(2) {
            if w[1] <= w[0] {
                return Err(NjoyError::EndfParse(
                    "covr: tlev array must sequentially increase".into(),
                ));
            }
        }
        let ndiv_f = ndiv as f64;
        let mut xlev = Vec::with_capacity(self.tlev.len() * ndiv as usize);
        let mut tlow = 0.0_f64;
        for &t in &self.tlev {
            let tx = (t - tlow) / ndiv_f;
            for j in 1..=ndiv {
                xlev.push(tlow + tx * j as f64);
            }
            tlow = t;
        }
        // covr.f90:302-304 — bump the topmost level by da = 1/1000.
        if let Some(last) = xlev.last_mut() {
            *last += 1.0 / 1000.0;
        }
        Ok(xlev)
    }
}

// ---------------------------------------------------------------------------
// Library-option parameters (nout > 0)
// ---------------------------------------------------------------------------

/// Parameters for the **library option** (`nout > 0`, `covr.f90:113-125`).
///
/// Groups cards 2b, 3b, and 3c. Drives the condensed BOXER-format output
/// library (whose writer, `subroutine press`, is **not** ported).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryOptions {
    /// `matype` — covariances (3) or correlations (4) (card 2b, `covr.f90:116`).
    pub matrix_type: MatrixOutputType,
    /// `hlibid` / `hinpid` — up to 6 characters of library identification
    /// (card 3b, `covr.f90:121-123`).
    pub lib_id: String,
    /// `hdescr` — up to 21 characters of descriptive text
    /// (card 3c, `covr.f90:123-125`).
    pub description: String,
}

impl Default for LibraryOptions {
    /// NJOY library-option defaults (`covr.f90:244-253`).
    fn default() -> Self {
        Self {
            matrix_type: MatrixOutputType::Covariances,
            lib_id: String::new(),
            description: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Output-mode dispatch enum
// ---------------------------------------------------------------------------

/// The two mutually exclusive COVR output modes (`covr.f90:63-65,186`).
///
/// Selected by the sign of `nout` on card 1: `nout <= 0` is the plot option,
/// `nout > 0` is the library option. Modelled as an enum for exhaustive
/// dispatch (no trait objects).
#[derive(Debug, Clone, PartialEq)]
pub enum CovrMode {
    /// Plot option (`nout <= 0`): VIEWR PostScript figures.
    Plot(PlotOptions),
    /// Library option (`nout > 0`): condensed BOXER-format library.
    Library(LibraryOptions),
}

// ---------------------------------------------------------------------------
// Top-level card deck
// ---------------------------------------------------------------------------

/// Maximum number of cases (`ncasemx`, `covr.f90:29`). The plot path further
/// limits the effective count to 60 in the manual, but shares this array cap.
pub const NCASE_MAX: usize = 100;

/// The full COVR input card deck (`covr.f90:69-284`).
///
/// Card 1 assigns the three logical unit numbers; [`CovrMode`] carries the
/// mode-specific cards; and [`CovrInput::cases`] is the card-4 list of
/// reaction selectors (up to [`NCASE_MAX`]).
#[derive(Debug, Clone, PartialEq)]
pub struct CovrInput {
    /// `nin` — input covariance tape unit (the ERRORR output tape). A negative
    /// value denotes a binary tape (`covr.f90:70`).
    pub nin: i32,
    /// `nout` — output covariance-library tape unit; `<= 0` selects the plot
    /// option, `> 0` the library option (`covr.f90:71`, default 0).
    pub nout: i32,
    /// `nplot` — VIEWR plot output unit; 0 = none (`covr.f90:73-74`, default 0).
    pub nplot: i32,
    /// Mode-specific parameters (plot vs library).
    pub mode: CovrMode,
    /// Card-4 reaction selectors, one per case (`covr.f90:255-260`).
    pub cases: Vec<ReactionSelector>,
}

impl CovrInput {
    /// Number of cases to run (`ncase`, `covr.f90:101,119`). Equal to
    /// `cases.len()`.
    pub fn ncase(&self) -> usize {
        self.cases.len()
    }

    /// True when this deck uses the plot option (`nout <= 0`).
    pub fn is_plot_option(&self) -> bool {
        matches!(self.mode, CovrMode::Plot(_))
    }

    /// Validate the card counts against the upstream limits.
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] when the case list exceeds [`NCASE_MAX`]
    /// (`covr.f90:230-231,249-250`) or is empty (`ncase` defaults to 1 upstream,
    /// so at least one card 4 is required).
    pub fn validate(&self) -> Result<(), NjoyError> {
        if self.cases.is_empty() {
            return Err(NjoyError::EndfParse(
                "covr: at least one card-4 reaction selector is required".into(),
            ));
        }
        if self.cases.len() > NCASE_MAX {
            return Err(NjoyError::EndfParse(format!(
                "covr: requested too many cases ({} > {NCASE_MAX})",
                self.cases.len()
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: exercise the round-trip integer encoders/decoders for every
    /// selector enum against the exact upstream codes at the cited lines.
    /// Result (2026-07-15): all codes round-trip; out-of-range returns None.
    #[test]
    fn selector_codes_round_trip() {
        // covr.f90:79-84
        for (c, code) in [
            (ColorStyle::Monochrome, 0),
            (ColorStyle::Color, 1),
            (ColorStyle::ColorWithLevels, 2),
        ] {
            assert_eq!(c.to_icolor(), code);
            assert_eq!(ColorStyle::from_icolor(code), Some(c));
        }
        assert_eq!(ColorStyle::from_icolor(7), None);

        // covr.f90:98-99, :929 (anything != 0 is relative)
        assert_eq!(CovarianceForm::from_irelco(0), CovarianceForm::Absolute);
        assert_eq!(CovarianceForm::from_irelco(1), CovarianceForm::Relative);
        assert_eq!(CovarianceForm::from_irelco(7), CovarianceForm::Relative);

        // covr.f90:104-106
        for (l, code) in [
            (LegendOption::FirstSubcaseOnly, -1),
            (LegendOption::AllPlots, 0),
            (LegendOption::None, 1),
        ] {
            assert_eq!(l.to_noleg(), code);
            assert_eq!(LegendOption::from_noleg(code), Some(l));
        }
        assert_eq!(LegendOption::from_noleg(5), None);

        // covr.f90:116-118, :247 (anything != 4 is covariances)
        assert_eq!(
            MatrixOutputType::from_matype(4),
            MatrixOutputType::Correlations
        );
        assert_eq!(
            MatrixOutputType::from_matype(3),
            MatrixOutputType::Covariances
        );
        assert_eq!(
            MatrixOutputType::from_matype(9),
            MatrixOutputType::Covariances
        );
    }

    /// Methodology: reproduce the shade-level expansion of `covr.f90:289-305`
    /// by hand for the default `tlev = [0.001, 0.1, 0.2, 0.3, 0.6, 1.0]` with
    /// `ndiv = 1`. With ndiv=1 each level maps to itself, and the topmost is
    /// bumped by da = 0.001, so xlev = [0.001, 0.1, 0.2, 0.3, 0.6, 1.001].
    /// Result (2026-07-15): matches to 1e-12; length 6.
    #[test]
    fn shade_levels_ndiv1() {
        let opts = PlotOptions::default();
        let xlev = opts.shade_levels().unwrap();
        let expect = [0.001, 0.1, 0.2, 0.3, 0.6, 1.0 + 0.001];
        assert_eq!(xlev.len(), expect.len());
        for (a, b) in xlev.iter().zip(expect.iter()) {
            assert!((a - b).abs() < 1e-12, "xlev {a} vs {b}");
        }
    }

    /// Methodology: shade-level expansion with `ndiv = 2` subdivides each
    /// interval `[tlow, tlev[i]]` into 2 equal steps. For tlev=[0.2, 0.6]:
    /// interval 1 = [0,0.2] -> {0.1, 0.2}; interval 2 = [0.2,0.6] -> {0.4, 0.6};
    /// then the last is bumped by 0.001 -> {0.1, 0.2, 0.4, 0.601}.
    /// Result (2026-07-15): matches to 1e-12; length 4 = nlev*ndiv.
    #[test]
    fn shade_levels_ndiv2() {
        let opts = PlotOptions {
            tlev: vec![0.2, 0.6],
            ndiv: 2,
            ..PlotOptions::default()
        };
        let xlev = opts.shade_levels().unwrap();
        let expect = [0.1, 0.2, 0.4, 0.6 + 0.001];
        assert_eq!(xlev.len(), 4);
        for (a, b) in xlev.iter().zip(expect.iter()) {
            assert!((a - b).abs() < 1e-12, "xlev {a} vs {b}");
        }
    }

    /// Methodology: a non-increasing tlev must be rejected (`covr.f90:206-212`).
    #[test]
    fn shade_levels_rejects_non_increasing() {
        let opts = PlotOptions {
            tlev: vec![0.2, 0.2, 0.6],
            ..PlotOptions::default()
        };
        assert!(opts.shade_levels().is_err());
    }

    /// Methodology: encode the worked examples from the card-4 documentation
    /// (`covr.f90:137-143`) as strip lists and check `is_mt_stripped`.
    ///  - `-4`  -> mstrip=[4,0,0]: strips mt=1, mt=3, mt=4; keeps mt=2, mt=102.
    ///  - `-62` -> mstrip=[62,0,0]: strips mt=1 and mt=62..90; keeps mt=61, mt=91.
    ///  - `-102`-> mstrip=[102,0,0]: strips mt=1 and mt=102; keeps mt=103 (not
    ///    a level band, so no range strip).
    /// Result (2026-07-15): all match the documented behaviour.
    #[test]
    fn mt_strip_examples() {
        // -4 case
        let s4 = [4, 0, 0];
        assert!(is_mt_stripped(1, s4));
        assert!(is_mt_stripped(3, s4));
        assert!(is_mt_stripped(4, s4));
        assert!(!is_mt_stripped(2, s4));
        assert!(!is_mt_stripped(102, s4));

        // -62 case: strips 1 and 62..=90
        let s62 = [62, 0, 0];
        assert!(is_mt_stripped(1, s62));
        assert!(is_mt_stripped(62, s62));
        assert!(is_mt_stripped(75, s62));
        assert!(is_mt_stripped(90, s62));
        assert!(!is_mt_stripped(61, s62));
        assert!(!is_mt_stripped(91, s62));

        // -102 case: exact match only (not in 51..=90 band)
        let s102 = [102, 0, 0];
        assert!(is_mt_stripped(1, s102));
        assert!(is_mt_stripped(102, s102));
        assert!(!is_mt_stripped(103, s102));

        // no active stripper -> nothing stripped, not even mt=1
        assert!(!is_mt_stripped(1, [0, 0, 0]));
    }

    /// Methodology: check the top-level deck helpers and validation limits.
    /// Result (2026-07-15): ncase counts cases; empty and >100 rejected.
    #[test]
    fn input_validation() {
        let mut input = CovrInput {
            nin: 20,
            nout: 0,
            nplot: 0,
            mode: CovrMode::Plot(PlotOptions::default()),
            cases: vec![ReactionSelector::auto(9228, 102)],
        };
        assert!(input.is_plot_option());
        assert_eq!(input.ncase(), 1);
        assert!(input.validate().is_ok());
        assert!(!input.cases[0].expands_mt_list());

        input.cases.clear();
        assert!(input.validate().is_err());

        input.cases = (0..=NCASE_MAX)
            .map(|i| ReactionSelector::auto(9228, i as i32))
            .collect();
        assert!(input.validate().is_err());
    }
}
