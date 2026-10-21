// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! File-6 continuum-energy-angle feed function — `getmf6`
//! (`groupr.f90:7527-8133`).
//!
//! [`Mf6Feed`] is the stateful analogue of the Fortran routine, modelled on
//! [`crate::groupr::two_body::TwoBodyFeed`] (which plays the same role for
//! `getdis`). [`Mf6Feed::new`] performs the `ed <= 0` **initialization** pass
//! (`:7572-7961`) — it finds the requested emitted particle's subsection(s) in
//! MF=6, applies the low-energy / `ismooth` patches, and transforms every
//! centre-of-mass incident-energy table into the lab frame through
//! [`crate::groupr::kinematics::cm2lab`]. [`Mf6Feed::feed`] is the `ed > 0`
//! **normal entry** (`:7963-8133`): it interpolates the emission probability,
//! marches the lab secondary-energy grid with
//! [`crate::groupr::kinematics::f6lab`], trapezoid-integrates each secondary
//! group, adds the discrete lab lines, and renormalises to the yield.
//!
//! # State
//! The Fortran `save` list is
//! `nne, ne, int, jlo, elo, jhi, ehi, terml, pspmax, langn, lepn, disc102,
//! idis, iyss, izss, jjss, jloss, nss, jzap, lct3` (`:7568-7571`). Everything
//! on it that survives between calls is a field of [`Mf6Feed`]; the pointer
//! bookkeeping (`iyss`/`izss`/`jjss`/`jloss`, indices into the flat `ddmf6`
//! scratch array) is replaced by an owned [`Vec`] of subsections, and `terml`
//! is a per-call scratch buffer because `:8027-8031` rewrites it at the top of
//! every `ed > 0` call.
//!
//! # Ported scope (read this before trusting a number)
//!
//! | `getmf6` branch | lines | status |
//! |---|---|---|
//! | initialization / tape walk / multi-subsection | 7572-7633, 7915-7955 | **ported** |
//! | LAW = 1 read + grid patches + `ismooth` | 7664-7822 | **ported** |
//! | LAW = 1 CM → lab (`cm2lab`) | 7812-7820 | **ported** (LANG 1, 2) |
//! | LAW = 7 read + `ll2lab` | 7876-7913 | **ported** |
//! | normal entry, `terpa` + panel search + `f6lab` march | 7963-8050 | **ported** |
//! | discrete lab lines | 8052-8082 | **ported** |
//! | normalisation / upscatter fold-back | 8108-8133 | **ported** |
//! | LAW = 6 (n-body phase space) | 7823-7875 | [`NjoyError::NotPorted`] — needs `f6psp` (LANG = 0) |
//! | LAW = 2,3,4,5 and LAW = -4 (`getdis`, `:7936-7940`) | 7634-7656, 7936-7940 | [`NjoyError::NotPorted`] — File-6 two-body (`getdis` `mft = 8`) |
//! | `disc102` relativistic capture gamma (`gam102`, `:8102-8105`) | 7620-7623, 8102-8105 | [`NjoyError::NotPorted`] |
//!
//! # Upstream defects reproduced (reported, deliberately not fixed)
//!
//! - **`idis` is write-only** (`:7983-7986`): maintained across subsections
//!   but never copied into the `idisc` dummy, so the caller gets the last
//!   `terpa` flag. See [`Mf6FeedAt::idisc`].
//! - **`jgmax`'s search is unbounded** (`:8002-8005`): reads past `eg` for an
//!   `ed` above the top boundary. Clamped here, see [`Mf6Feed::feed`].
//! - **The panel search reads one table too many** (`:8015-8016`): at
//!   `ie = nne` it dereferences the *next* subsection's first table, or
//!   uninitialised `jloss` for the last subsection. See [`Mf6Feed::feed`].
//! - **The low-energy patch hard-codes `NA = 0`** (`:7707-7715`): see
//!   `records::law1_low_energy_patch`.
//! - **The `ismooth` merge loop does not write back `NW`/`NEP`**
//!   (`:7729-7744` vs `:7758-7759`): see `records::ismooth_histogram`.
//!
//! # A defect in this crate's `f6lab`, *not* in upstream
//!
//! [`crate::groupr::kinematics::f6lab`] guards its low/high coefficient
//! interpolation with `lo_ptr != lo_first`, where `lo_first` is the pointer
//! *after* the leading `E' = 0` record is skipped. Upstream guards with
//! `llo /= ilo` (`groupr.f90:9216,9224`), where `ilo` is the unskipped first
//! continuum record — so upstream's guard is true on the first panel and this
//! crate's is false, silently dropping the `[0, E'_1]` panel of every table
//! that starts at `E' = 0`. Both `cm2lab` and `ll2lab` always emit such a
//! leading point (`groupr.f90:8163`, `:9045`), so **every CM and LAW=7 feed
//! this module produces loses its first secondary-energy panel**. Pinned by
//! `tests::leading_zero_energy_point_loses_the_first_panel_lab1_defect`;
//! fixing it belongs in `kinematics/lab1.rs`, which this port was told not to
//! touch.
//!
//! This is untrusted AI draft material: verify against upstream NJOY golden
//! output before trusting it numerically.

use crate::endf::interp::{terp1, IntLaw};
use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::errorr::weight::terpa;
use crate::groupr::kinematics::{cm2lab, ll2lab, Cm6Emission, Cm6Lang, Law7Curve, Law7Emission};
use crate::NjoyError;

mod records;
#[cfg(test)]
mod tests;

use records::{
    ismooth_histogram, ismooth_linlin, law1_low_energy_patch, pack_lab_list, pack_tab1, skip6,
    LabTable, Mf6Sub,
};

/// `emax = 1.e10` (`:7557`).
const EMAX: f64 = 1.0e10;
/// `small = 1.e-10` (`:7558`).
const SMALL: f64 = 1.0e-10;
/// `step = 1.2` (`:7560`) — the LAW-6 geometric energy ladder (unused here;
/// LAW 6 is `NotPorted`).
#[allow(dead_code)]
const STEP: f64 = 1.2;
/// `up = 1.00001` (`:7561`).
const UP: f64 = 1.000_01;
/// `dn = 0.99999` (`:7562`).
const DN: f64 = 0.999_99;
/// `eps = 0.02` (`:7563`) — the normalization-report tolerance.
const EPS: f64 = 0.02;
/// `alight = 5` (`:7565`) — the `LCT = 3` "light product" mass cut.
const ALIGHT: f64 = 5.0;
/// Port-only guard: upstream's `320` loop (`:8035-8050`) is unbounded and
/// relies on `f6lab` always advancing `epnext`. A malformed table could spin
/// forever; this port stops with [`NjoyError::NotConvergent`] instead.
const MAX_MARCH_STEPS: usize = 4_000_000;

/// Configuration a `getmf6` call inherits from GROUPR's module-level state.
///
/// Every field is one of the Fortran globals `getmf6` reads without receiving
/// it as an argument.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mf6FeedConfig {
    /// `awr` — target atomic-weight ratio, **already divided by `awrp`**
    /// (`getsig:6718`, `if (awrp.ne.zero) awr=awr/awrp`). Dimensionless.
    pub awr: f64,
    /// `awrp` — projectile atomic-weight ratio (`:11482`), `1` for a neutron.
    pub awrp: f64,
    /// `izap` — the **projectile** `ZA` (`NSUB/10`, `:11481`). Used as
    /// `bach`'s `iza1i`, as `zad` when `mfd = 3` (`:7585`), and in the
    /// upscatter fold-back test (`:8115`).
    pub izap: i32,
    /// `q` — the MF=3 `QI` of the reaction (`getsig:6748`) \[eV\].
    pub q: f64,
    /// `ismooth` — card-1 switch (`:1067`, `:1070`): sqrt(E) extension of the
    /// low-energy end of a neutron emission spectrum.
    pub ismooth: bool,
    /// `nl` — number of lab Legendre orders the feed produces. Fixed at
    /// construction because `cm2lab` / `ll2lab` bake it into the stored tables
    /// (`:7813`, `:7907`).
    pub nl: usize,
}

/// `getmf6`'s answer at one incident energy — the Fortran output arguments
/// `ans, enext, idisc, yld, iglo, ng2, nq`.
#[derive(Debug, Clone, PartialEq)]
pub struct Mf6FeedAt {
    /// `ans(l, j)` as `ans[l][j]`: `l = 0 .. nl-1` Legendre order, `j = 0 ..
    /// ng-1` secondary group (counting up from `iglo`).
    pub ans: Vec<Vec<f64>>,
    /// `ng2` — number of secondary groups filled (`:8085`, always `ng` here).
    pub ng2: usize,
    /// `iglo` — 1-based secondary group of column 0 (`:8084`, always 1 here).
    pub iglo: usize,
    /// `nq` — quadrature-order hint for `panel` (`:8086`, always 0 here).
    pub nq: usize,
    /// `yld` — the summed emission probability `sum pe` over subsections
    /// (`:7987`).
    pub yld: f64,
    /// `enext` — next incident energy at which the feed changes (`:7969`).
    pub enext: f64,
    /// `idisc` — discontinuity flag at `enext`.
    ///
    /// **Reproduces an upstream defect**: `getmf6` maintains a saved `idis`
    /// across `:7983-7986` but never copies it back into the `idisc` dummy,
    /// so what the caller actually receives is whatever the *last*
    /// subsection's `terpa` left there. See [`Mf6Feed::feed`]'s doc.
    pub idisc: i32,
}

/// The stateful `getmf6` (`groupr.f90:7527-8133`).
///
/// See the [module docs](self) for the ported/unported branch table.
#[derive(Debug, Clone)]
pub struct Mf6Feed {
    cfg: Mf6FeedConfig,
    /// `izat` — the material `ZA` from the MF=6 HEAD (`:7583`).
    izat: i32,
    /// `lct3` — the file-level `LCT` (`:7582`).
    lct3: i32,
    /// `lct` — the effective frame for the matched particle (`:7581`,
    /// `:7589`, `:7625-7628`).
    lct: i32,
    /// `awp` — emitted-particle AWR of the matched subsection (`:7617`).
    awp: f64,
    /// `aprime = awp / awrp` (`:7629-7630`).
    aprime: f64,
    /// `jzap` — emitted-particle `ZA` of the matched subsection (`:7615`).
    jzap: i32,
    /// `disc102` (`:7621`) — non-zero marks the discrete relativistic capture
    /// gamma path (`gam102`, not ported).
    disc102: f64,
    /// The matched subsections, `nss` of them (`:7929-7933`).
    subs: Vec<Mf6Sub>,
    /// `idis` (`:7574`) — saved, maintained, and (upstream) never read back.
    idis: i32,
    /// `elo` / `ehi` — the incident-energy span (`:7594-7595` during init,
    /// then the current panel bracket, `:8019-8020`).
    elo: f64,
    ehi: f64,
    /// `langn` / `lepn` (`:7989-7997`).
    langn: i32,
    lepn: i32,
    /// `nne` / `int` (`:8009-8012`).
    nne: usize,
    int_code: u32,
    /// The `:7957-7961` "no MF=6 gammas" exit: `enext = -1`.
    no_gammas: bool,
    /// Diagnostics upstream writes to `nsyso` via `mess`/`write` — collected
    /// rather than printed (`:7693`, `:7718`, `:7745`, `:7772`, `:7943`,
    /// `:8125-8127`).
    messages: Vec<String>,
}

impl Mf6Feed {
    /// `getmf6` with `ed <= 0` — the initialization pass (`:7572-7961`).
    ///
    /// Walks MF=6 of `(matd, mtd)` looking for the subsection whose `ZAP`
    /// matches the particle implied by `mfd` (`zad`, `:7584-7592`), reads its
    /// law data, applies the low-energy and `ismooth` grid patches, and
    /// transforms centre-of-mass tables to the lab frame. Extra subsections
    /// for the *same* `ZAP` are accumulated (`:7915-7934`).
    ///
    /// `mfd` is GROUPR's output-file selector, which is what picks the desired
    /// particle: `6` → neutron (`zad = 1`), `3` → the projectile
    /// (`zad = izap`), `18` → `zad = 0` **and** `lct` forced to 1, `21..26` →
    /// `1001, 1002, 1003, 2003, 2004, 9999` (`:7584-7592`).
    ///
    /// # Errors
    /// - [`NjoyError::SectionNotFound`] if MF=6/`mtd` is absent.
    /// - [`NjoyError::EndfParse`] for `'desired particle not found.'`
    ///   (`:7605-7606`), `'illegal law.'` (`:7662`), or a malformed record.
    /// - [`NjoyError::NotPorted`] for LAW 2-5, LAW 6, LAW 0 and the
    ///   `disc102` capture-gamma path (see the [module docs](self)).
    pub fn new(
        tape: &Tape,
        matd: i32,
        mfd: i32,
        mtd: i32,
        cfg: Mf6FeedConfig,
    ) -> Result<Self, NjoyError> {
        if cfg.nl == 0 {
            return Err(NjoyError::EndfParse("getmf6: nl must be >= 1".into()));
        }
        let section = tape
            .section(matd, 6, mtd)
            .ok_or(NjoyError::SectionNotFound {
                mat: matd,
                mf: 6,
                mt: mtd,
            })?;
        let mut cur = SectionCursor::new(&section.rows);

        // `:7577-7601` — HEAD, frame flag, and the desired-particle ZA.
        let head = cur.read_cont()?;
        let nk = head.n1.max(0);
        let mut lct = head.l2;
        let lct3 = lct;
        let izat = head.c1.round() as i32;
        let zad: f64 = match mfd {
            3 => cfg.izap as f64,
            18 => 0.0,
            21 => 1001.0,
            22 => 1002.0,
            23 => 1003.0,
            24 => 2003.0,
            25 => 2004.0,
            26 => 9999.0,
            _ => 1.0,
        };
        if mfd == 18 {
            lct = 1;
        }
        let jzad = zad.round() as i32;

        let mut feed = Mf6Feed {
            cfg,
            izat,
            lct3,
            lct,
            awp: 0.0,
            aprime: 0.0,
            jzap: 0,
            disc102: 0.0,
            subs: Vec::new(),
            idis: 0,
            elo: EMAX,
            ehi: 0.0,
            langn: 1,
            lepn: 2,
            nne: 0,
            int_code: 2,
            no_gammas: false,
            messages: Vec::new(),
        };

        // `:7600-7633` — label 100: scan the NK subsections for `jzad`.
        let mut ik = 0;
        let (mut law, mut lip, mut yield_packed);
        loop {
            ik += 1;
            if ik > nk {
                if mfd == 18 {
                    // `:7957-7961` — special exit, no MF=6 gammas.
                    feed.no_gammas = true;
                    return Ok(feed);
                }
                return Err(NjoyError::EndfParse(
                    "getmf6: desired particle not found. (groupr.f90:7605)".into(),
                ));
            }
            let yt = cur.read_tab1()?;
            feed.disc102 = 0.0;
            let jzap = yt.head.c1.round() as i32;
            let mut awp = yt.head.c2;
            lip = yt.head.l1;
            law = yt.head.l2;
            // `:7620-7623` — a ZAP=0 / LAW=2 subsection is the relativistic
            // discrete capture gamma; `awp` doubles as its energy.
            if jzap == 0 && law == 2 {
                feed.disc102 = awp;
                awp = 0.0;
            }
            // `:7625-7628` — LCT=3 means "CM for light products, lab for heavy".
            if lct3 == 3 {
                feed.lct = 1;
                if (1..=2004).contains(&jzap) {
                    feed.lct = 2;
                }
            }
            feed.awp = awp;
            feed.aprime = if cfg.awrp != 0.0 { awp / cfg.awrp } else { awp };
            feed.jzap = jzap;
            if jzap == jzad || (mfd == 26 && jzap > 2004) {
                yield_packed = pack_tab1(&yt);
                break;
            }
            skip6(&mut cur, law)?;
        }

        // `:7636-7656` — LAW=4 backs up to the first subsection and re-reads
        // it as the recoil partner; either way it falls into `getdis`.
        loop {
            // Label 140 (`:7657-7663`).
            if feed.disc102 > 0.0 {
                return Err(NjoyError::NotPorted(
                    "groupr::getmf6 discrete relativistic capture gamma (gam102, \
                     groupr.f90:7620-7623, 8102-8105)",
                ));
            }
            if (2..=5).contains(&law) {
                return Err(NjoyError::NotPorted(
                    "groupr::getmf6 LAW=2..5 File-6 discrete two-body scattering \
                     (getdis with mft=8, groupr.f90:7936-7940, 8096-8100)",
                ));
            }
            if law > 7 {
                return Err(NjoyError::EndfParse(format!(
                    "getmf6: illegal law = {law}. (groupr.f90:7662)"
                )));
            }
            let sub = match law {
                1 => feed.read_law1(&mut cur, yield_packed.clone(), law)?,
                6 => {
                    return Err(NjoyError::NotPorted(
                        "groupr::getmf6 LAW=6 n-body phase space -> LAW=1 conversion \
                         (groupr.f90:7823-7875); needs f6psp (LANG=0) inside cm2lab",
                    ))
                }
                7 => feed.read_law7(&mut cur, yield_packed.clone(), law)?,
                _ => {
                    return Err(NjoyError::NotPorted(
                        "groupr::getmf6 LAW=0 (unknown distribution): upstream falls \
                         through the law dispatch leaving no tables at all \
                         (groupr.f90:7664-7913)",
                    ))
                }
            };
            let ne = sub.tables.len();
            feed.subs.push(sub);
            let _ = lip;

            // `:7915-7934` — another subsection for the same particle?
            if ik >= nk {
                break;
            }
            ik += 1;
            let yt = match cur.read_tab1() {
                Ok(t) => t,
                Err(_) => break,
            };
            if yt.head.c1.round() as i32 != feed.jzap {
                break;
            }
            law = yt.head.l2;
            lip = yt.head.l1;
            yield_packed = pack_tab1(&yt);
            let _ = ne;
        }

        // `:7942-7955` — initialization complete.
        if feed.subs.len() > 1 {
            feed.messages.push(
                "getmf6: there are multiple subsections in mf6 for this emitted particle"
                    .to_string(),
            );
        }
        Ok(feed)
    }

    /// `enext` as the initialization pass leaves it (`:7953`, or `-1` for the
    /// `:7957-7961` no-gammas exit) \[eV\].
    pub fn enext_init(&self) -> f64 {
        if self.no_gammas {
            -1.0
        } else {
            self.elo
        }
    }

    /// `true` when MF=6 carried no subsection for the requested particle and
    /// `mfd = 18` (`:7603`, `:7957-7961`).
    pub fn no_gammas(&self) -> bool {
        self.no_gammas
    }

    /// Diagnostics upstream would have written to `nsyso`.
    pub fn messages(&self) -> &[String] {
        &self.messages
    }

    /// `lct3` — the file-level `LCT` from the MF=6 HEAD (`:7582`), before the
    /// per-subsection `LCT = 3` resolution (`:7625-7628`).
    pub fn lct3(&self) -> i32 {
        self.lct3
    }

    /// `lct` — the effective reference frame of the matched particle
    /// (1 = lab, 2 = CM) after `:7589` / `:7625-7628`.
    pub fn lct(&self) -> i32 {
        self.lct
    }

    /// `jzap` — the emitted particle's `ZA` (`:7615`).
    pub fn jzap(&self) -> i32 {
        self.jzap
    }

    /// `nss` — how many MF=6 subsections were accumulated for this particle
    /// (`:7929-7933`).
    pub fn nss(&self) -> usize {
        self.subs.len()
    }

    /// `getmf6` at `ed > 0` — the normal entry (`:7963-8133`).
    ///
    /// `eg` is the `ng + 1` ascending secondary-group boundary array \[eV\]
    /// (Fortran `eg(1..ng+1)`); `nl` must equal the `nl` given to
    /// [`Mf6Feed::new`], because the stored lab tables were built with it.
    ///
    /// # Upstream defects reproduced here
    /// - **`idis` is write-only.** `:7983-7986` maintain a saved `idis`
    ///   (promote a tied discontinuity, adopt the flag of an earlier break)
    ///   but no statement ever assigns it to the `idisc` dummy before the
    ///   `:8084` / `:8108` exits. The caller therefore receives the *last*
    ///   `terpa` call's `idisc`, and `idis` has no observable effect. Kept
    ///   faithfully: [`Mf6FeedAt::idisc`] is the last `terpa` flag, and
    ///   `idis` is tracked but unused.
    /// - **`jgmax`'s search is unbounded.** `:8002-8005` walk `eg(jgmax+1)`
    ///   upward with no stop at `ng+1`, so an `ed` above the top boundary
    ///   reads past `eg`. This port clamps at `ng + 1` (the same value the
    ///   `q > 0` branch uses) instead of reading out of bounds.
    /// - **The panel search dereferences one table too many.** `:8015-8016`
    ///   set `jhi = jloss(jj+ie)` for `ie = 1..nne`; at `ie = nne` that index
    ///   is the *next* subsection's first table, or — for the last
    ///   subsection — never written at all. This port iterates the `nne - 1`
    ///   real brackets, which agrees with upstream for every `ed` inside
    ///   `[elo, ehi]`.
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] for a bad group structure or an `nl`
    /// mismatch; [`NjoyError::NotPorted`] propagated from `f6lab` for a
    /// tabulated `LANG`; [`NjoyError::NotConvergent`] if the secondary-energy
    /// march fails to terminate.
    pub fn feed(&mut self, ed: f64, eg: &[f64], nl: usize) -> Result<Mf6FeedAt, NjoyError> {
        if nl != self.cfg.nl {
            return Err(NjoyError::EndfParse(format!(
                "getmf6: nl = {nl} does not match the nl = {} the lab tables were built with",
                self.cfg.nl
            )));
        }
        if eg.len() < 2 {
            return Err(NjoyError::EndfParse(
                "getmf6: eg needs >= 2 ascending boundaries".into(),
            ));
        }
        let ng = eg.len() - 1;

        // `:7963-7971` — clear the answer.
        let mut ans = vec![vec![0.0_f64; ng]; nl];
        let mut enext = EMAX;
        let mut yld = 0.0_f64;
        let mut idisc = 0_i32;
        let mut jgmax = ng + 1;

        // `:7972` — label 207: loop over subsections.
        for iss in 0..self.subs.len() {
            let (pe, eihi, idis_here) = {
                let sub = &self.subs[iss];
                let mut ip = 2usize;
                let mut ir = 1usize;
                terpa(&sub.yield_packed, ed, &mut ip, &mut ir)?
            };
            idisc = idis_here;
            // `:7983-7986`.
            if (eihi - enext).abs() < enext * SMALL && idisc > self.idis {
                self.idis = idisc;
            }
            if eihi < enext * (1.0 - SMALL) {
                self.idis = idisc;
                enext = eihi;
            }
            if pe == 0.0 {
                continue; // `:7986` -> label 450
            }
            yld += pe;

            // `:7988-7997`.
            let (law, lang, lep, int_raw, nne) = {
                let sub = &self.subs[iss];
                (sub.law, sub.lang, sub.lep, sub.int_code, sub.tables.len())
            };
            let mut lepn = lep;
            if law == 1 && self.lct == 2 {
                lepn = 2;
            }
            if law == 7 {
                lepn = 2;
            }
            let mut langn = lang;
            if law == 1 && self.lct == 2 {
                langn = 1;
            }
            if law == 1 && self.lct == 3 && self.awp < ALIGHT {
                langn = 1;
            }
            if law == 7 {
                langn = 1;
            }
            self.langn = langn;
            self.lepn = lepn;

            // `:7999-8006` — find jgmax (see the clamp note above).
            jgmax = ng + 1;
            if self.cfg.q <= 0.0 && self.jzap != 0 {
                jgmax = 1;
                while jgmax < ng + 1 && eg[jgmax] < ed * (1.0 - SMALL) {
                    jgmax += 1;
                }
            }

            // `:8008-8024` — find the incident-energy panel.
            self.nne = nne;
            let mut int_code = int_raw;
            if int_code == 2 {
                int_code = 22;
            }
            self.int_code = int_code;
            let mut panel: Option<usize> = None;
            for ie in 0..nne.saturating_sub(1) {
                let lo = self.subs[iss].tables[ie].e_in();
                let hi = self.subs[iss].tables[ie + 1].e_in();
                if ed >= lo * (1.0 - SMALL) && ed <= hi * (1.0 + SMALL) {
                    self.elo = lo;
                    self.ehi = hi;
                    panel = Some(ie);
                    break;
                }
            }
            let Some(ie) = panel else {
                continue; // `:8025` -> label 450
            };

            let lo_tab = self.subs[iss].tables[ie].clone();
            let hi_tab = self.subs[iss].tables[ie + 1].clone();
            let lo_c = lo_tab.continuum();
            let hi_c = hi_tab.continuum();
            let lep_law = IntLaw::from_code(lepn.max(0) as u32);

            // `:8027-8050` — march the lab secondary-energy grid.
            if !lo_c.points.is_empty() && !hi_c.points.is_empty() && lo_c.e_in < hi_c.e_in {
                let mut ep = 0.0_f64;
                let (mut terml, mut epnext) = crate::groupr::kinematics::f6lab(
                    &lo_c,
                    &hi_c,
                    int_code,
                    langn.max(0) as u32,
                    lep_law,
                    ed,
                    ep,
                    nl,
                )?;
                let mut jg = 1usize;
                let mut steps = 0usize;
                loop {
                    let mut en = epnext;
                    if en >= EMAX * (1.0 - SMALL) {
                        break;
                    }
                    if jg < ng && ep >= eg[jg] * (1.0 - SMALL) {
                        jg += 1;
                    }
                    if jg < ng && eg[jg] < en * (1.0 - SMALL) {
                        en = eg[jg];
                    }
                    let (term, next) = crate::groupr::kinematics::f6lab(
                        &lo_c,
                        &hi_c,
                        int_code,
                        langn.max(0) as u32,
                        lep_law,
                        ed,
                        en,
                        nl,
                    )?;
                    epnext = next;
                    for l in 0..nl {
                        ans[l][jg - 1] += pe * (en - ep) * (term[l] + terml[l]) / 2.0;
                        terml[l] = term[l];
                    }
                    ep = en;
                    steps += 1;
                    if steps > MAX_MARCH_STEPS {
                        return Err(NjoyError::NotConvergent { routine: "getmf6" });
                    }
                }
            }

            // `:8052-8082` — label 420: add the discrete lab lines.
            if self.lct != 2 && (self.lct != 3 || self.awp >= ALIGHT) {
                let ndlo = lo_tab.nd();
                let lines_lo = lo_tab.discrete();
                let lines_hi = hi_tab.discrete();
                let awr = self.cfg.awr;
                for i in 0..ndlo {
                    let Some(llo) = lines_lo.get(i) else { break };
                    let Some(lhi) = lines_hi.get(i) else { break };
                    let mut el = llo.ep;
                    let mut eh = lhi.ep;
                    if el < 0.0 {
                        el = ed * awr / (awr + 1.0) - el;
                    }
                    if eh < 0.0 {
                        eh = ed * awr / (awr + 1.0) - eh;
                    }
                    let e0 = terp1(self.elo, el, self.ehi, eh, ed, IntLaw::LinLin)?;
                    let g0 = terp1(
                        self.elo,
                        llo.coeffs.first().copied().unwrap_or(0.0),
                        self.ehi,
                        lhi.coeffs.first().copied().unwrap_or(0.0),
                        ed,
                        IntLaw::LinLin,
                    )?;
                    for j in 1..=ng {
                        let e1 = if j == 1 { 0.0 } else { eg[j - 1] };
                        let e2 = if j == ng { EMAX } else { eg[j] };
                        if e0 >= e1 * (1.0 - SMALL) && e0 < e2 * (1.0 - SMALL) {
                            ans[0][j - 1] += pe * g0;
                        }
                    }
                }
            }
        }

        // `:8084-8089` — label 450.
        let iglo = 1usize;
        let ng2 = ng;
        let nq = 0usize;
        if ed < self.ehi * (1.0 - SMALL) && self.ehi < enext * (1.0 - SMALL) {
            enext = self.ehi;
        }

        // `:8108-8132` — label 700: normalization + upscatter fold-back.
        if ed > 0.0 && yld != 0.0 {
            let mut test = 0.0_f64;
            let jj = jgmax + 1 - iglo; // `jj = jgmax - iglo + 1`
            for i in 1..=ng2 {
                test += ans[0][i - 1];
                if i > jj && self.jzap == self.cfg.izap && self.cfg.q <= 0.0 && jj >= 1 && jj <= ng2
                {
                    for l in 0..nl {
                        ans[l][jj - 1] += ans[l][i - 1];
                        ans[l][i - 1] = 0.0;
                    }
                }
            }
            if test != 0.0 {
                if (test - yld).abs() > EPS && (ed < UP * self.elo || ed > DN * self.ehi) {
                    self.messages
                        .push(format!("getmf6: normalization {ed:12.4e} {test:15.6e}"));
                }
                let fac = yld / test;
                for row in ans.iter_mut() {
                    for v in row.iter_mut().take(ng2) {
                        *v *= fac;
                    }
                }
            }
        }

        Ok(Mf6FeedAt {
            ans,
            ng2,
            iglo,
            nq,
            yld,
            enext,
            idisc,
        })
    }

    /// `:7664-7822` — read a LAW=1 subsection, patch its grids, and transform
    /// it to the lab frame.
    fn read_law1(
        &mut self,
        cur: &mut SectionCursor<'_>,
        yield_packed: Vec<f64>,
        law: i32,
    ) -> Result<Mf6Sub, NjoyError> {
        // `:7668-7674`.
        let tab2 = cur.read_tab2()?;
        let lang = tab2.head.l1;
        let lep = tab2.head.l2;
        let ne = tab2.head.n2.max(0) as usize;
        // `int = nint(tmp(l+7))` is the *first* region's INT only — upstream
        // ignores any further interpolation regions in the TAB2.
        let mut int_code = tab2.interp.first().map(|p| p.1).unwrap_or(2);
        if int_code == 2 {
            int_code = 22;
        }

        let mut tables = Vec::with_capacity(ne);
        let mut nnn = 0usize;
        for ie in 0..ne {
            let list = cur.read_list()?;
            let mut buf = Vec::with_capacity(6 + list.data.len());
            buf.extend_from_slice(&[
                list.head.c1,
                list.head.c2,
                list.head.l1 as f64,
                list.head.l2 as f64,
                list.head.n1 as f64,
                list.head.n2 as f64,
            ]);
            buf.extend_from_slice(&list.data);

            // `:7686-7687`.
            let e_in = buf[1];
            if e_in < self.elo {
                self.elo = e_in;
            }
            if e_in > self.ehi {
                self.ehi = e_in;
            }
            // `:7688-7697`.
            let nn = buf[5].round().max(0.0) as usize;
            if nn == 0 {
                return Err(NjoyError::EndfParse(
                    "getmf6: LAW=1 LIST record has NEP = 0".into(),
                ));
            }
            let ncyc = (buf[4] / nn as f64).round().max(0.0) as usize;
            if ie > 0 && nn != nnn && (11..=20).contains(&int_code) {
                int_code += 10;
                self.messages.push(
                    "getmf6: bad grids for corresponding-point interpolation - changing to \
                     unit-base interpolation"
                        .to_string(),
                );
            }
            nnn = nn;

            law1_low_energy_patch(&mut buf, nn, lep, ncyc, self.cfg.awr, &mut self.messages);
            if self.cfg.ismooth && self.jzap == 1 && lep == 1 {
                ismooth_histogram(&mut buf, &mut self.messages);
            } else if self.cfg.ismooth && self.jzap == 1 && lep == 2 {
                ismooth_linlin(&mut buf, &mut self.messages);
            }

            // `:7812-7820` — CM -> lab.
            if self.lct == 2 || (self.lct == 3 && self.awp <= ALIGHT) {
                let emission = self.build_cm_emission(&buf, lang, lep)?;
                let dist = cm2lab(&emission, self.cfg.nl)?;
                buf = pack_lab_list(e_in, &dist.points, self.cfg.nl);
            }
            tables.push(LabTable { buf });
        }

        Ok(Mf6Sub {
            yield_packed,
            law,
            lang,
            lep,
            int_code,
            tables,
        })
    }

    /// `:7876-7913` — read a LAW=7 subsection and convert it with `ll2lab`.
    ///
    /// Upstream never assigns the saved `int` on this path; the normal entry
    /// re-reads it from the stored TAB2 header (`ddmf6(iz+7)`, `:8010`), which
    /// is this TAB2's first region INT. That is what is stored here.
    fn read_law7(
        &mut self,
        cur: &mut SectionCursor<'_>,
        yield_packed: Vec<f64>,
        law: i32,
    ) -> Result<Mf6Sub, NjoyError> {
        let tab2 = cur.read_tab2()?;
        let lang = tab2.head.l1;
        let lep = tab2.head.l2;
        let ne = tab2.head.n2.max(0) as usize;
        let int_code = tab2.interp.first().map(|p| p.1).unwrap_or(2);

        let mut tables = Vec::with_capacity(ne);
        for _ in 0..ne {
            let mu_tab2 = cur.read_tab2()?;
            let e_in = mu_tab2.head.c2;
            if e_in < self.elo {
                self.elo = e_in;
            }
            if e_in > self.ehi {
                self.ehi = e_in;
            }
            let nmu = mu_tab2.head.n2.max(0) as usize;
            let mut curves = Vec::with_capacity(nmu);
            for _ in 0..nmu {
                let t = cur.read_tab1()?;
                curves.push(Law7Curve {
                    mu: t.head.c2,
                    f_of_ep: t,
                });
            }
            let emission = Law7Emission { e_in, curves };
            let dist = ll2lab(&emission, self.cfg.nl)?;
            tables.push(LabTable {
                buf: pack_lab_list(e_in, &dist.points, self.cfg.nl),
            });
        }

        Ok(Mf6Sub {
            yield_packed,
            law,
            lang,
            lep,
            int_code,
            tables,
        })
    }

    /// Build the [`Cm6Emission`] `cm2lab` consumes from one raw LAW=1 LIST
    /// record (`cnow` in `f6cm`, `:8260-8518`).
    fn build_cm_emission(
        &self,
        buf: &[f64],
        lang: i32,
        lep: i32,
    ) -> Result<Cm6Emission, NjoyError> {
        let tbl = LabTable { buf: buf.to_vec() };
        let n = tbl.stored_records();
        let nd = tbl.nd().min(n);
        let cm_lang = match lang {
            1 => Cm6Lang::LegendreInCm,
            2 => Cm6Lang::Kalbach,
            0 => {
                return Err(NjoyError::NotPorted(
                    "groupr::getmf6 LANG=0 (phase space) inside a LAW=1 CM subsection \
                     — f6psp (groupr.f90:8520-8717 dispatch) is not ported",
                ))
            }
            _ => {
                return Err(NjoyError::NotPorted(
                    "groupr::getmf6 LANG=11..15 (tabulated CM angular distribution) \
                     — f6ddx's tabulated branch (groupr.f90:8664-8717) is not ported",
                ))
            }
        };
        Ok(Cm6Emission {
            e_in: tbl.e_in(),
            lang: cm_lang,
            lep: IntLaw::from_code(lep.max(0) as u32),
            awr_target: self.cfg.awr,
            awp_emitted: self.aprime,
            za_projectile: self.cfg.izap,
            za_target: self.izat,
            za_emitted: self.jzap,
            points: (nd..n).map(|j| tbl.record(j)).collect(),
            discrete: (0..nd).map(|j| tbl.record(j)).collect(),
        })
    }
}
