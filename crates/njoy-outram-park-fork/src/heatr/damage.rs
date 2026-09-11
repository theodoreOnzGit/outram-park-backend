//! MT=444 damage-energy production (phase H7): the Lindhard-Robinson partition,
//! NJOY's default displacement-threshold table, and the two-body recoil channels.
//!
//! Split out of `heatr/mod.rs` (see the module doc there for the physics).

use crate::endf::MtReaction;
use crate::reconr::{eval_lin_lin, ReconrResult};

// ── H7: damage-energy production (MT=444) ───────────────────────────────────

/// Default atomic displacement threshold energy `E_d` \[eV\] for element `z`
/// (proton number), NJOY `heatr.f90`'s built-in table (used when the user does
/// not override it). Recoils below `E_d` produce no lattice damage. Values are
/// the standard ASTM/NJOY defaults: 25 eV for most elements, higher for the
/// refractory / heavy metals. `z=0` (unknown) falls back to 25 eV.
///
/// | Z | element(s) | `E_d` \[eV\] |
/// |---|---|---|
/// | 4, 6 | Be, C | 31 |
/// | 12, 14 | Mg, Si | 25 |
/// | 13 | Al | 27 |
/// | 20, 22–29, 40, 41 | Ca, Ti–Cu, Zr, Nb | 40 |
/// | 42, 47 | Mo, Ag | 60 |
/// | 73, 74 | Ta, W | 90 |
/// | 79 | Au | 30 |
/// | 82 | Pb | 25 |
/// | (all others) | — | 25 |
pub fn default_displacement_energy(z: u32) -> f64 {
    match z {
        4 | 6 => 31.0,
        12 | 14 => 25.0,
        13 => 27.0,
        20 | 22..=29 | 40 | 41 => 40.0,
        42 | 47 => 60.0,
        73 | 74 => 90.0,
        79 => 30.0,
        82 => 25.0,
        _ => 25.0,
    }
}

/// Lindhard-Robinson damage-partition function `df(E_R)` \[eV\]: of a recoiling
/// atom's kinetic energy `e_recoil` \[eV\], the portion that goes into further
/// atomic displacements (damage energy) rather than electronic excitation
/// (which does not displace atoms). Faithful port of `heatr.f90`'s `df`.
///
/// The recoiling atom has proton/mass numbers `(z_r, a_r)` and travels through a
/// lattice of atoms `(z_l, a_l)` (`a_*` are mass ratios, `= AWR`; for a
/// self-recoil — e.g. elastic scattering — recoil and lattice are the same
/// atom). `e_d` is the displacement threshold \[eV\]: recoils below it produce
/// no damage, so `df = 0`. The Robinson analytic fit to Lindhard's partition is
///
/// ```text
///   ε   = E_R / E_L                          (reduced recoil energy)
///   df  = E_R / [ 1 + f_L·(3.4008·ε^{1/6} + 0.40244·ε^{3/4} + ε) ]
/// ```
///
/// with `E_L` and `f_L` the Lindhard screening constants built from the Z's and
/// A's. As `E_R → E_d⁺` the bracket → 1 so `df → E_R` (nearly all recoil energy
/// displaces atoms); as `E_R ≫ E_L` electronic stopping dominates and `df ≪ E_R`.
pub(super) fn lindhard_damage(
    e_recoil: f64,
    z_r: f64,
    a_r: f64,
    z_l: f64,
    a_l: f64,
    e_d: f64,
) -> f64 {
    const C1: f64 = 30.724;
    const C2: f64 = 0.0793;
    const C3: f64 = 3.4008;
    const C4: f64 = 0.40244;
    if z_r == 0.0 || e_recoil < e_d {
        return 0.0;
    }
    let zr23 = z_r.powf(2.0 / 3.0);
    let zl23 = z_l.powf(2.0 / 3.0);
    let e_l = C1 * z_r * z_l * (zr23 + zl23).sqrt() * (a_r + a_l) / a_l;
    let denom = (zr23 + zl23).powf(0.75) * a_r.powf(1.5) * a_l.sqrt();
    let f_l = C2 * zr23 * z_l.sqrt() * (a_r + a_l).powf(1.5) / denom;
    let ep = e_recoil / e_l;
    e_recoil / (1.0 + f_l * (C3 * ep.powf(1.0 / 6.0) + C4 * ep.powf(0.75) + ep))
}

/// Damage-energy production cross section, ENDF **MT=444**, vs incident energy
/// \[eV\], in \[eV·barn\] (damage energy × cross section — same convention as the
/// MT=301 [`Kerma`]).
///
/// Built from a [`ReconrResult`] via [`DamageEnergy::from_reconr`]; evaluated
/// with [`DamageEnergy::eval`]. Covers the **two-body neutron-scattering**
/// recoil channels — **elastic** (MT=2) and **discrete inelastic levels**
/// (MT=51–90), the residual nucleus recoiling against the single emitted
/// neutron. Per incident energy `E`, summed over these reactions,
///
/// ```text
///   σ_444(E) = Σ σ_r(E) · ⟨df(E_R)⟩_r,
/// ```
///
/// each reaction's cross section times its recoil-averaged Lindhard damage
/// energy. For **isotropic** centre-of-mass scattering the recoil energy is
/// *uniform* on `[E_min, E_max]` (it is linear in the CM cosine), with
///
/// ```text
///   E_min = C·(1−g)²,   E_max = C·(1+g)²,   C = A/(A+1)²·E,
///   g = √(1 − E_thr/E),   E_thr = (A+1)/A·|Q|
/// ```
///
/// so `⟨df⟩ = (1/(E_max−E_min))·∫ df(E_R) dE_R` (with `df = 0` below `E_d`).
/// Elastic is the `Q = 0` case: `g = 1`, `E_min = 0`, `E_max = 4A/(A+1)²·E`
/// (backscatter maximum). MF=4 angular anisotropy (which reweights the recoil
/// distribution — `heatr.f90`'s 64-point Gauss-Legendre `disbar`) and the
/// continuum / (n,xn) / capture-recoil channels are the remaining H7 sub-steps.
#[derive(Debug, Clone, Default)]
pub struct DamageEnergy {
    /// Union incident-energy grid \[eV\], ascending.
    pub energy: Vec<f64>,
    /// `Σ σ_r(E)·⟨df⟩_r` \[eV·barn\], aligned with `energy`.
    pub d: Vec<f64>,
}

impl DamageEnergy {
    /// Compute the damage-energy cross section (MT=444) from a reconstructed
    /// evaluation, summed over the elastic (MT=2) and discrete-inelastic
    /// (MT=51–90) two-body recoil channels.
    ///
    /// The target's proton number `z` and displacement threshold `e_d` \[eV\]
    /// (pass [`default_displacement_energy`]`(z)` for the built-in default) set
    /// the Lindhard partition; mass ratio `A` comes from `recon.material.awr`.
    /// Returns an empty table if the material has none of these channels.
    pub fn from_reconr(recon: &ReconrResult, z: u32, e_d: f64) -> Self {
        let awr = recon.material.awr;
        let zf = z as f64;
        let contributes =
            |mt: MtReaction| mt == MtReaction::Mt2Elastic || (51..=90).contains(&mt.number());
        let mut energy: Vec<f64> = recon
            .sections
            .iter()
            .filter(|s| contributes(s.mt))
            .flat_map(|s| s.pairs.iter().map(|&(e, _)| e))
            .collect();
        energy.sort_by(|a, b| a.partial_cmp(b).unwrap());
        energy.dedup_by(|a, b| (*a - *b).abs() < 1.0e-12 * b.abs().max(1.0));

        let mut d = vec![0.0; energy.len()];
        for sec in &recon.sections {
            if !contributes(sec.mt) {
                continue;
            }
            // Elastic has Q=0 by definition; a discrete level uses its own QI.
            let q = if sec.mt == MtReaction::Mt2Elastic {
                0.0
            } else {
                sec.qi
            };
            for (i, &e) in energy.iter().enumerate() {
                let sigma = eval_lin_lin(&sec.pairs, e);
                if sigma == 0.0 {
                    continue;
                }
                let (e_min, e_max) = two_body_recoil_bounds(e, q, awr);
                d[i] += sigma * mean_recoil_damage(e_min, e_max, zf, awr, e_d);
            }
        }
        DamageEnergy { energy, d }
    }

    /// Evaluate the damage-energy cross section \[eV·barn\] at incident energy
    /// `e` \[eV\] (lin-lin interpolated, clamped at the tabulated ends).
    pub fn eval(&self, e: f64) -> f64 {
        if self.energy.is_empty() {
            return 0.0;
        }
        let pairs: Vec<(f64, f64)> = self
            .energy
            .iter()
            .copied()
            .zip(self.d.iter().copied())
            .collect();
        eval_lin_lin(&pairs, e)
    }
}

/// Recoil-energy bounds `[E_min, E_max]` \[eV\] of an isotropic-CM two-body
/// neutron-scattering reaction (elastic or a discrete inelastic level) at
/// incident energy `e` \[eV\] with reaction Q-value `q` \[eV\] (`0` for elastic,
/// negative for a level). The recoil energy is linear in the CM cosine, hence
/// *uniform* on this interval:
///
/// ```text
///   C = A/(A+1)²·E,   g = √(max(0, 1 − E_thr/E)),   E_thr = (A+1)/A·(−Q),
///   E_min = C·(1−g)²,   E_max = C·(1+g)².
/// ```
///
/// For `q = 0` this is `g = 1`, `[0, 4C]` — the elastic backscatter range. Ports
/// the recoil kinematics of `heatr.f90::disbar` (its `afact`, `arat`, `r`, `g`
/// with `awp = 1` for a neutron exit).
pub(super) fn two_body_recoil_bounds(e: f64, q: f64, awr: f64) -> (f64, f64) {
    let c = awr / (awr + 1.0).powi(2) * e;
    let e_thr = (awr + 1.0) / awr * (-q); // 0 for elastic; >0 for a level
    let g = (1.0 - e_thr / e).max(0.0).sqrt();
    (c * (1.0 - g).powi(2), c * (1.0 + g).powi(2))
}

/// Recoil-averaged Lindhard damage energy `⟨df⟩` \[eV\] for a recoil energy
/// *uniform* on `[e_min, e_max]`: the mean of [`lindhard_damage`] over that
/// range. Since `df = 0` below `e_d`, the average is
/// `(1/(e_max−e_min))·∫_{max(e_min,e_d)}^{e_max} df dE_R`, computed by composite
/// Simpson's rule over the smooth region (integrating from `max(e_min, e_d)`
/// avoids the `df` discontinuity at `e_d`). Zero if the range is empty or lies
/// entirely below `e_d`.
pub(super) fn mean_recoil_damage(e_min: f64, e_max: f64, z: f64, awr: f64, e_d: f64) -> f64 {
    let lo = e_min.max(e_d);
    if e_max <= lo {
        return 0.0;
    }
    // Composite Simpson over [lo, e_max]; df is smooth there, so a fixed even
    // panel count is accurate and cheap.
    const N: usize = 400; // even
    let h = (e_max - lo) / N as f64;
    let mut acc =
        lindhard_damage(lo, z, awr, z, awr, e_d) + lindhard_damage(e_max, z, awr, z, awr, e_d);
    for i in 1..N {
        let er = lo + i as f64 * h;
        let w = if i % 2 == 1 { 4.0 } else { 2.0 };
        acc += w * lindhard_damage(er, z, awr, z, awr, e_d);
    }
    let integral = acc * h / 3.0;
    integral / (e_max - e_min)
}
