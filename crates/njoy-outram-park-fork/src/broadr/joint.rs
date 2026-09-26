// Ported from NJOY2016 `src/broadr.f90` and `src/mathm.f90`
// (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine broadr`, l.354-1000 — reaction selection (l.465-540), the
//     union-grid table (l.470-495, 530-562), `emtr` (l.592-600), and the
//     output assembly (l.935-1000, labels 245-310);
//   - `subroutine bfile3`, l.1152-1253 — the three-page paging;
//   - `subroutine broadn`, l.1255-1508 — the joint node walk and midpoint stack;
//   - `subroutine bsigma`, l.1510-1682; `hunky` l.1762-1804; `funky`
//     l.1806-1839; `function hnabb` l.1841-1947;
//   - `mathm.f90`: `erfc` l.447-634 (SLATEC `derfc`), `csevl` l.705-742,
//     `initds` l.1121-1150.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! BROADR as upstream runs it: every low-threshold reaction broadened
//! **together**, on one union grid, by one walk.
//!
//! [`super::broadn::broadn_section`] broadens one reaction at a time on its
//! own grid (`nreac = 1`). That was the only option while RECONR kept a grid
//! per reaction. RECONR now writes every reaction on `lunion`'s union grid,
//! so upstream's joint form can be reproduced. In that form the slope-sign
//! node test and the convergence test run over all reactions at once, and
//! the output is one grid: MT=1's, which ACER takes as the ESZ grid
//! (`acefc.f90:5345`).
//!
//! Everything that decides where a point lands is ported line for line.
//! That includes paging (`bfile3` forces a node at every page end), the
//! `nstack = 12` ceiling, the unrounded midpoint in `broadn`'s one
//! `sigfig`-free branch, the energy round trip through velocity
//! (`et = e(k)**2/alpha`), and the kernel's own numerics: SLATEC `erfc`,
//! plus `hnabb`'s Taylor series when `f(a) - f(b)` loses significance.
//!
//! **`stounx`/`getunx` are a no-op here, and are not ported.** They subtract
//! and re-add MF=2/MT=152's infinitely-dilute cross section where the
//! resolved and unresolved ranges overlap, but only when MT=152 flags an
//! overlap point with a negative energy (`nunr = 0` otherwise,
//! `broadr.f90:1101-1104`). RECONR's `genunr` (`reconr.f90:1628-1735`) never
//! writes a negative energy, so on a RECONR PENDF, the only input this
//! function takes, upstream skips the correction as well.

use crate::{
    common::phys::BK_EV_PER_K,
    endf::{
        gety1::Gety1,
        records::{Cont, Tab1},
    },
    mixr::mix::sigfig,
    reconr::{ReconrResult, ReconrSection},
};

use super::BroadnTolerances;

// ── SLATEC erfc (mathm.f90:447-634) ──────────────────────────────────────────

#[allow(clippy::excessive_precision, clippy::unreadable_literal)]
const ERFCS: [f64; 21] = [
    -0.49046121234691808039984544033376e-1,
    -0.14226120510371364237824741899631e+0,
    0.10035582187599795575754676712933e-1,
    -0.57687646997674847650827025509167e-3,
    0.27419931252196061034422160791471e-4,
    -0.11043175507344507604135381295905e-5,
    0.38488755420345036949961311498174e-7,
    -0.11808582533875466969631751801581e-8,
    0.32334215826050909646402930953354e-10,
    -0.79910159470045487581607374708595e-12,
    0.17990725113961455611967245486634e-13,
    -0.37186354878186926382316828209493e-15,
    0.71035990037142529711689908394666e-17,
    -0.12612455119155225832495424853333e-18,
    0.20916406941769294369170500266666e-20,
    -0.32539731029314072982364160000000e-22,
    0.47668672097976748332373333333333e-24,
    -0.65980120782851343155199999999999e-26,
    0.86550114699637626197333333333333e-28,
    -0.10788925177498064213333333333333e-29,
    0.12811883993017002666666666666666e-31,
];

#[allow(clippy::excessive_precision, clippy::unreadable_literal)]
const ERC2CS: [f64; 49] = [
    -0.6960134660230950112739150826197e-1,
    -0.4110133936262089348982212084666e-1,
    0.3914495866689626881561143705244e-2,
    -0.4906395650548979161280935450774e-3,
    0.7157479001377036380760894141825e-4,
    -0.1153071634131232833808232847912e-4,
    0.1994670590201997635052314867709e-5,
    -0.3642666471599222873936118430711e-6,
    0.6944372610005012589931277214633e-7,
    -0.1371220902104366019534605141210e-7,
    0.2788389661007137131963860348087e-8,
    -0.5814164724331161551864791050316e-9,
    0.1238920491752753181180168817950e-9,
    -0.2690639145306743432390424937889e-10,
    0.5942614350847910982444709683840e-11,
    -0.1332386735758119579287754420570e-11,
    0.3028046806177132017173697243304e-12,
    -0.6966648814941032588795867588954e-13,
    0.1620854541053922969812893227628e-13,
    -0.3809934465250491999876913057729e-14,
    0.9040487815978831149368971012975e-15,
    -0.2164006195089607347809812047003e-15,
    0.5222102233995854984607980244172e-16,
    -0.1269729602364555336372415527780e-16,
    0.3109145504276197583836227412951e-17,
    -0.7663762920320385524009566714811e-18,
    0.1900819251362745202536929733290e-18,
    -0.4742207279069039545225655999965e-19,
    0.1189649200076528382880683078451e-19,
    -0.3000035590325780256845271313066e-20,
    0.7602993453043246173019385277098e-21,
    -0.1935909447606872881569811049130e-21,
    0.4951399124773337881000042386773e-22,
    -0.1271807481336371879608621989888e-22,
    0.3280049600469513043315841652053e-23,
    -0.8492320176822896568924792422399e-24,
    0.2206917892807560223519879987199e-24,
    -0.5755617245696528498312819507199e-25,
    0.1506191533639234250354144051199e-25,
    -0.3954502959018796953104285695999e-26,
    0.1041529704151500979984645051733e-26,
    -0.2751487795278765079450178901333e-27,
    0.7290058205497557408997703680000e-28,
    -0.1936939645915947804077501098666e-28,
    0.5160357112051487298370054826666e-29,
    -0.1378419322193094099389644800000e-29,
    0.3691326793107069042251093333333e-30,
    -0.9909389590624365420653226666666e-31,
    0.2666491705195388413323946666666e-31,
];

#[allow(clippy::excessive_precision, clippy::unreadable_literal)]
const ERFCCS: [f64; 59] = [
    0.715179310202924774503697709496e-1,
    -0.265324343376067157558893386681e-1,
    0.171115397792085588332699194606e-2,
    -0.163751663458517884163746404749e-3,
    0.198712935005520364995974806758e-4,
    -0.284371241276655508750175183152e-5,
    0.460616130896313036969379968464e-6,
    -0.822775302587920842057766536366e-7,
    0.159214187277090112989358340826e-7,
    -0.329507136225284321486631665072e-8,
    0.722343976040055546581261153890e-9,
    -0.166485581339872959344695966886e-9,
    0.401039258823766482077671768814e-10,
    -0.100481621442573113272170176283e-10,
    0.260827591330033380859341009439e-11,
    -0.699111056040402486557697812476e-12,
    0.192949233326170708624205749803e-12,
    -0.547013118875433106490125085271e-13,
    0.158966330976269744839084032762e-13,
    -0.472689398019755483920369584290e-14,
    0.143587337678498478672873997840e-14,
    -0.444951056181735839417250062829e-15,
    0.140481088476823343737305537466e-15,
    -0.451381838776421089625963281623e-16,
    0.147452154104513307787018713262e-16,
    -0.489262140694577615436841552532e-17,
    0.164761214141064673895301522827e-17,
    -0.562681717632940809299928521323e-18,
    0.194744338223207851429197867821e-18,
    -0.682630564294842072956664144723e-19,
    0.242198888729864924018301125438e-19,
    -0.869341413350307042563800861857e-20,
    0.315518034622808557122363401262e-20,
    -0.115737232404960874261239486742e-20,
    0.428894716160565394623737097442e-21,
    -0.160503074205761685005737770964e-21,
    0.606329875745380264495069923027e-22,
    -0.231140425169795849098840801367e-22,
    0.888877854066188552554702955697e-23,
    -0.344726057665137652230718495566e-23,
    0.134786546020696506827582774181e-23,
    -0.531179407112502173645873201807e-24,
    0.210934105861978316828954734537e-24,
    -0.843836558792378911598133256738e-25,
    0.339998252494520890627359576337e-25,
    -0.137945238807324209002238377110e-25,
    0.563449031183325261513392634811e-26,
    -0.231649043447706544823427752700e-26,
    0.958446284460181015263158381226e-27,
    -0.399072288033010972624224850193e-27,
    0.167212922594447736017228709669e-27,
    -0.704599152276601385638803782587e-28,
    0.297976840286420635412357989444e-28,
    -0.126252246646061929722422632994e-28,
    0.539543870454248793985299653154e-29,
    -0.238099288253145918675346190062e-29,
    0.109905283010276157359726683750e-29,
    -0.486771374164496572732518677435e-30,
    0.152587726411035756763200828211e-30,
];

/// SLATEC `csevl` (`mathm.f90:705-742`): a Chebyshev series of `n` terms.
fn csevl(x: f64, cs: &[f64], n: usize) -> f64 {
    let (mut b0, mut b1, mut b2) = (0.0f64, 0.0f64, 0.0f64);
    let twox = 2.0 * x;
    for i in 1..=n {
        b2 = b1;
        b1 = b0;
        b0 = twox * b1 - b2 + cs[n - i];
    }
    let _ = b2;
    (b0 - b2) / 2.0
}

/// SLATEC `initds` (`mathm.f90:1121-1150`). The error sum adds each
/// coefficient **as a default (single-precision) real**, `abs(real(os(i)))`.
fn initds(os: &[f64], eta: f64) -> usize {
    let nos = os.len();
    let mut err = 0.0f64;
    let mut ii = 0usize;
    let mut i = 0usize;
    while err <= eta && ii < nos {
        ii += 1;
        i = nos + 1 - ii;
        err += f64::from((os[i - 1] as f32).abs());
    }
    i
}

struct ErfcConsts {
    nterf: usize,
    nterfc: usize,
    nterc2: usize,
    xsml: f64,
    xmax: f64,
    sqeps: f64,
}

fn erfc_consts() -> &'static ErfcConsts {
    static C: std::sync::OnceLock<ErfcConsts> = std::sync::OnceLock::new();
    C.get_or_init(|| {
        let d1mach1 = f64::MIN_POSITIVE; // tiny(one)
        let d1mach3 = 2.0f64.powi(-53); // two**(-digits(one))
        let eta = d1mach3 / 10.0;
        let txmax = (-(SQRTPI * d1mach1).ln()).sqrt();
        ErfcConsts {
            nterf: initds(&ERFCS, eta),
            nterfc: initds(&ERFCCS, eta),
            nterc2: initds(&ERC2CS, eta),
            xsml: -(-(SQRTPI * d1mach3).ln()).sqrt(),
            xmax: txmax - txmax.ln() / (2.0 * txmax) - 1.0 / 10.0,
            sqeps: (2.0 * d1mach3).sqrt(),
        }
    })
}

#[allow(clippy::excessive_precision)]
const SQRTPI: f64 = 1.772_453_850_905_516_027_298_167_483_341_15;

/// SLATEC `derfc` as NJOY carries it (`mathm.f90:447-634`).
fn erfc(x: f64) -> f64 {
    let c = erfc_consts();
    if x <= c.xsml {
        return 2.0;
    }
    if x > c.xmax {
        return 0.0;
    }
    let mut y = x.abs();
    if y <= 1.0 {
        if y < c.sqeps {
            return 1.0 - 2.0 * x / SQRTPI;
        }
        return 1.0 - x * (1.0 + csevl(2.0 * x * x - 1.0, &ERFCS, c.nterf));
    }
    y *= y;
    let mut v = if y <= 4.0 {
        let xx = (8.0 / y - 5.0) / 3.0;
        (-y).exp() / x.abs() * (1.0 / 2.0 + csevl(xx, &ERC2CS, c.nterc2))
    } else {
        let xx = 8.0 / y - 1.0;
        (-y).exp() / x.abs() * (1.0 / 2.0 + csevl(xx, &ERFCCS, c.nterfc))
    };
    if x < 0.0 {
        v = 2.0 - v;
    }
    v
}

// ── funky / hunky / hnabb (broadr.f90:1762-1947) ─────────────────────────────

/// `f(n,a) = ∫_a^∞ z^n exp(-z²) dz / √π`, n = 0..4 (`funky`).
fn funky(aa: f64) -> [f64; 5] {
    const ALIM: f64 = 10.0;
    let resqpi = 1.0 / std::f64::consts::PI.sqrt();
    let a = aa;
    let asq = a * a;
    let mut expo = 0.0;
    if a < ALIM {
        expo = (-asq).exp();
    }
    let mut f = [0.0f64; 5];
    if a < ALIM {
        f[0] = 0.5 * erfc(a);
    }
    expo *= resqpi;
    f[1] = 0.5 * expo;
    expo *= a;
    f[2] = 0.5 * (f[0] + expo);
    expo *= a;
    f[3] = 0.5 * (2.0 * f[1] + expo);
    expo *= a;
    f[4] = 0.5 * (3.0 * f[2] + expo);
    f
}

/// `hnabb(n, a, b)` (`broadr.f90:1841-1947`): `h_n(a, b)` by a direct Taylor
/// expansion of the defining integral, for `b - a` small.
#[allow(clippy::many_single_char_names)]
fn hnabb(n: i32, aa: f64, bb: f64) -> f64 {
    const AERR: f64 = 1.0e30;
    const RERR: f64 = 1.0e-8;
    #[allow(clippy::excessive_precision)]
    const POW2: [f64; 5] = [
        1.414_213_562_373_1,
        2.0,
        2.828_427_124_746_2,
        4.0,
        5.656_854_249_492_4,
    ];
    const EXPLIM: f64 = 100.0;
    let resqpi = 1.0 / std::f64::consts::PI.sqrt();
    let (a, b);
    let mut sign = 1.0f64;
    if bb < aa {
        a = bb.abs();
        b = aa.abs();
        sign = -sign;
    } else {
        a = aa.abs();
        b = bb.abs();
    }
    if bb < 0.0 && n % 2 != 0 {
        sign = -sign;
    }
    let h = (b - a) * POW2[0];
    let x = POW2[0] * a;
    let xx = x * x;
    let asq = a * a;
    let mut con = 0.0;
    if asq < EXPLIM {
        con = (-asq).exp() * resqpi / POW2[n as usize];
    }
    let mut mflag = 0;
    let mut k: i32 = n;
    let mut kd: i32 = 0;
    let mut cm = [0.0f64; 51];
    let mut cmstar = [0.0f64; 51];
    cm[1] = 1.0;
    let mut xk = 1.0;
    if k != 0 {
        xk = x.powi(k);
    }
    let mut s = h * xk;
    let mut fact = h;
    for m in 2..=50i32 {
        fact = fact * h / f64::from(m);
        let kstar = k;
        let kdstar = kd + 1;
        for j in 1..=kdstar as usize {
            cmstar[j] = cm[j];
        }
        k = n - m + 1;
        if k < 0 {
            let kk = k % 2;
            k = 0;
            if kk != 0 {
                k = 1;
            }
        }
        kd = (n + m - 1 - k) / 2;
        cm[(kd + 1) as usize] = -cmstar[kdstar as usize];
        let mut qmn = cm[(kd + 1) as usize];
        if kd != 0 {
            for j in 1..=kd {
                let jalpha = (2 * j + k - 1 - kstar) / 2;
                let mut beta = 0.0;
                if jalpha != 0 {
                    beta = cmstar[jalpha as usize];
                }
                cm[j as usize] = f64::from(2 * j + k - 1) * cmstar[(jalpha + 1) as usize] - beta;
            }
            for j in 1..=kd {
                qmn = qmn * xx + cm[(kd + 1 - j) as usize];
            }
        }
        xk = 1.0;
        if k != 0 {
            xk = x.powi(k);
        }
        let term = fact * xk * qmn;
        s += term;
        let xn1 = f64::from(n + 1);
        let xn2 = (h * x).abs();
        if f64::from(m) < xn1.max(xn2) {
            continue;
        }
        let test = AERR + RERR * s.abs();
        if term.abs() > test {
            mflag = 0;
            continue;
        }
        if mflag == 1 {
            break;
        }
        mflag = 1;
    }
    con * s * sign
}

/// The module-level variables `funky`/`hunky`/`bsigma` share
/// (`broadr.f90:40-41`: `f, aa, x, y, oy, xx, yy, h, alast, s1, s2`).
#[derive(Debug, Clone, Default)]
struct Kernel {
    f: [f64; 5],
    h: [f64; 5],
    aa: f64,
    alast: f64,
    oy: f64,
    xx: f64,
    yy: f64,
    s1: f64,
    s2: f64,
}

impl Kernel {
    /// `hunky` (`broadr.f90:1762-1804`).
    fn hunky(&mut self) {
        const SMALL: f64 = 1.0e-12;
        const TOLER: f64 = 1.0e-5;
        self.h = self.f;
        self.f = funky(self.aa);
        for k in 0..5 {
            self.h[k] -= self.f[k];
            if self.h[k].abs() <= SMALL * self.f[k].abs() {
                self.h[k] = 0.0;
            }
            if self.h[k].abs() <= TOLER * self.f[k].abs() && self.aa != self.alast {
                self.h[k] = hnabb(k as i32, self.alast, self.aa);
            }
        }
        self.alast = self.aa;
        let (h, oy, yy, xx) = (&self.h, self.oy, self.yy, self.xx);
        self.s1 = (h[2] * oy + 2.0 * h[1]) * oy + h[0];
        self.s2 = ((h[4] + (6.0 * yy - xx) * h[2]) * oy + (4.0 * h[3] + (4.0 * yy - 2.0 * xx) * h[1]))
            * oy
            + (yy - xx) * h[0];
    }
}

const FZERO: [f64; 5] = [
    0.5,
    1.0 / (2.0 * 1.772_453_850_905_516),
    0.25,
    1.0 / (2.0 * 1.772_453_850_905_516),
    0.375,
];

// ── the paged table and the joint walk ───────────────────────────────────────

const NSTACK: usize = 12;
const NMAX: usize = 10;
const THERM: f64 = 0.0253;
const ESTP: f64 = 4.1;
const HALF: f64 = 0.5;
const SMALL: f64 = 1.0e-9;
const STEP: f64 = 2.01;
const RMAX: f64 = 3.0;
const ERRMIN: f64 = 1.0e-15;
const TRANGE: f64 = 0.4999;
const SSMALL: f64 = 1.0e-6;
const TENTH: f64 = 0.1;
/// `namax` (`broadr.f90:180`): the words `bfile3` pages through.
const NAMAX: usize = 15_000_000;

/// `broadn`'s `save`d locals (`broadr.f90:1311`) and the output table.
struct Walk {
    nreac: usize,
    alpha: f64,
    thnmax: f64,
    tol: BroadnTolerances,
    emtr: Vec<f64>,
    /// The whole input table in velocity units `sqrt(alpha*E)`, and
    /// `s[g*nreac + i]`. `bfile3` only ever holds three pages; which three
    /// is tracked by `off` (local index `L` is global `L + off`, 1-based).
    vel: Vec<f64>,
    sig: Vec<f64>,
    off: isize,
    klow: usize,
    khigh: usize,
    nlow: usize,
    nhigh: usize,
    ker: Kernel,
    y: f64,
    // saved stack
    ks: [usize; NSTACK + 1],
    js: [i32; NSTACK + 1],
    es: [f64; NSTACK + 1],
    ss: Vec<f64>, // ss[(is)*nreac + i]
    tt: Vec<f64>,
    sn: Vec<f64>,
    dl: Vec<f64>,
    /// output rows, `1 + nreac` words each
    out: Vec<f64>,
}

impl Walk {
    fn g(&self, l: usize) -> Option<usize> {
        let g = l as isize + self.off;
        if g >= 1 && (g as usize) <= self.vel.len() {
            Some(g as usize - 1)
        } else {
            None
        }
    }
    /// `e(l)`; a slot `bfile3` never loaded reads as zero.
    fn e(&self, l: usize) -> f64 {
        self.g(l).map_or(0.0, |g| self.vel[g])
    }
    fn s(&self, i: usize, l: usize) -> f64 {
        self.g(l).map_or(0.0, |g| self.sig[g * self.nreac + i])
    }

    /// `bsigma(kn, en, sn)` (`broadr.f90:1510-1682`), into `self.sn` or the
    /// stack slot given.
    fn bsigma(&mut self, kn: usize, en: f64) -> Vec<f64> {
        const ATOP: f64 = 4.0;
        const SIGMIN: f64 = 1.0e-15;
        let nreac = self.nreac;
        let (klow, khigh, nlow, nhigh) = (self.klow, self.khigh, self.nlow, self.nhigh);
        let mut aamin = ATOP;
        self.ker.aa = 0.0;
        let khm1 = khigh - 1;
        let mut sbt = vec![0.0f64; nreac];

        // search for panel containing en
        let mut k = kn;
        if k < khigh && self.e(k + 1) >= self.e(k) {
            while k < khigh && en >= self.e(k + 1) {
                k += 1;
            }
        }

        self.y = en;
        self.ker.oy = -1.0 / self.y;
        self.ker.yy = self.y * self.y;
        self.ker.f = FZERO;
        self.ker.alast = 0.0;

        // loop over intervals below current point
        let mut truncated = false;
        let mut l = k + 1;
        for _ll in klow..=k {
            l -= 1;
            let x = self.e(l);
            let xp = self.e(l + 1);
            self.ker.xx = xp * xp;
            if self.ker.xx <= x * x {
                continue;
            }
            self.ker.aa = self.y - x;
            self.ker.hunky();
            let denom = 1.0 / (self.ker.xx - x * x);
            for (i, sb) in sbt.iter_mut().enumerate() {
                let slope = (self.s(i, l + 1) - self.s(i, l)) * denom;
                *sb += self.s(i, l + 1) * self.ker.s1 + slope * self.ker.s2;
            }
            if self.ker.aa > ATOP {
                truncated = true;
                break;
            }
        }
        if !truncated {
            if self.ker.aa < aamin && nlow != klow {
                aamin = self.ker.aa;
            }
            // continue cross sections as 1/v to x=0
            self.ker.xx = 0.0;
            self.ker.aa = self.y;
            self.ker.hunky();
            let (oy, h) = (self.ker.oy, self.ker.h);
            let ek = self.e(klow);
            for (i, sb) in sbt.iter_mut().enumerate() {
                *sb -= self.s(i, klow) * ek * (oy * oy * h[1] + oy * h[0]);
            }
        }

        // label 170
        self.ker.f = FZERO;
        self.ker.alast = 0.0;
        self.ker.oy = -self.ker.oy;
        let mut to_210 = false;
        if k != khigh {
            for l in k..=khm1 {
                let x = self.e(l + 1);
                let xm = self.e(l);
                self.ker.xx = xm * xm;
                if self.ker.xx >= x * x {
                    continue;
                }
                self.ker.aa = x - self.y;
                self.ker.hunky();
                let denom = 1.0 / (x * x - self.ker.xx);
                for (i, sb) in sbt.iter_mut().enumerate() {
                    let slope = (self.s(i, l + 1) - self.s(i, l)) * denom;
                    *sb += self.s(i, l) * self.ker.s1 + slope * self.ker.s2;
                }
                if self.ker.aa > ATOP {
                    to_210 = true;
                    break;
                }
            }
            if !to_210 && self.ker.aa < aamin && nhigh != khigh {
                aamin = self.ker.aa;
            }
        }
        if !to_210 {
            // label 200: continue as constant to x=infinity
            let (f, oy) = (self.ker.f, self.ker.oy);
            let factor = (f[2] * oy + 2.0 * f[1]) * oy + f[0];
            for (i, sb) in sbt.iter_mut().enumerate() {
                *sb += self.s(i, khigh) * factor;
            }
        }

        // label 210: low-energy term
        self.y = -self.y;
        self.ker.aa = -self.y;
        'lbl230: {
            if self.ker.aa > ATOP {
                break 'lbl230;
            }
            self.ker.f = funky(self.ker.aa);
            self.ker.alast = self.ker.aa;
            self.ker.oy = -self.ker.oy;
            let ek = self.e(klow);
            self.ker.aa = ek - self.y;
            self.ker.xx = 0.0;
            self.ker.hunky();
            let (oy, h) = (self.ker.oy, self.ker.h);
            for (i, sb) in sbt.iter_mut().enumerate() {
                *sb -= self.s(i, klow) * ek * (oy * oy * h[1] + oy * h[0]);
            }
            if self.ker.aa > ATOP {
                break 'lbl230;
            }
            for l in klow..=khm1 {
                let x = self.e(l + 1);
                self.ker.aa = x - self.y;
                self.ker.xx = self.e(l) * self.e(l);
                if self.ker.xx == x * x {
                    continue;
                }
                self.ker.hunky();
                let denom = 1.0 / (x * x - self.ker.xx);
                for (i, sb) in sbt.iter_mut().enumerate() {
                    let slope = (self.s(i, l + 1) - self.s(i, l)) * denom;
                    *sb -= self.s(i, l) * self.ker.s1 + slope * self.ker.s2;
                }
                if self.ker.aa > ATOP {
                    break 'lbl230;
                }
            }
            let (f, oy) = (self.ker.f, self.ker.oy);
            let factor = (f[2] * oy + 2.0 * f[1]) * oy + f[0];
            for (i, sb) in sbt.iter_mut().enumerate() {
                *sb -= self.s(i, khigh) * factor;
            }
        }

        // label 230
        for sb in &mut sbt {
            if *sb < SIGMIN {
                *sb = 0.0;
            }
        }
        let _ = aamin; // upstream only prints a truncation warning
        sbt
    }

    fn set_ss(&mut self, is: usize, v: &[f64]) {
        let n = self.nreac;
        self.ss[is * n..is * n + n].copy_from_slice(v);
    }
    fn ssv(&self, i: usize, is: usize) -> f64 {
        self.ss[is * self.nreac + i]
    }

    /// Emit one output record (`loada(j, tt, ...)`).
    fn emit(&mut self) {
        self.out.extend_from_slice(&self.tt);
    }

    /// `slope sign` of reaction `i` at `k` with the 1/1000 dead band.
    fn dn_sign(&self, i: usize, k: usize) -> f64 {
        let mut dn = self.s(i, k + 1) - self.s(i, k);
        if dn.abs() < self.s(i, k).abs() / 1000.0 {
            dn = 0.0;
        }
        if dn >= 0.0 {
            1.0
        } else {
            -1.0
        }
    }

    /// `broadn(j, ...)` (`broadr.f90:1255-1508`) over the current middle page.
    /// Returns `false` once the last point has been written (`j < 0`).
    #[allow(clippy::too_many_lines)]
    fn broadn(&mut self, j: &mut i64) -> bool {
        let nreac = self.nreac;
        let alpha = self.alpha;
        let thnmax = self.thnmax;
        let tol = self.tol;
        let mut k = self.nlow;
        let mut klast = self.nlow - 1;
        let mut tt1old = 0.0f64;

        #[derive(PartialEq)]
        enum L {
            L120,
            L130,
            L180,
            L190,
        }
        let mut next;
        if self.klow == 1 {
            // label 110
            next = if self.es[1] > thnmax { L::L190 } else { L::L120 };
        } else {
            // first node in stack
            klast = k;
            self.ks[2] = k;
            let et = self.e(k) * self.e(k) / alpha;
            self.es[2] = sigfig(et, 7, 0);
            let xt = (alpha * self.es[2]).sqrt();
            for i in 0..nreac {
                let mut dn = self.s(i, k + 1) - self.s(i, k);
                if dn.abs() < self.s(i, k).abs() / 1000.0 {
                    dn = 0.0;
                }
                self.dl[i] = if dn < 0.0 { -1.0 } else { 1.0 };
            }
            let v = self.bsigma(k, xt);
            self.set_ss(2, &v);
            self.js[2] = 0;
            for i in 0..nreac {
                if (self.ssv(i, 2) - self.s(i, k)).abs() > tol.errthn * self.s(i, k) {
                    self.js[2] = 1;
                }
            }
            k += 1;
            next = L::L120;
        }

        loop {
            match next {
                L::L120 => {
                    // locate next node for stack
                    let mut et;
                    loop {
                        et = self.e(k) * self.e(k) / alpha;
                        if et >= thnmax || k >= self.nhigh {
                            break;
                        }
                        let test = sigfig(et, 7, 0);
                        if (self.es[2] - test).abs() < SMALL * test {
                            k += 1;
                            continue;
                        }
                        if k > klast + NMAX || et > STEP * self.es[2] {
                            break;
                        }
                        let test = sigfig(et, 3, 0);
                        if (et - test).abs() < SMALL * test || (et - THERM).abs() < SMALL * THERM {
                            break;
                        }
                        if (0..nreac).any(|i| self.dn_sign(i, k) != self.dl[i]) {
                            break;
                        }
                        k += 1;
                    }
                    // label 130
                    self.es[1] = sigfig(et, 7, 0);
                    let xt = (alpha * self.es[1]).sqrt();
                    let v = self.bsigma(k, xt);
                    self.set_ss(1, &v);
                    self.ks[1] = k;
                    self.js[1] = 0;
                    for i in 0..nreac {
                        if (self.ssv(i, 1) - self.s(i, k)).abs() > tol.errthn * self.s(i, k) {
                            self.js[1] = 1;
                        }
                    }
                    next = L::L130;
                }
                L::L130 => {
                    // labels 140-170: add points between nodes if needed
                    let mut is = 2usize;
                    'stack: loop {
                        // label 140
                        let mut converged = is >= NSTACK
                            || (self.ks[is - 1] == self.ks[is] + 1
                                && self.js[is - 1] == 0
                                && self.js[is] == 0);
                        let mut em = 0.0;
                        if !converged {
                            em = HALF * (self.es[is - 1] + self.es[is]);
                            let mut ndig = 9;
                            if em > TENTH && em < 1.0 {
                                ndig = 8;
                            }
                            if em > sigfig(self.es[is], 7, 1) {
                                if em < sigfig(self.es[is - 1], 7, -1) {
                                    em = sigfig(em, 7, 0);
                                }
                            } else {
                                em = sigfig(em, ndig, 0);
                            }
                            if em < sigfig(self.es[is], ndig, 1) || em > sigfig(self.es[is - 1], ndig, -1) {
                                converged = true;
                            }
                        }
                        let mut push = false;
                        if !converged {
                            let mut errt = tol.errthn;
                            if self.es[is - 1] < TRANGE {
                                errt /= 5.0;
                            }
                            let mut errm = tol.errmax;
                            if self.es[is - 1] < TRANGE {
                                errm /= 5.0;
                            }
                            let xm = (alpha * em).sqrt();
                            let sn = self.bsigma(klast, xm);
                            self.sn.copy_from_slice(&sn);
                            let dx = self.es[is - 1] - self.es[is];
                            let f = (em - self.es[is]) / dx;
                            let test = 1.0 - 1.0 / 100.0;
                            if f > test {
                                push = true;
                            } else {
                                let stot: f64 = self.sn.iter().sum();
                                for i in 0..nreac {
                                    let sni = self.sn[i];
                                    if stot < ERRMIN || (sni / stot).abs() < SSMALL {
                                        continue;
                                    }
                                    let (hi, lo) = (self.ssv(i, is - 1), self.ssv(i, is));
                                    if hi > RMAX * lo || hi < lo / RMAX {
                                        push = true;
                                        break;
                                    }
                                    let si = f * hi + (1.0 - f) * lo;
                                    let dy = (sni - si).abs();
                                    if dy <= errt * sni.abs() + ERRMIN {
                                        continue;
                                    }
                                    if dy > errm * sni.abs() + ERRMIN || dy * dx / 2.0 > tol.errint * em {
                                        push = true;
                                        break;
                                    }
                                }
                                if !push {
                                    let est = ESTP * (self.es[is] - self.tt[0]);
                                    if *j > 3 && dx > est {
                                        push = true;
                                    }
                                }
                            }
                        }
                        if push {
                            // label 170: not converged, add midpoint to stack
                            is += 1;
                            self.es[is] = self.es[is - 1];
                            self.es[is - 1] = em;
                            self.ks[is] = self.ks[is - 1];
                            self.ks[is - 1] = 0;
                            self.js[is] = self.js[is - 1];
                            self.js[is - 1] = 1; // km = 0
                            for i in 0..nreac {
                                let v = self.ssv(i, is - 1);
                                self.ss[is * nreac + i] = v;
                                self.ss[(is - 1) * nreac + i] = self.sn[i];
                            }
                            continue 'stack;
                        }
                        // label 150: converged, store top point of stack
                        self.tt[0] = self.es[is];
                        for i in 0..nreac {
                            self.tt[1 + i] = if self.tt[0] > self.emtr[i] && tt1old >= self.emtr[i] {
                                self.ssv(i, is)
                            } else {
                                0.0
                            };
                        }
                        tt1old = self.tt[0];
                        *j += 1;
                        self.emit();
                        is -= 1;
                        if is > 1 {
                            continue 'stack;
                        }
                        break 'stack;
                    }
                    // stack exhausted, get new node
                    self.es[2] = self.es[1];
                    self.ks[2] = self.ks[1];
                    self.js[2] = self.js[1];
                    for i in 0..nreac {
                        let v = self.ssv(i, 1);
                        self.ss[2 * nreac + i] = v;
                        self.dl[i] = self.dn_sign(i, k);
                    }
                    if k >= self.nhigh {
                        next = L::L180;
                    } else if self.es[1] > thnmax {
                        next = L::L190;
                    } else {
                        klast = k;
                        k += 1;
                        next = L::L120;
                    }
                }
                L::L180 => {
                    self.ks[2] = self.nlow - 1;
                    if self.nhigh < self.khigh {
                        return true;
                    }
                    next = L::L190;
                }
                L::L190 => {
                    // copy energies above thnmax to output; flag last with minus
                    loop {
                        *j += 1;
                        let et = self.e(k) * self.e(k) / alpha;
                        self.tt[0] = et;
                        for i in 0..nreac {
                            self.tt[1 + i] = self.s(i, k);
                        }
                        let last = k == self.nhigh && self.khigh == self.nhigh;
                        self.emit();
                        if last {
                            *j = -*j;
                        }
                        k += 1;
                        if k > self.nhigh {
                            break;
                        }
                    }
                    self.ks[2] = self.nlow - 1;
                    return *j > 0;
                }
            }
        }
    }

    /// `bfile3` (`broadr.f90:1152-1253`): page the table through `broadn`.
    fn bfile3(&mut self) {
        let n2in = self.vel.len();
        let mpage = NAMAX / (4 * (self.nreac + 1));
        let mut n2left = n2in;
        self.nlow = mpage + 1;
        self.nhigh = 2 * mpage;
        if n2left < mpage {
            self.nhigh = self.nlow + n2left - 1;
        }
        self.klow = self.nlow;
        let mut mlow = self.nlow;
        let mut mhigh = 3 * mpage;
        let mut nj: i64 = 0;
        // local mlow <-> global 1
        self.off = 1 - mlow as isize;
        let mut do_load = true;
        loop {
            if do_load {
                // label 110: load
                if n2left < mhigh - mlow + 1 {
                    mhigh = mlow + n2left - 1;
                }
                self.khigh = mhigh;
                n2left -= mhigh - mlow + 1;
            }
            // label 120
            if !self.broadn(&mut nj) {
                break;
            }
            // shift remaining unbroadened points one page forward
            let nw = self.khigh - self.nlow + 1;
            let kk = nw;
            self.off += self.nlow as isize - 1;
            self.klow = 1;
            mlow = kk + 1;
            if n2left > 0 {
                do_load = true;
                continue;
            }
            self.khigh = kk;
            self.nhigh = self.khigh;
            mhigh = self.khigh;
            let _ = mhigh;
            do_load = false;
        }
    }
}

// ── the driver (broadr.f90:354-1000) ─────────────────────────────────────────

/// `gety1` on a RECONR section, evaluated at ascending energies.
fn section_reader(sec: &ReconrSection) -> Gety1 {
    let np = sec.pairs.len() as i32;
    Gety1::new(&Tab1 {
        head: Cont {
            c1: 0.0,
            c2: sec.qi,
            l1: 0,
            l2: sec.lr,
            n1: 1,
            n2: np,
        },
        interp: vec![(np.max(1) as u32, 2)],
        pairs: sec.pairs.clone(),
    })
}

/// Upstream's "`go to 165`" list for ENDF-6 (`broadr.f90:504-513`): sections
/// the reaction loop never broadens, whatever their threshold.
fn skipped(mt: i32) -> bool {
    (mt > 200 && mt < 600) || mt > 850 || mt == 3 || mt == 4 || (46..=49).contains(&mt) || mt == 19
}

/// Doppler-broaden a RECONR result the way upstream BROADR does
/// (`broadr.f90:354-1000`): one walk over MT=1's grid, every low-threshold
/// reaction broadened together.
///
/// - `thnmx` is card 3's `thnmax`; upstream's default is `6.5e6` eV
///   (`broadr.f90:204`). The effective limit is `min(thnmx, eresh)` for a
///   resonance material, lowered to `0.99999 x` the first high threshold for
///   a non-resonance one.
/// - Which reactions are broadened (`broadr.f90:500-540`): MT=18 always;
///   otherwise any section starting at or below `emin`, where `emin = eresh`
///   for `LRP = 1` and 1 eV otherwise. Sections upstream skips outright (see
///   [`skipped`]) are not broadened.
/// - Output, section by section in tape order (labels 245-310):
///   - a broadened reaction is written on the new grid from one point before
///     its first non-zero value;
///   - MT=1, 3, 19, 46-49, and MT=4 when a level is broadened, are rebuilt
///     on the new grid as sums of the broadened reactions below `thnmax`,
///     and read from the input section above it;
///   - every other section is copied through unchanged.
///
/// Returns `None` when there is no MT=1 section to take the grid from. The
/// caller then keeps the per-reaction form.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn broadr_joint(
    result: &ReconrResult,
    temp_k: f64,
    tol: &BroadnTolerances,
    thnmx: f64,
) -> Option<ReconrResult> {
    const E6PT5: f64 = 6.5e6;
    const FACT: f64 = 0.99999;
    let secs = &result.sections;
    let mt1 = secs.iter().find(|s| s.mt.number() == 1)?;
    if mt1.pairs.len() < 2 || temp_k <= 0.0 {
        return None;
    }
    let lrp = result.material.lrp;
    let awr = result.material.awr;

    // thnmax and emin (broadr.f90:354-441). eresh is the PENDF's MF=2 upper
    // limit; a material with no MF=2 keeps thnmax = thnmx.
    let thnmx = sigfig(thnmx, 7, 0);
    let mut eresh = thnmx;
    if let Some(eh) = result.resonance_upper_limit {
        eresh = eh;
    }
    if eresh > E6PT5 {
        eresh = E6PT5;
    }
    let mut thnmax = if thnmx == 0.0 {
        eresh
    } else if thnmx > 0.0 {
        thnmx.min(eresh)
    } else {
        thnmx
    };
    let mut emin = 1.0;
    if thnmx < 0.0 {
        emin = -thnmx;
    }
    if lrp == 1 {
        emin = eresh;
    }
    let emax = result.material.emax;
    if emax > 0.0 && emax < thnmax {
        thnmax = emax;
    }

    // The union grid: MT=1's energies (broadr.f90:478-495). RECONR's grid has
    // no repeated energy, so upstream's `fact` nudge never fires here.
    let grid: Vec<f64> = mt1.pairs.iter().map(|p| p.0).collect();
    debug_assert!(grid.windows(2).all(|w| w[1] > w[0]));

    // Reaction selection (broadr.f90:497-562).
    let mut mtr: Vec<usize> = Vec::new(); // indices into secs
    for (idx, sec) in secs.iter().enumerate() {
        let mt = i32::from(sec.mt.number());
        if mt == 1 || skipped(mt) || sec.pairs.is_empty() {
            continue;
        }
        let enext = section_reader(sec).get(0.0).xnext;
        if mt == 18 || enext <= emin || (lrp == 1 && enext < eresh) {
            mtr.push(idx);
            continue;
        }
        if lrp == 0 && FACT * enext < thnmax {
            thnmax = FACT * enext;
        }
    }
    if thnmax == 0.0 && thnmx == 0.0 {
        thnmax = E6PT5;
    }
    if thnmax < 0.0 {
        thnmax = -thnmax;
    }
    let nreac = mtr.len();
    if nreac == 0 {
        return Some(result.clone());
    }

    // Table of the broadened reactions on the grid (broadr.f90:540-562).
    let mut sig = vec![0.0f64; grid.len() * nreac];
    for (i, &idx) in mtr.iter().enumerate() {
        let mut g = section_reader(&secs[idx]);
        let _ = g.get(0.0);
        for (k, &e) in grid.iter().enumerate() {
            sig[k * nreac + i] = g.get(e).y;
        }
    }
    let mt_of = |i: usize| i32::from(secs[mtr[i]].mt.number());
    let emtr: Vec<f64> = (0..nreac)
        .map(|i| {
            let q = secs[mtr[i]].qi;
            if q == 0.0 {
                0.0
            } else {
                -q * (awr + 1.0) / awr
            }
        })
        .collect();

    let tempef = temp_k;
    let alpha = awr / (BK_EV_PER_K * tempef);
    let mut walk = Walk {
        nreac,
        alpha,
        thnmax,
        tol: *tol,
        emtr,
        vel: grid.iter().map(|&e| (alpha * e).sqrt()).collect(),
        sig,
        off: 0,
        klow: 0,
        khigh: 0,
        nlow: 0,
        nhigh: 0,
        ker: Kernel::default(),
        y: 0.0,
        ks: [0; NSTACK + 1],
        js: [0; NSTACK + 1],
        es: [0.0; NSTACK + 1],
        ss: vec![0.0; (NSTACK + 1) * nreac],
        tt: vec![0.0; 1 + nreac],
        sn: vec![0.0; nreac],
        dl: vec![0.0; nreac],
        out: Vec::new(),
    };
    walk.bfile3();
    let w = 1 + nreac;
    let rows: Vec<&[f64]> = walk.out.chunks(w).collect();
    let n2out = rows.len();

    // mti: first output record with a positive value, per reaction.
    let mti: Vec<usize> = (0..nreac)
        .map(|i| rows.iter().position(|r| r[1 + i] > 0.0).map_or(1, |p| p + 1))
        .collect();
    let mt4br = (0..nreac).any(|i| (51..=91).contains(&mt_of(i)));
    let has = |m: i32| (0..nreac).any(|i| mt_of(i) == m);
    let (mt103, mt104, mt105, mt106, mt107) = (has(103), has(104), has(105), has(106), has(107));

    // Output (labels 245-310).
    let mut out_secs = Vec::with_capacity(secs.len());
    for sec in secs {
        let mth = i32::from(sec.mt.number());
        let summed =
            mth == 1 || mth == 3 || (mth == 4 && mt4br) || (46..=49).contains(&mth) || mth == 19;
        let ireac = if summed {
            None
        } else {
            (0..nreac).rev().find(|&i| mt_of(i) == mth)
        };
        if !summed && ireac.is_none() {
            out_secs.push(sec.clone());
            continue;
        }
        let mut jjj = 1usize;
        for i in 0..nreac {
            if mth == mt_of(i) {
                jjj = mti[i];
            }
            if mth == 4 && mt_of(i) == 51 {
                jjj = mti[i];
            }
            if mth == 4 && jjj == 1 && mt_of(i) == 91 {
                jjj = mti[i];
            }
        }
        if jjj > 1 {
            jjj -= 1;
        }
        let mut orig = section_reader(sec);
        let _ = orig.get(0.0);
        let mut pairs = Vec::with_capacity(n2out + 1 - jjj);
        for r in &rows[jjj - 1..] {
            let e = r[0];
            let v = if let Some(i) = ireac {
                r[1 + i]
            } else if e > thnmax {
                orig.get(e).y
            } else {
                let mut acc = 0.0;
                for i in 0..nreac {
                    let m = mt_of(i);
                    let iflag = (mth == 3 && m == 2)
                        || (mth == 4 && !(51..=91).contains(&m))
                        || (mth == 19 && m != 18)
                        || ((46..=49).contains(&mth) && m != mth - 40)
                        || mth >= 201
                        || (mt103 && (600..=649).contains(&m))
                        || (mt104 && (650..=699).contains(&m))
                        || (mt105 && (700..=749).contains(&m))
                        || (mt106 && (750..=799).contains(&m))
                        || (mt107 && (800..=849).contains(&m));
                    if !iflag {
                        acc += r[1 + i];
                    }
                }
                acc
            };
            pairs.push((e, sigfig(v, 7, 0)));
        }
        out_secs.push(ReconrSection {
            lr: sec.lr,
            mt: sec.mt,
            qi: sec.qi,
            pairs,
        });
    }

    Some(ReconrResult {
        material: result.material.clone(),
        sections: out_secs,
        resonance_upper_limit: result.resonance_upper_limit,
        unresolved_table: result.unresolved_table.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endf::mt::MtReaction;
    use crate::reconr::mf1::MaterialInfo;

    fn material(awr: f64) -> MaterialInfo {
        MaterialInfo {
            za: 92234.0,
            awr,
            lrp: 0,
            lfi: 0,
            nlib: 0,
            elis: 0.0,
            nfor: 6,
            emax: 2.0e7,
        }
    }

    fn result(awr: f64, f: impl Fn(f64) -> f64) -> ReconrResult {
        // a log grid, 200 points per decade, 1e-5 eV to 2e7 eV
        let n = 12 * 200 + 1;
        let grid: Vec<f64> = (0..n)
            .map(|i| sigfig(1e-5 * 10f64.powf(i as f64 / 200.0), 7, 0))
            .filter(|&e| e <= 2.0e7)
            .collect();
        let pairs: Vec<(f64, f64)> = grid.iter().map(|&e| (e, sigfig(f(e), 7, 0))).collect();
        let sec = |mt: i32| ReconrSection {
            lr: 0,
            mt: MtReaction::from_any(mt),
            qi: 0.0,
            pairs: pairs.clone(),
        };
        ReconrResult {
            material: material(awr),
            sections: vec![sec(1), sec(2)],
            resonance_upper_limit: None,
            unresolved_table: None,
        }
    }

    fn at(r: &ReconrResult, mt: i32, e: f64) -> f64 {
        let s = r.sections.iter().find(|s| s.mt.number() == mt).unwrap();
        let i = s.pairs.partition_point(|p| p.0 < e);
        let (x0, y0) = s.pairs[i - 1];
        let (x1, y1) = s.pairs[i];
        y0 + (e - x0) * (y1 - y0) / (x1 - x0)
    }

    /// Free-gas broadening of a constant: `σ[(1 + 1/(2y²)) erf(y) +
    /// exp(-y²)/(y√π)]`, `y² = A E / kT`. At A = 232, E = kT that is
    /// `1 + 1/464` to 1e-9.
    #[test]
    fn constant_broadens_to_the_free_gas_result() {
        let t = 293.6;
        let r = broadr_joint(&result(232.0304, |_| 10.0), t, &BroadnTolerances::default(), 6.5e6)
            .unwrap();
        let kt = BK_EV_PER_K * t;
        let got = at(&r, 2, kt);
        let want = 10.0 * (1.0 + 1.0 / (2.0 * 232.0304));
        assert!((got / want - 1.0).abs() < 2e-4, "got {got}, want {want}");
    }

    /// SIGMA1 leaves `1/v` exactly invariant.
    #[test]
    fn one_over_v_is_invariant() {
        let r = broadr_joint(
            &result(232.0304, |e| 100.0 * (0.0253f64 / e).sqrt()),
            293.6,
            &BroadnTolerances::default(),
            6.5e6,
        )
        .unwrap();
        for e in [1e-4, 0.0253, 1.0, 100.0] {
            let got = at(&r, 2, e);
            let want = 100.0 * (0.0253f64 / e).sqrt();
            assert!((got / want - 1.0).abs() < 2e-4, "E={e}: got {got}, want {want}");
        }
    }
}
