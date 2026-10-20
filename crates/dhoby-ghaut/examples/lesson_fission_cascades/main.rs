//! **The data of the Fission products chapter "A fragment's story"**
//! (lessons 4-9, maintainer design 2026-10-20): every number those pages in
//! `docs/site/science/fission-products/` take from boon-lay, written to
//! `cascade-data.js` there.
//!
//! ```bash
//! RUSTUP_TOOLCHAIN=1.95 cargo run --release --ignore-rust-version -p dhoby-ghaut --example lesson_fission_cascades [out_dir]
//! ```
//!
//! (`--ignore-rust-version`: dhoby-ghaut depends on kovan, whose `kstring`
//! declares rustc 1.96, as for `kovan-cli`.) It lives here, not beside
//! lessons 1-3's generator in outram-mc-libs, because it calls boon-lay and
//! outram-mc-libs must not depend on boon-lay; dhoby-ghaut already does.
//!
//! - **Decay chains** of the two representative fragments, Sr-95 (light) and
//!   Xe-139 (heavy), the pair boon-lay's `examples/rustbca_cascades.rs` uses:
//!   the main path through boon-lay's `DecayLibrary`, and sampled histories
//!   from its decay Monte Carlo (`boonlay.rs`).
//! - **Kicks**: for every decay on those paths, Q from NuDat mass excesses and
//!   the largest daughter recoil a beta, a gamma, a delayed neutron or an
//!   alpha can give (`physics.rs`), with the interval the masses' 3-figure
//!   rounding allows.
//! - **The fragments' birth energies** from U-235 MF=1/MT=458 `EFR`, split by
//!   momentum, their speeds, and what the largest beta kick could do to one
//!   in flight.
//! - **Displacement thresholds** from NJOY2016 `heatr.f90`'s default table, as
//!   ported in `njoy_outram_park_fork::heatr::default_displacement_energy`.
//! - **Diffusion**: TRISO-ATOPS's NP-MHTGR correlations through boon-lay
//!   (`diffusion_coefficient`, `diffusion_coefficient_sic_ag`) on a
//!   temperature grid, with where each is clamped; and the half-lives the
//!   "decay or move?" page races them against; the HTR-10 kernel radius from
//!   `outram_mc_libs::pebble_beds::fhr_pebble::TrisoSpec::HTR10_LI2014`.
//!
//! **Not here yet: the RustBCA fragment tracks** (pages A and B). boon-lay's
//! RustBCA copy takes a `rand` ChaCha8Rng that no crate outside boon-lay can
//! construct; the maintainer chose to move it to petir's LCG (gh:#979, after
//! the crates.io publish). `rustbca_tracks.rs` in this folder is written for
//! that API and is deliberately NOT declared as a module here, so it is not
//! compiled; once #979 lands, add `mod rustbca_tracks;` and call
//! `rustbca_tracks::write_tracks` from `main`.
//!
//! **Predictions, written 2026-10-20 before the first run:**
//!
//! - Every decay on both main paths is beta-minus; the largest beta recoil
//!   is tens of eV (Q of a few MeV on a daughter of mass ~100 u), the largest
//!   gamma recoil a few to tens of eV, both far below the ~100 MeV of the
//!   flight and of the same order as a displacement threshold.
//! - The fragments' first half-lives are seconds or longer, so the chance of
//!   a decay in one picosecond of flight is below 1e-11.
//! - A delayed-neutron recoil is up to tens of keV; an actinide alpha recoil
//!   about 0.1 MeV.
//! - The largest beta kick turns a fragment in flight by about a milliradian
//!   and changes its energy by well under 1 %.
//!
//! **Results, 2026-10-20 (first run; predictions above unedited):**
//!
//! - Every decay on both main paths is beta-minus, as predicted:
//!   Sr-95 -> Y-95 -> Zr-95 -> Nb-95 -> Mo-95 and Xe-139 -> Cs-139 ->
//!   Ba-139 -> La-139. **The beta recoil prediction was too low**: the
//!   largest beta recoils are 11-246 eV (245.7 eV for Sr-95, Q 6.1 MeV
//!   [6.0, 6.2] from the 3-figure masses), up to hundreds of eV rather than
//!   tens, because the first decays of the chains have Q of 4-6 MeV. The
//!   gamma upper bounds (a gamma of energy Q) are 5.7-210 eV.
//! - First half-lives 23.9 s (Sr-95) and 39.7 s (Xe-139); the chance of a
//!   decay in 1 ps of flight is 2.9e-14 and 1.7e-14, inside the prediction.
//! - Delayed-neutron recoil (I-137 and Br-87, beta-,n): at most 15.0 and
//!   15.4 keV, inside "up to tens of keV". Alpha recoils: Pu-239 86.7 keV
//!   [85.0, 88.5], U-235 79.7 keV [77.9, 81.5], as predicted (~0.1 MeV).
//! - In flight, the largest beta kick turns the fragment by at most 1.6 and
//!   1.3 mrad and changes its energy by at most 319 and 185 keV (0.3 % of
//!   100.5 and 68.7 MeV), as predicted.
//! - Diffusion: the clamps found from the function's output are the ones
//!   its doc lists (Cs and Sr kernel 700 C; Cs graphite 550 C, Sr graphite
//!   800 C, Ag graphite 490 C, at the 20 C grid points at or below them);
//!   Kr, Xe, Ag kernel and Ag in SiC are never flat.
//!
//! The page text and `check-fission.js` (which recomputes the recoils
//! independently) carry the same numbers.
//!
//! Driver glue (`main`, `js_*`, `write`): formats what `physics.rs` and
//! `boonlay.rs` compute; its output is checked by `check-fission.js`.
//! Research, education and V&V only (`RESPONSIBLE_USE.md`).

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
#[path = "boonlay.rs"]
mod boonlay;
#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
#[path = "physics.rs"]
mod physics;

#[cfg(any(target_arch = "wasm32", target_os = "android"))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn main() -> Result<(), String> {
    native::run()
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
mod native {
    use super::boonlay::*;
    use super::physics::*;
    use boon_lay::prelude::decay_library::DecayLibrary;
    use boon_lay::Nuclide;
    use std::collections::BTreeMap;
    use std::fmt::Write as _;

    fn arr(v: impl IntoIterator<Item = f64>) -> String {
        let s: Vec<String> = v.into_iter().map(|x| if x.is_finite() { format!("{x:.6e}") } else { "null".to_string() }).collect();
        format!("[{}]", s.join(","))
    }
    fn span(s: Span) -> String {
        arr([s.0, s.1, s.2])
    }
    fn delta(m: &BTreeMap<(u32, u32), f64>, z: u32, a: u32) -> Result<f64, String> {
        m.get(&(z, a)).copied().ok_or(format!("no NuDat mass excess for Z={z} A={a}"))
    }
    /// A span of recoils for a span of Q (the recoil grows with Q).
    fn map_span(q: Span, f: impl Fn(f64) -> f64) -> Span {
        (f(q.0), f(q.1.max(0.0)), f(q.2))
    }

    pub fn run() -> Result<(), String> {
        let out_dir = std::env::args().nth(1).unwrap_or_else(|| {
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/site/science/fission-products").to_string()
        });
        let src = fission_yields_data_src()?;
        let masses = mass_excess_table(&std::fs::read_to_string(src.join("nndc_nudat_z_n_and_mass_excess.csv")).map_err(|e| e.to_string())?);
        println!("NuDat masses from {}: {} entries", src.display(), masses.len());
        let dn = delta(&masses, 0, 1)?;
        let dhe = delta(&masses, 2, 4)?;
        let m_n = atomic_mass_kev(1, dn);
        let m_he = atomic_mass_kev(4, dhe);

        // The fragments: U-235 MT=458 EFR split between A = 95 and A = 139.
        let tape = concat!(env!("CARGO_MANIFEST_DIR"), "/../../reference-data/endf/n-092_U_235-ENDF8.0.endf");
        let text = std::fs::read_to_string(tape).map_err(|e| format!("{tape}: {e}"))?;
        let (efr, enp) = mt458_efr_enp(&text, 9228).ok_or("no MT=458 in the U-235 tape")?;
        let (fl, fh) = ((38u32, 95u32, Nuclide::Sr95), (54u32, 139u32, Nuclide::Xe139));
        let (el, eh) = fragment_energies_ev(efr, fl.1 as f64, fh.1 as f64);
        println!("U-235 MT=458: EFR {:.4} MeV, ENP {:.4} MeV; light {:.3} MeV, heavy {:.3} MeV", efr / 1e6, enp / 1e6, el / 1e6, eh / 1e6);

        let mut lib = DecayLibrary::new();
        let mut js = String::new();
        writeln!(js, "// The data of the Fission products chapter \"A fragment's story\" (lessons 4-9).").unwrap();
        writeln!(js, "// GENERATED by crates/dhoby-ghaut/examples/lesson_fission_cascades/main.rs:").unwrap();
        writeln!(js, "//   RUSTUP_TOOLCHAIN=1.95 cargo run --release --ignore-rust-version -p dhoby-ghaut --example lesson_fission_cascades").unwrap();
        writeln!(js, "// Do not edit; checked by check-fission.js. Sources: boon-lay's DecayLibrary (ENDF/B-VIII.0").unwrap();
        writeln!(js, "// depletion chain), its decay Monte Carlo and its TRISO-ATOPS diffusion correlations; NuDat mass").unwrap();
        writeln!(js, "// excesses (fission-yields-data 0.1.5, rounded there to 3 significant figures: every Q and recoil").unwrap();
        writeln!(js, "// is [central, low, high] over that rounding); U-235 MF=1/MT=458 (reference-data/endf); NJOY2016").unwrap();
        writeln!(js, "// heatr.f90's default displacement thresholds (njoy-outram-park-fork). Energies in eV unless named.").unwrap();
        writeln!(js, "window.CASCADE_DATA = {{").unwrap();

        // Fragments.
        writeln!(js, "  fragments: {{ efr: {efr:.6e}, enp: {enp:.6e}, pair: [").unwrap();
        for (label, (z, a, _), e) in [("light", fl, el), ("heavy", fh, eh)] {
            let m = atomic_mass_kev(a, delta(&masses, z, a)?);
            let t = e / 1e3;
            writeln!(js, "    {{ role: \"{label}\", z: {z}, a: {a}, e: {e:.6e}, mKeV: {m:.6e}, v: {:.6e}, pcKeV: {:.6e} }},", speed_m_s(t, m), pc_kev(t, m)).unwrap();
        }
        writeln!(js, "  ] }},").unwrap();

        // Displacement thresholds (NJOY's defaults) for the fuel and TRISO elements.
        write!(js, "  ed: [").unwrap();
        for (sym, z) in [("U", 92u32), ("O", 8), ("C", 6), ("Si", 14)] {
            write!(js, "[\"{sym}\", {z}, {:.1}],", njoy_outram_park_fork::heatr::default_displacement_energy(z)).unwrap();
        }
        writeln!(js, "],").unwrap();

        // Decay chains and the kicks along them.
        writeln!(js, "  chains: [").unwrap();
        for (z, a, n) in [fl, fh] {
            let path = main_path(&lib, n, 12);
            println!("{}: {} decays on the main path", pretty_name(&format!("{n:?}")), path.len());
            writeln!(js, "    {{ start: \"{}\", z: {z}, a: {a}, steps: [", pretty_name(&format!("{n:?}"))).unwrap();
            for s in &path {
                let (dp, dd) = (delta(&masses, s.z, s.a)?, delta(&masses, s.daughter_z, s.daughter_a)?);
                let md = atomic_mass_kev(s.daughter_a, dd);
                let q = q_from_excesses(&[(1.0, dp), (-1.0, dd)]);
                let rb = map_span(q, |x| beta_recoil_max_kev(x, md) * 1e3);
                let rg = map_span(q, |x| gamma_recoil_kev(x, md) * 1e3);
                println!(
                    "  {} -> {} ({}, br {:.4}): t1/2 {:.4e} s, Q {:.0} keV [{:.0}, {:.0}], beta recoil <= {:.1} eV, gamma recoil <= {:.2} eV",
                    pretty_name(&s.parent), pretty_name(&s.daughter), s.mode, s.branching, s.half_life_s, q.0, q.1, q.2, rb.0, rg.0
                );
                writeln!(
                    js,
                    "      {{ parent: \"{}\", z: {}, a: {}, t12: {:.6e}, heat: {:.6e}, mode: \"{}\", br: {:.6}, daughter: \"{}\", dz: {}, da: {}, qKeV: {}, betaRecoilMax: {}, gammaRecoilMax: {} }},",
                    pretty_name(&s.parent), s.z, s.a, s.half_life_s, s.decay_energy_ev, mode_label(&s.mode), s.branching,
                    pretty_name(&s.daughter), s.daughter_z, s.daughter_a, span(q), span(rb), span(rg)
                )
                .unwrap();
            }
            writeln!(js, "    ] }},").unwrap();
        }
        writeln!(js, "  ],").unwrap();

        // In flight: the largest beta kick (first decay of each fragment).
        writeln!(js, "  inflight: [").unwrap();
        for (z, a, n) in [fl, fh] {
            let e = if a == fl.1 { el } else { eh };
            let m = atomic_mass_kev(a, delta(&masses, z, a)?);
            let s = main_path(&lib, n, 1).into_iter().next().ok_or("fragment is stable")?;
            let q = q_from_excesses(&[(1.0, delta(&masses, s.z, s.a)?), (-1.0, delta(&masses, s.daughter_z, s.daughter_a)?)]);
            let dp = pc_kev(q.2, ME_KEV);
            let (ang, de) = in_flight_kick(e / 1e3, m, dp);
            let p_ps = 1.0 - (-(2f64.ln()) * 1e-12 / s.half_life_s).exp();
            println!("  in flight, {}: kick <= {:.0} keV/c, turn <= {:.3e} rad, dE <= {:.1} keV of {:.1} MeV; P(decay in 1 ps) {:.3e}", pretty_name(&s.parent), dp, ang, de, e / 1e6, p_ps);
            writeln!(js, "    {{ name: \"{}\", t12: {:.6e}, kickPcKeV: {dp:.6e}, angleRad: {ang:.6e}, dEKeV: {de:.6e}, pPerPs: {p_ps:.6e} }},", pretty_name(&s.parent), s.half_life_s).unwrap();
        }
        writeln!(js, "  ],").unwrap();

        // A delayed-neutron precursor and two actinide alpha emitters.
        let mut kicks = Vec::new();
        for n in [Nuclide::I137, Nuclide::Br87] {
            if let Some((br, d)) = branch_of(&lib, n, "BetaMinusAndNeutron") {
                let ((z, a), (dz, da)) = (n.get_z_a(), d.get_z_a());
                let md = atomic_mass_kev(da, delta(&masses, dz, da)?);
                let q = q_from_excesses(&[(1.0, delta(&masses, z, a)?), (-1.0, delta(&masses, dz, da)?), (-1.0, dn)]);
                let r = map_span(q, |x| two_body_recoil_kev(x, m_n, md) * 1e3);
                println!("  {} beta-,n (br {br:.4}) -> {}: Q_bn {:.0} keV [{:.0}, {:.0}], recoil <= {:.0} eV", pretty_name(&format!("{n:?}")), pretty_name(&format!("{d:?}")), q.0, q.1, q.2, r.0);
                kicks.push(format!(
                    "    {{ kind: \"neutron\", parent: \"{}\", daughter: \"{}\", br: {br:.6}, t12: {:.6e}, qKeV: {}, recoil: {} }},",
                    pretty_name(&format!("{n:?}")), pretty_name(&format!("{d:?}")), half_life_s(&lib, n).unwrap_or(f64::NAN), span(q), span(r)
                ));
            }
        }
        for n in [Nuclide::Pu239, Nuclide::U235] {
            if let Some((br, d)) = branch_of(&lib, n, "Alpha") {
                let ((z, a), (dz, da)) = (n.get_z_a(), d.get_z_a());
                let md = atomic_mass_kev(da, delta(&masses, dz, da)?);
                let q = q_from_excesses(&[(1.0, delta(&masses, z, a)?), (-1.0, delta(&masses, dz, da)?), (-1.0, dhe)]);
                let r = map_span(q, |x| two_body_recoil_kev(x, m_he, md) * 1e3);
                let ea = map_span(q, |x| two_body_recoil_kev(x, md, m_he) * 1e3);
                println!("  {} alpha (br {br:.4}) -> {}: Q {:.0} keV [{:.0}, {:.0}], recoil {:.1} keV, alpha {:.0} keV", pretty_name(&format!("{n:?}")), pretty_name(&format!("{d:?}")), q.0, q.1, q.2, r.0 / 1e3, ea.0 / 1e3);
                kicks.push(format!(
                    "    {{ kind: \"alpha\", parent: \"{}\", daughter: \"{}\", br: {br:.6}, t12: {:.6e}, qKeV: {}, recoil: {}, eAlpha: {} }},",
                    pretty_name(&format!("{n:?}")), pretty_name(&format!("{d:?}")), half_life_s(&lib, n).unwrap_or(f64::NAN), span(q), span(r), span(ea)
                ));
            }
        }
        writeln!(js, "  kicks: [\n{}\n  ],", kicks.join("\n")).unwrap();

        // Sampled histories for the page C demo (12 each, seeded).
        writeln!(js, "  histories: {{").unwrap();
        for (_, _, n) in [fl, fh] {
            let hs = sample_histories(&mut lib, n, 12, 979);
            let rows: Vec<String> = hs
                .iter()
                .map(|h| format!("[{}]", h.iter().map(|(name, t)| format!("[\"{}\", {}]", pretty_name(name), if t.is_finite() { format!("{t:.6e}") } else { "null".into() })).collect::<Vec<_>>().join(",")))
                .collect();
            writeln!(js, "    \"{}\": [\n      {}\n    ],", pretty_name(&format!("{n:?}")), rows.join(",\n      ")).unwrap();
        }
        writeln!(js, "  }},").unwrap();

        // Diffusion correlations and the half-lives they race.
        let t_c: Vec<f64> = (0..=80).map(|i| 400.0 + 20.0 * i as f64).collect();
        let series = diffusion_series(&t_c);
        writeln!(js, "  diffusion: {{ tC: {}, series: [", arr(t_c.iter().copied())).unwrap();
        for s in &series {
            println!("  {}: D(1000 C) {:.3e} m2/s, flat (clamped) up to {:?} C", s.label, s.d[30], s.flat_up_to_c);
            writeln!(js, "    {{ label: \"{}\", z: {}, material: \"{}\", flatUpToC: {}, d: {} }},", s.label, s.z, s.material, s.flat_up_to_c.map_or("null".into(), |v| format!("{v}")), arr(s.d.iter().copied())).unwrap();
        }
        writeln!(js, "  ] }},").unwrap();
        write!(js, "  race: [").unwrap();
        for n in [Nuclide::Kr85, Nuclide::Xe133, Nuclide::Xe135, Nuclide::Sr90, Nuclide::Cs137, Nuclide::Ag110m] {
            if let Some(t) = half_life_s(&lib, n) {
                write!(js, "{{ name: \"{}\", z: {}, t12: {t:.6e} }},", pretty_name(&format!("{n:?}")), n.get_z_a().0).unwrap();
            } else {
                println!("  {:?}: no half-life in the decay library, left out", n);
            }
        }
        writeln!(js, "],").unwrap();
        let kernel_m = outram_mc_libs::pebble_beds::fhr_pebble::TrisoSpec::HTR10_LI2014.kernel / 100.0;
        writeln!(js, "  kernelRadiusM: {kernel_m:.6e}").unwrap();
        writeln!(js, "}};").unwrap();

        let path = std::path::Path::new(&out_dir).join("cascade-data.js");
        std::fs::write(&path, js).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("wrote {}", path.display());
        Ok(())
    }
}
