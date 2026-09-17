//! **The eight tally filters added 2026-09-16, each against its own
//! mathematical definition rather than against itself.**
//!
//! # What was missing
//!
//! `tally/filter.rs` carried a standing `TODO: Zernike, SphericalHarmonics,
//! MuFilter, PolarAzimuthal, Surface, DelayedGroup, Time, Particle` — eight of
//! OpenMC's filters, five of which could not even have been *written* because
//! `FilterEvent` had no field for an angle, a time, a particle type or a delayed
//! group. All eight now exist, and `FilterEvent` carries the phase space they
//! need.
//!
//! # Methodology
//!
//! Each filter is checked against the property that defines it, computed here
//! independently:
//!
//! - **Zernike** — orthogonality on the unit disc. `Z_n^m` and `Z_n'^m'` are
//!   orthogonal under `int_0^1 int_0^2pi ... rho drho dtheta`, so a numerical
//!   quadrature of the filter's own moments must be diagonal. This tests the
//!   radial polynomials, the `m < 0` sine branch and the moment ordering all at
//!   once — none of which a self-consistency check would catch.
//! - **Spherical harmonics** — orthonormality over the sphere, the same way, plus
//!   `Y_0^0 = 1/sqrt(4 pi)` exactly.
//! - **Mu / Time / PolarAzimuthal** — bin-edge behaviour at the boundaries,
//!   including the inclusive top edge, which is where an off-by-one lives.
//! - **Surface / Particle / DelayedGroup** — rejection of events that carry no
//!   surface, a different particle, or a prompt (`None`) group. Rejection is the
//!   interesting half: a filter that accepts everything scores plausibly and
//!   wrongly.
//!
//! # The orthogonality gate tests CONVERGENCE, not smallness
//!
//! A fixed tolerance on a numerical Gram matrix cannot tell a real
//! non-orthogonality from the quadrature's own error — and at the first attempt
//! it did not: a `1e-9` gate failed at `2.9e-5`, which looked like a defect and
//! was the midpoint rule. The gate is therefore **refinement**: quadruple the
//! quadrature and the residual must fall by at least 8x. Midpoint is
//! second-order, so a quadrature residual falls ~16x; a genuine
//! non-orthogonality would not move at all.
//!
//! # Results (2026-09-16)
//!
//! | expansion | coarse | fine (4x) | ratio |
//! |---|---|---|---|
//! | Zernike, order 4, 15 moments | 2.945e-5 | 1.841e-6 | **16.0x** |
//! | Spherical harmonics, order 3, 16 moments | 8.750e-5 | 5.469e-6 | **16.0x** |
//!
//! Exactly the second-order rate, on both, so the residual is entirely the
//! quadrature's and both expansions are orthogonal to the precision measured.
//! `Y_0^0` also matches `1/sqrt(4 pi)` to 1e-14 in every direction.
//!
//! # What this does NOT claim
//!
//! These check the filters' mathematics and binning, not that transport threads
//! the right values into `FilterEvent`. The track-length and collision
//! estimators do not yet populate angle, time or particle type — stated in
//! `score_track_length`'s own comment and in `FilterEvent::default`'s docs, so a
//! caller cannot mistake a flat result for a physical one.

use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::particle::particle::ParticleType;
use outram_mc_libs::prelude::*;

fn at(position: Position) -> FilterEvent {
    FilterEvent {
        position,
        ..Default::default()
    }
}

fn going(u: f64, v: f64, w: f64) -> FilterEvent {
    let n = (u * u + v * v + w * w).sqrt();
    FilterEvent {
        direction: Direction {
            u: u / n,
            v: v / n,
            w: w / n,
        },
        ..Default::default()
    }
}

#[test]
fn zernike_moments_are_orthogonal_on_the_unit_disc() {
    const ORDER: usize = 4;
    let f = ZernikeFilter {
        order: ORDER,
        x0: 0.0,
        y0: 0.0,
        r: 1.0,
    };
    let n = f.n_bins();
    assert_eq!(n, (ORDER + 1) * (ORDER + 2) / 2, "moment count");

    // Polar quadrature of <Z_i, Z_j> = int Z_i Z_j rho drho dtheta, at two
    // resolutions. Midpoint quadrature has its own error, so the question is not
    // "is the off-diagonal zero" but "does it fall like the quadrature does".
    let gram_at = |nr: usize, nt: usize| -> Vec<f64> {
        let (NR, NT) = (nr, nt);
        let mut gram = vec![0.0f64; n * n];
        for ir in 0..NR {
            let rho = (ir as f64 + 0.5) / NR as f64;
            for it in 0..NT {
                let th = 2.0 * std::f64::consts::PI * (it as f64 + 0.5) / NT as f64;
                let ev = at(Position {
                    x: rho * th.cos(),
                    y: rho * th.sin(),
                    z: 0.0,
                });
                let z = f.expansion_moments(&ev).expect("inside the disc");
                let w = rho * (1.0 / NR as f64) * (2.0 * std::f64::consts::PI / NT as f64);
                for i in 0..n {
                    for j in 0..n {
                        gram[i * n + j] += w * z[i] * z[j];
                    }
                }
            }
        }
        gram
    };
    let summarise = |gram: &[f64]| -> (f64, f64) {
        let mut worst_off = 0.0f64;
        let mut min_diag = f64::INFINITY;
        for i in 0..n {
            min_diag = min_diag.min(gram[i * n + i].abs());
            for j in 0..n {
                if i != j {
                    worst_off = worst_off.max(gram[i * n + j].abs());
                }
            }
        }
        (min_diag, worst_off)
    };
    let (min_diag, worst_off) = summarise(&gram_at(400, 720));
    let (_, worst_off_fine) = summarise(&gram_at(1600, 2880));
    println!(
        "Zernike order {ORDER}: {n} moments, min |diagonal| = {min_diag:.6e}, worst \
         |off-diagonal| = {worst_off:.3e} at 400x720, {worst_off_fine:.3e} at 1600x2880 \
         (ratio {:.1}x)",
        worst_off / worst_off_fine.max(1e-300)
    );
    assert!(
        min_diag > 1.0e-3,
        "a moment integrated to zero: the ordering or the radial polynomial is wrong"
    );
    // The discriminating assertion is CONVERGENCE, not smallness: midpoint
    // quadrature on a 4x-refined grid must cut the residual by ~16x if the
    // residual is the quadrature's. A genuine non-orthogonality would not move.
    assert!(
        worst_off_fine < worst_off / 8.0,
        "refining the quadrature 4x only took the worst off-diagonal from {worst_off:.3e} to \
         {worst_off_fine:.3e}. A quadrature residual would fall ~16x; one that does not fall is \
         real non-orthogonality, meaning the radial polynomial, the sin/cos branch for m<0, or \
         the moment ordering is wrong."
    );
    assert!(
        worst_off_fine < 1.0e-5 * min_diag,
        "Zernike moments are not orthogonal even at the fine quadrature (worst off-diagonal \
         {worst_off_fine:.3e} against min diagonal {min_diag:.3e})."
    );

    // Outside the disc contributes nothing.
    assert!(f
        .expansion_moments(&at(Position {
            x: 1.5,
            y: 0.0,
            z: 0.0
        }))
        .is_none());
    // And the disc can be moved and scaled.
    let g = ZernikeFilter {
        order: 2,
        x0: 3.0,
        y0: -1.0,
        r: 2.0,
    };
    assert!(g
        .expansion_moments(&at(Position {
            x: 4.0,
            y: -1.0,
            z: 0.0
        }))
        .is_some());
    assert!(g
        .expansion_moments(&at(Position {
            x: 6.5,
            y: -1.0,
            z: 0.0
        }))
        .is_none());
}

#[test]
fn spherical_harmonic_moments_are_orthonormal_on_the_sphere() {
    const ORDER: usize = 3;
    let f = SphericalHarmonicsFilter { order: ORDER };
    let n = f.n_bins();
    assert_eq!(n, (ORDER + 1) * (ORDER + 1), "moment count");

    // Y_0^0 = 1/sqrt(4 pi), exactly, in every direction.
    let y00 = f.expansion_moments(&going(0.3, -0.5, 0.8)).unwrap()[0];
    let expect = 1.0 / (4.0 * std::f64::consts::PI).sqrt();
    assert!(
        (y00 - expect).abs() < 1.0e-14,
        "Y_0^0 = {y00:.12} but must be 1/sqrt(4 pi) = {expect:.12} in every direction"
    );

    let gram_at = |nmu: usize, nphi: usize| -> Vec<f64> {
        let (NMU, NPHI) = (nmu, nphi);
        let mut gram = vec![0.0f64; n * n];
        for im in 0..NMU {
            let mu = -1.0 + 2.0 * (im as f64 + 0.5) / NMU as f64;
            let st = (1.0 - mu * mu).max(0.0).sqrt();
            for ip in 0..NPHI {
                let phi = -std::f64::consts::PI
                    + 2.0 * std::f64::consts::PI * (ip as f64 + 0.5) / NPHI as f64;
                let ev = going(st * phi.cos(), st * phi.sin(), mu);
                let y = f
                    .expansion_moments(&ev)
                    .expect("harmonics are defined everywhere");
                let w = (2.0 / NMU as f64) * (2.0 * std::f64::consts::PI / NPHI as f64);
                for i in 0..n {
                    for j in 0..n {
                        gram[i * n + j] += w * y[i] * y[j];
                    }
                }
            }
        }
        gram
    };
    let worst_of = |gram: &[f64]| -> f64 {
        let mut worst = 0.0f64;
        for i in 0..n {
            for j in 0..n {
                let target = if i == j { 1.0 } else { 0.0 };
                worst = worst.max((gram[i * n + j] - target).abs());
            }
        }
        worst
    };
    let worst = worst_of(&gram_at(400, 800));
    let worst_fine = worst_of(&gram_at(1600, 3200));
    println!(
        "Spherical harmonics order {ORDER}: {n} moments, worst |Gram - I| = {worst:.3e} at \
         400x800, {worst_fine:.3e} at 1600x3200 (ratio {:.1}x)",
        worst / worst_fine.max(1e-300)
    );
    assert!(
        worst_fine < worst / 8.0,
        "refining the quadrature 4x only took worst |Gram - I| from {worst:.3e} to \
         {worst_fine:.3e}. A quadrature residual falls ~16x; one that does not is real \
         non-orthonormality -- the normalisation N_l^m, the sqrt(2) on m != 0, or the \
         associated Legendre recurrence."
    );
    assert!(
        worst_fine < 1.0e-5,
        "real spherical harmonics are not orthonormal even at the fine quadrature (worst \
         |Gram - I| = {worst_fine:.3e})."
    );
}

#[test]
fn edge_binned_filters_handle_their_boundaries() {
    let mu = MuFilter {
        bounds: vec![-1.0, -0.5, 0.0, 0.5, 1.0],
    };
    assert_eq!(mu.n_bins(), 4);
    let b = |x: f64| {
        mu.get_bin(&FilterEvent {
            mu: x,
            ..Default::default()
        })
    };
    assert_eq!(b(-1.0), Some(0), "the bottom edge belongs to the first bin");
    assert_eq!(b(-0.75), Some(0));
    assert_eq!(
        b(-0.5),
        Some(1),
        "an interior edge belongs to the bin above"
    );
    assert_eq!(b(0.999), Some(3));
    assert_eq!(b(1.0), Some(3), "the TOP edge is inclusive, not rejected");
    assert_eq!(b(1.0001), None, "outside the range scores nothing");
    assert_eq!(b(-1.0001), None);

    let t = TimeFilter {
        bounds: vec![0.0, 1.0e-6, 1.0e-3],
    };
    assert_eq!(t.n_bins(), 2);
    assert_eq!(
        t.get_bin(&FilterEvent {
            time: 0.0,
            ..Default::default()
        }),
        Some(0)
    );
    assert_eq!(
        t.get_bin(&FilterEvent {
            time: 1.0e-6,
            ..Default::default()
        }),
        Some(1)
    );
    assert_eq!(
        t.get_bin(&FilterEvent {
            time: 1.0e-3,
            ..Default::default()
        }),
        Some(1)
    );
    assert_eq!(
        t.get_bin(&FilterEvent {
            time: 1.0,
            ..Default::default()
        }),
        None
    );

    // Polar/azimuthal: polar slowest-varying, so bin = i_polar*n_azi + i_azi.
    let pa = PolarAzimuthalFilter {
        polar: vec![-1.0, 0.0, 1.0],
        azimuthal: vec![-std::f64::consts::PI, 0.0, std::f64::consts::PI],
    };
    assert_eq!(pa.n_bins(), 4);
    // +z, phi = atan2(0, 1) = 0 -> polar bin 1 (cos = +1 is the top edge, bin 1),
    // azimuthal bin 1 (phi = 0 is an interior edge -> bin above).
    assert_eq!(pa.get_bin(&going(0.0, 0.0, 1.0)), Some(1 * 2 + 1));
    // -z, phi = 0 -> polar bin 0.
    assert_eq!(pa.get_bin(&going(0.0, 0.0, -1.0)), Some(0 * 2 + 1));
    // +x with a small -y: phi slightly negative -> azimuthal bin 0, polar bin 1
    // (w = 0 is an interior edge, so the bin above).
    let e = going(1.0, -1.0e-6, 0.0);
    assert_eq!(pa.get_bin(&e), Some(1 * 2 + 0));
}

#[test]
fn rejecting_filters_actually_reject() {
    // Surface: only a surface-crossing event has one.
    let s = SurfaceFilter {
        surface_indices: vec![2, 5],
    };
    assert_eq!(s.n_bins(), 2);
    assert_eq!(
        s.get_bin(&FilterEvent {
            surface_idx: 5,
            ..Default::default()
        }),
        Some(1)
    );
    assert_eq!(
        s.get_bin(&FilterEvent {
            surface_idx: 3,
            ..Default::default()
        }),
        None
    );
    assert_eq!(
        s.get_bin(&FilterEvent::default()),
        None,
        "a non-crossing event carries surface_idx = usize::MAX and must not score"
    );

    // Particle: this crate transports neutrons only, so a photon bin stays empty.
    let p = ParticleFilter {
        particles: vec![ParticleType::Neutron, ParticleType::Photon],
    };
    assert_eq!(p.get_bin(&FilterEvent::default()), Some(0));
    assert_eq!(
        p.get_bin(&FilterEvent {
            particle: ParticleType::Photon,
            ..Default::default()
        }),
        Some(1)
    );
    assert_eq!(
        p.get_bin(&FilterEvent {
            particle: ParticleType::Electron,
            ..Default::default()
        }),
        None
    );

    // Delayed group: a prompt neutron carries None and must not score.
    let d = DelayedGroupFilter {
        groups: vec![0, 1, 2, 3, 4, 5],
    };
    assert_eq!(d.n_bins(), 6);
    assert_eq!(
        d.get_bin(&FilterEvent {
            delayed_group: Some(3),
            ..Default::default()
        }),
        Some(3)
    );
    assert_eq!(
        d.get_bin(&FilterEvent::default()),
        None,
        "a prompt neutron has no precursor group and must be rejected, not binned into group 0"
    );
    // A group outside the requested set is rejected rather than clamped.
    assert_eq!(
        DelayedGroupFilter { groups: vec![0, 1] }.get_bin(&FilterEvent {
            delayed_group: Some(4),
            ..Default::default()
        }),
        None
    );
}

/// `FilterKind` dispatches to the same answer the concrete filter gives, for
/// every variant — the property that makes replacing `Box<dyn Filter>` with an
/// enum a refactor rather than a rewrite.
#[test]
fn filterkind_dispatch_matches_the_concrete_filters() {
    let ev = FilterEvent {
        cell_idx: 1,
        material_idx: 0,
        universe_idx: 0,
        energy: 1.0e6,
        surface_idx: 2,
        position: Position {
            x: 0.1,
            y: 0.2,
            z: 0.3,
        },
        direction: Direction {
            u: 0.0,
            v: 0.0,
            w: 1.0,
        },
        mu: 0.25,
        time: 5.0e-7,
        particle: ParticleType::Neutron,
        delayed_group: Some(1),
    };

    let kinds = vec![
        (
            FilterKind::Cell(CellFilter {
                cell_indices: vec![0, 1],
            }),
            Some(1),
        ),
        (
            FilterKind::Surface(SurfaceFilter {
                surface_indices: vec![2],
            }),
            Some(0),
        ),
        (
            FilterKind::Mu(MuFilter {
                bounds: vec![-1.0, 0.0, 1.0],
            }),
            Some(1),
        ),
        (
            FilterKind::Time(TimeFilter {
                bounds: vec![0.0, 1.0e-6],
            }),
            Some(0),
        ),
        (
            FilterKind::Particle(ParticleFilter {
                particles: vec![ParticleType::Neutron],
            }),
            Some(0),
        ),
        (
            FilterKind::DelayedGroup(DelayedGroupFilter { groups: vec![0, 1] }),
            Some(1),
        ),
    ];
    for (k, want) in &kinds {
        assert_eq!(
            k.get_bin(&ev),
            *want,
            "dispatch disagreed for a filter kind"
        );
        assert!(k.n_bins() >= 1);
        assert!(!k.is_expansion(), "a single-bin filter is not an expansion");
    }

    // The three expansions report themselves as such and deposit moments.
    for k in [
        FilterKind::SpatialLegendre(SpatialLegendreFilter {
            order: 2,
            axis: LegendreAxis::Z,
            min: 0.0,
            max: 1.0,
        }),
        FilterKind::Zernike(ZernikeFilter {
            order: 2,
            x0: 0.0,
            y0: 0.0,
            r: 1.0,
        }),
        FilterKind::SphericalHarmonics(SphericalHarmonicsFilter { order: 2 }),
    ] {
        assert!(
            k.is_expansion(),
            "expansion filter did not report itself as one"
        );
        let m = k
            .expansion_moments(&ev)
            .expect("event is inside every domain here");
        assert_eq!(m.len(), k.n_bins(), "moment count must equal bin count");
    }
}
