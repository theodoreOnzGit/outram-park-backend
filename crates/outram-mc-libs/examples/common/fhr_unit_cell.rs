// SPDX-License-Identifier: GPL-3.0

//! **The FHR reference unit cell's material table** (tutorial rung 5, step 5:
//! the double-heterogeneity shortcuts). Shared by `dh_keff_vv.rs` (the record)
//! and the `dhoby-ghaut` Monte Carlo web demo's `dhshort` rung, which pulls
//! this file in with `#[path]`, so the browser draws the cell the record ran.
//! Moved here unchanged from `dh_keff_vv.rs` on 2026-10-07 (GitHub #785).

#![allow(dead_code)]

use outram_mc_libs::material::material::{Material, NuclideComponent};

/// Material / data temperature \[K\] — the FHR deck's operating temperature.
pub const TEMP_K: f64 = 600.0;

/// Li-7 enrichment of the FLiBe, as the reference deck specifies it. Li-6 is a
/// strong absorber, so the residual 50 pcm of it is not negligible bookkeeping.
pub const LI7_PURITY: f64 = 0.99995;

/// Avogadro's number / 1e24, so `rho * N_A_B / M` lands directly in atoms/b-cm.
pub const N_A_B: f64 = 0.602_214_076;

// Nuclide indices into the vector `dh_keff_vv.rs`'s `nuclides()` returns.
pub const U235: usize = 0;
pub const U238: usize = 1;
pub const O16: usize = 2;
pub const C_FREE: usize = 3;
pub const C_GRAPHITE: usize = 4;
pub const SI28: usize = 5;
pub const LI6: usize = 6;
pub const LI7: usize = 7;
pub const BE9: usize = 8;
pub const F19: usize = 9;

/// Representative HALEU UCO TRISO compositions \[atoms/b-cm\] plus FLiBe.
///
/// Illustrative of the material class, **not** a benchmark specification. This
/// example measures the *difference between treatments*, which is insensitive
/// to the exact densities so long as every arm shares them.
///
/// # Carbon bookkeeping
///
/// Two carbon entries, deliberately:
///
/// - **`C_FREE`** — the kernel's oxycarbide carbon and the SiC carbon. Neither
///   sits in a graphite lattice, and the reference deck treats both as free gas.
/// - **`C_GRAPHITE`** — buffer, IPyC, OPyC, fuel-zone matrix and the outer
///   shell, all graphite, all carrying the S(alpha,beta) law.
///
/// C-13 (1.1 % of natural carbon) is neglected, as are Si-29/30. Both apply
/// identically to every arm, so neither can bias the comparison this example
/// exists to make — though they would matter for an absolute k.
pub fn materials() -> Vec<Material> {
    let m = |id: i32, name: &str, comps: Vec<(usize, f64)>| Material {
        id,
        name: name.into(),
        temperature: TEMP_K,
        components: comps
            .into_iter()
            .map(|(nuclide_idx, atom_density)| NuclideComponent {
                nuclide_idx,
                atom_density,
            })
            .collect(),
    };

    // FLiBe = 2LiF.BeF2 = Li2BeF4. Density correlation rho(T) = (2416 -
    // 0.49072*T)/1000 g/cm^3, the same one the reference deck uses.
    let flibe_rho = (2416.0 - 0.49072 * TEMP_K) / 1000.0;
    let (m_li7, m_li6, m_be9, m_f19) = (7.016_003, 6.015_123, 9.012_183, 18.998_403);
    let li7_per_formula = 2.0 * LI7_PURITY;
    let li6_per_formula = 2.0 * (1.0 - LI7_PURITY);
    let m_formula = li7_per_formula * m_li7 + li6_per_formula * m_li6 + m_be9 + 4.0 * m_f19;
    // Formula units per barn-cm; multiply by the per-formula count for each.
    let n_formula = flibe_rho * N_A_B / m_formula;

    vec![
        // 0 kernel — 19.9 % HALEU UCO. Kernel carbon is not graphite.
        m(
            0,
            "UCO kernel",
            vec![
                (U235, 4.40e-3),
                (U238, 1.77e-2),
                (O16, 2.27e-2),
                (C_FREE, 9.10e-3),
            ],
        ),
        // 1 buffer — porous graphite
        m(1, "buffer", vec![(C_GRAPHITE, 5.02e-2)]),
        // 2 IPyC — pyrolytic graphite
        m(2, "IPyC", vec![(C_GRAPHITE, 9.53e-2)]),
        // 3 SiC — carbon here is silicon-bound, not graphite
        m(3, "SiC", vec![(SI28, 4.79e-2), (C_FREE, 4.79e-2)]),
        // 4 OPyC — pyrolytic graphite
        m(4, "OPyC", vec![(C_GRAPHITE, 9.53e-2)]),
        // 5 matrix — graphite binder in the fuel zone
        m(5, "matrix graphite", vec![(C_GRAPHITE, 8.53e-2)]),
        // 6 shell — fuel-free graphite outer shell
        m(6, "shell graphite", vec![(C_GRAPHITE, 8.78e-2)]),
        // 7 coolant — FLiBe, free-gas in both codes
        m(
            7,
            "FLiBe coolant",
            vec![
                (F19, 4.0 * n_formula),
                (LI7, li7_per_formula * n_formula),
                (LI6, li6_per_formula * n_formula),
                (BE9, n_formula),
            ],
        ),
    ]
}
