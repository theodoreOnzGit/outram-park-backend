//! **Does the MF=6 continuum angular law harden Godiva's spectrum?** A paired
//! internal ablation of the *spectral* effect of bead `op-og56`, and a partial
//! probe at bead `op-os8x`.
//!
//! # Why this exists
//!
//! When the continuum angular law landed, its write-ups asserted a side effect
//! and did not measure it:
//!
//! > For a centre-of-mass law (`LCT = 2`, which both uranium isotopes use) the
//! > lab energy is `E' = E_cm + E_trans + 2·μ_cm·√(E_cm·E_trans)`, so a
//! > forward-peaked cosine also **raises** `⟨E'_lab⟩` and hardens the spectrum.
//! > This crate's spectrum is already 0.45 % too hard against OpenMC
//! > (`op-os8x`), so this change moves that residual the **wrong way**.
//!
//! That is a prediction with a sign, and leaving it as prose would be the exact
//! failure this crate's own record keeps warning about — a claim nobody can
//! falsify. This measures it.
//!
//! # What this can and cannot settle
//!
//! `op-os8x` is *defined* as a cross-code comparison: our flux spectrum against
//! OpenMC's on the same evaluation, currently `+0.45 %` in mean `E` (3.5 σ) and
//! `−1.03 %` in the flux fraction below 300 keV. **This example cannot close
//! it**, because it never runs OpenMC — it compares this code against itself
//! with one mechanism removed.
//!
//! What it *can* do is give the **sign and size of the contribution** that one
//! mechanism makes to spectral hardness, on the same integral measures the
//! cross-code study reports. If the continuum angular law hardens the spectrum
//! by an amount comparable to the residual, it is part of `op-os8x`'s story and
//! makes it worse. If it is negligible, `op-os8x` lives elsewhere and this
//! change is exonerated as a contributor. Either answer is worth having, and
//! neither needs OpenMC.
//!
//! # Method
//!
//! Two arms over the **same seeds**, differing only in
//! [`Nuclide::with_isotropic_continuum_scattering`]:
//!
//! - **ANISO** — the evaluated MF=6 angular law (`LANG = 1` here; both uranium
//!   isotopes are Legendre).
//! - **ISO** — the pre-`op-og56` behaviour.
//!
//! Each arm tallies the flux in 50 logarithmic bins over `1e-3 … 2e7 eV` — the
//! same grid `godiva_spectrum_vs_openmc.rs` uses, so the numbers are directly
//! comparable to that study's. Three integral measures are reported, chosen
//! because they are the ones the cross-code study reports:
//!
//! | measure | why |
//! |---|---|
//! | mean `E` | the obvious hardness measure; dominated by the fast tail |
//! | mean `ln E` | hardness weighted per lethargy, far less tail-sensitive |
//! | flux fraction below 300 keV | where a softening shows up first |
//!
//! Reporting mean `E` *and* mean `ln E` matters: they weight the tail very
//! differently, and a change that moves one but not the other is telling you
//! something about *where* in energy it acts. The cross-code study found both
//! moved together (`+0.45 %` and `+0.05 %`), which is why both are here.
//!
//! Per-bin flux is pooled across seeds before the moments are taken, and the
//! spread across seeds gives the uncertainty — a single seed cannot resolve a
//! sub-percent spectral shift.
//!
//! # Prediction, recorded before running
//!
//! **ANISO should be HARDER than ISO**: mean `E` up, mean `ln E` up, flux
//! fraction below 300 keV down. Magnitude unknown, but expected small — the
//! same reasoning that made the reactivity worth small applies here, since
//! U-238's MT=91 angular law is exactly isotropic below 1.2 MeV and only reaches
//! `⟨μ_cm⟩ = +0.27` at 14 MeV.
//!
//! **If ANISO comes back SOFTER, the sign argument above is wrong** and every
//! doc repeating it needs correcting rather than explaining away. The argument
//! has a specific weak point worth naming in advance: it assumes the `⟨μ_cm⟩`
//! shift dominates, but the continuum competes with other channels for
//! collisions and a change in *where* neutrons scatter can move the spectrum
//! independently of the per-collision energy transfer.
//!
//! # Results (2026-09-16, 8 seeds per arm, ENDF/B-VIII.0)
//!
//! | measure | ANISO | ISO | relative change | |
//! |---|---|---|---|---|
//! | mean `E` \[eV\] | 1.47409e6 | 1.47521e6 | **−0.076 % ± 0.109** | 0.7 σ |
//! | mean `ln E` | 13.6835 | 13.6846 | **−0.008 % ± 0.008** | 1.0 σ |
//! | flux fraction < 300 keV | 0.155354 | 0.155109 | **+0.157 % ± 0.226** | 0.7 σ |
//!
//! **THE PREDICTION WAS NOT SUPPORTED.** It said harder; all three central
//! values say softer. Nothing is resolved at 2 σ, so it is not refuted either,
//! and the honest reading is a **bound**: the spectral effect of this law is
//! below about `0.22 %` in mean `E` at 2 σ, against the `+0.45 %` residual
//! `op-os8x` is about.
//!
//! **The useful conclusion is an exclusion.** Whichever way the sign really
//! goes, this law is **not** the explanation for `op-os8x` — too small to
//! account for it if it hardens, and pointing away from it if it softens. That
//! is what this measurement bought, and it is worth more than the sign would
//! have been.
//!
//! **Do not read the three rows as three independent votes.** They come from
//! the same tallied spectrum in the same runs and are strongly correlated;
//! their agreeing in sign is close to one observation, not three.
//!
//! **After-the-fact hypothesis, labelled as such** — formed after seeing these
//! numbers, untested: in a 55.8 %-leakage bare sphere, raising the transport
//! mean free path preferentially removes the *fast* neutrons most likely to
//! escape, which softens the surviving in-core flux. That opposes the
//! per-collision hardening and is tied to the same leakage that produced the
//! `−41 ± 43 pcm` reactivity effect. It is the weak point named in the
//! prediction above, arriving from a direction the prediction did not consider.
//!
//! **The discriminating measurement, not done:** run this same comparison under
//! a **reflective** boundary (`k_inf`, no leakage), where the leakage-selection
//! term vanishes and only the per-collision term survives. If the spectrum
//! hardens there, both effects are real and they compete; if it does not, the
//! per-collision argument is simply wrong. That is the decomposition `op-tm9f`
//! used to show its own fix was leakage-only, applied here.
//!
//! ```text
//! OUTRAM_GODIVA_SEEDS=8 cargo run --release -p outram-mc-libs \
//!     --features endf-pebble-cases --example godiva_continuum_spectrum_ablation
//! ```

#[cfg(target_os = "android")]
fn main() {
    println!("godiva_continuum_spectrum_ablation is desktop-only (reads reference-data/endf/).");
}

#[cfg(not(target_os = "android"))]
fn main() {
    desktop::run();
}

#[cfg(not(target_os = "android"))]
mod desktop {
    use njoy_outram_park_fork::reference_data::reference_endf;
    use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
    use outram_mc_libs::geometry::geometry::Geometry;
    use outram_mc_libs::geometry::position::Position;
    use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
    use outram_mc_libs::geometry::universe::Universe;
    use outram_mc_libs::material::material::{Material, NuclideComponent};
    use outram_mc_libs::material::nuclide::Nuclide;
    use outram_mc_libs::physics::keff::KeffSettings;
    use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};
    use outram_mc_libs::tally::filter::{EnergyFilter, FilterKind};
    use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};
    use std::time::Instant;

    const TEMP_K: f64 = 293.6;
    const RADIUS_CM: f64 = 8.7407;
    const HISTORIES: usize = 5000;
    const INACTIVE: usize = 40;
    const ACTIVE: usize = 120;
    /// 50 log bins over 1e-3 .. 2e7 eV — the grid `godiva_spectrum_vs_openmc.rs`
    /// uses, so these numbers sit beside that study's directly.
    const N_BINS: usize = 50;
    const E_LO: f64 = 1.0e-3;
    const E_HI: f64 = 2.0e7;
    /// The boundary the cross-code study reports its flux fraction below \[eV\].
    const SOFT_CUT_EV: f64 = 3.0e5;

    const NUCLIDES: &[(&str, &str, f64)] = &[
        ("n-092_U_234-ENDF8.0.endf", "U234", 4.9184e-4),
        ("n-092_U_235-ENDF8.0.endf", "U235", 4.4994e-2),
        ("n-092_U_238.endf", "U238", 2.4984e-3),
    ];

    fn log_grid(n: usize) -> Vec<f64> {
        let (l0, l1) = (E_LO.ln(), E_HI.ln());
        (0..=n)
            .map(|i| (l0 + (l1 - l0) * i as f64 / n as f64).exp())
            .collect()
    }

    fn godiva_geometry() -> Geometry {
        Geometry {
            surfaces: vec![SurfaceKind::Sphere(Sphere {
                x0: 0.0,
                y0: 0.0,
                z0: 0.0,
                r: RADIUS_CM,
                bc: BoundaryType::Vacuum,
            })],
            cells: vec![Cell::material(
                1,
                vec![RegionToken::HalfSpace {
                    surface_idx: 0,
                    sense: HalfSpaceSense::Inside,
                }],
                0,
                TEMP_K,
            )],
            universes: vec![Universe {
                id: 0,
                cell_indices: vec![0],
            }],
            lattices: vec![],
            root_universe: 0,
        }
    }

    /// The three integral hardness measures of one tallied spectrum.
    ///
    /// Bin centres are **geometric** (`√(e_lo·e_hi)`), which is the right centre
    /// for a logarithmic grid: the arithmetic centre of a bin spanning half a
    /// decade sits well above the flux-weighted middle and would bias `mean E`
    /// upward by a fixed amount in both arms. It would cancel in the difference,
    /// but it would make the absolute numbers non-comparable to the cross-code
    /// study, which is half the point of using its grid.
    struct Hardness {
        mean_e: f64,
        mean_ln_e: f64,
        frac_below_cut: f64,
    }

    fn hardness(edges: &[f64], flux: &[f64]) -> Hardness {
        let (mut num_e, mut num_ln, mut den, mut below) = (0.0, 0.0, 0.0, 0.0);
        for (i, &f) in flux.iter().enumerate() {
            let (lo, hi) = (edges[i], edges[i + 1]);
            let centre = (lo * hi).sqrt();
            num_e += f * centre;
            num_ln += f * centre.ln();
            den += f;
            if hi <= SOFT_CUT_EV {
                below += f;
            }
        }
        Hardness {
            mean_e: if den > 0.0 { num_e / den } else { 0.0 },
            mean_ln_e: if den > 0.0 { num_ln / den } else { 0.0 },
            frac_below_cut: if den > 0.0 { below / den } else { 0.0 },
        }
    }

    fn stats(x: &[f64]) -> (f64, f64) {
        let n = x.len() as f64;
        let mean = x.iter().sum::<f64>() / n;
        let var = x.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
        (mean, (var / n).sqrt())
    }

    /// Run one arm over `seeds`, returning the per-seed hardness measures.
    fn run_arm(
        nuclides: &[Nuclide],
        materials: &[Material],
        edges: &[f64],
        seeds: &[u64],
    ) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        let (mut me, mut mln, mut fb) = (Vec::new(), Vec::new(), Vec::new());
        for &seed in seeds {
            let mut tally = Tally {
                id: 1,
                name: "godiva flux spectrum".into(),
                filters: vec![FilterKind::Energy(EnergyFilter {
                    bins: edges.to_vec(),
            })],
                scores: vec![ScoreType::Flux],
                bins: vec![TallyBin::default(); N_BINS],
            };
            let settings = KeffSettings {
                n_particles: HISTORIES,
                n_inactive: INACTIVE,
                n_active: ACTIVE,
                temperature_k: TEMP_K,
                seed,
                ..KeffSettings::default()
            };
            let src = SourceBox {
                lower: Position::new(-RADIUS_CM, -RADIUS_CM, -RADIUS_CM),
                upper: Position::new(RADIUS_CM, RADIUS_CM, RADIUS_CM),
            };
            let _ = run_keff_csg(
                &godiva_geometry(),
                materials,
                nuclides,
                src,
                &settings,
                Some(&mut tally),
            );
            let flux: Vec<f64> = tally.bins.iter().map(|b| b.sum).collect();
            let h = hardness(edges, &flux);
            me.push(h.mean_e);
            mln.push(h.mean_ln_e);
            fb.push(h.frac_below_cut);
        }
        (me, mln, fb)
    }

    pub fn run() {
        let n_seeds: usize = std::env::var("OUTRAM_GODIVA_SEEDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8);

        let t0 = Instant::now();
        println!("Reconstructing HEU isotopes from ENDF/B-VIII.0 (RECONR + BROADR @ {TEMP_K} K)…");
        let mut aniso = Vec::new();
        for &(file, name, _) in NUCLIDES {
            let Some(p) = reference_endf(file) else {
                println!("  missing {file} — set OUTRAM_PARK_ENDF_DIR; skipping.");
                return;
            };
            aniso.push(
                Nuclide::from_endf_file(&p, name, TEMP_K, 1.0e-3)
                    .unwrap_or_else(|e| panic!("from_endf_file({}): {e}", p.display())),
            );
        }
        // The control, before spending any transport time: if no nuclide carries
        // a continuum angular law, the ablation removes nothing and any null
        // result below would be an artefact rather than a measurement.
        assert!(
            aniso.iter().any(|n| n.has_continuum_anisotropy()),
            "no nuclide carries a continuum angular law, so this ablation would remove nothing"
        );
        let iso: Vec<Nuclide> = aniso
            .iter()
            .cloned()
            .map(Nuclide::with_isotropic_continuum_scattering)
            .collect();
        assert!(
            iso.iter().all(|n| !n.has_continuum_anisotropy()),
            "the ablation is a no-op -- it left an angular law in place"
        );
        println!("Nuclear data ready in {:.1} s.\n", t0.elapsed().as_secs_f64());

        let materials = vec![Material {
            id: 1,
            name: "Godiva HEU".into(),
            temperature: TEMP_K,
            components: NUCLIDES
                .iter()
                .enumerate()
                .map(|(i, &(_, _, d))| NuclideComponent {
                    nuclide_idx: i,
                    atom_density: d,
                })
                .collect(),
        }];
        let edges = log_grid(N_BINS);
        let seeds: Vec<u64> = (1..=n_seeds as u64).collect();

        println!(
            "{n_seeds} seeds per arm, {HISTORIES} histories × [{INACTIVE} + {ACTIVE}], \
             {N_BINS} log bins…"
        );
        let t = Instant::now();
        let (a_e, a_ln, a_fb) = run_arm(&aniso, &materials, &edges, &seeds);
        println!("  ANISO arm done in {:.1} s", t.elapsed().as_secs_f64());
        let t = Instant::now();
        let (i_e, i_ln, i_fb) = run_arm(&iso, &materials, &edges, &seeds);
        println!("  ISO   arm done in {:.1} s\n", t.elapsed().as_secs_f64());

        println!("Godiva flux-spectrum hardness, ANISO (evaluated MF=6 angle) vs ISO:");
        println!(
            "  {:<24} {:>14} {:>14} {:>20}",
            "measure", "ANISO", "ISO", "relative change"
        );

        let mut harder_votes = 0i32;
        for (label, a, b, higher_is_harder) in [
            ("mean E [eV]", &a_e, &i_e, true),
            ("mean ln E [ln eV]", &a_ln, &i_ln, true),
            ("flux fraction < 300 keV", &a_fb, &i_fb, false),
        ] {
            let (ma, sa) = stats(a);
            let (mb, sb) = stats(b);
            // Relative change and its error, propagated.
            let rel = if mb != 0.0 { (ma - mb) / mb.abs() } else { 0.0 };
            let rel_err = if mb != 0.0 {
                ((sa / mb).powi(2) + (ma * sb / (mb * mb)).powi(2)).sqrt()
            } else {
                0.0
            };
            let sigma = if rel_err > 0.0 {
                (rel / rel_err).abs()
            } else {
                0.0
            };
            println!(
                "  {label:<24} {ma:>14.5e} {mb:>14.5e} {:>20}",
                format!("{:+.3} % ± {:.3} ({:.1}σ)", rel * 100.0, rel_err * 100.0, sigma)
            );
            if sigma >= 2.0 {
                harder_votes += if (rel > 0.0) == higher_is_harder { 1 } else { -1 };
            }
        }

        println!(
            "\n  Prediction recorded before this ran: ANISO HARDER than ISO — mean E up,\n  \
             mean ln E up, flux below 300 keV down. For a CM law the lab energy is\n  \
             E' = E_cm + E_trans + 2 mu_cm sqrt(E_cm E_trans), so a forward-peaked cosine\n  \
             raises <E'_lab>."
        );
        match harder_votes {
            v if v > 0 => println!(
                "  VERDICT: harder, as predicted ({v} of 3 measures resolved at >=2 sigma agree)."
            ),
            v if v < 0 => println!(
                "  VERDICT: SOFTER -- the prediction is WRONG ({} measures resolved at >=2 sigma \
                 disagree).\n  Correct the sign argument wherever it is repeated rather than \
                 explaining it away.",
                -v
            ),
            _ => println!(
                "  VERDICT: nothing resolved at 2 sigma. This run bounds the spectral effect \
                 rather than\n  measuring it -- raise OUTRAM_GODIVA_SEEDS. A null here is \
                 itself informative: it would\n  mean the continuum angular law is NOT a \
                 contributor to op-os8x at this precision."
            ),
        }
        println!(
            "\n  NOTE: this compares the code against ITSELF. op-os8x is a comparison against\n  \
             OpenMC (+0.45 % in mean E, -1.03 % below 300 keV) and this example cannot close\n  \
             it -- only say how much of it this one mechanism could account for."
        );
    }
}
