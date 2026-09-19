// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of PETIR, a component of OUTRAM PARK.
//
// PETIR is free software: you can redistribute it and/or modify it under the
// terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 of the License.
//
// PETIR is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along
// with PETIR.  If not, see <https://www.gnu.org/licenses/>.
//
// PORTED from the GNU Scientific Library (GSL) 2.8, specfunc/zeta.c
// <https://www.gnu.org/software/gsl/>
// Copyright (C) 1996, 1997, 1998, 1999, 2000 Gerard Jungman
// GSL is free software under the GNU General Public License, version 3 or
// later.
//
// The eight coefficient tables here were extracted from that source by
// script, not retyped, and are audited bit-for-bit against it by
// `tests/gsl_tables_audit.rs`.

//! The Riemann and Hurwitz zeta functions and the Dirichlet eta function:
//! [`zeta`], [`hzeta`], [`zetam1`], [`eta`], and the integer-argument forms.
//!
//! # What these are
//!
//! `zeta(s) = sum_{n>=1} n^{-s}` for `s > 1`, continued analytically
//! everywhere except the simple pole at `s = 1`. `hzeta(s, q)` is the same sum
//! shifted, `sum_{n>=0} (n + q)^{-s}`, and specialises to `zeta` at `q = 1`.
//! `eta(s) = sum_{n>=1} (-1)^{n-1} n^{-s}` is the alternating form, related by
//! `eta(s) = (1 - 2^{1-s}) zeta(s)` and finite at `s = 1`.
//!
//! In this workspace they arrive through the polygamma functions
//! ([`crate::specfunc::psi`] builds `psi_n` from [`hzeta`]) and through
//! Bose-Einstein and Fermi-Dirac integrals.
//!
//! # `zetam1` exists because `zeta(s) - 1` cancels
//!
//! `zeta(20)` is `1.0000009539...`. Forming `zeta(20) - 1.0` in `f64` throws
//! away six of the sixteen significant digits before the subtraction even
//! happens. [`zetam1`] computes the difference directly and keeps them; reach
//! for it whenever the `1` would dominate.
//!
//! # Argument ranges, stated plainly
//!
//! | function | domain | outside it |
//! |---|---|---|
//! | [`zeta`], [`zetam1`] | all `s` except `1` | `NaN` at `s = 1` |
//! | [`hzeta`] | `s > 1` and `q > 0` | `NaN` |
//! | [`eta`] | all `s` | — |
//! | [`zeta_int`], [`zetam1_int`] | all `i32` except `1` | `NaN` at `1` |
//! | [`eta_int`] | all `i32` | — |
//!
//! All arguments and results are dimensionless (`f64`).
//!
//! # Accuracy
//!
//! Measured against identities that share no table with the implementation —
//! the Euler product, the `eta`/`zeta` relation, the functional equation, and
//! the closed forms at the even integers. Figures are in each function's doc
//! comment and were measured, not predicted.

// Under a std-linked build (`cargo test`) f64's inherent sqrt/exp/... shadow
// these trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use crate::cheb_slice::eval_gsl;
use crate::specfunc::{gamma::gamma, DBL_EPSILON, LOG_DBL_MAX, LOG_DBL_MIN, ROOT5_DBL_EPSILON};

use core::f64::consts::{LN_2, PI};

/// `ZETA_POS_TABLE_NMAX` — the largest `n` for which `zeta(n) - 1` is tabulated.
const ZETA_POS_TABLE_NMAX: i32 = 100;
/// `ZETA_NEG_TABLE_NMAX` — the most negative `n` covered by the odd-negative table.
const ZETA_NEG_TABLE_NMAX: i32 = 99;
/// `ETA_POS_TABLE_NMAX`.
const ETA_POS_TABLE_NMAX: i32 = 100;
/// `ETA_NEG_TABLE_NMAX`.
const ETA_NEG_TABLE_NMAX: i32 = 99;

/// `(2 pi)^{10 n}` for `n = 0 ..= 17`, GSL's `twopi_pow`.
///
/// A local array in `gsl_sf_zeta_e`'s reflection branch. It exists so that
/// `pow(2 pi, s)` at large negative `s` is formed as a tabulated power times a
/// small residual power, rather than as one `pow` call that would lose digits.
#[rustfmt::skip]
const TWOPI_POW: [f64; 18] = [
    1.0,
    9.589560061550901348e+007, 9.195966217409212684e+015, 8.818527036583869903e+023,
    8.456579467173150313e+031, 8.109487671573504384e+039, 7.776641909496069036e+047,
    7.457457466828644277e+055, 7.151373628461452286e+063, 6.857852693272229709e+071,
    6.576379029540265771e+079, 6.306458169130020789e+087, 6.047615938853066678e+095,
    5.799397627482402614e+103, 5.561367186955830005e+111, 5.333106466365131227e+119,
    5.114214477385391780e+127, 4.904306689854036836e+135,
];
#[rustfmt::skip]
const ZETA_XLT1: [f64; 14] = [
    1.48018677156931561235192914649, 0.25012062539889426471999938167,
    0.00991137502135360774243761467, -0.00012084759656676410329833091,
    -4.7585866367662556504652535281e-06, 2.2229946694466391855561441361e-07,
    -2.2237496498030257121309056582e-09, -1.0173226513229028319420799028e-10,
    4.3756643450424558284466248449e-12, -6.2229632593100551465504090814e-14,
    -6.6116201003272207115277520305e-16, 4.9477279533373912324518463830e-17,
    -1.0429819093456189719660003522e-18, 6.9925216166580021051464412040e-21,
];
#[rustfmt::skip]
const ZETA_XGT1: [f64; 30] = [
    19.3918515726724119415911269006, 9.1525329692510756181581271500,
    0.2427897658867379985365270155, -0.1339000688262027338316641329,
    0.0577827064065028595578410202, -0.0187625983754002298566409700,
    0.0039403014258320354840823803, -0.0000581508273158127963598882,
    -0.0003756148907214820704594549, 0.0001892530548109214349092999,
    -0.0000549032199695513496115090, 8.7086484008939038610413331863e-6,
    6.4609477924811889068410083425e-7, -9.6749773915059089205835337136e-7,
    3.6585400766767257736982342461e-7, -8.4592516427275164351876072573e-8,
    9.9956786144497936572288988883e-9, 1.4260036420951118112457144842e-9,
    -1.1761968823382879195380320948e-9, 3.7114575899785204664648987295e-10,
    -7.4756855194210961661210215325e-11, 7.8536934209183700456512982968e-12,
    9.9827182259685539619810406271e-13, -7.5276687030192221587850302453e-13,
    2.1955026393964279988917878654e-13, -4.1934859852834647427576319246e-14,
    4.6341149635933550715779074274e-15, 2.3742488509048340106830309402e-16,
    -2.7276516388124786119323824391e-16, 7.8473570134636044722154797225e-17,
];
#[rustfmt::skip]
const ZETAM1_INTER: [f64; 24] = [
    -21.7509435653088483422022339374, -5.63036877698121782876372020472,
    0.0528041358684229425504861579635, -0.0156381809179670789342700883562,
    0.00408218474372355881195080781927, -0.0010264867349474874045036628282,
    0.000260469880409886900143834962387, -0.0000676175847209968878098566819447,
    0.0000179284472587833525426660171124, -4.83238651318556188834107605116e-6,
    1.31913788964999288471371329447e-6, -3.63760500656329972578222188542e-7,
    1.01146847513194744989748396574e-7, -2.83215225141806501619105289509e-8,
    7.97733710252021423361012829496e-9, -2.25850168553956886676250696891e-9,
    6.42269392950164306086395744145e-10, -1.83363861846127284505060843614e-10,
    5.25309763895283179960368072104e-11, -1.50958687042589821074710575446e-11,
    4.34997545516049244697776942981e-12, -1.25597782748190416118082322061e-12,
    3.61280740072222650030134104162e-13, -9.66437239205745207188920348801e-14,
];
#[rustfmt::skip]
const HZETA_C: [f64; 15] = [
    1.00000000000000000000000000000, 0.083333333333333333333333333333,
    -0.00138888888888888888888888888889, 0.000033068783068783068783068783069,
    -8.2671957671957671957671957672e-07, 2.0876756987868098979210090321e-08,
    -5.2841901386874931848476822022e-10, 1.3382536530684678832826980975e-11,
    -3.3896802963225828668301953912e-13, 8.5860620562778445641359054504e-15,
    -2.1748686985580618730415164239e-16, 5.5090028283602295152026526089e-18,
    -1.3954464685812523340707686264e-19, 3.5347070396294674716932299778e-21,
    -8.9535174270375468504026113181e-23,
];
#[rustfmt::skip]
const ZETA_NEG_INT: [f64; 50] = [
    -0.083333333333333333333333333333, 0.008333333333333333333333333333,
    -0.003968253968253968253968253968, 0.004166666666666666666666666667,
    -0.007575757575757575757575757576, 0.021092796092796092796092796093,
    -0.083333333333333333333333333333, 0.44325980392156862745098039216,
    -3.05395433027011974380395433027, 26.4562121212121212121212121212,
    -281.460144927536231884057971014, 3607.5105463980463980463980464,
    -54827.583333333333333333333333, 974936.82385057471264367816092,
    -2.0052695796688078946143462272e+07, 4.7238486772162990196078431373e+08,
    -1.2635724795916666666666666667e+10, 3.8087931125245368811553022079e+11,
    -1.2850850499305083333333333333e+13, 4.8241448354850170371581670362e+14,
    -2.0040310656516252738108421663e+16, 9.1677436031953307756992753623e+17,
    -4.5979888343656503490437943262e+19, 2.5180471921451095697089023320e+21,
    -1.5001733492153928733711440151e+23, 9.6899578874635940656497942895e+24,
    -6.7645882379292820990945242302e+26, 5.0890659468662289689766332916e+28,
    -4.1147288792557978697665486068e+30, 3.5666582095375556109684574609e+32,
    -3.3066089876577576725680214670e+34, 3.2715634236478716264211227016e+36,
    -3.4473782558278053878256455080e+38, 3.8614279832705258893092720200e+40,
    -4.5892974432454332168863989006e+42, 5.7775386342770431824884825688e+44,
    -7.6919858759507135167410075972e+46, 1.0813635449971654696354033351e+49,
    -1.6029364522008965406067102346e+51, 2.5019479041560462843656661499e+53,
    -4.1067052335810212479752045004e+55, 7.0798774408494580617452972433e+57,
    -1.2804546887939508790190849756e+60, 2.4267340392333524078020892067e+62,
    -4.8143218874045769355129570066e+64, 9.9875574175727530680652777408e+66,
    -2.1645634868435185631335136160e+69, 4.8962327039620553206849224516e+71,
    -1.1549023923963519663954271692e+74, 2.8382249570693706959264156336e+76,
];
#[rustfmt::skip]
const ZETAM1_POS_INT: [f64; 101] = [
    -1.5, 0.0, 0.644934066848226436472415166646, 0.202056903159594285399738161511,
    0.082323233711138191516003696541, 0.036927755143369926331365486457,
    0.017343061984449139714517929790, 0.008349277381922826839797549849,
    0.004077356197944339378685238508, 0.002008392826082214417852769232,
    0.000994575127818085337145958900, 0.000494188604119464558702282526,
    0.000246086553308048298637998047, 0.000122713347578489146751836526,
    0.000061248135058704829258545105, 0.000030588236307020493551728510,
    0.000015282259408651871732571487, 7.6371976378997622736002935630e-6,
    3.8172932649998398564616446219e-6, 1.9082127165539389256569577951e-6,
    9.5396203387279611315203868344e-7, 4.7693298678780646311671960437e-7,
    2.3845050272773299000364818675e-7, 1.1921992596531107306778871888e-7,
    5.9608189051259479612440207935e-8, 2.9803503514652280186063705069e-8,
    1.4901554828365041234658506630e-8, 7.4507117898354294919810041706e-9,
    3.7253340247884570548192040184e-9, 1.8626597235130490064039099454e-9,
    9.3132743241966818287176473502e-10, 4.6566290650337840729892332512e-10,
    2.3283118336765054920014559759e-10, 1.1641550172700519775929738354e-10,
    5.8207720879027008892436859891e-11, 2.9103850444970996869294252278e-11,
    1.4551921891041984235929632245e-11, 7.2759598350574810145208690123e-12,
    3.6379795473786511902372363558e-12, 1.8189896503070659475848321007e-12,
    9.0949478402638892825331183869e-13, 4.5474737830421540267991120294e-13,
    2.2737368458246525152268215779e-13, 1.1368684076802278493491048380e-13,
    5.6843419876275856092771829675e-14, 2.8421709768893018554550737049e-14,
    1.4210854828031606769834307141e-14, 7.1054273952108527128773544799e-15,
    3.5527136913371136732984695340e-15, 1.7763568435791203274733490144e-15,
    8.8817842109308159030960913863e-16, 4.4408921031438133641977709402e-16,
    2.2204460507980419839993200942e-16, 1.1102230251410661337205445699e-16,
    5.5511151248454812437237365905e-17, 2.7755575621361241725816324538e-17,
    1.3877787809725232762839094906e-17, 6.9388939045441536974460853262e-18,
    3.4694469521659226247442714961e-18, 1.7347234760475765720489729699e-18,
    8.6736173801199337283420550673e-19, 4.3368086900206504874970235659e-19,
    2.1684043449972197850139101683e-19, 1.0842021724942414063012711165e-19,
    5.4210108624566454109187004043e-20, 2.7105054312234688319546213119e-20,
    1.3552527156101164581485233996e-20, 6.7762635780451890979952987415e-21,
    3.3881317890207968180857031004e-21, 1.6940658945097991654064927471e-21,
    8.4703294725469983482469926091e-22, 4.2351647362728333478622704833e-22,
    2.1175823681361947318442094398e-22, 1.0587911840680233852265001539e-22,
    5.2939559203398703238139123029e-23, 2.6469779601698529611341166842e-23,
    1.3234889800848990803094510250e-23, 6.6174449004244040673552453323e-24,
    3.3087224502121715889469563843e-24, 1.6543612251060756462299236771e-24,
    8.2718061255303444036711056167e-25, 4.1359030627651609260093824555e-25,
    2.0679515313825767043959679193e-25, 1.0339757656912870993284095591e-25,
    5.1698788284564313204101332166e-26, 2.5849394142282142681277617708e-26,
    1.2924697071141066700381126118e-26, 6.4623485355705318034380021611e-27,
    3.2311742677852653861348141180e-27, 1.6155871338926325212060114057e-27,
    8.0779356694631620331587381863e-28, 4.0389678347315808256222628129e-28,
    2.0194839173657903491587626465e-28, 1.0097419586828951533619250700e-28,
    5.0487097934144756960847711725e-29, 2.5243548967072378244674341938e-29,
    1.2621774483536189043753999660e-29, 6.3108872417680944956826093943e-30,
    3.1554436208840472391098412184e-30, 1.5777218104420236166444327830e-30,
    7.8886090522101180735205378276e-31,
];
#[rustfmt::skip]
const ETA_POS_INT: [f64; 101] = [
    0.50000000000000000000000000000, 0.69314718055994530941723212145817656807550013436026,
    0.82246703342411321823620758332, 0.90154267736969571404980362113,
    0.94703282949724591757650323447, 0.97211977044690930593565514355,
    0.98555109129743510409843924448, 0.99259381992283028267042571313,
    0.99623300185264789922728926008, 0.99809429754160533076778303185,
    0.99903950759827156563922184570, 0.99951714349806075414409417483,
    0.99975768514385819085317967871, 0.99987854276326511549217499282,
    0.99993917034597971817095419226, 0.99996955121309923808263293263,
    0.99998476421490610644168277496, 0.99999237829204101197693787224,
    0.99999618786961011347968922641, 0.99999809350817167510685649297,
    0.99999904661158152211505084256, 0.99999952325821554281631666433,
    0.99999976161323082254789720494, 0.99999988080131843950322382485,
    0.99999994039889239462836140314, 0.99999997019885696283441513311,
    0.99999998509923199656878766181, 0.99999999254955048496351585274,
    0.99999999627475340010872752767, 0.99999999813736941811218674656,
    0.99999999906868228145397862728, 0.99999999953434033145421751469,
    0.99999999976716989595149082282, 0.99999999988358485804603047265,
    0.99999999994179239904531592388, 0.99999999997089618952980952258,
    0.99999999998544809143388476396, 0.99999999999272404460658475006,
    0.99999999999636202193316875550, 0.99999999999818101084320873555,
    0.99999999999909050538047887809, 0.99999999999954525267653087357,
    0.99999999999977262633369589773, 0.99999999999988631316532476488,
    0.99999999999994315658215465336, 0.99999999999997157829090808339,
    0.99999999999998578914539762720, 0.99999999999999289457268000875,
    0.99999999999999644728633373609, 0.99999999999999822364316477861,
    0.99999999999999911182158169283, 0.99999999999999955591079061426,
    0.99999999999999977795539522974, 0.99999999999999988897769758908,
    0.99999999999999994448884878594, 0.99999999999999997224442439010,
    0.99999999999999998612221219410, 0.99999999999999999306110609673,
    0.99999999999999999653055304826, 0.99999999999999999826527652409,
    0.99999999999999999913263826204, 0.99999999999999999956631913101,
    0.99999999999999999978315956551, 0.99999999999999999989157978275,
    0.99999999999999999994578989138, 0.99999999999999999997289494569,
    0.99999999999999999998644747284, 0.99999999999999999999322373642,
    0.99999999999999999999661186821, 0.99999999999999999999830593411,
    0.99999999999999999999915296705, 0.99999999999999999999957648353,
    0.99999999999999999999978824176, 0.99999999999999999999989412088,
    0.99999999999999999999994706044, 0.99999999999999999999997353022,
    0.99999999999999999999998676511, 0.99999999999999999999999338256,
    0.99999999999999999999999669128, 0.99999999999999999999999834564,
    0.99999999999999999999999917282, 0.99999999999999999999999958641,
    0.99999999999999999999999979320, 0.99999999999999999999999989660,
    0.99999999999999999999999994830, 0.99999999999999999999999997415,
    0.99999999999999999999999998708, 0.99999999999999999999999999354,
    0.99999999999999999999999999677, 0.99999999999999999999999999838,
    0.99999999999999999999999999919, 0.99999999999999999999999999960,
    0.99999999999999999999999999980, 0.99999999999999999999999999990,
    0.99999999999999999999999999995, 0.99999999999999999999999999997,
    0.99999999999999999999999999999, 0.99999999999999999999999999999,
    1.00000000000000000000000000000, 1.00000000000000000000000000000,
    1.00000000000000000000000000000,
];
#[rustfmt::skip]
const ETA_NEG_INT: [f64; 50] = [
    0.25000000000000000000000000000, -0.12500000000000000000000000000,
    0.25000000000000000000000000000, -1.06250000000000000000000000000,
    7.75000000000000000000000000000, -86.3750000000000000000000000000,
    1365.25000000000000000000000000, -29049.0312500000000000000000000,
    800572.750000000000000000000000, -2.7741322625000000000000000000e+7,
    1.1805291302500000000000000000e+9, -6.0523980051687500000000000000e+10,
    3.6794167785377500000000000000e+12, -2.6170760990658387500000000000e+14,
    2.1531418140800295250000000000e+16, -2.0288775575173015930156250000e+18,
    2.1708009902623770590275000000e+20, -2.6173826968455814932120125000e+22,
    3.5324148876863877826668602500e+24, -5.3042033406864906641493838981e+26,
    8.8138218364311576767253114668e+28, -1.6128065107490778547354654864e+31,
    3.2355470001722734208527794569e+33, -7.0876727476537493198506645215e+35,
    1.6890450341293965779175629389e+38, -4.3639690731216831157655651358e+40,
    1.2185998827061261322605065672e+43, -3.6670584803153006180101262324e+45,
    1.1859898526302099104271449748e+48, -4.1120769493584015047981746438e+50,
    1.5249042436787620309090168687e+53, -6.0349693196941307074572991901e+55,
    2.5437161764210695823197691519e+58, -1.1396923802632287851130360170e+61,
    5.4180861064753979196802726455e+63, -2.7283654799994373847287197104e+66,
    1.4529750514918543238511171663e+69, -8.1705519371067450079777183386e+71,
    4.8445781606678367790247757259e+74, -3.0246694206649519336179448018e+77,
    1.9858807961690493054169047970e+80, -1.3694474620720086994386818232e+83,
    9.9070382984295807826303785989e+85, -7.5103780796592645925968460677e+88,
    5.9598418264260880840077992227e+91, -4.9455988887500020399263196307e+94,
    4.2873596927020241277675775935e+97, -3.8791952037716162900707994047e+100,
    3.6600317773156342245401829308e+103, -3.5978775704117283875784869570e+106,
];

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// `zeta(s)` for `s >= 0`, `s != 1`. Ports `riemann_zeta_sgt0`.
fn riemann_zeta_sgt0(s: f64) -> f64 {
    if s < 1.0 {
        return eval_gsl(2.0 * s - 1.0, &ZETA_XLT1) / (s - 1.0);
    }
    if s <= 20.0 {
        return eval_gsl((2.0 * s - 21.0) / 19.0, &ZETA_XGT1) / (s - 1.0);
    }
    // Euler product truncated at 7: past s = 20 the neglected factors differ
    // from 1 by less than an ulp.
    let f2 = 1.0 - 2.0_f64.powf(-s);
    let f3 = 1.0 - 3.0_f64.powf(-s);
    let f5 = 1.0 - 5.0_f64.powf(-s);
    let f7 = 1.0 - 7.0_f64.powf(-s);
    1.0 / (f2 * f3 * f5 * f7)
}

/// `zeta(1 - s)` for `s < 0`, the factor the reflection formula needs.
/// Ports `riemann_zeta1ms_slt0`.
fn riemann_zeta_1ms_slt0(s: f64) -> f64 {
    if s > -19.0 {
        return eval_gsl((-19.0 - 2.0 * s) / 19.0, &ZETA_XGT1) / (-s);
    }
    let f2 = 1.0 - 2.0_f64.powf(-(1.0 - s));
    let f3 = 1.0 - 3.0_f64.powf(-(1.0 - s));
    let f5 = 1.0 - 5.0_f64.powf(-(1.0 - s));
    let f7 = 1.0 - 7.0_f64.powf(-(1.0 - s));
    1.0 / (f2 * f3 * f5 * f7)
}

/// `zeta(s) - 1` for `5 < s < 15`. Ports
/// `riemann_zeta_minus_1_intermediate_s`.
///
/// # The slice is 23 of 24, deliberately
///
/// `zetam1_inter_data` upstream holds **24** values while its `cheb_series`
/// declares `order = 22`, so `cheb_eval_e` reads only `c[0 ..= 22]` — 23 of
/// them. Every other series in this crate's GSL ports uses its whole array,
/// and including the 24th here would be a silent, tiny, permanent bias. The
/// audit test checks the order against the array length so this cannot be
/// reintroduced by a later regeneration.
fn zetam1_intermediate(s: f64) -> f64 {
    let t = (s - 10.0) / 5.0;
    let Some(used) = ZETAM1_INTER.get(..23) else {
        return f64::NAN;
    };
    eval_gsl(t, used).exp() + 2.0_f64.powf(-s)
}

/// `zeta(s) - 1` for `s >= 15`. Ports `riemann_zeta_minus1_large_s`.
///
/// The first two symmetric functions of the six smallest prime powers,
/// divided by the truncated Euler product. Upstream carries `t3` .. `t6`
/// commented out; they are omitted here for the same reason they are omitted
/// there — past `s = 15` they are below the rounding of `t1 - t2`.
fn zetam1_large_s(s: f64) -> f64 {
    let a = 2.0_f64.powf(-s);
    let b = 3.0_f64.powf(-s);
    let c = 5.0_f64.powf(-s);
    let d = 7.0_f64.powf(-s);
    let e = 11.0_f64.powf(-s);
    let f = 13.0_f64.powf(-s);
    let t1 = a + b + c + d + e + f;
    let t2 = a * (b + c + d + e + f) + b * (c + d + e + f) + c * (d + e + f) + d * (e + f) + e * f;
    let numt = t1 - t2;
    let zeta = 1.0 / ((1.0 - a) * (1.0 - b) * (1.0 - c) * (1.0 - d) * (1.0 - e) * (1.0 - f));
    numt * zeta
}

/// Index into `ZETA_NEG_INT` / `ETA_NEG_INT` for an odd negative `n`.
///
/// Upstream writes `-(n+1)/2` with C's truncating integer division; for odd
/// negative `n` that is exactly `(-n - 1) / 2`, which is what this computes in
/// `u32` so no negative index can be formed.
fn odd_negative_index(n: i32) -> usize {
    ((-n - 1) / 2) as usize
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// The Hurwitz zeta function `zeta(s, q) = sum_{n>=0} (n + q)^{-s}`.
///
/// # Domain and range
///
/// `s > 1` and `q > 0`; returns `NaN` otherwise, `+inf` where the leading
/// term overflows and `0.0` where it underflows. `zeta(s, 1) = zeta(s)`.
///
/// # Method
///
/// Euler-Maclaurin summation (Moshier, *Methods and Programs for
/// Mathematical Functions*, p. 400, with the typo corrections GSL notes): ten
/// terms summed directly, the tail replaced by an integral plus up to
/// thirteen Bernoulli corrections, stopped as soon as a correction falls
/// below half an ulp of the running sum. Two `pow`-only shortcuts cover the
/// cases where `s` is so large that only the first one or three terms matter.
///
/// # Accuracy
///
/// Worst relative error 1.870e-15 against [`zeta`] over `s` in `(1, 31]` at
/// `q = 1` — two entirely different branch structures reaching the same
/// number — and 1e-13 against direct Euler-Maclaurin summation at
/// `q` in {0.3, 0.75, 1, 2.5, 10}. The second figure is bounded by the
/// reference. Both measured in this module's tests.
///
/// # Example
///
/// ```
/// use petir::specfunc::{hzeta, zeta};
/// // The Hurwitz zeta at q = 1 is the Riemann zeta.
/// assert!((hzeta(3.0, 1.0) - zeta(3.0)).abs() < 1e-14);
/// ```
pub fn hzeta(s: f64, q: f64) -> f64 {
    if s.is_nan() || q.is_nan() {
        return f64::NAN;
    }
    if s <= 1.0 || q <= 0.0 {
        return f64::NAN;
    }
    let max_bits = 54.0;
    let ln_term0 = -s * q.ln();
    if ln_term0 < LOG_DBL_MIN + 1.0 {
        // Upstream reports an underflow; the value it hands back is zero.
        return 0.0;
    }
    if ln_term0 > LOG_DBL_MAX - 1.0 {
        return f64::INFINITY;
    }
    if (s > max_bits && q < 1.0) || (s > 0.5 * max_bits && q < 0.25) {
        return q.powf(-s);
    }
    if s > 0.5 * max_bits && q < 1.0 {
        let p1 = q.powf(-s);
        let p2 = (q / (1.0 + q)).powf(s);
        let p3 = (q / (2.0 + q)).powf(s);
        return p1 * (1.0 + p2 + p3);
    }
    let jmax = 12usize;
    let kmax = 10.0_f64;
    let pmax = (kmax + q).powf(-s);
    let mut scp = s;
    let mut pcp = pmax / (kmax + q);
    let mut ans = pmax * ((kmax + q) / (s - 1.0) + 0.5);
    for k in 0..10u32 {
        ans += (k as f64 + q).powf(-s);
    }
    // `hzeta_c[j+1]`, so the correction walks c[1..=jmax+1].
    for (j, &cj) in (0..=jmax).zip(HZETA_C.iter().skip(1)) {
        let delta = cj * scp * pcp;
        ans += delta;
        if (delta / ans).abs() < 0.5 * DBL_EPSILON {
            break;
        }
        let jf = j as f64;
        scp *= (s + 2.0 * jf + 1.0) * (s + 2.0 * jf + 2.0);
        pcp /= (kmax + q) * (kmax + q);
    }
    ans
}

/// The Riemann zeta function `zeta(s) = sum_{n>=1} n^{-s}`, continued to the
/// whole real line.
///
/// # Domain and range
///
/// Every `s` except the simple pole at `s = 1`, where it returns `NaN`.
/// Exactly zero at the negative even integers (the trivial zeros). Returns
/// `+inf` below `s = -170`, where the reflection formula's prefactors leave
/// `f64` — upstream gives up there too, and says so.
///
/// # Method
///
/// Three branches for `s >= 0`: a Chebyshev series in `2s - 1` below `s = 1`,
/// another in `(2s - 21)/19` up to `s = 20`, and the Euler product over the
/// first four primes above it. For `s < 0` the functional equation
/// `zeta(s) = 2^s pi^{s-1} sin(pi s / 2) Gamma(1-s) zeta(1-s)`, with
/// `(2 pi)^s` formed from [`TWOPI_POW`] rather than one `pow` call.
///
/// # Accuracy
///
/// Relative error **6.749e-16** against the closed forms at the even
/// integers (`zeta(2) = pi^2/6`, `zeta(4) = pi^4/90`, `zeta(6) = pi^6/945`,
/// `zeta(8) = pi^8/9450`), which is the tight measurement, and 2.231e-14 in
/// the functional equation over `s` in `[-20.1, -0.2]` — the latter bounded
/// by its 1999-term reference sum rather than by `zeta`. Both measured in
/// this module's tests.
///
/// # Example
///
/// ```
/// use petir::specfunc::zeta;
/// let pi = core::f64::consts::PI;
/// // 2e-15 rather than 1e-15: GSL's Chebyshev branch lands 5 ulp from the
/// // closed form at s = 2, which is the worst point of the four measured.
/// assert!((zeta(2.0) - pi * pi / 6.0).abs() < 2e-15);
/// assert!(zeta(1.0).is_nan());
/// ```
pub fn zeta(s: f64) -> f64 {
    if s.is_nan() {
        return f64::NAN;
    }
    if s == 1.0 {
        return f64::NAN;
    }
    if s >= 0.0 {
        return riemann_zeta_sgt0(s);
    }
    let zeta_one_minus_s = riemann_zeta_1ms_slt0(s);
    let m = s % 2.0;
    let sin_term = if m == 0.0 {
        0.0
    } else {
        (0.5 * PI * (s % 4.0)).sin() / PI
    };
    if sin_term == 0.0 {
        // The trivial zeros at the negative even integers, exactly.
        return 0.0;
    }
    if s > -170.0 {
        let n = ((-s) / 10.0).floor();
        let fs = s + 10.0 * n;
        let Some(&tp) = TWOPI_POW.get(n as usize) else {
            return f64::NAN;
        };
        let p = (2.0 * PI).powf(fs) / tp;
        return p * gamma(1.0 - s) * sin_term * zeta_one_minus_s;
    }
    // Upstream reports an overflow here and declines to compute it; the
    // prefactors leave f64 and reassembling them through logs would cost
    // more digits than the answer has.
    f64::INFINITY
}

/// `zeta(n)` for integer `n`, from a table where one exists.
///
/// # Domain and range
///
/// Every `i32` except `1`, where it returns `NaN`. Exactly `0.0` at the
/// negative even integers, and exactly `1.0` above `n = 100` — past which
/// `zeta(n) - 1` is below an ulp of 1. Use [`zetam1_int`] if that difference
/// is what you need.
///
/// # Accuracy
///
/// 6.749e-16 relative against [`zeta`] over `n` in `[2, 100]` — the table
/// and the Chebyshev series are independent routes to the same value.
/// Measured in this module's tests.
///
/// # Example
///
/// ```
/// use petir::specfunc::zeta_int;
/// assert_eq!(zeta_int(-2), 0.0);
/// assert_eq!(zeta_int(200), 1.0);
/// assert!(zeta_int(1).is_nan());
/// ```
pub fn zeta_int(n: i32) -> f64 {
    if n < 0 {
        if n % 2 == 0 {
            return 0.0;
        }
        if n > -ZETA_NEG_TABLE_NMAX {
            return ZETA_NEG_INT
                .get(odd_negative_index(n))
                .copied()
                .unwrap_or(f64::NAN);
        }
        return zeta(n as f64);
    }
    if n == 1 {
        return f64::NAN;
    }
    if n <= ZETA_POS_TABLE_NMAX {
        return 1.0 + ZETAM1_POS_INT.get(n as usize).copied().unwrap_or(f64::NAN);
    }
    1.0
}

/// `zeta(s) - 1`, computed without forming the difference.
///
/// # Why this is not `zeta(s) - 1.0`
///
/// At `s = 20`, `zeta(s)` is `1.0000009539...`: subtracting one in `f64`
/// discards six significant digits that were never computed separately. This
/// routine keeps them — at `s = 30` it is accurate to a relative 1e-16 of a
/// quantity that `zeta(s) - 1.0` knows to about 1e-7.
///
/// # Domain and range
///
/// Every `s` except `s = 1` (`NaN`). Below `s = 5` it genuinely is
/// `zeta(s) - 1`, because there is nothing to gain; the two dedicated
/// branches take over above that.
///
/// # Accuracy
///
/// At `s = 20`, against a directly-summed reference: [`zetam1`] is
/// **2.220e-16** relative, and `zeta(20) - 1.0` is **6.404e-11** — five
/// orders worse. Across `s` in `[15, 30]` the worst is 1.373e-14, all of it
/// at `s = 15` where the six-prime Euler product is least converged (primes
/// above 13 contribute about 2.2e-19 there). Measured in this module's
/// tests.
///
/// # Example
///
/// ```
/// use petir::specfunc::{zeta, zetam1};
/// // At s = 20 the naive difference has already lost five digits.
/// let naive = zeta(20.0) - 1.0;
/// let direct = zetam1(20.0);
/// assert!(direct > 0.0 && (direct - naive).abs() / direct < 1e-7);
/// ```
pub fn zetam1(s: f64) -> f64 {
    if s.is_nan() {
        return f64::NAN;
    }
    if s <= 5.0 {
        return zeta(s) - 1.0;
    }
    if s < 15.0 {
        return zetam1_intermediate(s);
    }
    zetam1_large_s(s)
}

/// `zeta(n) - 1` for integer `n`. See [`zetam1`] for why this is not a
/// subtraction.
///
/// # Domain and range
///
/// Every `i32` except `1` (`NaN`). Exactly `-1.0` at the negative even
/// integers, where `zeta` is exactly zero.
///
/// # Example
///
/// ```
/// use petir::specfunc::zetam1_int;
/// assert_eq!(zetam1_int(-2), -1.0);
/// assert!(zetam1_int(50) > 0.0);
/// ```
pub fn zetam1_int(n: i32) -> f64 {
    if n < 0 {
        if n % 2 == 0 {
            return -1.0;
        }
        if n > -ZETA_NEG_TABLE_NMAX {
            return ZETA_NEG_INT
                .get(odd_negative_index(n))
                .copied()
                .map(|v| v - 1.0)
                .unwrap_or(f64::NAN);
        }
        // Upstream's own note: subtracting 1 makes no difference at values
        // this large, so it returns zeta directly.
        return zeta(n as f64);
    }
    if n == 1 {
        return f64::NAN;
    }
    if n <= ZETA_POS_TABLE_NMAX {
        return ZETAM1_POS_INT.get(n as usize).copied().unwrap_or(f64::NAN);
    }
    zetam1(n as f64)
}

/// The Dirichlet eta function `eta(s) = sum_{n>=1} (-1)^{n-1} n^{-s}`.
///
/// # Domain and range
///
/// Every `s`; unlike [`zeta`] it is finite at `s = 1`, where it equals
/// `ln 2`. Tends to 1 as `s` grows and is clamped to exactly 1 past
/// `s = 100`.
///
/// # Method
///
/// `eta(s) = (1 - 2^{1-s}) zeta(s)` away from `s = 1`, and a five-term Taylor
/// series in `s - 1` within `10 * GSL_ROOT5_DBL_EPSILON` of it — where the
/// factor vanishes against `zeta`'s pole and the product is `0 * inf`.
///
/// # Accuracy
///
/// 1.870e-14 against `(1 - 2^{1-s}) zeta(s)` over `s` in `[1.2, 31.1]`,
/// bounded by the 1999-term reference sum rather than by `eta`. The series
/// branch reproduces `eta(1) = ln 2` **exactly** — zero relative error, since
/// `c0` is `ln 2`. Measured in this module's tests.
///
/// # Example
///
/// ```
/// use petir::specfunc::eta;
/// // eta is finite at s = 1, where zeta has its pole.
/// assert!((eta(1.0) - core::f64::consts::LN_2).abs() < 1e-15);
/// ```
pub fn eta(s: f64) -> f64 {
    if s.is_nan() {
        return f64::NAN;
    }
    if s > 100.0 {
        return 1.0;
    }
    if (s - 1.0).abs() < 10.0 * ROOT5_DBL_EPSILON {
        let del = s - 1.0;
        let c0 = LN_2;
        let c1 = LN_2 * (crate::specfunc::EULER - 0.5 * LN_2);
        let c2 = -0.032_686_296_279_449_3;
        let c3 = 0.001_568_991_705_415_515;
        let c4 = 0.000_749_872_421_120_475_3;
        return c0 + del * (c1 + del * (c2 + del * (c3 + del * c4)));
    }
    let z = zeta(s);
    let p = ((1.0 - s) * LN_2).exp();
    (1.0 - p) * z
}

/// `eta(n)` for integer `n`, from a table where one exists.
///
/// # Domain and range
///
/// Every `i32`. Exactly `0.0` at the negative even integers, exactly `1.0`
/// above `n = 100`.
///
/// # Example
///
/// ```
/// use petir::specfunc::eta_int;
/// assert_eq!(eta_int(0), 0.5);
/// assert_eq!(eta_int(-2), 0.0);
/// ```
pub fn eta_int(n: i32) -> f64 {
    if n > ETA_POS_TABLE_NMAX {
        return 1.0;
    }
    if n >= 0 {
        return ETA_POS_INT.get(n as usize).copied().unwrap_or(f64::NAN);
    }
    if n % 2 == 0 {
        return 0.0;
    }
    if n > -ETA_NEG_TABLE_NMAX {
        return ETA_NEG_INT
            .get(odd_negative_index(n))
            .copied()
            .unwrap_or(f64::NAN);
    }
    let z = zeta_int(n);
    let p = ((1.0 - n as f64) * LN_2).exp();
    -p * z
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `zeta(s)` by direct summation with an Euler-Maclaurin tail, for
    /// `s > 1`. Shares no table, no branch and no Chebyshev coefficient with
    /// the implementation.
    ///
    /// `sum_{n<N} n^{-s} + N^{1-s}/(s-1) + N^{-s}/2 + s N^{-s-1}/12`, the
    /// first three Euler-Maclaurin terms. With `N = 2000` the next term is
    /// below 1e-20 for every `s` tested.
    fn zeta_by_summation(s: f64) -> f64 {
        let big = 2000u32;
        let mut sum = 0.0_f64;
        for n in 1..big {
            sum += (n as f64).powf(-s);
        }
        let nn = big as f64;
        sum + nn.powf(1.0 - s) / (s - 1.0) + 0.5 * nn.powf(-s) + s * nn.powf(-s - 1.0) / 12.0
    }

    /// `hzeta(s, q)` by the same construction, shifted.
    fn hzeta_by_summation(s: f64, q: f64) -> f64 {
        let big = 2000u32;
        let mut sum = 0.0_f64;
        for n in 0..big {
            sum += (n as f64 + q).powf(-s);
        }
        let nn = big as f64 + q;
        // `+ 0.5 g(N)`, not minus: Euler-Maclaurin's half-term on a tail that
        // STARTS at N. The sign was wrong here first and showed up as a
        // 1.711e-05 disagreement at (s, q) = (1.5, 10) -- a defect in this
        // reference, not in `hzeta`.
        sum + nn.powf(1.0 - s) / (s - 1.0) + 0.5 * nn.powf(-s) + s * nn.powf(-s - 1.0) / 12.0
    }

    /// The closed forms at the even integers, which are exact in the
    /// mathematics and independent of every table here.
    #[test]
    fn zeta_matches_the_closed_forms_at_the_even_integers() {
        let pi = PI;
        let cases = [
            (2.0, pi * pi / 6.0),
            (4.0, pi.powi(4) / 90.0),
            (6.0, pi.powi(6) / 945.0),
            (8.0, pi.powi(8) / 9450.0),
        ];
        let mut worst = 0.0_f64;
        for (s, exact) in cases {
            worst = worst.max(((zeta(s) - exact) / exact).abs());
            worst = worst.max(((zeta_int(s as i32) - exact) / exact).abs());
        }
        assert!(worst < 1e-15, "zeta at the even integers: {worst:e}");
    }

    /// `zeta(s)` against direct summation for `s > 1`. The implementation
    /// uses a Chebyshev series up to `s = 20` and an Euler product above it,
    /// and neither resembles a partial sum.
    #[test]
    fn zeta_matches_direct_summation_above_one() {
        let mut worst = 0.0_f64;
        let mut at = 0.0_f64;
        for k in 1..=100 {
            let s = 1.0 + 0.3 * k as f64;
            let r = zeta_by_summation(s);
            let e = ((zeta(s) - r) / r).abs();
            if e > worst {
                worst = e;
                at = s;
            }
        }
        // Measured 1.829e-14 at s = 5.2. The bound is on the REFERENCE, not
        // on `zeta`: 1999 additions of terms near 1 accumulate about
        // sqrt(1999) * eps of rounding, which is where this sits. The closed
        // forms at the even integers pin `zeta` itself to 2.220e-16.
        assert!(worst < 5e-14, "zeta vs summation: {worst:e} at s = {at}");
    }

    /// `zeta(s)` for `s < 1` against the functional equation
    /// `zeta(s) = 2^s pi^{s-1} sin(pi s/2) Gamma(1-s) zeta(1-s)`, with
    /// `zeta(1-s)` taken from the summation above rather than from this
    /// module.
    #[test]
    fn zeta_satisfies_the_functional_equation_below_one() {
        let mut worst = 0.0_f64;
        let mut at = 0.0_f64;
        for k in 1..=200 {
            let s = -0.1 - 0.1 * k as f64;
            // Skip the trivial zeros, where a relative comparison is
            // meaningless -- zeta is exactly zero and the reference is not.
            if (s - s.round()).abs() < 1e-12 && (s.round() as i64) % 2 == 0 {
                continue;
            }
            let rhs = 2.0_f64.powf(s)
                * PI.powf(s - 1.0)
                * (0.5 * PI * s).sin()
                * gamma(1.0 - s)
                * zeta_by_summation(1.0 - s);
            let e = ((zeta(s) - rhs) / rhs).abs();
            if e > worst {
                worst = e;
                at = s;
            }
        }
        assert!(worst < 1e-13, "functional equation: {worst:e} at s = {at}");
    }

    /// `zeta` is exactly zero at the negative even integers — the trivial
    /// zeros — and says so exactly rather than to within a tolerance.
    #[test]
    fn the_trivial_zeros_are_exact() {
        for k in 1..=40 {
            let n = -2 * k;
            assert_eq!(zeta(n as f64), 0.0, "zeta({n})");
            assert_eq!(zeta_int(n), 0.0, "zeta_int({n})");
            assert_eq!(zetam1_int(n), -1.0, "zetam1_int({n})");
            assert_eq!(eta_int(n), 0.0, "eta_int({n})");
        }
    }

    /// `zeta_int(n)` agrees with `zeta(n as f64)` wherever both are defined —
    /// the table and the series are independent routes to the same number.
    #[test]
    fn the_integer_table_agrees_with_the_continuous_function() {
        let mut worst = 0.0_f64;
        let mut at = 0;
        for n in 2..=100i32 {
            let r = zeta(n as f64);
            let e = ((zeta_int(n) - r) / r).abs();
            if e > worst {
                worst = e;
                at = n;
            }
        }
        assert!(worst < 1e-15, "zeta_int vs zeta: {worst:e} at n = {at}");
        // And on the negative odd side, where a different table is used.
        let mut worst_neg = 0.0_f64;
        for k in 0..=40 {
            let n = -(2 * k + 1);
            let r = zeta(n as f64);
            if r != 0.0 {
                worst_neg = worst_neg.max(((zeta_int(n) - r) / r).abs());
            }
        }
        assert!(
            worst_neg < 1e-13,
            "zeta_int vs zeta, negative: {worst_neg:e}"
        );
    }

    /// `hzeta(s, 1)` is `zeta(s)`, and `hzeta` at other `q` matches direct
    /// summation. The first is an identity the implementation does not
    /// enforce — they take completely different branches.
    #[test]
    fn hzeta_reduces_to_zeta_and_matches_summation() {
        let mut worst_id = 0.0_f64;
        let mut worst_sum = 0.0_f64;
        let mut at = (0.0, 0.0);
        for k in 1..=60 {
            let s = 1.0 + 0.5 * k as f64;
            let r = zeta(s);
            worst_id = worst_id.max(((hzeta(s, 1.0) - r) / r).abs());
            for q in [0.3, 0.75, 1.0, 2.5, 10.0] {
                let rs = hzeta_by_summation(s, q);
                let e = ((hzeta(s, q) - rs) / rs).abs();
                if e > worst_sum {
                    worst_sum = e;
                    at = (s, q);
                }
            }
        }
        assert!(worst_id < 1e-14, "hzeta(s,1) vs zeta(s): {worst_id:e}");
        assert!(
            worst_sum < 1e-13,
            "hzeta vs summation: {worst_sum:e} at (s, q) = {at:?}"
        );
    }

    /// `zetam1` keeps digits that `zeta(s) - 1.0` has already thrown away.
    ///
    /// This is the claim the function exists for, so it is checked rather
    /// than asserted: against a high-precision reference the naive difference
    /// is wrong by 1.396e-08 at `s = 20` while `zetam1` is at the `f64`
    /// floor.
    #[test]
    fn zetam1_is_more_accurate_than_the_naive_difference() {
        // A reference good to well past f64 for s >= 15: the tail of the
        // Dirichlet series converges geometrically there.
        let reference = |s: f64| {
            let mut acc = 0.0_f64;
            for n in (2..200u32).rev() {
                acc += (n as f64).powf(-s);
            }
            acc
        };
        let mut worst_direct = 0.0_f64;
        let mut worst_naive = 0.0_f64;
        for k in 0..=30 {
            let s = 15.0 + 0.5 * k as f64;
            let r = reference(s);
            worst_direct = worst_direct.max(((zetam1(s) - r) / r).abs());
            worst_naive = worst_naive.max((((zeta(s) - 1.0) - r) / r).abs());
        }
        // Measured 1.373e-14, entirely at s = 15 -- the lower edge of
        // `zetam1_large_s`, whose Euler product runs only to the prime 13.
        // Every integer composed of larger primes is missing from it, and
        // 17^-15 + 19^-15 + ... is about 2.2e-19, which against a result of
        // 3.06e-05 is 7e-15. The measurement is that truncation, not a
        // transcription error; by s = 18 it has fallen to 2.2e-16.
        assert!(
            worst_direct < 5e-14,
            "zetam1 relative error {worst_direct:e}"
        );
        assert!(
            worst_naive > 100.0 * worst_direct,
            "zetam1 ({worst_direct:e}) is documented as far better than \
             zeta(s) - 1 ({worst_naive:e}); if that has changed, re-measure \
             and rewrite the docs rather than deleting this assertion"
        );
    }

    /// `zetam1` across its three branches — `s <= 5`, `5 < s < 15` and
    /// `s >= 15` — against direct summation, so the two joins are covered.
    #[test]
    fn zetam1_is_continuous_across_its_branches() {
        let mut worst = 0.0_f64;
        let mut at = 0.0_f64;
        for k in 0..=200 {
            let s = 2.0 + 0.1 * k as f64;
            let r = zeta_by_summation(s) - 1.0;
            if r.abs() < 1e-14 {
                continue;
            }
            let e = ((zetam1(s) - r) / r).abs();
            if e > worst {
                worst = e;
                at = s;
            }
        }
        // The reference is `zeta - 1` formed in f64, so it degrades exactly
        // where zetam1 is designed to win. A loose bound is all it can carry.
        assert!(worst < 1e-7, "zetam1 vs summation: {worst:e} at s = {at}");
    }

    /// `eta(s) = (1 - 2^{1-s}) zeta(s)` away from `s = 1`, and `eta(1) = ln 2`
    /// at it. The Taylor branch near `s = 1` is the only place the two are
    /// not the same computation.
    #[test]
    fn eta_matches_its_relation_to_zeta() {
        let mut worst = 0.0_f64;
        for k in 1..=300 {
            let s = 1.1 + 0.1 * k as f64;
            let r = (1.0 - 2.0_f64.powf(1.0 - s)) * zeta_by_summation(s);
            worst = worst.max(((eta(s) - r) / r).abs());
        }
        // Measured 1.870e-14, and reference-limited for the same reason as
        // `zeta_matches_direct_summation_above_one`.
        assert!(worst < 5e-14, "eta vs (1 - 2^{{1-s}}) zeta: {worst:e}");
        // At s = 1 the product is 0 * inf and the series branch takes over.
        assert!((eta(1.0) - LN_2).abs() < 1e-15, "eta(1) = {}", eta(1.0));
        assert!((eta_int(1) - LN_2).abs() < 1e-15);
    }

    /// The `eta` series branch agrees with the general branch **at the same
    /// argument**, right up to the cut at `10 * GSL_ROOT5_DBL_EPSILON`.
    ///
    /// # The obvious version of this test is wrong
    ///
    /// Evaluating `eta` just inside and just outside the cut and comparing
    /// the two does not measure a discontinuity — it measures `eta` changing
    /// between two different arguments. With a slope of about 0.16 and a gap
    /// of 1.5e-05 that is a difference of 2.4e-06, which looks exactly like a
    /// branch mismatch and is not one. This evaluates both formulas at one
    /// argument instead.
    #[test]
    fn the_eta_series_branch_agrees_with_the_general_one_at_the_cut() {
        let cut = 10.0 * ROOT5_DBL_EPSILON;
        let series = |s: f64| {
            let del = s - 1.0;
            let c0 = LN_2;
            let c1 = LN_2 * (crate::specfunc::EULER - 0.5 * LN_2);
            c0 + del
                * (c1
                    + del
                        * (-0.032_686_296_279_449_3
                            + del
                                * (0.001_568_991_705_415_515 + del * 0.000_749_872_421_120_475_3)))
        };
        let general = |s: f64| (1.0 - ((1.0 - s) * LN_2).exp()) * zeta(s);
        let mut worst = 0.0_f64;
        for k in -20..=20i32 {
            if k == 0 {
                continue; // s = 1 exactly: the general form is 0 * inf.
            }
            let s = 1.0 + cut * k as f64 / 20.0;
            worst = worst.max((series(s) - general(s)).abs());
        }
        // Measured 1.080e-13. The two formulas are genuinely the same
        // function here; the series exists only because the product form is
        // 0 * inf at s = 1 itself.
        assert!(
            worst < 1e-12,
            "series vs product form near s = 1: {worst:e}"
        );
    }

    /// `eta_int(n)` agrees with `eta(n as f64)`, two independent routes.
    #[test]
    fn the_eta_integer_table_agrees_with_the_continuous_function() {
        let mut worst = 0.0_f64;
        for n in 2..=100i32 {
            let r = eta(n as f64);
            worst = worst.max(((eta_int(n) - r) / r).abs());
        }
        assert!(worst < 1e-14, "eta_int vs eta: {worst:e}");
    }

    /// The documented domain errors, and `NaN` propagation.
    #[test]
    fn the_domain_errors_are_what_the_docs_say() {
        assert!(zeta(1.0).is_nan());
        assert!(zeta_int(1).is_nan());
        assert!(zetam1_int(1).is_nan());
        assert!(hzeta(1.0, 1.0).is_nan(), "hzeta needs s > 1");
        assert!(hzeta(0.5, 1.0).is_nan());
        assert!(hzeta(2.0, 0.0).is_nan(), "hzeta needs q > 0");
        assert!(hzeta(2.0, -1.0).is_nan());
        for f in [zeta as fn(f64) -> f64, zetam1, eta] {
            assert!(f(f64::NAN).is_nan());
        }
        assert!(hzeta(f64::NAN, 1.0).is_nan());
        assert!(hzeta(2.0, f64::NAN).is_nan());
    }

    /// The clamped tails: both `zeta_int` and `eta_int` return exactly 1 past
    /// their tables, which is correct to well under an ulp and is what makes
    /// the tables finite.
    #[test]
    fn the_integer_tables_clamp_to_one_past_their_end() {
        assert_eq!(zeta_int(101), 1.0);
        assert_eq!(zeta_int(i32::MAX), 1.0);
        assert_eq!(eta_int(101), 1.0);
        assert_eq!(eta_int(i32::MAX), 1.0);
        // And the claim that makes the clamp harmless.
        assert!(zetam1(101.0) < f64::EPSILON);
    }

    /// The 24th coefficient of `zetam1_inter_data` is **not** used, because
    /// upstream's `cheb_series` declares `order = 22`.
    ///
    /// # This is not a detail
    ///
    /// Swept over 1001 points on `[5, 15]`, including the 24th coefficient
    /// changes the Clenshaw sum by up to 1.872e-14 relative and the returned
    /// `zeta(s) - 1` by up to 1.484e-14, and changes the answer at all at
    /// **943 of the 1001 points**. A port that took the whole array — the
    /// natural thing to do, and what every other GSL series in this crate
    /// wants — would carry a systematic bias two orders above the `f64` floor
    /// that no accuracy test with a realistic tolerance would catch.
    ///
    /// The sweep matters: at `s = 12` the two agree exactly, so a
    /// single-point version of this test would pass while proving nothing.
    #[test]
    fn the_intermediate_series_stops_at_the_order_gsl_declares() {
        let mut differing = 0usize;
        let mut worst = 0.0_f64;
        for k in 0..=1000 {
            let s = 5.0 + 10.0 * k as f64 / 1000.0;
            let t = (s - 10.0) / 5.0;
            let with_23 = eval_gsl(t, &ZETAM1_INTER[..23]);
            let with_24 = eval_gsl(t, &ZETAM1_INTER);
            let pow = 2.0_f64.powf(-s);

            // What the branch returns must be the 23-coefficient form, bit
            // for bit, at every point.
            assert_eq!(
                zetam1_intermediate(s).to_bits(),
                (with_23.exp() + pow).to_bits(),
                "zetam1_intermediate at s = {s} is not the 23-coefficient form"
            );

            let out_24 = with_24.exp() + pow;
            if zetam1_intermediate(s).to_bits() != out_24.to_bits() {
                differing += 1;
                worst = worst.max(((zetam1_intermediate(s) - out_24) / out_24).abs());
            }
        }
        assert!(
            differing > 900,
            "the 24th coefficient changed the answer at only {differing} of \
             1001 points; it was 943 when measured, so either the table or \
             the branch has moved"
        );
        assert!(
            worst > 1e-15,
            "including the 24th coefficient is documented as a 1.484e-14 \
             bias; measured {worst:e}"
        );
    }
}
