// Translated from NJOY2016.80 `src/groupr.f90`: `panel` (:5858-6091), `getwtf`
// (:5115-5307, `iwt` 2 and 3), `getflx` (:6439, both branches since
// 2026-11-13), `getsig` (:6646, with `getunr` for `nz > 1`), `getff` (:7038, the
// `mfd = 3` vector case, label 100) and `displa` (:6093, labels 250, 310 and the
// `nz = 1` matrix branch).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **A faithful `panel`** (~~for the vector case, `nl = nz = 1`, `mfd = 3`~~
//! **CORRECTED 2026-11-13, gh:#1369:** for every `nl` and `nz`; `nz > 1` takes its
//! flux from [`FluxSource::Table`], `genflx`'s table), beside the trapezoid
//! [`super::panel::group_integral`] (kept; this does not replace it).
//!
//! What `group_integral` omits and this keeps: the `rndoff = 1.000002` nudge of
//! a panel's lower edge and the `delta = 0.999995` nudge of its upper edge (so
//! the cross section is sampled just inside a discontinuity, `idisc`),
//! `sigfig(eq, 9, 0)` on the quadrature abscissae, the flux integral as the
//! trapezoid of the flux at the nudged edges, the Lobatto order `nq` (2, 6 or
//! 10; for a vector `getff` returns `nq = 0`, so `nq = 2` throughout), the
//! "zero cross section over the whole panel" reset, the thermal-group removal
//! for MT 221-250, and `displa`'s `sigfig(.,7,0)` rounding of flux and ratio.

use crate::endf::gety1::Gety1;
use crate::mixr::mix::sigfig;
use crate::NjoyError;
use petir::mathf::RealMath;

const RNDOFF: f64 = 1.000002;
const DELTA: f64 = 0.999995;
const EMAX: f64 = 1.0e10;
const SMALL: f64 = 1.0e-10;
const SMIN: f64 = 1.0e-9;
const TTEST: f64 = 0.1;

const QP6: [f64; 6] = [-1.0, -0.76505532, -0.28523152, 0.28523152, 0.76505532, 1.0];
const QW6: [f64; 6] = [0.06666667, 0.37847496, 0.55485838, 0.55485838, 0.37847496, 0.06666667];
const QP10: [f64; 10] = [
    -1.0, -0.9195339082, -0.7387738651, -0.4779249498, -0.1652789577, 0.1652789577, 0.4779249498, 0.7387738651,
    0.9195339082, 1.0,
];
const QW10: [f64; 10] = [
    0.0222222222, 0.1333059908, 0.2248893420, 0.2920426836, 0.3275397612, 0.3275397612, 0.2920426836, 0.2248893420,
    0.1333059908, 0.0222222222,
];

/// The `save`d state of `getwtf`'s tabulated branch (`save ip,ir,ipl,step`, `:5173`).
/// ~~A struct here~~ Since 2026-11-13 (gh:#1370) the one in [`crate::groupr::genwtf`],
/// which `iwt = 1` and `10` update too.
type WtfState = crate::groupr::genwtf::TabState;

thread_local! {
    static WTF: std::cell::RefCell<WtfState> = const { std::cell::RefCell::new(WtfState { ip: 2, ir: 1, ipl: 0, step: 1.10 }) };
}

thread_local! {
    /// `wght(1..6)` as `genwtf` sets it for `iwt = 4` (`:5036-5052`): `eb, tb, ab, ec, tc,
    /// ac`. `None` until [`genwtf_iwt4`] runs.
    static WGHT4: std::cell::Cell<Option<[f64; 6]>> = const { std::cell::Cell::new(None) };
}

/// `genwtf` for `iwt = 4` (`groupr.f90:5036-5052`): store `wght(1..6) = eb, tb, ab, ec,
/// tc, ac` for [`getwtf`], with `ab, ac` from
/// [`crate::groupr::weights::ThermalFissionParams::normalisation`] (`ab = 1, ac = 0`
/// when `eb > 50 tb`).
pub fn genwtf_iwt4(p: &crate::groupr::weights::ThermalFissionParams) {
    let (ab, ac) = p.normalisation();
    WGHT4.with(|w| w.set(Some([p.thermal_break_ev, p.thermal_temp_ev, ab, p.fission_break_ev, p.fission_temp_ev, ac])));
}

/// `getwtf(e = 0)`: reset the saved tabulated-weight state (`:5177-5184`).
pub fn getwtf_init() {
    WTF.with(|w| *w.borrow_mut() = WtfState { ip: 2, ir: 1, ipl: 0, step: 1.10 });
}

/// `getwtf` for `iwt = 2` (constant), `iwt = 3` (`1/E`), `iwt = 4` (thermal + `1/E` +
/// fission, after [`genwtf_iwt4`]), `iwt = 6` (thermal -- `1/E` -- fission + fusion;
/// both since 2026-11-13, gh:#1369 round 5) and the built-in
/// tabulated weights `iwt = 5, 8, 9` (`terpa` over `wght`, `:5186-5196`):
/// `(wtf, enext, idis)`. **Since 2026-11-13 (gh:#1370)** also `iwt = 1` (card 8b, after
/// [`crate::groupr::genwtf::genwtf_iwt1`]), `iwt = 7` and `12` (at `temp(jtemp)`,
/// [`crate::groupr::genwtf::getwtf_set_temperature`]),
/// `iwt = 10` (CLAW with a thermal Maxwellian below `0.15 T/300`) and `iwt = 11`.
///
/// # Errors
/// [`NjoyError::NotPorted`] for `iwt = 0`, where upstream's `getwtf` sets no `wtf`;
/// [`NjoyError::InvalidInput`] for `iwt = 4` before [`genwtf_iwt4`], `iwt = 1` before
/// `genwtf_iwt1`, or `iwt = 7, 10, 12` before `getwtf_set_temperature`.
pub fn getwtf(iwt: i32, e: f64) -> Result<(f64, f64, i32), NjoyError> {
    if e == 0.0 {
        getwtf_init();
        return Ok((0.0, EMAX, 0));
    }
    let mut idis = 0;
    let (wtf, mut enext) = match iwt.abs() {
        2 => (1.0, EMAX),
        3 => (1.0 / e, EMAX),
        n @ (5 | 8 | 9) => {
            let tab = match n {
                5 => crate::groupr::weights::epri_cell_lwr_tab1(),
                8 => crate::groupr::weights::fast_reactor_tab1(),
                _ => crate::groupr::weights::claw_tab1(),
            };
            WTF.with(|w| -> Result<(f64, f64), NjoyError> {
                let mut st = w.borrow_mut();
                let (mut ip, mut ir) = (st.ip, st.ir);
                let (wtf, mut enext, id) = crate::errorr::weight::terpa(&tab, e, &mut ip, &mut ir)?;
                st.ip = ip;
                st.ir = ir;
                idis = id;
                if wtf != 0.0 {
                    if st.ip != st.ipl {
                        st.step = 1.0001 * (enext / e).r_powf(0.1);
                        st.ipl = st.ip;
                    }
                    let mut enxt = st.step * e;
                    if enxt > 1.01 * e {
                        enxt = 1.01 * e;
                    }
                    if enext > enxt {
                        idis = 0;
                        enext = enxt;
                    }
                }
                Ok((wtf, enext))
            })?
        }
        4 => match WGHT4.with(std::cell::Cell::get) {
            Some(w) => crate::groupr::weights::getwtf_iwt4(&w, e),
            None => return Err(NjoyError::InvalidInput("groupr getwtf: iwt = 4 without genwtf's card 8c (eb, tb, ec, tc)".into())),
        },
        6 => crate::groupr::weights::getwtf_iwt6(e),
        // gh:#1370: `iwt = 1` (card 8b), 7, 10, 11 and 12 (`:5186-5300`).
        n @ (1 | 10) => {
            let (wtf, enext, id) = WTF.with(|w| crate::groupr::genwtf::getwtf_iwt1_or_10(&mut w.borrow_mut(), n, e))?;
            idis = id;
            (wtf, enext)
        }
        7 => crate::groupr::weights::getwtf_iwt67(7, e, crate::groupr::genwtf::temp_jtemp()?),
        11 => crate::groupr::weights::getwtf_iwt1112(11, e, 0.0),
        12 => crate::groupr::weights::getwtf_iwt1112(12, e, crate::groupr::genwtf::temp_jtemp()?),
        _ => return Err(NjoyError::NotPorted("groupr getwtf for iwt = 0 (wtf is never set upstream) in the tape driver")),
    };
    if matches!(iwt.abs(), 2 | 3) {
        enext = 1.01 * e;
    }
    Ok((wtf, sigfig(enext, 6, 1), idis))
}

/// The cross-section feeder, `getsig` on a PENDF MF=3 TAB1 (`gety1`).
#[derive(Debug, Clone)]
pub struct SigFeed {
    /// The PENDF TAB1 being read; `None` for the non-cross-section quantities
    /// `MT = 257-259` (`getsig` `:6684-6690`), which have no section.
    gety: Option<Gety1>,
    /// `mtd` of the reaction (for the thermal-group removal).
    pub mtd: i32,
    /// `mt`, the section `getsig` read (`getunr`'s reaction).
    mt: i32,
    /// `stounr`'s table and the run's `sigz`, for `getunr` when `nz > 1`.
    unr: Option<(std::sync::Arc<crate::groupr::unresolved::UnresolvedTable>, Vec<f64>)>,
}

impl SigFeed {
    /// `getsig(e = 0)`: returns the feeder and `first`, the first energy.
    pub fn new(tab: &crate::endf::records::Tab1, mtd: i32) -> (Self, f64) {
        let mut g = Gety1::new(tab);
        let first = g.get(0.0).xnext;
        (Self { gety: Some(g), mtd, mt: mtd, unr: None }, first)
    }
    /// `getsig(e = 0)` for `MT = 257` (average energy), `258` (average lethargy)
    /// or `259` (average reciprocal velocity), `groupr.f90:6684-6690`: no PENDF
    /// section is read, `enext = emin = 1e-5` and `lrflag = 0`.
    pub fn non_cross_section(mtd: i32) -> (Self, f64) {
        (Self { gety: None, mtd, mt: mtd, unr: None }, 1.0e-5)
    }
    /// `getsig(e)`: `(sigma, enext, idis)`.
    ///
    /// For `MT = 257-259` (`:6762-6776`): `idis = 0`, `enext = 1.01 e`, and
    /// `e`, `log(1e7/e)` or `1/sqrt(1.919e8 e)` in place of a cross section.
    pub fn get(&mut self, e: f64) -> (f64, f64, i32) {
        match &mut self.gety {
            Some(g) => {
                let v = g.get(e);
                (v.y, v.xnext, i32::from(v.idis))
            }
            None => {
                const STEP: f64 = 1.01;
                let s = match self.mtd {
                    259 => 1.0 / (1.919e8 * e).sqrt(),
                    258 => petir::mathf::RealMath::r_ln(1.0e7 / e),
                    _ => e,
                };
                (s, STEP * e, 0)
            }
        }
    }

    /// `getsig(e)` into `sig(nz, nl)`, stored `[iz * nl + il]` (`:6777-6797`):
    /// the `gety1` value for every `(iz, il)`, then, when `nz > 1`, `getunr` on
    /// each `il` (MT=261 for MT=1's `il > 1`). Returns `(enext, idis)`; when the
    /// last `getunr` finds `e` outside its range (`en < 0`) both stay as `gety1`
    /// gave them.
    ///
    /// # Errors
    /// From `getunr`.
    pub fn get_nz(&mut self, e: f64, nl: usize, nz: usize, out: &mut [f64], xtot: &mut f64) -> Result<(f64, i32), NjoyError> {
        let (s, mut enext, mut idis) = self.get(e);
        for v in out.iter_mut().take(nl * nz) {
            *v = s;
        }
        if nz <= 1 || self.gety.is_none() {
            return Ok((enext, idis));
        }
        let mut en = enext;
        for il in 0..nl {
            let mtt = if il > 0 && self.mt == 1 { 261 } else { self.mt };
            let mut col: Vec<f64> = (0..nz).map(|iz| out[iz * nl + il]).collect();
            en = match &self.unr {
                Some((unr, sigz)) => crate::groupr::flux_table::getunr(Some(unr.as_ref()), sigz, mtt, e, &mut col, xtot)?,
                None => -1.0,
            };
            for iz in 0..nz {
                out[iz * nl + il] = col[iz];
            }
        }
        if en < 0.0 {
            return Ok((enext, idis));
        }
        if en < enext {
            idis = 0;
            enext = en;
        }
        Ok((enext, idis))
    }

    /// The `stounr` table and the run's `sigz` for `getunr` (`nz > 1`), and the
    /// MT `getsig` read (`mt`, `:6692-6713`).
    #[must_use]
    pub fn with_unr(mut self, unr: Option<std::sync::Arc<crate::groupr::unresolved::UnresolvedTable>>, sigz: &[f64], mt: i32) -> Self {
        self.unr = unr.map(|u| (u, sigz.to_vec()));
        self.mt = mt;
        self
    }
}

/// [`FeedOut::idisc`] when `getff` does not assign its `idisc` argument
/// (`gam102`, `:10866-10918`): `panel`'s `idiscf` keeps `getflx`'s value.
pub const IDISC_UNSET: i32 = i32::MIN;

/// What `getff` hands `panel` at one energy (`nl = 1`).
#[derive(Debug, Clone)]
pub struct FeedOut {
    /// `en`: next energy at which the feed changes.
    pub en: f64,
    /// `idisc`: `en` is a discontinuity.
    pub idisc: i32,
    /// `nq`: quadrature hint.
    pub nq: usize,
    /// `ff(il, 1..ng)` as `ff[il][ig]`.
    pub ff: Vec<Vec<f64>>,
    /// `iglo`: first final group of `ff` (`-1` marks the yield-only feed below `econst`).
    pub iglo: i32,
}

/// `getff` for the reactions this driver writes: the vector case (label 100:
/// `ff = 1`, `en = emax`, `nq = 0`) or the File-4 two-body feed (label 400,
/// `getdis`), including `MT = 251-253` (`mubar`, `xi`, `gamma`).
#[derive(Debug, Clone)]
pub enum PanelFeed {
    /// Self-shielded group cross sections (`mfd = 3`).
    Vector,
    /// `getdis`.
    TwoBody(crate::groupr::two_body::TwoBodyFeed),
    /// `getdis` for `nl` Legendre orders (`mfd = 6`, label 400).
    TwoBodyNl(crate::groupr::two_body::TwoBodyFeed, usize),
    /// `getff` label 200: `getfle` x `getsed` continuum (`MT = 91` etc.).
    Continuum(crate::groupr::continuum::ContinuumFeed),
    /// `getff` label 700: `getaed` thermal angle-energy (`MT = 221-250`).
    Thermal(crate::groupr::thermal::ThermalFeed),
    /// `getff` label 300: photon production (`mfd = 16/17`).
    Photon(crate::groupr::photon_prod::PhotonFeed),
    /// `getff` label 100 for a production reaction (`mfd` in `[20 000 000,
    /// 40 000 000)`): `ff = yld`, the product yield `getyld` interpolates with
    /// `terpa` in its packed TAB1 (`:7083-7090`, `:6625-6629`). Holds the
    /// packed TAB1 and `terpa`'s saved `ip`, `ir`.
    Yield(Vec<f64>, usize, usize),
    /// `getff` labels 800/810: `getmf6` on the MF=6 subsections of one emitted
    /// particle (`mfd = 8` neutrons, `mfd = 18` photons), over the secondary
    /// group boundaries `eg` (`egn` or `egg`) for `nl` Legendre orders.
    Mf6(crate::groupr::mf6_feed::Mf6Feed, Vec<f64>, usize),
    /// `getff` label 100 for `MT = 452, 455, 456` (`:7076-7095`): `ng = 2`,
    /// `ff = (yld, 1)` with `yld` from `getyld` on MF=1, for `displa`'s ratio form.
    Nu(crate::groupr::spectrum::NuYield),
    /// `getff` label 810 when the MF=6 photon is a LAW=2 line (`disc102`,
    /// `getmf6` `:7618-7623`, `:7977`): `gam102`.
    Gam102(Gam102),
}

/// `gam102` (`groupr.f90:10866-10918`), `law = 2`: the relativistic discrete
/// capture gamma of ENDF/B-VII+ H-1 MF=6/MT=102, put whole in the gamma group
/// holding `edis = disc102 + ed awr/(awr+1)`.
#[derive(Debug, Clone)]
pub struct Gam102 {
    /// `disc102`, the photon subsection's `AWP` (the line energy) \[eV\].
    pub disc102: f64,
    /// `awr` (`getsig`: the MF=3 head's C2 over `awrp`).
    pub awr: f64,
    /// `egg(1..ngg+1)`.
    pub egg: Vec<f64>,
    /// `nl`.
    pub nl: usize,
    /// `iglo`, which `gam102` assigns only when `edis` falls in a group;
    /// otherwise the caller's value stands.
    pub iglo: usize,
}

impl Gam102 {
    fn at(&mut self, ed: f64) -> FeedOut {
        const BIG: f64 = 1.0e10;
        let ngg = self.egg.len() - 1;
        let edis = self.disc102 + ed * self.awr / (self.awr + 1.0);
        for i in 1..=ngg {
            if edis >= self.egg[i - 1] && edis < self.egg[i] {
                self.iglo = i;
            }
        }
        let mut ff = vec![vec![0.0]; self.nl];
        ff[0][0] = 1.0;
        let il = self.iglo;
        // `ed < egg(iglo+1)` etc., with `iglo` 1-based (`egg(iglo)` is `self.egg[il-1]`).
        let en = if il >= 1 && ed < self.egg[il] {
            self.egg[il]
        } else if il >= 1 && ed < self.egg[il - 1] {
            self.egg[il - 1]
        } else {
            BIG
        };
        FeedOut { en, idisc: IDISC_UNSET, nq: 0, ff, iglo: il as i32 }
    }
}

impl PanelFeed {
    /// Number of final gamma groups the feed spans (0 for neutron feeds).
    fn ngg(&self) -> usize {
        match self {
            PanelFeed::Photon(f) => f.ngg(),
            PanelFeed::Mf6(_, eg, _) => eg.len() - 1,
            PanelFeed::Gam102(g) => g.egg.len() - 1,
            _ => 0,
        }
    }

    /// `getff(e)`.
    ///
    /// # Errors
    /// From `getdis`.
    pub fn at(&mut self, e: f64) -> Result<FeedOut, NjoyError> {
        match self {
            PanelFeed::Vector => Ok(FeedOut { en: EMAX, idisc: 0, nq: 0, ff: vec![vec![1.0]], iglo: 1 }),
            PanelFeed::TwoBody(f) => {
                let a = f.feed(e, 1)?;
                let mut ff: Vec<f64> = a.ff[0].clone();
                ff.truncate(a.ng);
                Ok(FeedOut { en: a.enext, idisc: i32::from(a.idisc), nq: a.nq, ff: vec![ff], iglo: a.iglo as i32 })
            }
            PanelFeed::Continuum(f) => f.at(e),
            PanelFeed::Thermal(f) => f.at(e),
            PanelFeed::Photon(f) => f.at(e),
            PanelFeed::Mf6(f, eg, nl) => {
                // `getmf6` fills every secondary group from `iglo = 1` (`:8084-8086`).
                let a = f.feed(e, eg, *nl)?;
                let ff: Vec<Vec<f64>> = a.ans.iter().map(|r| r[..a.ng2].to_vec()).collect();
                Ok(FeedOut { en: a.enext, idisc: a.idisc, nq: a.nq, ff, iglo: a.iglo as i32 })
            }
            PanelFeed::Gam102(g) => Ok(g.at(e)),
            PanelFeed::Yield(t, ip, ir) => {
                // label 100: `enext = emax`, `idisc = 0`, then `getyld`'s break.
                let (yld, en, idis) = crate::errorr::weight::terpa(t, e, ip, ir)?;
                let (mut enext, mut idisc) = (EMAX, 0);
                if en < enext * (1.0 - SMALL) {
                    idisc = idis;
                    enext = en;
                }
                Ok(FeedOut { en: enext, idisc, nq: 0, ff: vec![vec![yld]], iglo: 1 })
            }
            PanelFeed::Nu(y) => {
                // label 100: `enext = emax`, `idisc = 0`, then `getyld`'s break.
                let (yld, en, idis) = y.at(e)?;
                let (mut enext, mut idisc) = (EMAX, 0);
                if en < enext * (1.0 - SMALL) {
                    idisc = idis;
                    enext = en;
                }
                Ok(FeedOut { en: enext, idisc, nq: 0, ff: vec![vec![yld, 1.0]], iglo: 1 })
            }
            PanelFeed::TwoBodyNl(f, nl) => {
                let a = f.feed(e, *nl)?;
                let ff: Vec<Vec<f64>> = a.ff.iter().take(*nl).map(|r| r[..a.ng].to_vec()).collect();
                Ok(FeedOut { en: a.enext, idisc: i32::from(a.idisc), nq: a.nq, ff, iglo: a.iglo as i32 })
            }
        }
    }
}

/// The weighting flux `panel` asks `getflx` for (`groupr.f90:6439-6519`):
/// `getwtf` when `nsigz = 1`, the `genflx` table when `nsigz > 1`.
#[derive(Debug, Clone)]
pub enum FluxSource {
    /// `nsigz = 1`: `getwtf(iwt, e)` for every `il`.
    Wtf(i32),
    /// `nsigz > 1`: interpolation in `genflx`'s table.
    Table(crate::groupr::flux_table::FluxCursor),
}

impl FluxSource {
    /// `getflx(e = 0)`.
    fn init(&mut self) {
        match self {
            FluxSource::Wtf(_) => getwtf_init(),
            FluxSource::Table(c) => {
                c.init();
            }
        }
    }

    /// `getflx(e)`: `flux[iz * nl + il]`, `xtot` (table only); returns `(enext, idis)`.
    fn get(&mut self, e: f64, nl: usize, nz: usize, flux: &mut [f64], xtot: &mut f64) -> Result<(f64, i32), NjoyError> {
        match self {
            FluxSource::Wtf(iwt) => {
                let (wtf, en, idis) = getwtf(*iwt, e)?;
                for v in flux.iter_mut().take(nz * nl) {
                    *v = wtf;
                }
                Ok((en, idis))
            }
            FluxSource::Table(c) => c.get(e, nl, nz, flux, xtot),
        }
    }
}

/// `panel`'s saved state for one reaction (`ipan = 0` resets it). `stmp`,
/// `slst`, `ftmp` and `flst` are `(nz, nl)` arrays stored `[iz * nl + il]`.
#[derive(Debug, Clone, Default)]
pub struct PanelState {
    elast: f64,
    idisc: i32,
    enext: f64,
    ehigh: f64,
    stmp: Vec<f64>,
    slst: Vec<f64>,
    ftmp: Vec<f64>,
    flst: Vec<f64>,
    ff: Vec<Vec<f64>>,
    nl: usize,
    nz: usize,
    ng1: usize,
    ig1: i32,
    nq: usize,
    /// The module variable `xtot`, set by `getflx` and `getunr`.
    xtot: f64,
}

impl PanelState {
    fn sized(nl: usize, nz: usize) -> Self {
        let n = nl.max(1) * nz.max(1);
        Self { nl, nz: nz.max(1), stmp: vec![0.0; n], slst: vec![0.0; n], ftmp: vec![0.0; n], flst: vec![0.0; n], ..Self::default() }
    }
}

fn load_ff(st: &mut PanelState, f: &FeedOut) {
    st.ng1 = f.ff[0].len();
    st.ig1 = f.iglo;
    st.ff = f.ff.clone();
    // `getff` label 100 sets `ff(il, 1) = yld` for every `il` (`:7109-7112`).
    while st.ff.len() < st.nl {
        let r = st.ff[0].clone();
        st.ff.push(r);
    }
}

/// `getflx` then `getsig` at `e`, into `(flst, slst)` or (`hi`) `(ftmp, stmp)`;
/// returns `(en, idiscf, enext, idisc)`.
fn flux_and_sig(st: &mut PanelState, flux: &mut FluxSource, sig: &mut SigFeed, e: f64, hi: bool) -> Result<(f64, i32, f64, i32), NjoyError> {
    let (nl, nz) = (st.nl.max(1), st.nz.max(1));
    let mut fx = vec![0.0; nl * nz];
    let mut sx = vec![0.0; nl * nz];
    let (en, idiscf) = flux.get(e, nl, nz, &mut fx, &mut st.xtot)?;
    let (enext, idisc) = sig.get_nz(e, nl, nz, &mut sx, &mut st.xtot)?;
    if hi {
        st.ftmp = fx;
        st.stmp = sx;
    } else {
        st.flst = fx;
        st.slst = sx;
    }
    Ok((en, idiscf, enext, idisc))
}

/// One call of `panel` (`groupr.f90:5858-6091`). `ans` is `ans(il, iz, ig)`
/// stored `[(ig * nz + iz) * nl + il]` with `ig` 1-based (index 0 unused):
/// `ig = 1` is the flux integral, `ig >= 2` the rates. `elo`, `ehi` are updated
/// as upstream's by-reference arguments are.
///
/// # Errors
/// From `getflx`, `getsig`, the feed, or a bad `nq`.
#[allow(clippy::too_many_arguments)]
pub fn panel(
    st: &mut PanelState,
    sig: &mut SigFeed,
    feed: &mut PanelFeed,
    flux: &mut FluxSource,
    elo: &mut f64,
    ehi: &mut f64,
    ans: &mut [f64],
    ng: &mut usize,
    iglo: &mut i32,
) -> Result<(), NjoyError> {
    let elow = *elo;
    let nl = st.nl.max(1);
    let nz = st.nz.max(1);
    let idx = |il: usize, iz: usize, ig: usize| (ig * nz + iz) * nl + il;
    if *elo > *ehi * (1.0 + SMALL) {
        return Err(NjoyError::InvalidInput("panel: elo.gt.ehi.".into()));
    }
    let mut en;
    let mut idiscf;
    let mut nqp = 0usize;
    let mut skip_to_220 = false;
    en = EMAX;
    idiscf = 0;
    if (*elo - st.elast).abs() >= st.elast * SMALL {
        if *elo * RNDOFF < *ehi {
            *elo *= RNDOFF;
        }
        st.elast = *elo;
        let (e1, d1, e2, d2) = flux_and_sig(st, flux, sig, *elo, false)?;
        en = e1;
        idiscf = d1;
        st.enext = e2;
        st.idisc = d2;
        if st.enext >= EMAX * (1.0 - SMALL) {
            skip_to_220 = true;
        } else {
            if (en - st.enext).abs() < st.enext * SMALL && idiscf > st.idisc {
                st.idisc = idiscf;
            }
            if en < st.enext * (1.0 - SMALL) {
                st.idisc = idiscf;
            }
            if en < st.enext * (1.0 - SMALL) {
                st.enext = en;
            }
            let fo = feed.at(*elo)?;
            en = fo.en;
            if fo.idisc != IDISC_UNSET {
                idiscf = fo.idisc;
            }
            load_ff(st, &fo);
            if (en - st.enext).abs() < st.enext * SMALL && idiscf > st.idisc {
                st.idisc = idiscf;
            }
            if en < st.enext * (1.0 - SMALL) {
                st.idisc = idiscf;
            }
            if en < st.enext * (1.0 - SMALL) {
                st.enext = en;
            }
            st.nq = (fo.nq + 2).min(10);
        }
    }
    if !skip_to_220 {
        // upper boundary
        if st.enext < DELTA * *ehi {
            *ehi = st.enext;
            st.ehigh = *ehi;
            if st.idisc > 0 && *ehi * DELTA > *elo {
                st.ehigh = *ehi * DELTA;
            }
        } else {
            st.ehigh = DELTA * *ehi;
        }
        let eh = st.ehigh;
        let (e1, d1, e2, d2) = flux_and_sig(st, flux, sig, eh, true)?;
        en = e1;
        idiscf = d1;
        st.enext = e2;
        st.idisc = d2;
        let mut zero_panel = false;
        if st.enext > EMAX * (1.0 - SMALL) {
            if st.stmp[0] == 0.0 {
                zero_panel = true;
            }
        } else {
            if (en - st.enext).abs() < st.enext * SMALL && idiscf > st.idisc {
                st.idisc = idiscf;
            }
            if en < st.enext * (1.0 - SMALL) {
                st.idisc = idiscf;
            }
            if en < st.enext * (1.0 - SMALL) {
                st.enext = en;
            }
        }
        if zero_panel {
            skip_to_220 = true;
        } else {
            let aq = (*ehi + elow) / 2.0;
            let bq = (*ehi - elow) / 2.0;
            for iz in 0..nz {
                for il in 0..nl {
                    let k = iz * nl + il;
                    ans[idx(il, iz, 1)] += (st.ftmp[k] + st.flst[k]) * bq;
                }
            }
            let nq = st.nq;
            for iq in 1..=nq {
                let (eqq, wq) = match nq {
                    2 => (aq + bq * if iq == 1 { -1.0 } else { 1.0 }, bq),
                    6 => (aq + bq * QP6[iq - 1], bq * QW6[iq - 1]),
                    10 => (aq + bq * QP10[iq - 1], bq * QW10[iq - 1]),
                    _ => return Err(NjoyError::InvalidInput("panel: bad nq in panel".into())),
                };
                let mut eq = sigfig(eqq, 9, 0);
                let t1 = (eq - elow) / (*ehi - elow);
                if iq > 1 {
                    if eq > st.ehigh {
                        eq = st.ehigh;
                    }
                    let fo = feed.at(eq)?;
                    en = fo.en;
                    if fo.idisc != IDISC_UNSET {
                        idiscf = fo.idisc;
                    }
                    nqp = fo.nq;
                    load_ff(st, &fo);
                }
                if *iglo == 0 {
                    *iglo = st.ig1;
                }
                for iz in 0..nz {
                    for il in 0..nl {
                        let k = iz * nl + il;
                        let a = st.stmp[k] * st.ftmp[k];
                        let b = st.slst[k] * st.flst[k];
                        let rr = (b + (a - b) * t1) * wq;
                        for ig in 1..=st.ng1 {
                            if st.ff[il][ig - 1] != 0.0 {
                                let igt = st.ig1 + ig as i32 - *iglo + 1;
                                if igt > 1 {
                                    let igt = igt as usize;
                                    let j = idx(il, iz, igt);
                                    if j >= ans.len() {
                                        return Err(NjoyError::InvalidInput("panel: ans too short".into()));
                                    }
                                    ans[j] += rr * st.ff[il][ig - 1];
                                    if igt > *ng {
                                        *ng = igt;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // thermal-group removal
            if (221..=250).contains(&sig.mtd) && *elo >= TTEST && st.stmp[0].abs() <= SMIN {
                for iz in 0..nz {
                    for il in 0..nl {
                        ans[idx(il, iz, 2)] = 0.0;
                    }
                }
                *ng = 2;
            }
        }
    }
    if skip_to_220 {
        // label 115
        for iz in 0..nz {
            for il in 0..nl {
                ans[idx(il, iz, 1)] = 1.0;
                ans[idx(il, iz, 2)] = 0.0;
            }
        }
        *ng = 2;
        *iglo = 1;
        en = EMAX;
        nqp = 0;
    }
    // label 220
    st.elast = st.ehigh;
    st.slst = st.stmp.clone();
    st.flst = st.ftmp.clone();
    if (en - st.enext).abs() < st.enext * SMALL && idiscf > st.idisc {
        st.idisc = idiscf;
    }
    if en < st.enext * (1.0 - SMALL) {
        st.idisc = idiscf;
    }
    if en < st.enext * (1.0 - SMALL) {
        st.enext = en;
    }
    if st.enext <= *ehi {
        st.enext = RNDOFF * *ehi;
    }
    st.nq = (nqp + 2).min(10);
    Ok(())
}

/// One finished group of the GENDF: `(ig, ng2, ig2lo, data)` where `data` is
/// the `nl*nz*ng2` words of the record.
pub type GroupRow = (usize, usize, i32, Vec<f64>);

/// `displa`, label 310 (`:6292-6378`): self-shielded cross sections and
/// matrices, and every `itr = 0` record. `ans(il, iz, ig2)` is
/// `ans[(ig2 * nz + iz) * nl + il]`; `(ng2, ig2lo, igzero, data)` out, with
/// `data` the `nl * nz * ng2` words. `irat` is set for `MT = 251-253, 452-456`.
/// As upstream, `izero` is reset per `ig2`, not per `il`, and `ilo` is fixed by
/// the first `il` with a non-zero word.
fn displa310(mut ans: Vec<f64>, mut ng2: usize, mut ig2lo: i32, irat: bool, nl: usize, nz: usize) -> (usize, i32, bool, Vec<f64>) {
    let big = 1.0e10;
    let idx = |il: usize, iz: usize, ig: usize| (ig * nz + iz) * nl + il;
    let mut ilo = 0usize;
    let mut ihi = ng2;
    let mut igzero = false;
    let mut result = vec![0.0f64; nz];
    for ig2 in 2..=ng2 {
        let mut izero = 0;
        for il in 0..nl {
            for iz in 0..nz {
                result[iz] = 0.0;
                if ig2 == 2 {
                    ans[idx(il, iz, 1)] = sigfig(ans[idx(il, iz, 1)], 7, 0);
                    if irat {
                        ans[idx(il, iz, 3)] = sigfig(ans[idx(il, iz, 3)], 7, 0);
                    }
                }
                if ans[idx(il, iz, ig2)] != 0.0 {
                    let locd = if irat && ig2 == 2 { 3 } else { 1 };
                    if ans[idx(il, iz, locd)] == 0.0 {
                        ans[idx(il, iz, locd)] = big;
                    }
                    result[iz] = sigfig(ans[idx(il, iz, ig2)] / ans[idx(il, iz, locd)], 7, 0);
                    izero = 1;
                }
            }
            if ilo == 0 && (izero > 0 || ig2 == ng2) {
                ilo = ig2;
            }
            if ilo == 0 && irat {
                ilo = ig2;
            }
            if ilo != 0 {
                for iz in 0..nz {
                    ans[idx(il, iz, ig2 - ilo + 2)] = result[iz];
                }
                if izero > 0 {
                    ihi = ig2;
                }
                if izero == 1 {
                    igzero = true;
                }
            }
        }
    }
    ig2lo += ilo as i32 - 2;
    ng2 = ihi - ilo + 2;
    let data = ans[idx(0, 0, 1)..idx(0, 0, 1 + ng2)].to_vec();
    (ng2, ig2lo, igzero, data)
}


/// `displa`, the infinite-dilution transfer matrix branch (`:6205-6245`,
/// `itr = 1`, `nz = 1`, `nl > 1`): `ans(il, 1, ig2)` is `ans[ig2 * nl + il]`.
/// Returns `(ng2, ig2lo, igzero, data)` with `data` the `nl * ng2` words.
fn displa_matrix(mut ans: Vec<f64>, mut ng2: usize, mut ig2lo: i32, nl: usize) -> (usize, i32, bool, Vec<f64>) {
    let (smin, big) = (1.0e-9, 1.0e10);
    let mut ilo = 0usize;
    let mut ihi = ng2;
    let mut igzero = false;
    for ig2 in 2..=ng2 {
        let mut izero = 0;
        let mut result = vec![0.0f64; nl];
        for il in 0..nl {
            if ans[ig2 * nl + il] != 0.0 {
                if ans[nl + il] == 0.0 {
                    ans[nl + il] = big;
                }
                ans[nl + il] = sigfig(ans[nl + il], 7, 0);
                result[il] = sigfig(ans[ig2 * nl + il] / ans[nl + il], 7, 0);
                if result[il].abs() >= smin {
                    izero = 1;
                }
            }
        }
        if ilo == 0 && (izero > 0 || ig2 == ng2) {
            ilo = ig2;
        }
        if ilo != 0 {
            let d = (ig2 - ilo + 2) * nl;
            ans[d..d + nl].copy_from_slice(&result);
            if izero > 0 {
                ihi = ig2;
            }
            if izero == 1 {
                igzero = true;
            }
        }
    }
    ig2lo += ilo as i32 - 2;
    ng2 = ihi - ilo + 2;
    let data = ans[nl..(1 + ng2) * nl].to_vec();
    (ng2, ig2lo, igzero, data)
}

/// `displa`, label 250 (`:6247-6290`): the isotropic infinite-dilution transfer
/// matrix (`itr = 1`, `nl = nz = 1`). `ans[1]` is the flux, `ans[2..]` the rates.
fn displa_isotropic(mut ans: Vec<f64>, mut ng2: usize, mut ig2lo: i32) -> (usize, i32, bool, Vec<f64>) {
    let (smin, big) = (1.0e-9, 1.0e10);
    let mut ilo = 0usize;
    let mut ihi = ng2;
    let mut igzero = false;
    let mut j = 2usize;
    for ig2 in 2..=ng2 {
        if ilo > 0 {
            j += 1;
        }
        if ans[1] == 0.0 {
            ans[1] = big;
        }
        ans[1] = sigfig(ans[1], 7, 0);
        ans[j] = ans[ig2] / ans[1];
        ans[j] = sigfig(ans[j], 7, 0);
        if ans[j].abs() < smin / 1000.0 {
            ans[j] = 0.0;
        }
        if ans[j] != 0.0 {
            if ilo == 0 {
                ilo = ig2;
            }
            ihi = ig2;
            igzero = true;
        }
    }
    if !igzero {
        ilo = ng2;
    }
    ig2lo += ilo as i32 - 2;
    ng2 = ihi - ilo + 2;
    let data = ans[1..1 + ng2].to_vec();
    (ng2, ig2lo, igzero, data)
}

/// The group loop of `groupr` (`:766-830`) with `displa` for one reaction, at
/// infinite dilution (`nz = 1`, `getflx` = `getwtf(iwt)`). Returns the records
/// to write (`igzero || ig == ngn`).
///
/// `egn` are the `ngn + 1` boundaries, `first` the cross section's first
/// energy, `emaxx` the evaluation-format limit (`:425-431`).
///
/// # Errors
/// From [`panel`].
#[allow(clippy::too_many_arguments)]
pub fn group_general(
    sig: &mut SigFeed,
    feed: &mut PanelFeed,
    iwt: i32,
    egn: &[f64],
    first: f64,
    emaxx: f64,
    irat: bool,
    nl: usize,
) -> Result<Vec<GroupRow>, NjoyError> {
    group_general_nz(sig, feed, &mut FluxSource::Wtf(iwt), 1, egn, first, emaxx, irat, nl)
}

/// [`group_general`] for `nz` sigma-zeros and a given flux (`init`'s `nz`,
/// `getflx`'s two branches). Each record's words are `ans(il, iz, ig2)` in that
/// order (`il` fastest, `:893-916`).
///
/// # Errors
/// From [`panel`].
#[allow(clippy::too_many_arguments)]
pub fn group_general_nz(
    sig: &mut SigFeed,
    feed: &mut PanelFeed,
    flux: &mut FluxSource,
    nz: usize,
    egn: &[f64],
    first: f64,
    emaxx: f64,
    irat: bool,
    nl: usize,
) -> Result<Vec<GroupRow>, NjoyError> {
    let ngn = egn.len() - 1;
    let nz = nz.max(1);
    let nlz = nl.max(1) * nz;
    // `getflx(ee = 0)` at the start of each reaction (`:6457-6467`): `getwtf`'s
    // reset when `nsigz = 1`, the table's first rows when `nsigz > 1`.
    flux.init();
    let mut st = PanelState::sized(nl, nz);
    let mut out = Vec::new();
    // `econst` constant-spectrum range (`groupr.f90:757-785`, `getff` label 300 with
    // `mfd = 16`): the spectrum is written first as the `ig = 0` record (`displa`
    // label 460), then groups `1..=jconst` carry only flux and yield (label 470).
    // `getff` label 200 (`mfd = 6`, fission) has the same range (`:7168-7175`).
    let (jconst, en0) = match &*feed {
        PanelFeed::Photon(f) => (f.jconst(), f.initial_energy()),
        PanelFeed::Continuum(f) => (f.jconst(), f.initial_energy()),
        _ => (0, 0.0),
    };
    if jconst > 0 {
        if nl != 1 {
            return Err(NjoyError::NotPorted("groupr: constant-spectrum range with nl > 1"));
        }
        // label 505: `ans(1, iz, j) = ff(1, i)` for every `iz`; label 460 rounds
        // `ans(1, 1, ·)` only.
        let fo = feed.at(en0)?;
        let ng2 = fo.ff[0].len();
        let mut data = Vec::with_capacity(ng2 * nz);
        for &v in &fo.ff[0] {
            for iz in 0..nz {
                data.push(if iz == 0 { sigfig(v, 7, 0) } else { v });
            }
        }
        out.push((0, ng2, fo.iglo, data));
    }
    for ig in 1..=ngn {
        let (mut elo, ehi) = (egn[ig - 1], egn[ig]);
        if ehi <= first {
            continue;
        }
        if elo >= emaxx && ig != ngn {
            continue;
        }
        let mut ans = vec![0.0f64; (ngn.max(feed.ngg()) + 3) * nlz];
        let (mut ng, mut iglo) = (2usize, 0i32);
        let mut enext = ehi;
        loop {
            panel(&mut st, sig, feed, flux, &mut elo, &mut enext, &mut ans, &mut ng, &mut iglo)?;
            if enext == ehi {
                break;
            }
            elo = enext;
            enext = ehi;
        }
        if jconst > 0 && ig <= jconst {
            // `:815-826`: rate / flux for every `(il, iz)`, `ig2lo = 0`; `displa`
            // label 470 only rounds `prod`.
            for k in 0..nlz {
                ans[2 * nlz + k] /= ans[nlz + k];
            }
            out.push((ig, ng, 0, ans[nlz..(1 + ng) * nlz].to_vec()));
            continue;
        }
        // `displa`'s `itr = 1` families (`:6143-6146`).
        let matrix = matches!(feed, PanelFeed::TwoBodyNl(..) | PanelFeed::Continuum(..) | PanelFeed::Thermal(..) | PanelFeed::Photon(..) | PanelFeed::Mf6(..) | PanelFeed::Gam102(..));
        // `:6202-6205`: `nz > 1` or `itr = 0` → label 310; else `nl = 1` → 250.
        let (ng2, ig2lo, igzero, data) = if nz > 1 || !matrix {
            displa310(ans, ng, iglo, irat, nl.max(1), nz)
        } else if nl == 1 {
            displa_isotropic(ans, ng, iglo)
        } else {
            displa_matrix(ans, ng, iglo, nl)
        };
        if igzero || ig == ngn {
            out.push((ig, ng2, ig2lo, data));
        }
    }
    Ok(out)
}

/// Vector-case wrapper: `(ig, flux, sigma, iglo)` per written group.
///
/// # Errors
/// From [`panel`].
pub fn group_vector(
    sig: &mut SigFeed,
    iwt: i32,
    egn: &[f64],
    first: f64,
    emaxx: f64,
) -> Result<Vec<(usize, f64, f64, i32)>, NjoyError> {
    let rows = group_general(sig, &mut PanelFeed::Vector, iwt, egn, first, emaxx, false, 1)?;
    Ok(rows.into_iter().map(|(ig, _, lo, d)| (ig, d[0], d[1], lo)).collect())
}
