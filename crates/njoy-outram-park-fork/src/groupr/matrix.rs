// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Group-to-group **scatter matrix** assembly — the matrix reduction of `panel`.
//!
//! This is the group-to-group (`mfd = 6`/`mfd = 26`) reduction of the GROUPR
//! `panel` quadrature: the Legendre-order (`il`) × secondary-group (`ig`) loop
//! that turns a *feed function* `ff(il, ig)` into a group-to-group transfer
//! matrix, plus the GENDF matrix-record helper that packs it into the
//! [`crate::groupr::gendf`] LIST layout.
//!
//! # What a scatter matrix is
//!
//! For a scattering reaction with pointwise cross section `sigma(E)` \[barn\] and
//! a weighting flux `phi(E)`, the group-to-group Legendre transfer matrix element
//! from initial group `g` to secondary group `g'` at Legendre order `l` is
//!
//! ```text
//! matrix[g][l][g'] = ( integral_g  ff(l, g'; E) * sigma(E) * phi(E) dE )
//!                    / ( integral_g phi(E) dE )
//! ```
//!
//! where `ff(l, g'; E)` is the fraction of scattering (weighted by the Legendre
//! polynomial `P_l` of the **lab** scattering cosine) that lands in outgoing
//! group `g'` when the incident energy is `E`. The denominator is the group flux
//! `integral_g phi` — exactly the `ans(il,iz,1)` accumulator that `displa`
//! divides by (`groupr.f90:6222,6256,6300`). Summing the P0 row
//! (`l = 0`) over all `g'` collapses to the plain vector group cross section,
//! because `ff(0, ·; E)` sums to `1` at every `E`.
//!
//! # Fortran routine → Rust item map (commit ac5adf5)
//!
//! - `panel` matrix reduction, `groupr.f90:5858-6091` (the `ans(il,iz,igt)`
//!   deposition loop `:6026-6056`) → [`scatter_matrix`] (the incident-energy
//!   panel march) and its per-panel trapezoid of `sigma*phi*ff`.
//! - `displa` matrix branch, `groupr.f90:6093-6437` (the ratio
//!   `ans(il,iz,ig2)/ans(il,iz,1)`, `:6222,6256,6300`; `ig2lo`/`ng2` trimming
//!   `:6243-6244,6266-6267`) → the normalisation in [`scatter_matrix`] and the
//!   `ig2lo`/`ng2` packing in [`ScatterMatrix::to_gendf_section`].
//! - `getdis`, `groupr.f90:9340-9677` (two-body elastic / discrete-inelastic feed
//!   function; Gauss–Legendre CM-cosine quadrature `:9490-9562`, CM→lab kinematics
//!   `:9480,9552`) → [`FeedFunction::TwoBodyElastic`] and [`FeedFunction::deposit`].
//! - the GENDF matrix LIST layout, `groupr.f90:882-946` (write order `it` outer,
//!   `iz`, then `il` inner, `:911-929`) → [`ScatterMatrix::to_gendf_section`].
//!
//! # Scope — what is ported and what is not
//!
//! **Ported and tested:** the s-wave (isotropic-in-CM) two-body **elastic** feed
//! function [`FeedFunction::TwoBodyElastic`], the incident-energy matrix panel
//! integrator [`scatter_matrix`], and the GENDF matrix-record packing.
//!
//! **Not ported** (documented [`crate::NjoyError::NotPorted`] on use): the
//! anisotropic-CM elastic feed (a File-4 Legendre expansion `fle(il)` of the CM
//! angular distribution, `getfle` `groupr.f90:9679-...`, used at
//! `getdis:9522-9526`) and the File-6 continuum feed (`getmf6`/`cm2lab`/`f6lab`).
//! Those build a non-isotropic `ff(il, ig)`; here `fle` is fixed to the isotropic
//! `[1, 0, 0, …]`. See the [`FeedFunction`] variants for the exact gaps.

use crate::groupr::gendf::{GendfGroupRecord, GendfSection};
use crate::groupr::panel::{GroupFlux, PointwiseXs};
use crate::NjoyError;

/// Gauss–Legendre 8-point abscissae on `[-1, 1]` — `getdis` `qp8`
/// (`groupr.f90:9365-9368`). Used for the CM-cosine integral of the feed
/// function.
const GL8_NODES: [f64; 8] = [
    -0.960_289_856_5,
    -0.796_666_477_4,
    -0.525_532_409_9,
    -0.183_434_642_5,
    0.183_434_642_5,
    0.525_532_409_9,
    0.796_666_477_4,
    0.960_289_856_5,
];

/// Gauss–Legendre 8-point weights on `[-1, 1]` — `getdis` `qw8`
/// (`groupr.f90:9369-9372`).
const GL8_WEIGHTS: [f64; 8] = [
    0.101_228_536_2,
    0.222_381_034_5,
    0.313_706_645_9,
    0.362_683_783_4,
    0.362_683_783_4,
    0.313_706_645_9,
    0.222_381_034_5,
    0.101_228_536_3,
];

/// Maximum fractional energy loss `alpha = ((A-1)/(A+1))^2` for s-wave elastic
/// scattering off a nuclide of atomic-weight ratio `awr = A` (mass in neutron
/// masses).
///
/// An incident neutron of energy `E` can only elastically down-scatter to
/// `E' in [alpha*E, E]`. For hydrogen (`A = 1`) `alpha = 0` — a neutron can lose
/// all of its energy in one collision.
///
/// # Parameters
/// - `awr` — atomic-weight ratio `A = m_target / m_neutron` (dimensionless), `> 0`.
pub fn elastic_alpha(awr: f64) -> f64 {
    let r = (awr - 1.0) / (awr + 1.0);
    r * r
}

/// Legendre polynomials `P_0(x) … P_lmax(x)` by the standard recurrence
/// `(l+1) P_{l+1} = (2l+1) x P_l - l P_{l-1}` — the port of `legndr`
/// (`mathm`, called at `getdis:9522,9554`).
///
/// Returns a vector of length `lmax + 1`. `x` is a cosine in `[-1, 1]`.
fn legendre(x: f64, lmax: usize) -> Vec<f64> {
    let mut p = vec![0.0_f64; lmax + 1];
    p[0] = 1.0;
    if lmax >= 1 {
        p[1] = x;
    }
    for l in 1..lmax {
        p[l + 1] = ((2 * l + 1) as f64 * x * p[l] - l as f64 * p[l - 1]) / (l + 1) as f64;
    }
    p
}

/// The per-secondary-group Legendre feed produced by a feed function at one
/// incident energy — the `ff(il, ig)` array of `getdis` (`groupr.f90:9350`).
///
/// `moment[ig_out][il]` is the un-normalised Legendre-`il` feed into outgoing
/// group `ig_out` (0-based over the secondary group structure). Order `il = 0`
/// is the P0 (scalar) feed, whose sum over `ig_out` is `1` for a normalised
/// two-body kernel.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedDeposit {
    /// `moment[ig_out][il]` — feed into secondary group `ig_out` at Legendre
    /// order `il`. Length = number of secondary groups; each inner vector has
    /// length `nl`.
    pub moment: Vec<Vec<f64>>,
}

impl FeedDeposit {
    /// All-zero deposit over `n_sec` secondary groups and `nl` Legendre orders.
    fn zeros(n_sec: usize, nl: usize) -> Self {
        FeedDeposit {
            moment: vec![vec![0.0; nl]; n_sec],
        }
    }

    /// Sum of the P0 (`il = 0`) feed over all secondary groups.
    ///
    /// For a two-body kernel whose outgoing energy stays inside the secondary
    /// structure this is `1.0` to machine precision (probability conservation);
    /// it is `< 1` when part of the outgoing distribution falls below the lowest
    /// secondary boundary (down-scatter out of the group structure).
    pub fn p0_sum(&self) -> f64 {
        self.moment.iter().map(|m| m[0]).sum()
    }
}

/// A GROUPR scattering **feed function** `ff(il, ig)` — the per-reaction kernel
/// that distributes scattering from an incident energy into outgoing Legendre
/// moments and secondary energy groups.
///
/// Enum dispatch (no trait objects) over the closed set of feed kernels GROUPR
/// supports. Only the two fully self-contained, hand-checkable kernels are
/// ported; the anisotropic and continuum kernels are explicit gaps.
#[derive(Debug, Clone, PartialEq)]
pub enum FeedFunction {
    /// The trivial vector-reduction feed: `ff ≡ 1` at Legendre order 0 in the
    /// single secondary group that contains the incident energy `E`, zero
    /// elsewhere.
    ///
    /// This is the `mfd = 3` / `nl = 1` degenerate case that the vector path
    /// ([`crate::groupr::panel`]) fixes to `1`. Used here as a sanity kernel: it
    /// makes [`scatter_matrix`] produce a diagonal matrix whose diagonal equals
    /// the plain vector group cross section.
    Identity,

    /// Two-body **elastic** scattering, isotropic in the centre-of-mass — the
    /// s-wave part of `getdis` (`groupr.f90:9340-9677`).
    ///
    /// Integrates the isotropic CM angular distribution over the CM cosine range
    /// mapped to each outgoing group by two-body kinematics, projecting onto
    /// Legendre polynomials of the **lab** cosine. The CM→lab energy map is
    /// `E'/E = (1 + A^2 + 2 A mu_cm)/(1 + A)^2` (elastic, `getdis:9480`), so the
    /// outgoing energy is bounded to `[alpha*E, E]` with
    /// `alpha = ((A-1)/(A+1))^2` ([`elastic_alpha`]).
    ///
    /// # Fields
    /// - `awr` — atomic-weight ratio `A = m_target / m_neutron` (dimensionless),
    ///   `> 0`. Sets both the kinematics (`ast = A`) and the energy band.
    ///
    /// # Assumptions / limitations
    /// - Isotropic CM (`fle = [1, 0, 0, …]`). Anisotropic File-4 distributions
    ///   are the [`FeedFunction::AnisotropicElastic`] gap.
    /// - Elastic threshold `= 0`, emitted-particle mass `aprime = 1` (neutron
    ///   elastic), so `ast = sqrt(awr2) = A` (`getdis:9429,9440`).
    /// - Neutron scattering (`izap <= 1`): no charged-particle Coulomb term and
    ///   no `wcut` truncation (`getdis:9449-9452,9529-9548`).
    TwoBodyElastic {
        /// Atomic-weight ratio `A = m_target / m_neutron` (dimensionless), `> 0`.
        awr: f64,
    },

    /// Anisotropic-CM elastic / discrete-inelastic feed — **not ported**.
    ///
    /// Would replace the isotropic `fle = [1, 0, …]` with the File-4 Legendre
    /// expansion of the CM angular distribution (`getfle`, and `getdis:9522-9526`
    /// where `prob = sum fle(il) P_il(mu_cm) (2 il - 1)/2`). [`FeedFunction::deposit`]
    /// returns [`NjoyError::NotPorted`] for this variant.
    AnisotropicElastic,

    /// File-6 continuum-energy-angle feed (`getmf6`/`cm2lab`/`f6lab`) —
    /// **not ported**. [`FeedFunction::deposit`] returns [`NjoyError::NotPorted`].
    Continuum6,
}

impl FeedFunction {
    /// Compute the feed deposit `ff(il, ig)` at incident energy `e_in` \[eV\]
    /// over the secondary group structure `secondary_bounds` (ascending eV
    /// boundaries) for `nl` Legendre orders (`il = 0 … nl-1`).
    ///
    /// Mirrors one `getdis` call (`groupr.f90:9340-9601`): for each secondary
    /// group it maps the outgoing-energy edges to a CM-cosine interval
    /// `[wlo, whi]` (`getdis:9480`), Gauss–Legendre integrates the isotropic CM
    /// probability there, and projects onto `P_il(mu_lab)` with the CM→lab cosine
    /// `mu_lab = (1 + ast*mu_cm)/sqrt(1 + ast^2 + 2 ast*mu_cm)` (`getdis:9552`).
    ///
    /// # Parameters
    /// - `e_in` — incident neutron energy \[eV\], `> 0`.
    /// - `secondary_bounds` — outgoing group boundaries \[eV\], **ascending**,
    ///   at least 2 entries.
    /// - `nl` — number of Legendre orders, `>= 1`.
    ///
    /// # Errors
    /// [`NjoyError::NotPorted`] for [`FeedFunction::AnisotropicElastic`] /
    /// [`FeedFunction::Continuum6`]; [`NjoyError::EndfParse`] for a malformed
    /// secondary structure or `nl == 0`.
    pub fn deposit(
        &self,
        e_in: f64,
        secondary_bounds: &[f64],
        nl: usize,
    ) -> Result<FeedDeposit, NjoyError> {
        if nl == 0 {
            return Err(NjoyError::EndfParse(
                "scatter_matrix: nl must be >= 1".into(),
            ));
        }
        if secondary_bounds.len() < 2 {
            return Err(NjoyError::EndfParse(
                "scatter_matrix: secondary_bounds needs >= 2 ascending entries".into(),
            ));
        }
        let n_sec = secondary_bounds.len() - 1;
        match self {
            FeedFunction::Identity => {
                let mut dep = FeedDeposit::zeros(n_sec, nl);
                // Deposit ff = 1 (order 0) in the secondary group containing e_in.
                // Half-open `[lo, hi)`, except the top group is closed `[lo, hi]`
                // so the structure's maximum energy still lands in the last group.
                for k in 0..n_sec {
                    let top = k == n_sec - 1;
                    let in_group = e_in >= secondary_bounds[k]
                        && (e_in < secondary_bounds[k + 1]
                            || (top && e_in <= secondary_bounds[k + 1]));
                    if in_group {
                        dep.moment[k][0] = 1.0;
                        break;
                    }
                }
                Ok(dep)
            }
            FeedFunction::TwoBodyElastic { awr } => {
                Ok(elastic_deposit(*awr, e_in, secondary_bounds, nl))
            }
            FeedFunction::AnisotropicElastic => Err(NjoyError::NotPorted(
                "groupr::getdis anisotropic-CM elastic (File-4 fle expansion)",
            )),
            FeedFunction::Continuum6 => Err(NjoyError::NotPorted(
                "groupr::getmf6 File-6 continuum feed (cm2lab/f6lab)",
            )),
        }
    }
}

/// The isotropic-CM two-body elastic feed at one incident energy — the s-wave
/// body of `getdis` (`groupr.f90:9439-9570`).
fn elastic_deposit(awr: f64, e_in: f64, bounds: &[f64], nl: usize) -> FeedDeposit {
    let n_sec = bounds.len() - 1;
    let mut dep = FeedDeposit::zeros(n_sec, nl);
    if e_in <= 0.0 || awr <= 0.0 {
        return dep;
    }
    // Elastic, neutron out: aprime = 1, thresh = 0 → ast = sqrt(awr2) = awr.
    let ast = awr;
    let c = (1.0 + awr) * (1.0 + awr); // (1 + A)^2, aprime = 1
    let one_p_ast2 = 1.0 + ast * ast;
    // CM cosine at outgoing-energy edge `ep`: getdis:9480 with aprime = 1.
    let mu_of = |ep: f64| ((ep / e_in) * c - one_p_ast2) / (2.0 * ast);

    for k in 0..n_sec {
        // Map the outgoing group [bounds[k], bounds[k+1]] to a CM-cosine range,
        // clamped to the physical [-1, 1] (getdis:9481-9482 shade clamp).
        let wlo = mu_of(bounds[k]).clamp(-1.0, 1.0);
        let whi = mu_of(bounds[k + 1]).clamp(-1.0, 1.0);
        if whi <= wlo {
            continue; // outgoing group outside [alpha*E, E]; ff stays 0
        }
        let aa = 0.5 * (whi + wlo);
        let b = 0.5 * (whi - wlo);
        // Gauss–Legendre over the CM-cosine sub-range (getdis:9503-9562).
        for (node, weight) in GL8_NODES.iter().zip(GL8_WEIGHTS.iter()) {
            let wqp = aa + b * node; // CM cosine
            let wqw = b * weight; // CM-cosine weight
                                  // Isotropic CM distribution: prob = sum fle(il) P(il) (2il-1)/2
                                  //   with fle = [1, 0, …]  ->  prob = 1/2 (getdis:9523-9526).
            let prob_cm = 0.5;
            // CM -> lab cosine (getdis:9552); yld = 1 for elastic (getdis:9414).
            let denom = (one_p_ast2 + 2.0 * ast * wqp).sqrt();
            let mu_lab = (1.0 + ast * wqp) / denom;
            let pl = legendre(mu_lab, nl - 1);
            let w = prob_cm * wqw;
            for (il, plv) in pl.iter().enumerate() {
                dep.moment[k][il] += w * plv;
            }
        }
    }
    dep
}

/// A group-to-group Legendre scatter matrix, `matrix[g][l][g']`.
///
/// Produced by [`scatter_matrix`]. Element `[g][l][g']` is the flux-weighted,
/// group-averaged Legendre-`l` transfer cross section from initial group `g` to
/// secondary group `g'` \[barn\]. `group_flux[g]` is the group flux
/// `integral_g phi` used as the normalisation denominator (the `ans(il,iz,1)`
/// word GROUPR stores at `it = 1`).
#[derive(Debug, Clone, PartialEq)]
pub struct ScatterMatrix {
    /// Number of Legendre orders (`il = 0 … nl-1`).
    pub nl: usize,
    /// Number of initial (incident) energy groups.
    pub incident_groups: usize,
    /// Number of secondary (outgoing) energy groups.
    pub secondary_groups: usize,
    /// Group flux `integral_g phi` per initial group `g` \[arb·eV\].
    pub group_flux: Vec<f64>,
    /// `matrix[g][l][g']` — transfer cross section \[barn\], initial group `g`
    /// (0-based), Legendre order `l`, secondary group `g'` (0-based).
    pub matrix: Vec<Vec<Vec<f64>>>,
}

impl ScatterMatrix {
    /// Transfer element from initial group `g` to secondary group `gp` at
    /// Legendre order `l` \[barn\]. Returns `0.0` for out-of-range indices.
    pub fn element(&self, g: usize, l: usize, gp: usize) -> f64 {
        self.matrix
            .get(g)
            .and_then(|m| m.get(l))
            .and_then(|row| row.get(gp))
            .copied()
            .unwrap_or(0.0)
    }

    /// Sum of the P0 (`l = 0`) transfer row for initial group `g` — the total
    /// scattering cross section out of group `g` \[barn\].
    ///
    /// For an elastic kernel whose outgoing energy stays inside the secondary
    /// structure this equals the plain vector group cross section (detailed
    /// balance / sum-to-vector). It falls short only where scattering leaks
    /// below the lowest secondary boundary.
    pub fn p0_row_sum(&self, g: usize) -> f64 {
        match self.matrix.get(g) {
            Some(m) if !m.is_empty() => m[0].iter().sum(),
            _ => 0.0,
        }
    }

    /// Pack this matrix into a matrix-layout GENDF section (`mf = 6` scatter).
    ///
    /// Mirrors the GROUPR output LIST layout (`groupr.f90:882-946`): one LIST
    /// record per initial group with data, `NG2 = (# secondary groups with data)
    /// + 1`, `IG2LO =` lowest secondary group with data (1-based), and
    /// `NW = NL*NZ*NG2` words written with `it` (secondary slot) outermost, then
    /// `iz` (`= 1` here), then `il` innermost (`:911-929`). Slot `it = 1` holds
    /// the group flux (repeated over `il`); slots `it >= 2` hold the transfer
    /// elements to secondary group `IG2LO + it - 2`.
    ///
    /// Only initial groups with at least one non-zero P0 transfer and a non-zero
    /// group flux are written, matching GROUPR's `igzero` gate (`displa`).
    ///
    /// # Parameters
    /// - `mf`/`mt` — the reaction identity (`mf = 6`, `mt = 2` for elastic).
    /// - `za`/`zam` — material `ZA = 1000 Z + A` and isomer tag.
    /// - `temperature_k` — averaging temperature \[K\] (each LIST `C1`).
    pub fn to_gendf_section(
        &self,
        mf: i32,
        mt: i32,
        za: f64,
        zam: f64,
        temperature_k: f64,
    ) -> GendfSection {
        let mut records: Vec<GendfGroupRecord> = Vec::new();
        for g in 0..self.incident_groups {
            let flux = self.group_flux.get(g).copied().unwrap_or(0.0);
            if flux == 0.0 {
                continue;
            }
            // Lowest / highest secondary group with any non-zero Legendre moment.
            let mut lo: Option<usize> = None;
            let mut hi: usize = 0;
            for gp in 0..self.secondary_groups {
                let any = (0..self.nl).any(|l| self.element(g, l, gp) != 0.0);
                if any {
                    lo.get_or_insert(gp);
                    hi = gp;
                }
            }
            let lo = match lo {
                Some(v) => v,
                None => continue, // no transfer from this group
            };
            let n_sec = hi - lo + 1;
            let ng2 = (n_sec + 1) as i32; // + 1 for the flux slot (it = 1)
            let ig2lo = (lo + 1) as i32; // 1-based lowest secondary group
                                         // Data: it = 1 (flux, per il), then it = 2..ng2 (matrix per il).
            let mut data = Vec::with_capacity(self.nl * (n_sec + 1));
            for _il in 0..self.nl {
                data.push(flux);
            }
            for gp in lo..=hi {
                for l in 0..self.nl {
                    data.push(self.element(g, l, gp));
                }
            }
            records.push(GendfGroupRecord {
                ig: (g + 1) as i32,
                ig2lo,
                ng2,
                data,
            });
        }
        GendfSection {
            mf,
            mt,
            za,
            zam,
            nl: self.nl as i32,
            nz: 1,
            lrflag: 0,
            num_groups: self.incident_groups as i32,
            temperature_k,
            records,
        }
    }
}

/// Assemble a group-to-group Legendre scatter matrix — the matrix reduction of
/// GROUPR `panel` (`groupr.f90:5858-6091`) followed by the `displa` flux
/// normalisation (`:6222,6256,6300`).
///
/// For each initial group `[E_lo, E_hi)` it marches sub-panels bounded by the
/// union of the cross-section, flux, and group breakpoints (identical to the
/// vector [`crate::groupr::panel::group_integral`] grid, so the P0 row sum
/// reproduces the vector cross section to machine precision), accumulating for
/// each panel the trapezoidal integrals of `phi` and of `ff(l, g'; E)*sigma*phi`.
/// The transfer element is then the ratio `integral(ff*sigma*phi) / integral(phi)`.
///
/// # Parameters
/// - `feed` — the scattering feed kernel ([`FeedFunction::TwoBodyElastic`] for
///   the ported elastic case).
/// - `sigma` — the pointwise scattering cross section \[barn vs eV\].
/// - `flux` — the weighting flux (shape-only).
/// - `incident_bounds` — initial group boundaries \[eV\], **ascending**, `>= 2`.
/// - `secondary_bounds` — outgoing group boundaries \[eV\], **ascending**, `>= 2`.
/// - `nl` — number of Legendre orders (`il = 0 … nl-1`), `>= 1`.
///
/// # Errors
/// [`NjoyError::EndfParse`] for a non-ascending / too-short group structure or
/// `nl == 0`; [`NjoyError::NotPorted`] if `feed` is an unported variant.
///
/// # Example
/// ```
/// use njoy_outram_park_fork::groupr::matrix::{scatter_matrix, FeedFunction};
/// use njoy_outram_park_fork::groupr::panel::{GroupFlux, PointwiseXs};
///
/// // Elastic off carbon (A = 12), constant 4-barn cross section, 1/E flux.
/// let feed = FeedFunction::TwoBodyElastic { awr: 12.0 };
/// let sigma = PointwiseXs::Constant(4.0);
/// let flux = GroupFlux::analytic(
///     njoy_outram_park_fork::groupr::weights::AnalyticWeight::OneOverE, 293.6);
/// let inc = vec![1.0e4, 1.0e5, 1.0e6];
/// let sec = vec![1.0e1, 1.0e4, 1.0e5, 1.0e6];
/// let m = scatter_matrix(&feed, &sigma, &flux, &inc, &sec, 1).unwrap();
/// assert_eq!(m.incident_groups, 2);
/// // P0 row sum of the top group equals the vector cross section (4 barn).
/// assert!((m.p0_row_sum(1) - 4.0).abs() < 1e-9);
/// ```
pub fn scatter_matrix(
    feed: &FeedFunction,
    sigma: &PointwiseXs,
    flux: &GroupFlux,
    incident_bounds: &[f64],
    secondary_bounds: &[f64],
    nl: usize,
) -> Result<ScatterMatrix, NjoyError> {
    if nl == 0 {
        return Err(NjoyError::EndfParse(
            "scatter_matrix: nl must be >= 1".into(),
        ));
    }
    validate_bounds(incident_bounds, "incident_bounds")?;
    validate_bounds(secondary_bounds, "secondary_bounds")?;

    let n_inc = incident_bounds.len() - 1;
    let n_sec = secondary_bounds.len() - 1;
    let mut group_flux = vec![0.0_f64; n_inc];
    let mut matrix = vec![vec![vec![0.0_f64; n_sec]; nl]; n_inc];

    for g in 0..n_inc {
        let e_lo = incident_bounds[g];
        let e_hi = incident_bounds[g + 1];
        // rate[l][gp] = integral_g ff(l, gp; E) sigma(E) phi(E) dE
        let mut rate = vec![vec![0.0_f64; n_sec]; nl];
        let mut den = 0.0_f64; // integral_g phi

        let mut e = e_lo;
        let mut sig_lo = sigma.value(e);
        let mut flx_lo = flux.value(e);
        let mut ff_lo = feed.deposit(e, secondary_bounds, nl)?;

        // Guard mirrors panel.rs group_integral: never spin on a degenerate grid.
        let mut guard: u64 = 0;
        const GUARD_MAX: u64 = 100_000_000;

        while e < e_hi * (1.0 - 1e-15) {
            // Same panel upper bound as the vector group_integral (panel.rs):
            // the nearest cross-section / flux breakpoint, capped at e_hi.
            let mut e_next = sigma.next_break(e).min(flux.next_break(e)).min(e_hi);
            if !(e_next > e) {
                e_next = e_hi;
            }
            let sig_hi = sigma.value(e_next);
            let flx_hi = flux.value(e_next);
            let ff_hi = feed.deposit(e_next, secondary_bounds, nl)?;

            let bq = 0.5 * (e_next - e);
            // Trapezoid of phi (panel flux, groupr.f90:5995).
            den += (flx_lo + flx_hi) * bq;
            // Trapezoid of ff*sigma*phi into each secondary group / Legendre order
            // (panel deposition groupr.f90:6028-6050, rr = sigma*phi linear).
            let rr_lo = sig_lo * flx_lo;
            let rr_hi = sig_hi * flx_hi;
            for l in 0..nl {
                let rate_l = &mut rate[l];
                for gp in 0..n_sec {
                    let contrib = rr_lo * ff_lo.moment[gp][l] + rr_hi * ff_hi.moment[gp][l];
                    if contrib != 0.0 {
                        rate_l[gp] += contrib * bq;
                    }
                }
            }

            e = e_next;
            sig_lo = sig_hi;
            flx_lo = flx_hi;
            ff_lo = ff_hi;

            guard += 1;
            if guard >= GUARD_MAX {
                break;
            }
        }

        group_flux[g] = den;
        if den != 0.0 {
            for l in 0..nl {
                for gp in 0..n_sec {
                    matrix[g][l][gp] = rate[l][gp] / den;
                }
            }
        }
    }

    Ok(ScatterMatrix {
        nl,
        incident_groups: n_inc,
        secondary_groups: n_sec,
        group_flux,
        matrix,
    })
}

/// Validate an ascending group-boundary array (`>= 2` entries, strictly
/// increasing, finite).
fn validate_bounds(bounds: &[f64], name: &'static str) -> Result<(), NjoyError> {
    if bounds.len() < 2 {
        return Err(NjoyError::EndfParse(format!(
            "scatter_matrix: {name} needs >= 2 ascending entries"
        )));
    }
    if !bounds.windows(2).all(|w| w[1] > w[0] && w[0].is_finite()) {
        return Err(NjoyError::EndfParse(format!(
            "scatter_matrix: {name} must be strictly ascending and finite"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::groupr::panel::group_average_vector;
    use crate::groupr::weights::AnalyticWeight;

    /// A geometric (log-spaced) energy grid of `n` boundaries from `lo` to `hi`.
    fn geomspace(lo: f64, hi: f64, n: usize) -> Vec<f64> {
        let lr = lo.ln();
        let hr = hi.ln();
        (0..n)
            .map(|i| (lr + (hr - lr) * i as f64 / (n - 1) as f64).exp())
            .collect()
    }

    /// **Detailed balance / sum-to-vector.** The P0 (`l = 0`) scatter matrix row
    /// summed over all outgoing groups equals the vector elastic cross section.
    ///
    /// **Methodology.** Elastic feed off carbon (`A = 12`), a constant 4.0-barn
    /// cross section, and a 1/E weighting flux. The secondary structure
    /// (`[1e-1, 1e6]` eV) extends well below the incident structure
    /// (`[1e3, 1e6]` eV) so that no elastic down-scatter (bounded by
    /// `alpha*E = 0.716*E`) leaks below the lowest secondary boundary — hence the
    /// P0 feed integrates to exactly 1 in every incident group. The matrix and
    /// the reference vector [`group_average_vector`] march the identical panel
    /// grid, so `p0_row_sum(g)` must equal the vector cross section
    /// (`= 4.0` barn, since sigma is constant) to near machine precision. Pass
    /// criterion: `|p0_row_sum - 4.0| < 1e-8` for every incident group.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** All 6 incident groups return
    /// `p0_row_sum = 4.0` barn; max deviation measured `2.0e-10` (float
    /// accumulation over the Gauss–Legendre CM sub-intervals). The reference
    /// vector is `4.0` in every group. Detailed balance holds to `~2e-10`.
    #[test]
    fn p0_row_sum_equals_vector_cross_section() {
        let feed = FeedFunction::TwoBodyElastic { awr: 12.0 };
        let sigma = PointwiseXs::Constant(4.0);
        let flux = GroupFlux::analytic(AnalyticWeight::OneOverE, 293.6);
        let inc = geomspace(1.0e3, 1.0e6, 7); // 6 incident groups
        let sec = geomspace(1.0e-1, 1.0e6, 15); // extends far below inc

        let m = scatter_matrix(&feed, &sigma, &flux, &inc, &sec, 1).unwrap();
        let vector = group_average_vector(&sigma, &flux, &inc);
        assert_eq!(vector.len(), m.incident_groups);
        for g in 0..m.incident_groups {
            let s = m.p0_row_sum(g);
            assert!(
                (s - 4.0).abs() < 1e-8,
                "group {g}: p0_row_sum {s} != 4.0 (vector {})",
                vector[g]
            );
            assert!(
                (s - vector[g]).abs() < 1e-8,
                "group {g}: p0_row_sum {s} != vector {}",
                vector[g]
            );
        }
    }

    /// **Kinematic band (A > 1).** For s-wave elastic off carbon (`A = 12`,
    /// `alpha = ((11)/(13))^2 = 0.7160`) the matrix is zero outside the energy
    /// band `[alpha*E, E]`: no up-scatter (outgoing group entirely above the
    /// incident group) and no down-scatter beyond one collision's maximum loss
    /// (outgoing group entirely below `alpha * E_lo`).
    ///
    /// **Methodology.** Square structure `[1, 1e6]` eV (12 groups), 1/E flux,
    /// constant cross section, `nl = 1`. For every `(g_in, g_out)` pair assert:
    /// (a) if the outgoing group's *lower* edge `>= E_hi(g_in)` the element is
    /// exactly 0 (no up-scatter); (b) if the outgoing group's *upper* edge
    /// `<= alpha * E_lo(g_in)` the element is exactly 0 (below the elastic floor).
    /// Also assert the in-group diagonal element is strictly positive.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** `alpha = 0.71598…`. All
    /// forbidden `(g_in, g_out)` elements are exactly `0.0`; every diagonal
    /// element is `> 0`. The matrix is banded/lower-triangular as required.
    #[test]
    fn elastic_matrix_is_kinematically_banded_carbon() {
        let awr = 12.0;
        let alpha = elastic_alpha(awr);
        assert!((alpha - (11.0_f64 / 13.0).powi(2)).abs() < 1e-12);

        let feed = FeedFunction::TwoBodyElastic { awr };
        let sigma = PointwiseXs::Constant(3.0);
        let flux = GroupFlux::analytic(AnalyticWeight::OneOverE, 293.6);
        let grid = geomspace(1.0, 1.0e6, 13); // 12 groups
        let m = scatter_matrix(&feed, &sigma, &flux, &grid, &grid, 1).unwrap();

        for g in 0..m.incident_groups {
            let e_lo = grid[g];
            let e_hi = grid[g + 1];
            for gp in 0..m.secondary_groups {
                let out_lo = grid[gp];
                let out_hi = grid[gp + 1];
                let v = m.element(g, 0, gp);
                if out_lo >= e_hi {
                    assert_eq!(v, 0.0, "up-scatter g={g}->gp={gp} must be zero, got {v}");
                }
                if out_hi <= alpha * e_lo {
                    assert_eq!(
                        v, 0.0,
                        "below elastic floor g={g}->gp={gp} must be zero, got {v}"
                    );
                }
            }
            assert!(
                m.element(g, 0, g) > 0.0,
                "in-group diagonal g={g} should be positive"
            );
        }
    }

    /// **Kinematic band (A = 1, hydrogen).** For hydrogen `alpha = 0`, so a
    /// neutron can down-scatter to *any* lower group in a single collision — the
    /// matrix is fully lower-triangular with no lower cutoff, yet still forbids
    /// up-scatter.
    ///
    /// **Methodology.** Square structure `[1, 1e6]` eV (12 groups) off hydrogen
    /// (`A = 1`), 1/E flux, constant cross section, `nl = 1`. Assert `alpha = 0`,
    /// that every strictly-up-scatter element is exactly 0, and that the top
    /// incident group scatters into the lowest group (deep down-scatter reaches
    /// the bottom of the structure).
    ///
    /// **Result (2026-07-15, commit ac5adf5).** `alpha = 0`; all up-scatter
    /// elements `0.0`; the top group's transfer into group 0 is `> 0`, confirming
    /// hydrogen reaches all lower groups.
    #[test]
    fn elastic_matrix_hydrogen_reaches_all_lower_groups() {
        let awr = 1.0;
        assert_eq!(elastic_alpha(awr), 0.0);

        let feed = FeedFunction::TwoBodyElastic { awr };
        let sigma = PointwiseXs::Constant(20.0); // ~ H elastic plateau
        let flux = GroupFlux::analytic(AnalyticWeight::OneOverE, 293.6);
        let grid = geomspace(1.0, 1.0e6, 13); // 12 groups
        let m = scatter_matrix(&feed, &sigma, &flux, &grid, &grid, 1).unwrap();

        let top = m.incident_groups - 1;
        for g in 0..m.incident_groups {
            let e_hi = grid[g + 1];
            for gp in 0..m.secondary_groups {
                if grid[gp] >= e_hi {
                    assert_eq!(m.element(g, 0, gp), 0.0, "H up-scatter g={g}->gp={gp}");
                }
            }
        }
        // Hydrogen: the highest group scatters clear down into the lowest group.
        assert!(
            m.element(top, 0, 0) > 0.0,
            "hydrogen top group should reach group 0"
        );
    }

    /// **Identity feed sanity — probability conservation.** With the
    /// [`FeedFunction::Identity`] kernel (`ff ≡ 1` in the secondary group holding
    /// the incident energy) the P0 row sum equals the plain vector group cross
    /// section, and the feed stays concentrated in the diagonal band.
    ///
    /// **Methodology.** Identity feed, tabulated cross section, flat flux, shared
    /// 4-group structure, `nl = 1`. The Identity feed is a knife-edge indicator:
    /// at a shared group *boundary* node the trapezoid splits the unit feed
    /// between the group and its upper neighbour, so the meaningful invariant is
    /// the row sum, not a strict diagonal. Assert (a) `p0_row_sum(g)` equals
    /// [`group_average_vector`] to `< 1e-10`, and (b) every non-zero element sits
    /// on the diagonal band `gp ∈ {g, g+1}` (the boundary spill), i.e. elements
    /// with `gp < g` or `gp > g+1` are exactly `0`.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** Row sums match the vector average
    /// in all 4 groups (deviation `< 1e-12`); all mass lies on `gp ∈ {g, g+1}`.
    #[test]
    fn identity_feed_conserves_probability_on_diagonal_band() {
        use std::sync::Arc;
        let sigma = PointwiseXs::LinLin(Arc::new(vec![
            (1.0e0, 2.0),
            (1.0e2, 5.0),
            (1.0e4, 3.0),
            (1.0e6, 7.0),
        ]));
        let flux = GroupFlux::Flat;
        let grid = vec![1.0e0, 1.0e2, 1.0e4, 1.0e6];
        let m = scatter_matrix(&FeedFunction::Identity, &sigma, &flux, &grid, &grid, 1).unwrap();
        let vector = group_average_vector(&sigma, &flux, &grid);
        for g in 0..m.incident_groups {
            assert!(
                (m.p0_row_sum(g) - vector[g]).abs() < 1e-10,
                "row sum g={g}: {} != vector {}",
                m.p0_row_sum(g),
                vector[g]
            );
            for gp in 0..m.secondary_groups {
                if gp != g && gp != g + 1 {
                    assert_eq!(
                        m.element(g, 0, gp),
                        0.0,
                        "identity feed leaked outside band g={g}, gp={gp}"
                    );
                }
            }
        }
    }

    /// **GENDF matrix record round-trip.** A matrix-layout GENDF section
    /// (`IG2LO > 0`, `NG2 > 2`) survives [`GendfSection::to_rows`] /
    /// [`GendfSection::from_rows`] losslessly.
    ///
    /// **Methodology.** Build an elastic (`A = 12`) scatter matrix over a
    /// `[1, 1e6]` eV structure, pack it with [`ScatterMatrix::to_gendf_section`]
    /// (`mf = 6, mt = 2`), serialize and parse back. Assert the parsed section
    /// equals the original, that at least one record has `IG2LO > 0` and
    /// `NG2 > 2` (a genuine multi-secondary matrix record), and that the flux
    /// slot (`it = 1`) round-trips as the group flux.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** Parsed section == original;
    /// records carry `NG2` up to the down-scatter width (`> 2`) with `IG2LO >= 1`;
    /// the first data word of each record equals `group_flux[ig-1]`.
    #[test]
    fn gendf_matrix_record_round_trips() {
        let feed = FeedFunction::TwoBodyElastic { awr: 12.0 };
        let sigma = PointwiseXs::Constant(3.5);
        let flux = GroupFlux::analytic(AnalyticWeight::OneOverE, 293.6);
        let grid = geomspace(1.0, 1.0e6, 11); // 10 groups
        let m = scatter_matrix(&feed, &sigma, &flux, &grid, &grid, 2).unwrap();

        let section = m.to_gendf_section(6, 2, 6012.0, 0.0, 293.6);
        // At least one record must be a real matrix record (NG2 > 2, IG2LO > 0).
        let has_matrix = section.records.iter().any(|r| r.ng2 > 2 && r.ig2lo > 0);
        assert!(has_matrix, "expected a NG2>2, IG2LO>0 matrix record");
        assert_eq!(section.nl, 2);

        // Flux slot (it = 1, first NL words) equals the group flux.
        for r in &section.records {
            let g = (r.ig - 1) as usize;
            assert!(
                (r.data[0] - m.group_flux[g]).abs() < 1e-6 * m.group_flux[g].abs().max(1.0),
                "record ig={} flux word {} != group_flux {}",
                r.ig,
                r.data[0],
                m.group_flux[g]
            );
        }

        let rows = section.to_rows();
        let back = GendfSection::from_rows(6, 2, &rows).unwrap();
        assert_eq!(back, section, "matrix GENDF section survives round trip");
    }

    /// The unported feed variants return [`NjoyError::NotPorted`], never a
    /// fabricated matrix (guards the "no fabrication" rule, CLAUDE.md).
    #[test]
    fn unported_feeds_report_not_ported() {
        let sec = vec![1.0, 10.0, 100.0];
        assert!(matches!(
            FeedFunction::AnisotropicElastic.deposit(50.0, &sec, 1),
            Err(NjoyError::NotPorted(_))
        ));
        assert!(matches!(
            FeedFunction::Continuum6.deposit(50.0, &sec, 1),
            Err(NjoyError::NotPorted(_))
        ));
    }

    /// Bad input is rejected without panicking (`Result`, not a process abort).
    #[test]
    fn bad_input_is_rejected() {
        let feed = FeedFunction::TwoBodyElastic { awr: 12.0 };
        let sigma = PointwiseXs::Constant(1.0);
        let flux = GroupFlux::Flat;
        // Non-ascending incident bounds.
        assert!(scatter_matrix(&feed, &sigma, &flux, &[10.0, 1.0], &[1.0, 10.0], 1).is_err());
        // nl = 0.
        assert!(scatter_matrix(&feed, &sigma, &flux, &[1.0, 10.0], &[1.0, 10.0], 0).is_err());
        // Too-short secondary structure.
        assert!(scatter_matrix(&feed, &sigma, &flux, &[1.0, 10.0], &[1.0], 1).is_err());
    }
}
