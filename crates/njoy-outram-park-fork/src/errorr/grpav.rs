// Ported from NJOY2016 `src/errorr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine grpav`, l.8821-9095 (`grpav`) — ngout=0, infinite-dilution path.
//   - `subroutine epanel`, l.9645-9714 (`PanelState::epanel`).
//   - `subroutine egtsig`, l.10288-10333 (`XsSampler::locate`, `classify_mt`).
//   - `subroutine rdgout`/`rdsig`, l.2755-2937 (`UnionGroupXs::flux_vector`/`sigma_for`).
//   - `subroutine covcal` sub-threshold flux fill, l.1855-1866 (`UnionGroupXs::flux_vector`).
// and from `src/endf.f90`:
//   - `subroutine gety1`, l.1940-2051 (`XsSampler`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Multigroup cross sections on the union grid — `grpav` and its feeders
//! (`errorr.f90`).
//!
//! With no input GENDF tape (`ngout = 0`) ERRORR group-averages the PENDF
//! cross sections itself, on the *union* grid, with a stripped-down copy of
//! GROUPR's panel integrator: [`PanelState::epanel`] marches sub-panels
//! bounded by the next cross-section grid point ([`XsSampler`], a port of
//! `gety1`) and the next weight stop ([`super::weight::WeightSampler`]) and
//! applies a trapezoid to `phi` and to `sigma*phi` on each. The per-group
//! results — `ans(1,1,1) = ∫phi`, `ans(1,1,2)/ans(1,1,1) = sigma_g` — are
//! what upstream writes to its scratch "gendf" tape (`ngout = -10`) and
//! reads back through `rdgout`; here they live in [`UnionGroupXs`].
//!
//! **Scope:** infinite dilution only (`nsigz = 1`, `sigz = 1e10`,
//! `errorr.f90:8907-8910`), so the flux is the bare weight function and no
//! self-shielded `nscr2` total is read. `mfcov = 34` (`egtlgc`) is not
//! ported. Nubar (`MT = 452/455/456`) cannot be group-averaged here, as
//! upstream says ("use groupr first").
//!
//! `gety1`'s two numerical idioms are reproduced because they move panel
//! boundaries: the first non-zero grid point is *shaded down* by
//! `0.999999` so the threshold rise is a finite ramp, and a doubled grid
//! point (`x(i+1) == x(i)`) is reported as a discontinuity so `epanel`
//! evaluates just below it (`delta = 0.999995`).

use std::collections::BTreeMap;

use crate::endf::interp::{terp1, IntLaw};
use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::NjoyError;

use super::weight::WeightSampler;

/// `down` — `gety1`'s threshold shading factor (`endf.f90:1958`).
const DOWN: f64 = 0.999999;
/// `xbig` — `gety1`'s "past the table" sentinel (`endf.f90:1959`).
const XBIG: f64 = 1.0e12;
/// `delta` — `epanel`'s step below a discontinuity (`errorr.f90:9662`).
const DELTA: f64 = 0.999995;
/// `big` — `egtflx`'s initial `enext` at infinite dilution
/// (`errorr.f90:10244`).
const BIG: f64 = 1.0e10;

/// Map a covariance `MT` to the PENDF `(MF, MT)` `egtsig` reads
/// (`errorr.f90:10306-10318`, `mfd = 3`).
///
/// # Errors
/// [`NjoyError::EndfParse`] for an `MT` upstream maps to `mt = 0` (e.g.
/// 201–206, 208–250 other than 207, 254–260, 262–599) — a fatal `mt=0.`
/// in `egtsig`.
pub fn classify_mt(mtd: i32) -> Result<(i32, i32), NjoyError> {
    let mt = match mtd {
        m if m <= 200 => m,
        207 | 261 => mtd,
        600..=699 | 700..=799 | 800..=891 => mtd,
        251 | 252 | 253 => 2,
        _ => 0,
    };
    if mt == 0 {
        return Err(NjoyError::EndfParse(format!(
            "errorr::egtsig: mt=0 (no pendf cross section for mtd={mtd})"
        )));
    }
    Ok((3, mt))
}

/// A pointwise TAB1 cross section sampled in ascending energy order — a port
/// of `gety1` (`endf.f90:1940-2051`) over an in-memory table.
#[derive(Debug, Clone)]
pub struct XsSampler {
    x: Vec<f64>,
    y: Vec<f64>,
    /// `(nbt, int)` interpolation regions (1-based `nbt`).
    interp: Vec<(u32, u32)>,
    /// Current upper grid point (1-based `ip`).
    ip: usize,
    /// Current interpolation region (1-based `ir`).
    ir: usize,
    xlast: f64,
    ylast: f64,
}

impl XsSampler {
    /// Initialise on a TAB1 (`gety1(x = 0)`, `endf.f90:1962-1993`).
    /// Returns the sampler and `xnext` — the threshold `egtsig` reports.
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] on an empty table.
    pub fn new(pairs: &[(f64, f64)], interp: &[(u32, u32)]) -> Result<(Self, f64), NjoyError> {
        if pairs.is_empty() {
            return Err(NjoyError::EndfParse("errorr::gety1: empty TAB1".into()));
        }
        let np = pairs.len();
        let x: Vec<f64> = pairs.iter().map(|p| p.0).collect();
        let y: Vec<f64> = pairs.iter().map(|p| p.1).collect();
        let interp = if interp.is_empty() {
            vec![(np as u32, 2)]
        } else {
            interp.to_vec()
        };
        let mut s = XsSampler {
            x,
            y,
            interp,
            ip: 1,
            ir: 1,
            xlast: 0.0,
            ylast: 0.0,
        };
        // label 100: check for zero extension as in mf13.
        loop {
            s.xlast = s.x[s.ip - 1];
            s.ylast = s.y[s.ip - 1];
            let mut xnext = s.xlast;
            if s.ylast != 0.0 {
                s.xlast *= DOWN;
                return Ok((s, xnext));
            }
            if s.ip >= np - 1 {
                return Ok((s, xnext));
            }
            let ynext = s.y[s.ip];
            if ynext != 0.0 && s.ip > 1 {
                xnext *= DOWN;
            }
            if ynext != 0.0 {
                return Ok((s, xnext));
            }
            s.ip += 1;
            let nbt = s.interp[s.ir - 1].0 as usize;
            if s.ip > nbt {
                s.ir += 1;
            }
        }
    }

    /// Locate a reaction on a PENDF tape and initialise (`egtsig(e = 0)`,
    /// `errorr.f90:10303-10322`). Returns `(sampler, thresh)`.
    ///
    /// # Errors
    /// [`NjoyError::SectionNotFound`] if the PENDF lacks the section;
    /// [`NjoyError::EndfParse`] from [`classify_mt`] or a malformed TAB1.
    pub fn locate(pendf: &Tape, mat: i32, mtd: i32) -> Result<(Self, f64), NjoyError> {
        let (mf, mt) = classify_mt(mtd)?;
        let section = pendf
            .section(mat, mf, mt)
            .ok_or(NjoyError::SectionNotFound { mat, mf, mt })?;
        let mut cur = SectionCursor::new(&section.rows);
        let _head = cur.read_cont()?;
        let tab1 = cur.read_tab1()?;
        Self::new(&tab1.pairs, &tab1.interp)
    }

    /// `y1(x)` and the next grid point (`gety1(x > 0)`,
    /// `endf.f90:1996-2050`): `(y1, xnext, idis)`. Calls must ascend.
    pub fn get(&mut self, x: f64) -> Result<(f64, f64, i32), NjoyError> {
        let np = self.x.len();
        loop {
            // label 110
            if x < self.xlast {
                // label 200: outside range
                return Ok((0.0, self.xlast, 0));
            }
            let xip = self.x[self.ip - 1];
            if x < xip {
                // label 120: interpolate
                let law = self.interp[self.ir - 1].1;
                let mut idis = i32::from(law == 1);
                let y1 = terp1(
                    self.xlast,
                    self.ylast,
                    xip,
                    self.y[self.ip - 1],
                    x,
                    IntLaw::from_code(law),
                )?;
                let xnext = xip;
                if self.ip < np && self.x[self.ip] == xnext {
                    idis = 1;
                }
                return Ok((y1, xnext, idis));
            }
            if self.ip == np {
                // label 300: last point
                let y1 = self.y[np - 1];
                self.xlast = XBIG;
                return Ok((y1, XBIG, 0));
            }
            // move up to the next range
            self.xlast = xip;
            self.ylast = self.y[self.ip - 1];
            self.ip += 1;
            let nbt = self.interp[self.ir - 1].0 as usize;
            if self.ip > nbt {
                self.ir += 1;
            }
        }
    }
}

/// The `save`d state of `epanel` (`errorr.f90:9655-9663`): last panel top,
/// the integrand factors there, and the pending discontinuity flags.
#[derive(Debug, Clone)]
pub struct PanelState {
    enext: f64,
    flst: f64,
    slst: f64,
    elast: f64,
    idisc: i32,
    idiscf: i32,
}

impl PanelState {
    /// Fresh state (Fortran initial values `elast = 0`, `idisc = 0`).
    pub fn new() -> Self {
        PanelState {
            enext: 0.0,
            flst: 0.0,
            slst: 0.0,
            elast: 0.0,
            idisc: 0,
            idiscf: 0,
        }
    }

    fn merge_next(&mut self, en: f64, idiscf: i32) {
        if en == self.enext && idiscf > self.idisc {
            self.idisc = idiscf;
        }
        if en < self.enext {
            self.idisc = idiscf;
            self.enext = en;
        }
    }

    /// Integrate one panel from `elo` up to `min(ehi, next stop)`
    /// (`subroutine epanel`, `errorr.f90:9645-9714`, `mfcov != 34`).
    ///
    /// On return `ehi` holds the panel's actual upper bound (upstream
    /// modifies its `ehi` argument in place), and `ans = [∫phi, ∫sigma*phi]`
    /// has this panel's trapezoid contributions added.
    pub fn epanel(
        &mut self,
        xs: &mut XsSampler,
        wt: &mut WeightSampler,
        elo: f64,
        ehi: &mut f64,
        ans: &mut [f64; 2],
    ) -> Result<(), NjoyError> {
        // retrieve factors at the lower boundary
        if self.idisc != 0 || elo != self.elast {
            self.elast = elo;
            let (s, enext, idisc) = xs.get(elo)?;
            self.slst = s;
            self.enext = enext;
            self.idisc = idisc;
            let (f, en, idiscf) = wt.flux(elo)?;
            self.flst = f;
            self.idiscf = idiscf;
            let _ = BIG;
            self.merge_next(en, idiscf);
        }
        // retrieve cross section and flux at the upper boundary
        if self.enext < *ehi {
            *ehi = self.enext;
        }
        let mut ehigh = *ehi;
        if self.idisc > 0 {
            ehigh = *ehi * DELTA;
        }
        let (sig, enext, idisc) = xs.get(ehigh)?;
        self.enext = enext;
        self.idisc = idisc;
        let (flux, en, idiscf) = wt.flux(ehigh)?;
        self.idiscf = idiscf;
        self.merge_next(en, idiscf);
        // compute group cross sections and fluxes
        let bq = (ehigh - elo) / 2.0;
        ans[0] += (flux + self.flst) * bq;
        ans[1] += (sig * flux + self.slst * self.flst) * bq;
        // save last values
        self.elast = *ehi;
        self.slst = sig;
        self.flst = flux;
        self.merge_next(en, idiscf);
        Ok(())
    }
}

impl Default for PanelState {
    fn default() -> Self {
        Self::new()
    }
}

/// One reaction's union-group records (`grpav`'s `ngout` scratch records,
/// `errorr.f90:9048-9058`): a record exists for group `ig` only when
/// `sigma_g != 0` or `ig` is the last group.
#[derive(Debug, Clone, Default)]
pub struct UnionGroupRecords {
    /// `ans(1,1,1)` — group flux integral `∫phi`, `0` where no record.
    pub flux: Vec<f64>,
    /// `ans(1,1,2)/ans(1,1,1)` — group cross section, `0` where no record.
    pub sigma: Vec<f64>,
}

/// Every group-averaged reaction on the union grid (the in-memory `ngout`).
#[derive(Debug, Clone)]
pub struct UnionGroupXs {
    /// `un(1:nunion+1)`.
    pub un: Vec<f64>,
    /// Records keyed by `MT`, in `iga` order of insertion.
    pub records: BTreeMap<i32, UnionGroupRecords>,
    /// `grpav` messages (thresholds above the union grid, MT=3 note).
    pub messages: Vec<String>,
}

impl UnionGroupXs {
    /// `nunion`.
    pub fn nunion(&self) -> usize {
        self.un.len() - 1
    }

    /// The fine-group flux vector `flx(1:nunion)` as `covcal` stores it
    /// (`rdgout` with `mfd = 3, mti = -1`, `errorr.f90:2861-2870`, then the
    /// sub-threshold fill of `errorr.f90:1855-1866`).
    ///
    /// Taken from the `MT = 1` records when present; otherwise the union of
    /// every reaction's flux records (later sections overwrite), with any
    /// still-empty groups filled downward assuming constant `dn/dE`.
    pub fn flux_vector(&self) -> Vec<f64> {
        let n = self.nunion();
        if let Some(r) = self.records.get(&1) {
            return r.flux.clone();
        }
        let mut flx = vec![0.0; n];
        for r in self.records.values() {
            for (jg, &f) in r.flux.iter().enumerate() {
                if r.sigma[jg] != 0.0 || jg + 1 == n {
                    flx[jg] = f;
                }
            }
        }
        // covcal: estimate sub-threshold fluxes with dn/de constant.
        let mut dne = 1.0e-10;
        for is in (0..n).rev() {
            let flux = flx[is];
            let de = self.un[is + 1] - self.un[is];
            if flux != 0.0 && de != 0.0 {
                dne = flux / de;
            } else {
                flx[is] = dne * de;
            }
        }
        flx
    }

    /// `sig(1:nunion)` for one reaction (`rdsig`/`rdgout`, `mfd = 3`,
    /// `errorr.f90:2755-2786, 2876-2937`): `MT = 3` is constructed as
    /// total minus elastic; a missing reaction yields zeros (upstream's
    /// "calculation continued with sigma=0" message).
    pub fn sigma_for(&self, mt: i32) -> Vec<f64> {
        let n = self.nunion();
        let get = |m: i32| self.records.get(&m).map(|r| r.sigma.clone());
        if mt == 3 {
            let mut s = get(1).unwrap_or_else(|| vec![0.0; n]);
            if let Some(el) = get(2) {
                for (a, b) in s.iter_mut().zip(el) {
                    *a -= b;
                }
            }
            return s;
        }
        get(mt).unwrap_or_else(|| vec![0.0; n])
    }
}

/// Compute multigroup cross sections on the union grid for every reaction
/// in `iga` (`subroutine grpav`, `errorr.f90:8821-9095`, `ngout = 0`,
/// infinite dilution).
///
/// `weight` is the initialised weight sampler (`egnwtf` has run); it is
/// re-initialised per reaction as `egtflx(e = 0)` does. `un` is the union
/// grid; `etop = un(nunion+1)`.
///
/// # Errors
/// - [`NjoyError::EndfParse`] for nubar (`MT` 452/455/456, upstream's
///   "cannot group average" fatal) or an unmappable `MT`.
/// - [`NjoyError::SectionNotFound`] if the PENDF lacks a reaction's MF=3.
pub fn grpav(
    pendf: &Tape,
    matd: i32,
    iga: &[i32],
    un: &[f64],
    weight: &WeightSampler,
) -> Result<UnionGroupXs, NjoyError> {
    let nunion = un.len() - 1;
    let etop = un[nunion];
    let mut out = UnionGroupXs {
        un: un.to_vec(),
        records: BTreeMap::new(),
        messages: Vec::new(),
    };
    for &mtd in iga {
        if mtd == 452 || mtd == 455 || mtd == 456 {
            return Err(NjoyError::EndfParse(format!(
                "errorr::grpav: cannot group average mt={mtd} (use groupr first, then errorr with ngout.ne.0)"
            )));
        }
        if mtd == 3 {
            out.messages
                .push("grpav: mt3 cross sections are constructed from total minus elastic".into());
            continue;
        }
        let (mut xs, thresh) = XsSampler::locate(pendf, matd, mtd)?;
        if thresh > etop {
            out.messages.push(format!(
                "grpav: mf 3 mt {mtd} has threshold gt highest union energy."
            ));
            continue;
        }
        let mut wt = weight.clone();
        let mut state = PanelState::new();
        let mut rec = UnionGroupRecords {
            flux: vec![0.0; nunion],
            sigma: vec![0.0; nunion],
        };
        for ig in 0..nunion {
            let mut elo = un[ig];
            let ehi = un[ig + 1];
            if ehi <= thresh {
                continue;
            }
            let mut enext = ehi;
            let mut ans = [0.0f64; 2];
            loop {
                state.epanel(&mut xs, &mut wt, elo, &mut enext, &mut ans)?;
                if enext == ehi {
                    break;
                }
                elo = enext;
                enext = ehi;
            }
            let sigma = if ans[0] != 0.0 { ans[1] / ans[0] } else { 0.0 };
            if sigma != 0.0 || ig + 1 == nunion {
                rec.flux[ig] = ans[0];
                rec.sigma[ig] = sigma;
            }
        }
        out.records.insert(mtd, rec);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errorr::weight::ErrorrWeight;

    /// `gety1` port: threshold shading, interior interpolation, a doubled
    /// point flagged as a discontinuity, last-point handling.
    #[test]
    fn xs_sampler_reproduces_gety1_idioms() {
        // leading zeros then a rise, with a doubled point at 3.0
        let pairs = [
            (1.0, 0.0),
            (2.0, 0.0),
            (2.5, 5.0),
            (3.0, 6.0),
            (3.0, 10.0),
            (4.0, 10.0),
        ];
        let (mut s, thresh) = XsSampler::new(&pairs, &[(6, 2)]).unwrap();
        // init stops at the last zero point (ip=2): xnext = down*2.0
        assert!((thresh - DOWN * 2.0).abs() < 1e-12);
        // below xlast -> zero, xnext = xlast
        let (y, xn, _) = s.get(1.5).unwrap();
        assert_eq!(y, 0.0);
        assert!((xn - 2.0).abs() < 1e-12);
        // interior
        let (y, xn, idis) = s.get(2.25).unwrap();
        assert!((y - 2.5).abs() < 1e-12);
        assert_eq!((xn, idis), (2.5, 0));
        // approaching the doubled point: xnext=3.0 flagged
        let (y, xn, idis) = s.get(2.75).unwrap();
        assert!((y - 5.5).abs() < 1e-12);
        assert_eq!((xn, idis), (3.0, 1));
        // after the jump
        let (y, _, _) = s.get(3.5).unwrap();
        assert!((y - 10.0).abs() < 1e-12);
        // at the last point: value kept, xbig beyond
        let (y, xn, _) = s.get(4.0).unwrap();
        assert_eq!((y, xn), (10.0, XBIG));
        let (y, _, _) = s.get(5.0).unwrap();
        assert_eq!(y, 0.0);

        // first point non-zero: xlast shaded so [down*x1, x1) ramps to y1
        let (mut s, thresh) = XsSampler::new(&[(10.0, 4.0), (20.0, 4.0)], &[(2, 2)]).unwrap();
        assert_eq!(thresh, 10.0);
        let (y, _, _) = s.get(DOWN * 10.0).unwrap();
        assert!((y - 4.0).abs() < 1e-12);
    }

    /// A constant cross section with a `1/E` weight group-averages to the
    /// constant, and the group flux is `ln(ehi/elo)` to the trapezoid's
    /// accuracy on 1 % panels.
    #[test]
    fn grpav_constant_xs_one_over_e() {
        let pairs: Vec<(f64, f64)> = vec![(1e-5, 3.0), (2e7, 3.0)];
        let un = [1.0, 10.0, 100.0];
        let (mut xs, thresh) = XsSampler::new(&pairs, &[(2, 2)]).unwrap();
        assert_eq!(thresh, 1e-5);
        let mut wt = WeightSampler::new(ErrorrWeight::OneOverE, 300.0);
        let mut st = PanelState::new();
        for ig in 0..2 {
            let mut elo = un[ig];
            let ehi = un[ig + 1];
            let mut enext = ehi;
            let mut ans = [0.0; 2];
            loop {
                st.epanel(&mut xs, &mut wt, elo, &mut enext, &mut ans)
                    .unwrap();
                if enext == ehi {
                    break;
                }
                elo = enext;
                enext = ehi;
            }
            assert!((ans[1] / ans[0] - 3.0).abs() < 1e-12);
            assert!(
                (ans[0] - (10.0f64).ln()).abs() < 2e-5 * (10.0f64).ln(),
                "flux {}",
                ans[0]
            );
        }
    }

    #[test]
    fn classify_mt_matches_egtsig() {
        assert_eq!(classify_mt(1).unwrap(), (3, 1));
        assert_eq!(classify_mt(102).unwrap(), (3, 102));
        assert_eq!(classify_mt(251).unwrap(), (3, 2));
        assert_eq!(classify_mt(649).unwrap(), (3, 649));
        assert_eq!(classify_mt(851).unwrap(), (3, 851));
        assert!(classify_mt(203).is_err());
        assert!(classify_mt(452).is_err());
    }

    /// `flux_vector` falls back to the union of partials and the constant
    /// `dn/dE` fill when `MT = 1` is absent.
    #[test]
    fn flux_vector_fill_without_total() {
        let mut records = BTreeMap::new();
        records.insert(
            16,
            UnionGroupRecords {
                flux: vec![0.0, 0.0, 2.0, 4.0],
                sigma: vec![0.0, 0.0, 1.0, 1.0],
            },
        );
        let u = UnionGroupXs {
            un: vec![1.0, 2.0, 3.0, 5.0, 9.0],
            records,
            messages: vec![],
        };
        let f = u.flux_vector();
        // group 3: 2.0 over de=2 -> dne=1; group 2 filled 1*1, group 1 filled 1*1
        assert_eq!(f, vec![1.0, 1.0, 2.0, 4.0]);
    }
}
