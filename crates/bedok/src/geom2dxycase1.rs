//! A 2-D x-y test case: a UO2 square encased in moderator.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `geom2dxycase1.m`, `main_exec_diff3d_standalone` snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.
//!
//! # NOTHING IN THE SNAPSHOT CAN RUN THIS CASE
//!
//! Read this before reaching for it. `geom2dxycase1.m` builds a **two
//! dimensional** problem:
//!
//! - the geometry has `Lx` and `Ly` but **no `Lz`**, and `Vi` is an *area*;
//! - the boundaries are named `left`, `right`, `top`, `bottom`, not
//!   `xmin`/`xmax`/`ymin`/`ymax`/`zmin`/`zmax`;
//! - `whichsigma` is `maxix` by `maxiy`, with no third index.
//!
//! Every solver in the snapshot is 3-D and takes the `xyz` boundary names, so
//! none of them can consume this. `main_exec_diff3d.m` confirms it: the call is
//! there, **commented out**, alongside the live 3-D cases. It is a legacy case
//! for a 2-D solver that was not shipped.
//!
//! It is translated because it is part of the snapshot and its data is worth
//! keeping, not because it can be run. It returns its own [`Case2d`] rather
//! than the crate's 3-D [`crate::types::Geometry`], because forcing it into a
//! 3-D type would mean inventing an axial dimension the reference does not
//! have — an interpretation, not a translation.
//!
//! # The problem
//!
//! A 8 x 8 cm UO2 square centred in a 24 x 24 cm moderator block, vacuum on all
//! four sides. One energy group, two materials, and both share
//! `Sigma_tot = 5` and `Sigma_s = 0.9 * Sigma_tot`; only the fuel fissions,
//! with `Sigma_f = 0.05 * Sigma_tot` and `nu = 1`.
//!
//! # Its own quoted result
//!
//! The file's header records:
//!
//! > `k_eff = 0.487` at `Lux = Luy = Lpx = Lpy = 8`
//!
//! which is the configuration built here. [`REFERENCE_K_EFF`] carries it. **It
//! has not been reproduced** — there is no solver to reproduce it with — and it
//! is quoted from a comment, not from a publication. It is recorded so that
//! whoever writes a 2-D solver has a target waiting.

/// The `k_eff` `geom2dxycase1.m`'s header quotes for this configuration.
///
/// **Not reproduced**: nothing in the snapshot can solve a 2-D case. Quoted
/// from the file's own comment.
pub const REFERENCE_K_EFF: f64 = 0.487;

/// The fuel square's half-extent and the moderator margin, cm.
///
/// The reference sets `Lux = Luy = Lpx = Lpy = 8`, so the fuel is 8 cm across
/// and sits in an 8 cm margin on every side.
pub const L: f64 = 8.0;

/// A 2-D case, as the reference builds it.
///
/// Deliberately **not** [`crate::types::Geometry`]; see the module docs.
#[derive(Clone, Debug, PartialEq)]
pub struct Case2d {
    /// `geometry.Xtot` — `Lux + 2*Lpx`, cm.
    pub xtot: f64,
    /// `geometry.Ytot` — `Luy + 2*Lpy`, cm.
    pub ytot: f64,
    /// `geometry.Lx` — cell width in `x`, one per cell.
    pub lx: Vec<f64>,
    /// `geometry.Ly` — cell width in `y`, one per cell.
    pub ly: Vec<f64>,
    /// `geometry.Vi` — the **area** of each cell, cm². Not a volume.
    pub vi: Vec<f64>,
    /// `geometry.Ctr` — each cell's centre `(x, y)`, cm.
    pub ctr: Vec<(f64, f64)>,
    /// `whichsigma(ix, iy)` — **1** for fuel, **2** for moderator.
    ///
    /// Note this case has no void: every cell carries a material, so unlike the
    /// 3-D cases `0` never appears.
    pub whichsigma: Vec<Vec<usize>>,
    /// `sigmavalues.tot` per material, cm⁻¹.
    pub tot: [f64; 2],
    /// `sigmavalues.f` per material, cm⁻¹. Only the fuel fissions.
    pub f: [f64; 2],
    /// `sigmavalues.s(m, 1, 1)` — within-group scattering per material, cm⁻¹.
    pub s: [f64; 2],
    /// `constants.nu` per material.
    pub nu: [f64; 2],
    /// `constants.chi` per material.
    ///
    /// **Both entries are 1**, not a normalised spectrum — with one energy
    /// group there is nowhere else for a fission neutron to go.
    pub chi: [f64; 2],
}

/// `[params, geometry, constants, whichsigma, sigmavalues] = geom2dxycase1(params)`.
///
/// Builds the 2-D case on a `maxix` by `maxiy` mesh. All four boundaries are
/// vacuum, so they are not carried on [`Case2d`] — there is nothing to choose.
///
/// # Panics
///
/// If `maxix` or `maxiy` is zero.
pub fn geom2dxycase1(maxix: usize, maxiy: usize) -> Case2d {
    assert!(maxix > 0 && maxiy > 0, "the mesh must have at least one cell per axis");

    let xtot = L + 2.0 * L;
    let ytot = L + 2.0 * L;
    let sx = xtot / maxix as f64;
    let sy = ytot / maxiy as f64;
    let n = maxix * maxiy;

    let mut ctr = Vec::with_capacity(n);
    for ix in 0..maxix {
        for iy in 0..maxiy {
            ctr.push(((ix as f64 + 0.5) * sx, (iy as f64 + 0.5) * sy));
        }
    }

    // Fuel where the cell centre lies inside the central square, moderator
    // elsewhere. The reference writes this as a `continue` on the fuel branch,
    // leaving the preallocated `1`; the sense is the same.
    let mut whichsigma = vec![vec![2usize; maxiy]; maxix];
    for (ix, row) in whichsigma.iter_mut().enumerate() {
        for (iy, cell) in row.iter_mut().enumerate() {
            let cx = (ix as f64 + 0.5) * sx;
            let cy = (iy as f64 + 0.5) * sy;
            if (L..=L + L).contains(&cx) && (L..=L + L).contains(&cy) {
                *cell = 1;
            }
        }
    }

    let usigmat = 5.0;
    let psigmat = 5.0;

    Case2d {
        xtot,
        ytot,
        lx: vec![sx; n],
        ly: vec![sy; n],
        vi: vec![sx * sy; n],
        ctr,
        whichsigma,
        tot: [usigmat, psigmat],
        f: [0.05 * usigmat, 0.0],
        s: [0.9 * usigmat, 0.9 * psigmat],
        nu: [1.0, 1.0],
        chi: [1.0, 1.0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The geometry and cross sections match the reference's own arithmetic.
    ///
    /// # Methodology
    ///
    /// Everything here is a closed form, so it is checked against the formulas
    /// rather than against a solve: the domain is `3L` on a side, the cells
    /// tile it exactly, cell areas sum to the total area, and the two materials
    /// differ only in that the moderator does not fission.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// 24 x 24 cm domain on 1 x 1 cm cells; areas sum to 576 cm2
    /// exactly. `Sigma_tot = 5`, `Sigma_s = 4.5`, so **absorption is
    /// 0.5 /cm in both materials**, and only the fuel fissions at
    /// `Sigma_f = 0.25`.
    ///
    /// **Interpretation.** Fuel and moderator are distinguished *only*
    /// by fission here — same total, same scattering, same absorption.
    /// That makes this a clean test of the fission source and the
    /// geometry rather than of material contrast.
    #[test]
    fn the_geometry_and_cross_sections_match_the_reference() {
        let c = geom2dxycase1(24, 24);

        eprintln!("domain      = {} x {} cm", c.xtot, c.ytot);
        eprintln!("cell        = {} x {} cm", c.lx[0], c.ly[0]);
        eprintln!("cell area   = {} cm2", c.vi[0]);
        assert_eq!(c.xtot, 24.0);
        assert_eq!(c.ytot, 24.0);
        assert_eq!(c.lx[0], 1.0);
        assert_eq!(c.vi[0], 1.0);

        // The areas tile the domain exactly.
        let total: f64 = c.vi.iter().sum();
        eprintln!("total area  = {total} cm2 (expect {})", c.xtot * c.ytot);
        assert!((total - c.xtot * c.ytot).abs() < 1e-9);

        // Cross sections.
        assert_eq!(c.tot, [5.0, 5.0]);
        assert_eq!(c.f, [0.25, 0.0], "only the fuel fissions");
        assert_eq!(c.s, [4.5, 4.5]);
        assert_eq!(c.nu, [1.0, 1.0]);
        // Both chi entries are 1 — see the field docs.
        assert_eq!(c.chi, [1.0, 1.0]);

        // Absorption is total less scattering: 5 - 4.5 = 0.5 in both.
        for m in 0..2 {
            let absorption = c.tot[m] - c.s[m];
            assert!((absorption - 0.5).abs() < 1e-12, "material {}", m + 1);
        }
        eprintln!("absorption  = 0.5 /cm in both materials");
    }

    /// The fuel square is centred and occupies the middle ninth of the domain.
    ///
    /// # Methodology
    ///
    /// The fuel is `L` across in a `3L` domain, so on a mesh that resolves it
    /// exactly the fuel should be one ninth of the cells, centred. This checks
    /// the material map directly rather than the formula that built it, and
    /// confirms there is **no void** — unlike every 3-D case, `0` never appears.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// **64 fuel cells of 576 (11.1%)**, exactly one ninth, spanning
    /// rows 8 to 15 of 24 — centred. No void anywhere.
    ///
    /// **Interpretation.** The centre-in-square test reproduces the
    /// intended geometry exactly on a mesh that resolves it. Note the
    /// absence of void: every 3-D case in this crate uses `0` for
    /// outside-the-core, and a reader carrying that assumption here
    /// would be wrong.
    #[test]
    fn the_fuel_square_is_centred_and_one_ninth_of_the_domain() {
        let c = geom2dxycase1(24, 24);

        let fuel: usize = c.whichsigma.iter().flatten().filter(|m| **m == 1).count();
        let total = 24 * 24;
        eprintln!("fuel cells  = {fuel} of {total} ({:.1}%)", fuel as f64 / total as f64 * 100.0);
        eprintln!("expected    = {} (one ninth)", total / 9);
        assert_eq!(fuel, total / 9);

        // No void anywhere.
        assert!(
            c.whichsigma.iter().flatten().all(|m| *m == 1 || *m == 2),
            "a 2-D case has no void material"
        );

        // Centred: the fuel occupies the middle third on each axis.
        let rows_with_fuel: Vec<usize> = (0..24)
            .filter(|ix| c.whichsigma[*ix].contains(&1))
            .collect();
        eprintln!("fuel rows   = {}..={}", rows_with_fuel[0], rows_with_fuel[rows_with_fuel.len() - 1]);
        assert_eq!(rows_with_fuel.len(), 8);
        assert_eq!(rows_with_fuel[0], 8);
        assert_eq!(rows_with_fuel[7], 15);
    }

    /// The quoted `k_eff` is recorded but **not** reproduced.
    ///
    /// # Methodology
    ///
    /// This asserts nothing about physics — there is no 2-D solver to run. It
    /// exists so the target is discoverable from the test suite rather than
    /// only from a comment, and so the constant cannot drift from the module
    /// docs unnoticed.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// `k_eff = 0.487` recorded; **not** reproduced.
    ///
    /// **Interpretation.** Deeply subcritical, which is right for an
    /// 8 cm fuel square with `nu = 1` bounded by vacuum. It stays a
    /// quoted number until someone writes a 2-D solver.
    #[test]
    fn the_quoted_eigenvalue_is_recorded_for_a_future_two_dimensional_solver() {
        eprintln!("geom2dxycase1.m header quotes k_eff = {REFERENCE_K_EFF}");
        eprintln!("at Lux = Luy = Lpx = Lpy = {L}");
        eprintln!("NOT reproduced: no solver in this crate accepts a 2-D case.");

        // The quote is only meaningful for the configuration it was taken at,
        // so check the builder actually produces that configuration rather
        // than asserting the constant equals itself.
        let c = geom2dxycase1(24, 24);
        assert_eq!(c.xtot, 3.0 * L, "the quote assumes a 3L domain");
        assert_eq!(c.ytot, 3.0 * L);
        let fuel = c.whichsigma.iter().flatten().filter(|m| **m == 1).count();
        assert_eq!(fuel * 9, 24 * 24, "and an 8 cm fuel square within it");

        // An infinite medium of this fuel would have
        // k_inf = nu*Sigma_f / Sigma_a = 0.25 / 0.5 = 0.5, so a finite,
        // vacuum-bounded block must come out below that. The quoted 0.487 does.
        let k_inf = c.f[0] / (c.tot[0] - c.s[0]);
        eprintln!("k_inf of the fuel alone = {k_inf}");
        assert!(
            REFERENCE_K_EFF < k_inf,
            "a leaking finite system cannot exceed k_inf = {k_inf}"
        );
    }
}
