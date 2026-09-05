//! Preconfigured geometry + material presets (op-dyt).
//!
//! Each [`GeometryPreset`] variant is a *touch-selectable card* in the geometry
//! picker screen. Selecting one fully determines a concrete geometry + material
//! configuration the transport driver in [`crate::transport`] consumes — the
//! user never hand-builds a [`outram_mc_libs::geometry::geometry::Geometry`] or a
//! [`outram_mc_libs::material::material::Material`] directly.
//!
//! # Which transport driver backs which preset
//!
//! - [`GeometryPreset::BareSphere`] and [`GeometryPreset::LwrCell`] build a
//!   [`outram_mc_libs::geometry::geometry::Geometry`] (general CSG) and run
//!   through [`outram_mc_libs::physics::transport_csg::run_keff_csg`] — this is
//!   the **only** driver in the crate today that accepts an optional
//!   [`outram_mc_libs::tally::tally::Tally`], so these two presets are the ones
//!   that can show the neutron-spectrum overlay (op-iom).
//! - [`GeometryPreset::PebbleBed`] and [`GeometryPreset::TmsrPebbleBed`] are
//!   doubly-heterogeneous random-packed media, driven by
//!   [`outram_mc_libs::pebble_beds::keff_delta::run_keff_delta`] (delta/Woodcock
//!   tracking over [`outram_mc_libs::pebble_beds::sphere_packing::PackedSpheres`]).
//!   That driver has **no tally parameter** in this crate version, so the
//!   spectrum screen shows an explanatory gap message for these two instead of a
//!   chart — see [`crate::spectrum`].
//!
//! # Data source
//!
//! Every nuclide is built with [`Nuclide::from_core`] — the embedded LOW-tier
//! CORE windowed-multipole + fast-MGXS library in `njoy-outram-park-fork`
//! (`docs/wmp-nuclide-manifest.md`), no network, no HDF5. Bare-metal-sphere and
//! pin-cell atom densities mirror the existing verification tests in
//! `crates/outram-mc-libs/tests/openmc_notebooks/{pincell,flux_spectrum}.rs` and
//! `examples/triso_delta_tracking.rs`. Where a preset would need data this
//! offline environment does not have (e.g. the ENDF/B-VIII.0 H-in-H2O thermal
//! scattering law file for a truly thermal LWR pin, or an independently
//! re-verified ICSBEP Jezebel record), a representative in-crate stand-in is
//! used instead and the approximation is documented on the preset's `blurb()`
//! and in the crate README.

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind, XPlane, YPlane, ZCylinder};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
use outram_mc_libs::pebble_beds::sphere_packing::PackedSpheres;
use outram_mc_libs::physics::transport_csg::SourceBox;

/// Which bare-metal-sphere composition [`GeometryPreset::BareSphere`] carries.
///
/// Both are fast, unreflected, highly-enriched-metal spheres — the classic
/// "bare critical assembly" benchmark shape (ICSBEP HEU-MET-FAST-001 /
/// PU-MET-FAST-001 families) — differing only in fissile isotope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SphereVariant {
    /// Highly-enriched-uranium metal sphere (Godiva-like, HEU-MET-FAST-001).
    Godiva,
    /// Plutonium metal sphere (Jezebel-like, PU-MET-FAST-001).
    Jezebel,
}

/// A touch-selectable preconfigured geometry + material case (op-dyt).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryPreset {
    /// Randomly-packed fissile kernels in a hydrogenous matrix inside a
    /// reflective cube — a tractable stand-in for a graphite pebble's TRISO
    /// kernel dispersion (op-dyt "pebble bed").
    PebbleBed,
    /// A single UO2/Zircaloy/light-water LWR fuel pin in a reflective square
    /// cell — the `pincell` notebook geometry (op-dyt "LWR cell").
    LwrCell,
    /// Randomly-packed fissile kernels in a FLiBe-like molten-salt matrix — a
    /// TMSR/LFTR-flavoured pebble bed distinguished from the plain pebble bed
    /// by its salt (not hydrogenous) matrix (op-dyt "TMSR-like pebble bed").
    TmsrPebbleBed,
    /// A bare, unreflected fissile-metal sphere (op-dyt "bare-metal sphere").
    BareSphere(SphereVariant),
}

/// All four top-level preset cards, in the order the geometry picker lists
/// them. [`GeometryPreset::BareSphere`] defaults to
/// [`SphereVariant::Godiva`] here; the geometry-picker screen offers a second
/// tap to switch to [`SphereVariant::Jezebel`] before running.
pub const ALL_PRESETS: [GeometryPreset; 4] = [
    GeometryPreset::PebbleBed,
    GeometryPreset::LwrCell,
    GeometryPreset::TmsrPebbleBed,
    GeometryPreset::BareSphere(SphereVariant::Godiva),
];

impl GeometryPreset {
    /// Short card title for the touch pick-list.
    pub fn title(&self) -> &'static str {
        match self {
            GeometryPreset::PebbleBed => "Pebble bed",
            GeometryPreset::LwrCell => "LWR cell",
            GeometryPreset::TmsrPebbleBed => "TMSR-like pebble bed",
            GeometryPreset::BareSphere(SphereVariant::Godiva) => "Bare sphere: Godiva (HEU)",
            GeometryPreset::BareSphere(SphereVariant::Jezebel) => "Bare sphere: Jezebel (Pu)",
        }
    }

    /// Two-to-four line description shown under the card title: what the
    /// configuration physically is, which driver runs it, and any documented
    /// approximation.
    pub fn blurb(&self) -> &'static str {
        match self {
            GeometryPreset::PebbleBed => {
                "HEU-metal fuel kernels (r=0.04cm) RSA-packed to pf=0.30 in an \
                 H-1 matrix, reflective 1cm cube. Driver: delta (Woodcock) \
                 tracking. Stand-in: this crate has no graphite/carbon nuclide \
                 + S(a,b) yet (see outram-mc-libs CLAUDE.md scope), so the \
                 matrix is hydrogenous, not graphite. No spectrum overlay \
                 (delta-tracking driver has no tally hook yet)."
            }
            GeometryPreset::LwrCell => {
                "UO2 fuel pin / void gap / natural-Zr clad / light-water \
                 moderator, 1.26cm square reflective pitch (openmc `pincell` \
                 notebook geometry). Driver: general CSG (run_keff_csg). \
                 Approximation: free-gas H (no bound-atom S(a,b) - this needs \
                 an external ENDF/B-VIII.0 tsl-HinH2O.endf file not present in \
                 this offline environment), so the spectrum thermalizes only \
                 approximately. Spectrum overlay available."
            }
            GeometryPreset::TmsrPebbleBed => {
                "Same HEU-kernel dispersion as the pebble bed, but the matrix \
                 is a FLiBe-like molten salt (Li-7/Be-9/F-19, the CORE \
                 library's LFTR nuclide package) instead of hydrogen - a \
                 salt-cooled, non-hydrogenous pebble-bed flavour. Densities are \
                 representative (rho=1.94 g/cm3 Li2BeF4, Li-7 only - natural Li \
                 is ~92.5% Li-7, Li-6's larger absorption is neglected). Driver: \
                 delta tracking. No spectrum overlay (same driver-hook gap as \
                 the plain pebble bed)."
            }
            GeometryPreset::BareSphere(SphereVariant::Godiva) => {
                "Bare HEU-metal sphere (r=8.7407cm), HEU-MET-FAST-001-like atom \
                 densities from this crate's own Godiva verification tests. \
                 Driver: single-cell CSG sphere through run_keff_csg (same \
                 physics as the crate's dedicated run_keff bare-sphere driver, \
                 routed through the CSG path so the spectrum tally attaches). \
                 Spectrum overlay available."
            }
            GeometryPreset::BareSphere(SphereVariant::Jezebel) => {
                "Bare Pu-metal sphere (r=6.3849cm), PU-MET-FAST-001-like Pu-239/ \
                 240/241 atom densities reproduced from commonly published \
                 reactor-physics literature values, NOT independently \
                 re-verified against the primary ICSBEP handbook in this \
                 session - treat as illustrative, not benchmark-grade. The \
                 real assembly's ~1 at% Ga alloying addition is omitted (Ga \
                 is in njoy-outram-park-fork's EXTENDED nuclide set, not the \
                 embedded CORE library this preset reads from). Driver: \
                 single-cell CSG sphere through run_keff_csg. Spectrum \
                 overlay available."
            }
        }
    }

    /// One-line card summary (fits a narrow phone terminal) — the short form
    /// of [`Self::blurb`] used on the geometry-picker cards; the full blurb
    /// text is available via the card's "info" expansion.
    pub fn summary(&self) -> &'static str {
        match self {
            GeometryPreset::PebbleBed => {
                "RSA-packed HEU kernels in H matrix, reflective cube. Delta tracking."
            }
            GeometryPreset::LwrCell => {
                "UO2/Zr/water pin cell, reflective pitch (openmc `pincell`). CSG + spectrum."
            }
            GeometryPreset::TmsrPebbleBed => {
                "Same kernels in a FLiBe-like salt matrix. Delta tracking."
            }
            GeometryPreset::BareSphere(SphereVariant::Godiva) => {
                "Bare HEU sphere r=8.74cm. CSG + spectrum."
            }
            GeometryPreset::BareSphere(SphereVariant::Jezebel) => {
                "Bare Pu sphere r=6.38cm. CSG + spectrum."
            }
        }
    }

    /// Whether this preset's driver can attach an energy-filter flux tally, so
    /// the spectrum-overlay screen (op-iom) has real data to show. `false` for
    /// the two delta-tracking (pebble-bed) presets — see the module docs.
    pub fn supports_spectrum(&self) -> bool {
        matches!(
            self,
            GeometryPreset::LwrCell | GeometryPreset::BareSphere(_)
        )
    }

    /// Cycle to the next preset in [`ALL_PRESETS`] order, wrapping — used for
    /// keyboard left/right navigation on the geometry picker. Touch users tap a
    /// card directly instead.
    pub fn next(self) -> Self {
        let names_match =
            |a: &Self, b: &Self| std::mem::discriminant(a) == std::mem::discriminant(b);
        let i = ALL_PRESETS
            .iter()
            .position(|p| names_match(p, &self))
            .unwrap_or(0);
        ALL_PRESETS[(i + 1) % ALL_PRESETS.len()]
    }

    /// Swap the sphere variant (Godiva <-> Jezebel); a no-op on every other
    /// preset. Used by the geometry picker's "switch isotope" tap target.
    pub fn toggle_sphere_variant(self) -> Self {
        match self {
            GeometryPreset::BareSphere(SphereVariant::Godiva) => {
                GeometryPreset::BareSphere(SphereVariant::Jezebel)
            }
            GeometryPreset::BareSphere(SphereVariant::Jezebel) => {
                GeometryPreset::BareSphere(SphereVariant::Godiva)
            }
            other => other,
        }
    }
}

/// The fully-built case a preset expands to: geometry/media + materials +
/// nuclides + how to source an initial fission source, ready for
/// [`crate::transport::run_case`].
pub enum BuiltCase {
    /// General CSG geometry, run through `run_keff_csg` (tally-capable).
    Csg {
        geom: Geometry,
        materials: Vec<Material>,
        nuclides: Vec<Nuclide>,
        source: SourceBox,
    },
    /// Doubly-heterogeneous random-packed medium, run through `run_keff_delta`
    /// (no tally hook in this crate version).
    Delta {
        half_width: f64,
        materials: Vec<Material>,
        nuclides: Vec<Nuclide>,
        majorant: Majorant,
        packed: PackedSpheres,
        fuel_material_idx: usize,
        matrix_material_idx: usize,
    },
}

/// Natural-zirconium isotopes (name, atom fraction) for the LWR cell's clad —
/// mirrors `crates/outram-mc-libs/tests/openmc_notebooks/pincell.rs`.
const ZR_ISOTOPES: &[(&str, f64)] = &[
    ("Zr90", 0.5145),
    ("Zr91", 0.1122),
    ("Zr92", 0.1715),
    ("Zr94", 0.1738),
    ("Zr96", 0.0280),
];

impl GeometryPreset {
    /// Build this preset's concrete geometry/materials/nuclides. Called inside
    /// the background transport thread (see [`crate::transport`]) — nuclide
    /// construction parses the embedded CORE WMP blob, cheap but non-trivial,
    /// so it is not done on the UI thread.
    ///
    /// # Errors
    /// A `String` describing the failure if a nuclide name is not in the CORE
    /// library (should not happen for these fixed presets; surfaced instead of
    /// panicking so a data-tier regression shows up as an in-TUI error, not a
    /// crash).
    pub fn build(&self) -> Result<BuiltCase, String> {
        match self {
            GeometryPreset::BareSphere(variant) => build_bare_sphere(*variant),
            GeometryPreset::LwrCell => build_lwr_cell(),
            GeometryPreset::PebbleBed => build_pebble_bed(false),
            GeometryPreset::TmsrPebbleBed => build_pebble_bed(true),
        }
    }
}

fn nuc(name: &str) -> Result<Nuclide, String> {
    Nuclide::from_core(name).map_err(|e| format!("{name} not in CORE WMP library: {e}"))
}

/// Bare fissile-metal sphere, routed through the general CSG path (a single
/// sphere cell, vacuum boundary) so `run_keff_csg`'s optional tally attaches —
/// see the module docs on why the sphere is CSG-wrapped rather than using the
/// dedicated `run_keff` bare-sphere driver directly.
fn build_bare_sphere(variant: SphereVariant) -> Result<BuiltCase, String> {
    let (radius_cm, nuclides, components) = match variant {
        SphereVariant::Godiva => {
            let nuclides = vec![nuc("U234")?, nuc("U235")?, nuc("U238")?];
            let components = vec![
                NuclideComponent {
                    nuclide_idx: 0,
                    atom_density: 4.9184e-4,
                },
                NuclideComponent {
                    nuclide_idx: 1,
                    atom_density: 4.4994e-2,
                },
                NuclideComponent {
                    nuclide_idx: 2,
                    atom_density: 2.4984e-3,
                },
            ];
            (8.7407_f64, nuclides, components)
        }
        SphereVariant::Jezebel => {
            // Ga-69/71 (the ~1 at% gallium alloying addition of the real
            // Jezebel assembly) are in njoy-outram-park-fork's EXTENDED
            // nuclide set, not the embedded CORE library `Nuclide::from_core`
            // reads from (see `docs/wmp-nuclide-manifest.md`), so this
            // preset omits it — a documented simplification, not an
            // oversight: unalloyed Pu-239/240/241 metal, no Ga.
            let nuclides = vec![nuc("Pu239")?, nuc("Pu240")?, nuc("Pu241")?];
            let components = vec![
                NuclideComponent {
                    nuclide_idx: 0,
                    atom_density: 3.7047e-2,
                },
                NuclideComponent {
                    nuclide_idx: 1,
                    atom_density: 1.7512e-3,
                },
                NuclideComponent {
                    nuclide_idx: 2,
                    atom_density: 1.1674e-4,
                },
            ];
            (6.3849_f64, nuclides, components)
        }
    };
    let material = Material {
        id: 1,
        name: variant_name(variant).into(),
        temperature: 293.6,
        components,
    };

    let surfaces = vec![SurfaceKind::Sphere(Sphere {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        r: radius_cm,
        bc: BoundaryType::Vacuum,
    })];
    let cells = vec![Cell::material(
        1,
        vec![RegionToken::HalfSpace {
            surface_idx: 0,
            sense: HalfSpaceSense::Inside,
        }],
        0,
        293.6,
    )];
    let universes = vec![Universe {
        id: 0,
        cell_indices: vec![0],
    }];
    let geom = Geometry {
        surfaces,
        cells,
        universes,
        lattices: vec![],
        root_universe: 0,
    };

    // Source box: a cube inscribed inside the sphere (half-diagonal < r), so
    // every rejection-sampled point in run_keff_csg's initial source lands
    // inside the one fissile cell without wasted rejections.
    let half = radius_cm / 2.0;
    let source = SourceBox {
        lower: Position::new(-half, -half, -half),
        upper: Position::new(half, half, half),
    };

    Ok(BuiltCase::Csg {
        geom,
        materials: vec![material],
        nuclides,
        source,
    })
}

fn variant_name(v: SphereVariant) -> &'static str {
    match v {
        SphereVariant::Godiva => "Godiva HEU",
        SphereVariant::Jezebel => "Jezebel Pu",
    }
}

/// UO2/Zircaloy/light-water LWR pin cell (openmc `pincell` notebook geometry),
/// mirroring `crates/outram-mc-libs/tests/openmc_notebooks/pincell.rs`'s
/// `uo2_pincell_geometry`/`pincell_materials`, but WITHOUT the H-in-H2O S(a,b)
/// thermal-scattering table (that test's LIVE thermal case is data-gated on an
/// external ENDF/B-VIII.0 file this offline environment does not carry — see
/// `DATA_POLICY.md`). Free-gas hydrogen only: a documented approximation, not a
/// benchmark-accuracy thermal pin.
fn build_lwr_cell() -> Result<BuiltCase, String> {
    let mut nuclides = vec![nuc("U235")?, nuc("U238")?, nuc("O16")?];
    for (name, _) in ZR_ISOTOPES {
        nuclides.push(nuc(name)?);
    }
    nuclides.push(nuc("H1")?);
    // Indices: 0 U235, 1 U238, 2 O16, 3..=7 Zr isotopes, 8 H1.

    let n_uo2 = 0.022312; // rho=10.0 g/cm3, M=269.91 g/mol UO2 formula unit.
    let uo2 = Material {
        id: 1,
        name: "UO2".into(),
        temperature: 293.6,
        components: vec![
            NuclideComponent {
                nuclide_idx: 0,
                atom_density: 0.03 * n_uo2,
            },
            NuclideComponent {
                nuclide_idx: 1,
                atom_density: 0.97 * n_uo2,
            },
            NuclideComponent {
                nuclide_idx: 2,
                atom_density: 2.0 * n_uo2,
            },
        ],
    };
    let n_zr = 0.043572; // rho=6.6 g/cm3, M=91.22 g/mol natural Zr.
    let zr_components: Vec<NuclideComponent> = ZR_ISOTOPES
        .iter()
        .enumerate()
        .map(|(i, (_, frac))| NuclideComponent {
            nuclide_idx: 3 + i,
            atom_density: frac * n_zr,
        })
        .collect();
    let zirc = Material {
        id: 2,
        name: "Zircaloy".into(),
        temperature: 293.6,
        components: zr_components,
    };
    let n_h2o = 0.033427; // rho=1.0 g/cm3, M=18.015 g/mol H2O.
    let water = Material {
        id: 3,
        name: "water (free-gas H, no S(a,b))".into(),
        temperature: 293.6,
        components: vec![
            NuclideComponent {
                nuclide_idx: 8,
                atom_density: 2.0 * n_h2o,
            },
            NuclideComponent {
                nuclide_idx: 2,
                atom_density: n_h2o,
            },
        ],
    };
    let materials = vec![uo2, zirc, water];

    const T: f64 = 293.6;
    let half = 0.63;
    let surfaces = vec![
        SurfaceKind::ZCylinder(ZCylinder {
            x0: 0.0,
            y0: 0.0,
            r: 0.39,
            bc: BoundaryType::Transmissive,
        }),
        SurfaceKind::ZCylinder(ZCylinder {
            x0: 0.0,
            y0: 0.0,
            r: 0.40,
            bc: BoundaryType::Transmissive,
        }),
        SurfaceKind::ZCylinder(ZCylinder {
            x0: 0.0,
            y0: 0.0,
            r: 0.46,
            bc: BoundaryType::Transmissive,
        }),
        SurfaceKind::XPlane(XPlane {
            x0: -half,
            bc: BoundaryType::Reflective,
        }),
        SurfaceKind::XPlane(XPlane {
            x0: half,
            bc: BoundaryType::Reflective,
        }),
        SurfaceKind::YPlane(YPlane {
            y0: -half,
            bc: BoundaryType::Reflective,
        }),
        SurfaceKind::YPlane(YPlane {
            y0: half,
            bc: BoundaryType::Reflective,
        }),
    ];
    let fuel = Cell::material(
        1,
        vec![RegionToken::HalfSpace {
            surface_idx: 0,
            sense: HalfSpaceSense::Inside,
        }],
        0,
        T,
    );
    let gap = Cell {
        id: 2,
        region: vec![
            RegionToken::HalfSpace {
                surface_idx: 0,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::HalfSpace {
                surface_idx: 1,
                sense: HalfSpaceSense::Inside,
            },
            RegionToken::Intersection,
        ],
        fill: outram_mc_libs::geometry::cell::CellFill::Void,
        temperature: T,
        translation: Position::ZERO,
    };
    let clad = Cell::material(
        3,
        vec![
            RegionToken::HalfSpace {
                surface_idx: 1,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::HalfSpace {
                surface_idx: 2,
                sense: HalfSpaceSense::Inside,
            },
            RegionToken::Intersection,
        ],
        1,
        T,
    );
    let water_cell = Cell::material(
        4,
        vec![
            RegionToken::HalfSpace {
                surface_idx: 2,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::HalfSpace {
                surface_idx: 3,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 4,
                sense: HalfSpaceSense::Inside,
            },
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 5,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 6,
                sense: HalfSpaceSense::Inside,
            },
            RegionToken::Intersection,
        ],
        2,
        T,
    );
    let geom = Geometry {
        surfaces,
        cells: vec![fuel, gap, clad, water_cell],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0, 1, 2, 3],
        }],
        lattices: vec![],
        root_universe: 0,
    };
    let source = SourceBox {
        lower: Position::new(-0.39, -0.39, -1.0),
        upper: Position::new(0.39, 0.39, 1.0),
    };

    Ok(BuiltCase::Csg {
        geom,
        materials,
        nuclides,
        source,
    })
}

/// Doubly-heterogeneous randomly-packed fissile-kernel medium in a reflective
/// 1cm cube, mirroring `crates/outram-mc-libs/examples/triso_delta_tracking.rs`.
/// `tmsr` selects the matrix: `false` = H-1 (plain pebble bed), `true` = a
/// FLiBe-like Li-7/Be-9/F-19 salt (TMSR-like pebble bed).
fn build_pebble_bed(tmsr: bool) -> Result<BuiltCase, String> {
    let half = 0.5;
    let radius = 0.04;
    let pf = 0.30;
    let seed = 20240715; // packing RNG seed; independent of the run's own seed.

    let mut nuclides = vec![nuc("U234")?, nuc("U235")?, nuc("U238")?];
    let fuel_components = vec![
        NuclideComponent {
            nuclide_idx: 0,
            atom_density: 4.9184e-4,
        },
        NuclideComponent {
            nuclide_idx: 1,
            atom_density: 4.4994e-2,
        },
        NuclideComponent {
            nuclide_idx: 2,
            atom_density: 2.4984e-3,
        },
    ];

    let (matrix_name, matrix_components) = if tmsr {
        nuclides.push(nuc("Li7")?);
        nuclides.push(nuc("Be9")?);
        nuclides.push(nuc("F19")?);
        // Representative FLiBe (Li2BeF4), rho=1.94 g/cm3, M=98.89 g/mol formula
        // unit, Li-7 only (natural Li is ~92.5% Li-7; Li-6's much larger
        // thermal absorption is neglected in this simplification — see the
        // preset blurb / README). N_formula = rho*N_A/M = 0.0118143 /barn-cm.
        let n_formula = 0.0118143;
        (
            "FLiBe-like salt (Li-7 only)",
            vec![
                NuclideComponent {
                    nuclide_idx: 3,
                    atom_density: 2.0 * n_formula,
                },
                NuclideComponent {
                    nuclide_idx: 4,
                    atom_density: 1.0 * n_formula,
                },
                NuclideComponent {
                    nuclide_idx: 5,
                    atom_density: 4.0 * n_formula,
                },
            ],
        )
    } else {
        nuclides.push(nuc("H1")?);
        (
            "H matrix",
            vec![NuclideComponent {
                nuclide_idx: 3,
                atom_density: 4.0e-2,
            }],
        )
    };

    let materials = vec![
        Material {
            id: 1,
            name: "HEU kernel".into(),
            temperature: 293.6,
            components: fuel_components,
        },
        Material {
            id: 2,
            name: matrix_name.into(),
            temperature: 293.6,
            components: matrix_components,
        },
    ];

    let packed = PackedSpheres::pack(radius, half, pf, seed)
        .map_err(|e| format!("RSA sphere packing failed at pf={pf}: {e}"))?;
    let majorant = Majorant::bounding(&materials, &nuclides, 1.0e-4, 2.0e7, 1024, 16, 0.1);

    Ok(BuiltCase::Delta {
        half_width: half,
        materials,
        nuclides,
        majorant,
        packed,
        fuel_material_idx: 0,
        matrix_material_idx: 1,
    })
}
