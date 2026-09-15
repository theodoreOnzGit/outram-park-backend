//! Depletion chain — decay constants, branching, neutron-reaction targets, and
//! fission yields, assembled into a burnup matrix [`DepletionMatrix`].
//!
//! # What this represents
//!
//! A [`DepletionChain`] is the graph of a nuclide inventory's evolution under
//! combined **radioactive decay** and **neutron-induced transmutation**. Each
//! tracked nuclide carries:
//!
//! * an optional half-life (`None` = stable) → decay constant
//!   `lambda = ln(2) / T_half` in `1/s`;
//! * its decay branches (daughter + branching ratio);
//! * its neutron reactions (`(n,gamma)`, fission, `(n,2n)`) and their targets;
//! * its per-fission product yields (for fissionable nuclides).
//!
//! [`DepletionChain::build_matrix`] combines that structure with one-group
//! [`ReactionRates`] (already flux-multiplied, `1/s`) to assemble the burnup
//! matrix `A` in `dN/dt = A N`, following the sign/index convention documented
//! on [`DepletionMatrix`]. This mirrors OpenMC's `openmc/deplete/chain.py`
//! (`Chain.form_matrix`), reduced to the closed reaction set this crate models.
//!
//! # Provenance
//!
//! * **Chain structure & algorithm:** OpenMC `openmc/deplete/chain.py`
//!   (`Chain`, `form_matrix`), MIT.
//! * **The `simple()` regression chain:** transcribed verbatim from OpenMC's
//!   `examples/pincell_depletion/chain_simple.xml`
//!   (`/home/teddy0/Documents/research/openmc/examples/pincell_depletion/chain_simple.xml`),
//!   MIT — the 9-nuclide chain the `depletion.ipynb` notebook uses. Half-lives
//!   in seconds; fission yields dimensionless per fission at thermal
//!   (0.0253 eV).
//! * **Decay data (live):** `openmc-endf-8-depletion-lib-a` (Z <= 47) and
//!   `openmc-endf-8-depletion-lib-b` (Z >= 48), which package OpenMC's
//!   ENDF/B-VIII decay chain (`chain_endfb80_*.xml`) as Rust getters. Consumed
//!   here for the I-135 / Xe-135 half-lives via [`DepletionChain::simple_from_data`].
//! * **Fission yields (live):** `fission-yields-data` v0.1.4 (ENDF/B-VIII.0
//!   parent-independent yields). Consumed via the raw `.value` f64 field to
//!   avoid a `uom` major-version clash (this crate is on `uom` 0.38;
//!   `fission-yields-data` is on `uom` 0.37 — its quantity types are never named
//!   here, only their `.value` read).
//!
//! This is an OUTRAM PARK translation for education / research / V&V, **not**
//! the official OpenMC software, and not for reactor operation.
//!
//! # Known limitations of the consumed data crates (documented, not worked around)
//!
//! 1. **Neutron-reaction targets are unreadable from the decay libraries.** On
//!    `SerdeNuclideData` the `reaction` field (the `<reaction type="(n,gamma)"
//!    target=...>` entries) is **private** — only `name`, `half_life_seconds`,
//!    `decay_energy_electronvolt`, and `raw_decay_data` (the decay branches) are
//!    public. So neutron-reaction *targets* cannot be pulled from these libs;
//!    they come from the hardcoded `chain_simple.xml` transcription instead.
//!    Only decay constants / decay branches are cross-checkable against the libs.
//! 2. **U-235 thermal yields are not in `fission-yields-data` 0.1.4's public
//!    API.** The per-nuclide accessors (`u235_thermal_fission_yield`, …) live in
//!    `pub(crate)` modules and are not re-exported by the crate `prelude`; the
//!    only publicly reachable thermal accessor is `u232_thermal_fission_yield`,
//!    and the general `fission_yield_linear_interpolation` needs a `uom`-0.37
//!    `Energy` argument this crate cannot construct without the version clash.
//!    So the U-234/235/238 yields used by `build_matrix` are the
//!    `chain_simple.xml`-transcribed values (cross-checked against the XML in
//!    the tests), and the *live* fission-yield consumption is demonstrated with
//!    the reachable `u232_thermal_fission_yield` accessor.

use std::collections::HashMap;
use std::f64::consts::LN_2;

use super::matrix::DepletionMatrix;
use super::ReactionRates;

/// A neutron-reaction channel type in the depletion chain.
///
/// A closed set (enum dispatch, per the workspace design rules — no trait
/// objects). Each variant maps to the corresponding [`super::MicroRate`] field
/// that [`DepletionChain::build_matrix`] reads:
///
/// * [`ReactionKind::Gamma`] ↔ `MicroRate::gamma` — radiative capture `(n,gamma)`.
/// * [`ReactionKind::Fission`] ↔ `MicroRate::fission` — fission (products from
///   the nuclide's fission yields, not a single target).
/// * [`ReactionKind::TwoN`] ↔ `MicroRate::n2n` — `(n,2n)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReactionKind {
    /// Radiative capture `(n,gamma)`: one atom absorbed, capture daughter produced.
    Gamma,
    /// Fission: the nuclide is destroyed and fission products are produced
    /// according to its [`NuclideData`] fission yields.
    Fission,
    /// `(n,2n)`: one atom removed, the `A-1` daughter produced.
    TwoN,
}

/// A single radioactive-decay branch: which daughter, and with what probability.
#[derive(Debug, Clone, PartialEq)]
pub struct DecayBranch {
    /// Daughter nuclide name (e.g. `"Xe135"`). If it is not a tracked nuclide,
    /// the branch contributes only removal (the daughter leaves the chain).
    pub target: String,
    /// Branching ratio (dimensionless, `0..=1`) — the fraction of decays of the
    /// parent that follow this branch.
    pub branching: f64,
}

/// A single neutron-reaction channel: its kind and (for non-fission channels)
/// the single daughter it produces.
#[derive(Debug, Clone, PartialEq)]
pub struct NeutronReaction {
    /// Which reaction channel this is (selects the [`super::MicroRate`] field).
    pub kind: ReactionKind,
    /// The daughter nuclide for [`ReactionKind::Gamma`] / [`ReactionKind::TwoN`].
    ///
    /// `None` means "no in-chain daughter" — either the reaction is fission
    /// (products come from the fission yields, so this is `None`) or the target
    /// leaves the tracked set (`chain_simple.xml`'s `target="Nothing"` for the
    /// Gd-157 `(n,gamma)` burnable-poison sink), i.e. a pure removal with no
    /// production term.
    pub target: Option<String>,
}

/// All chain data for one tracked nuclide.
///
/// Fields are public for read access (the struct is a plain data record); build
/// instances through [`DepletionChain::simple`] / [`DepletionChain::simple_from_data`]
/// rather than by hand so the internal name→index map stays consistent.
#[derive(Debug, Clone, PartialEq)]
pub struct NuclideData {
    /// Nuclide name in GND form (`"I135"`, `"Xe135"`, `"U235"`, …). This is the
    /// key used everywhere: matrix rows, [`ReactionRates`] lookups, indexing.
    pub name: String,
    /// Half-life in **seconds**, or `None` if the nuclide is treated as stable.
    /// The decay constant is `lambda = ln(2) / half_life_seconds` (`1/s`).
    pub half_life_seconds: Option<f64>,
    /// Radioactive-decay branches out of this nuclide.
    pub decays: Vec<DecayBranch>,
    /// Neutron-reaction channels this nuclide undergoes under irradiation. Only
    /// the channels listed here are read from [`ReactionRates`]; a nuclide with
    /// no reactions is inert under flux (only its decay, if any, applies).
    pub reactions: Vec<NeutronReaction>,
    /// Per-fission product yields (dimensionless atoms produced per fission),
    /// keyed by product name. Empty unless the nuclide has a
    /// [`ReactionKind::Fission`] channel.
    pub fission_yields: Vec<(String, f64)>,
    /// Fission Q-value in **eV** (energy released per fission), transcribed from
    /// `chain_simple.xml` for provenance. Not used by [`DepletionChain::build_matrix`]
    /// (which assembles number-density rates, not energy), but kept so the chain
    /// records what the reference specified. `None` for non-fissionable nuclides.
    pub fission_q_ev: Option<f64>,
}

/// A decay + transmutation chain that assembles a burnup matrix.
///
/// Holds the tracked nuclides in a fixed order (the matrix-row order) plus a
/// name→index map for O(1) lookup. Construct with [`DepletionChain::simple`]
/// (fast, hardcoded `chain_simple.xml` transcription) or
/// [`DepletionChain::simple_from_data`] (pulls the fission-product half-lives
/// live from the ENDF/B-VIII decay libraries and cross-checks them).
#[derive(Debug, Clone)]
pub struct DepletionChain {
    /// Tracked nuclides, in matrix-row order.
    nuclides: Vec<NuclideData>,
    /// Name → row index, kept in sync with `nuclides`.
    index: HashMap<String, usize>,
}

/// Decay constant `lambda = ln(2) / T_half` in `1/s`, or `0.0` for a stable
/// nuclide (`half_life = None` or a non-positive half-life).
fn decay_constant(half_life_seconds: Option<f64>) -> f64 {
    match half_life_seconds {
        Some(t) if t > 0.0 => LN_2 / t,
        _ => 0.0,
    }
}

impl DepletionChain {
    /// Build a chain from an explicit, ordered list of nuclide records.
    ///
    /// The order given becomes the matrix-row order. Panics if two records share
    /// a name (names must be unique to index the matrix unambiguously).
    fn from_nuclides(nuclides: Vec<NuclideData>) -> Self {
        let mut index = HashMap::with_capacity(nuclides.len());
        for (i, n) in nuclides.iter().enumerate() {
            let prev = index.insert(n.name.clone(), i);
            assert!(
                prev.is_none(),
                "duplicate nuclide name in chain: {}",
                n.name
            );
        }
        Self { nuclides, index }
    }

    /// The OpenMC `chain_simple.xml` regression chain — 9 nuclides, hardcoded.
    ///
    /// This is the exact chain the `pincell_depletion/depletion.ipynb` notebook
    /// uses, transcribed verbatim from
    /// `openmc/examples/pincell_depletion/chain_simple.xml` (MIT). Row order:
    /// `I135, Xe135, Xe136, Cs135, Gd157, Gd156, U234, U235, U238`.
    ///
    /// Half-lives (s): I135 = 2.36520e4, Xe135 = 3.29040e4; the rest stable.
    /// Decays: I135 →(beta) Xe135, Xe135 →(beta) Cs135, both branching 1.0.
    /// Reactions: I135 & Xe135 have `(n,gamma)` → Xe136; Gd156 `(n,gamma)` →
    /// Gd157; Gd157 `(n,gamma)` → "Nothing" (out-of-chain sink); U234/235/238
    /// fission with the thermal yields tabulated in the XML.
    ///
    /// Use this for notebook reproduction where the reference chain must match
    /// bit-for-bit. For a version that also *demonstrates* consuming the
    /// external decay-data crates, see [`DepletionChain::simple_from_data`].
    pub fn simple() -> Self {
        Self::simple_with_half_lives(2.36520e4, 3.29040e4)
    }

    /// Same 9-nuclide chain as [`DepletionChain::simple`], but with the I-135 and
    /// Xe-135 half-lives passed in — the internal builder both constructors share.
    fn simple_with_half_lives(i135_half_life_s: f64, xe135_half_life_s: f64) -> Self {
        // Fission-product order in the chain_simple.xml <products> lists.
        let fp = |gd157: f64, gd156: f64, i135: f64, xe135: f64, xe136: f64, cs135: f64| {
            vec![
                ("Gd157".to_string(), gd157),
                ("Gd156".to_string(), gd156),
                ("I135".to_string(), i135),
                ("Xe135".to_string(), xe135),
                ("Xe136".to_string(), xe136),
                ("Cs135".to_string(), cs135),
            ]
        };
        let gamma_to = |target: &str| NeutronReaction {
            kind: ReactionKind::Gamma,
            target: Some(target.to_string()),
        };

        let nuclides = vec![
            // I135: beta -> Xe135 (b=1.0); (n,gamma) -> Xe136.
            NuclideData {
                name: "I135".to_string(),
                half_life_seconds: Some(i135_half_life_s),
                decays: vec![DecayBranch {
                    target: "Xe135".to_string(),
                    branching: 1.0,
                }],
                reactions: vec![gamma_to("Xe136")],
                fission_yields: Vec::new(),
                fission_q_ev: None,
            },
            // Xe135: beta -> Cs135 (b=1.0); (n,gamma) -> Xe136.
            NuclideData {
                name: "Xe135".to_string(),
                half_life_seconds: Some(xe135_half_life_s),
                decays: vec![DecayBranch {
                    target: "Cs135".to_string(),
                    branching: 1.0,
                }],
                reactions: vec![gamma_to("Xe136")],
                fission_yields: Vec::new(),
                fission_q_ev: None,
            },
            // Xe136: stable, no reactions.
            NuclideData {
                name: "Xe136".to_string(),
                half_life_seconds: None,
                decays: Vec::new(),
                reactions: Vec::new(),
                fission_yields: Vec::new(),
                fission_q_ev: None,
            },
            // Cs135: stable, no reactions.
            NuclideData {
                name: "Cs135".to_string(),
                half_life_seconds: None,
                decays: Vec::new(),
                reactions: Vec::new(),
                fission_yields: Vec::new(),
                fission_q_ev: None,
            },
            // Gd157: (n,gamma) -> "Nothing" (out-of-chain sink -> pure removal).
            NuclideData {
                name: "Gd157".to_string(),
                half_life_seconds: None,
                decays: Vec::new(),
                reactions: vec![NeutronReaction {
                    kind: ReactionKind::Gamma,
                    target: None,
                }],
                fission_yields: Vec::new(),
                fission_q_ev: None,
            },
            // Gd156: (n,gamma) -> Gd157.
            NuclideData {
                name: "Gd156".to_string(),
                half_life_seconds: None,
                decays: Vec::new(),
                reactions: vec![gamma_to("Gd157")],
                fission_yields: Vec::new(),
                fission_q_ev: None,
            },
            // U234: fission, Q = 191840000 eV, thermal yields.
            NuclideData {
                name: "U234".to_string(),
                half_life_seconds: None,
                decays: Vec::new(),
                reactions: vec![NeutronReaction {
                    kind: ReactionKind::Fission,
                    target: None,
                }],
                fission_yields: fp(
                    1.093250e-04,
                    2.087260e-04,
                    2.780820e-02,
                    6.759540e-03,
                    2.392300e-02,
                    4.356330e-05,
                ),
                fission_q_ev: Some(191_840_000.0),
            },
            // U235: fission, Q = 193410000 eV, thermal yields.
            NuclideData {
                name: "U235".to_string(),
                half_life_seconds: None,
                decays: Vec::new(),
                reactions: vec![NeutronReaction {
                    kind: ReactionKind::Fission,
                    target: None,
                }],
                fission_yields: fp(
                    6.142710e-5,
                    1.483250e-04,
                    0.0292737,
                    0.002566345,
                    0.0219242,
                    4.9097e-6,
                ),
                fission_q_ev: Some(193_410_000.0),
            },
            // U238: fission, Q = 197790000 eV, thermal yields.
            NuclideData {
                name: "U238".to_string(),
                half_life_seconds: None,
                decays: Vec::new(),
                reactions: vec![NeutronReaction {
                    kind: ReactionKind::Fission,
                    target: None,
                }],
                fission_yields: fp(
                    4.141120e-04,
                    7.605360e-04,
                    0.0135457,
                    0.00026864,
                    0.0024432,
                    3.7100e-07,
                ),
                fission_q_ev: Some(197_790_000.0),
            },
        ];

        Self::from_nuclides(nuclides)
    }

    /// The `chain_simple.xml` chain, but with the I-135 and Xe-135 half-lives
    /// pulled **live** from the ENDF/B-VIII decay libraries, and a live read of
    /// the fission-yield data crate — proving the "consume open data, don't
    /// rebuild it" requirement rather than trusting the hardcoded table.
    ///
    /// # What it consumes
    ///
    /// * `openmc-endf-8-depletion-lib-b`: `iodine::get_iodine_xml_serde_data()`
    ///   and `xenon::get_xenon_xml_serde_data()` — the I-135 / Xe-135
    ///   `half_life_seconds`. These are cross-checked against the
    ///   `chain_simple.xml` values (2.36520e4 s, 3.29040e4 s) to a relative
    ///   tolerance of `1e-6`; a mismatch panics with a descriptive message.
    /// * `fission-yields-data`: `u232_thermal_fission_yield(Xe135).value` — read
    ///   to exercise the yield crate's public API (see the module-level note on
    ///   why the U-235 accessor is not publicly reachable in 0.1.4). The value is
    ///   validated as finite and non-negative.
    ///
    /// The neutron-reaction targets and the U-234/235/238 fission yields still
    /// come from the hardcoded `chain_simple.xml` transcription — see the
    /// module-level "Known limitations" note for why they cannot be read from
    /// the libs. Everything else is identical to [`DepletionChain::simple`].
    ///
    /// # Measured (2026-07-15, lib versions a=1.0.1 / b=1.0.0, yields=0.1.4)
    ///
    /// * I135 lib `half_life_seconds` = `23652.0` s — matches XML `2.36520e4` exactly.
    /// * Xe135 lib `half_life_seconds` = `32904.0` s — matches XML `3.29040e4` exactly.
    /// * `u232_thermal_fission_yield(Xe135).value` = `0.00645867` (finite, > 0).
    ///
    /// Note the decay libs' I-135 record actually splits its beta decay into
    /// `Xe135` (b = 0.8349109) and the metastable `Xe135_m1` (b = 0.1650891);
    /// `chain_simple.xml` lumps both into `Xe135` at b = 1.0, and this chain
    /// follows the notebook's lumped convention (Xe135_m1 is not tracked).
    pub fn simple_from_data() -> Self {
        let i135_half_life = half_life_from_decay_lib(
            openmc_endf_8_depletion_lib_b::decay_xml_info_serde::iodine::get_iodine_xml_serde_data(
            ),
            "I135",
        )
        .expect("I135 must be present in the ENDF/B-VIII iodine decay library");
        let xe135_half_life = half_life_from_decay_lib(
            openmc_endf_8_depletion_lib_b::decay_xml_info_serde::xenon::get_xenon_xml_serde_data(),
            "Xe135",
        )
        .expect("Xe135 must be present in the ENDF/B-VIII xenon decay library");

        // Cross-check the live decay-lib half-lives against chain_simple.xml.
        assert_close_rel(
            i135_half_life,
            2.36520e4,
            1e-6,
            "I135 half-life (decay lib vs chain_simple.xml)",
        );
        assert_close_rel(
            xe135_half_life,
            3.29040e4,
            1e-6,
            "Xe135 half-life (decay lib vs chain_simple.xml)",
        );

        // Live read of the fission-yield crate (see doc note re: U-235
        // unreachability): read via the raw `.value` f64 to avoid the uom clash.
        let xe135_nuc =
            fission_yields_data::prelude::parse_nuclide_allow_underscore_isomer("Xe135")
                .expect("\"Xe135\" must parse as a fission_yields_data::Nuclide");
        let u232_xe135_yield: f64 =
            fission_yields_data::prelude::u232_thermal_fission_yield(xe135_nuc).value;
        assert!(
            u232_xe135_yield.is_finite() && u232_xe135_yield >= 0.0,
            "fission-yields-data returned a non-physical U232->Xe135 thermal yield: {u232_xe135_yield}"
        );

        Self::simple_with_half_lives(i135_half_life, xe135_half_life)
    }

    /// Nuclide names in matrix-row order (the order of matrix rows/columns).
    pub fn nuclide_names(&self) -> Vec<&str> {
        self.nuclides.iter().map(|n| n.name.as_str()).collect()
    }

    /// Number of tracked nuclides — the burnup matrix order.
    pub fn len(&self) -> usize {
        self.nuclides.len()
    }

    /// Whether the chain tracks no nuclides.
    pub fn is_empty(&self) -> bool {
        self.nuclides.is_empty()
    }

    /// The matrix row/column index of `name`, or `None` if it is not tracked.
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.index.get(name).copied()
    }

    /// The decay constant `lambda = ln(2) / T_half` in `1/s` for `name`, or
    /// `None` if the nuclide is not tracked. A stable tracked nuclide returns
    /// `Some(0.0)`.
    pub fn decay_constant_of(&self, name: &str) -> Option<f64> {
        self.index_of(name)
            .map(|i| decay_constant(self.nuclides[i].half_life_seconds))
    }

    /// Assemble the burnup matrix `A` (`dN/dt = A N`, units `1/s`) from one-group
    /// [`ReactionRates`].
    ///
    /// # Convention (see [`DepletionMatrix`])
    ///
    /// `dN[row]/dt = sum_col A[row][col] N[col]`; off-diagonal `A[i][j] >= 0` is
    /// the production rate of `i` from `j`; diagonal `A[j][j] <= 0` is the total
    /// removal rate of `j`.
    ///
    /// # What it does, per nuclide `j` (column)
    ///
    /// * **Decay** (`lambda_j = ln2 / T_half`, `0` if stable): for each decay
    ///   branch to an in-chain target `t` with branching `b`,
    ///   `A[idx(t)][j] += b * lambda_j`; and once, `A[j][j] -= lambda_j` (total
    ///   decay removal, whether or not the daughters are tracked).
    /// * **Neutron reactions** — only the channels the chain declares for `j` are
    ///   read from `rates.rate_for(name_j)`:
    ///   * `(n,gamma)` (rate `= gamma`): `A[j][j] -= gamma`; if the gamma target
    ///     is in-chain, `A[idx(target)][j] += gamma`. An out-of-chain / `Nothing`
    ///     target is removal only (no production).
    ///   * fission (rate `= fission`): `A[j][j] -= fission`; for each in-chain
    ///     fission product `p` with yield `y`, `A[idx(p)][j] += fission * y`.
    ///   * `(n,2n)` (rate `= n2n`): `A[j][j] -= n2n`; if the target is in-chain,
    ///     `A[idx(target)][j] += n2n`.
    ///
    /// Rates are already flux-multiplied (`1/s`); the pure-decay case is
    /// `build_matrix(&ReactionRates::zero())`, exposed as [`DepletionChain::decay_matrix`].
    pub fn build_matrix(&self, rates: &ReactionRates) -> DepletionMatrix {
        let n = self.nuclides.len();
        let mut a = DepletionMatrix::zeros(n);

        for (j, nuc) in self.nuclides.iter().enumerate() {
            // --- Radioactive decay ---
            let lambda = decay_constant(nuc.half_life_seconds);
            if lambda != 0.0 {
                for branch in &nuc.decays {
                    if let Some(t) = self.index_of(&branch.target) {
                        a.add(t, j, branch.branching * lambda);
                    }
                }
                a.add(j, j, -lambda);
            }

            // --- Neutron reactions (only channels the chain declares for j) ---
            if nuc.reactions.is_empty() {
                continue;
            }
            let rate = rates.rate_for(&nuc.name);
            for reaction in &nuc.reactions {
                match reaction.kind {
                    ReactionKind::Gamma => {
                        let g = rate.gamma;
                        if g == 0.0 {
                            continue;
                        }
                        a.add(j, j, -g);
                        if let Some(target) = &reaction.target {
                            if let Some(t) = self.index_of(target) {
                                a.add(t, j, g);
                            }
                        }
                    }
                    ReactionKind::Fission => {
                        let f = rate.fission;
                        if f == 0.0 {
                            continue;
                        }
                        a.add(j, j, -f);
                        for (product, yld) in &nuc.fission_yields {
                            if let Some(p) = self.index_of(product) {
                                a.add(p, j, f * yld);
                            }
                        }
                    }
                    ReactionKind::TwoN => {
                        let r = rate.n2n;
                        if r == 0.0 {
                            continue;
                        }
                        a.add(j, j, -r);
                        if let Some(target) = &reaction.target {
                            if let Some(t) = self.index_of(target) {
                                a.add(t, j, r);
                            }
                        }
                    }
                }
            }
        }

        a
    }

    /// The pure-decay burnup matrix (no flux / no transmutation), i.e.
    /// `build_matrix(&ReactionRates::zero())`.
    ///
    /// Convenience for decay-only inventory evolution and for the atom-conservation
    /// checks the `cram` module relies on: for a decay chain whose daughters are
    /// all in-chain, every column sums to zero.
    pub fn decay_matrix(&self) -> DepletionMatrix {
        self.build_matrix(&ReactionRates::zero())
    }
}

/// Find `name` in a decay-library `SerdeNuclideVec` and return its half-life in
/// seconds (`None` if absent, or if the record has no half-life = stable).
///
/// This is the read side of "consume the decay libraries": the libs expose only
/// `name`, `half_life_seconds`, `decay_energy_electronvolt`, and the decay
/// branches publicly (neutron-reaction targets are private — see the module
/// note), which is exactly enough to source decay constants.
fn half_life_from_decay_lib(
    data: openmc_endf_8_depletion_lib_b::SerdeNuclideVec,
    name: &str,
) -> Option<f64> {
    data.nuclides
        .into_iter()
        .find(|nuc| nuc.name == name)
        .and_then(|nuc| nuc.half_life_seconds)
}

/// Panic unless `|a - b| <= tol * max(|a|, |b|, 1)`, with a descriptive message.
fn assert_close_rel(a: f64, b: f64, tol: f64, what: &str) {
    let scale = a.abs().max(b.abs()).max(1.0);
    assert!(
        (a - b).abs() <= tol * scale,
        "{what}: {a} vs {b} differ by more than relative tol {tol}"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::depletion::{MicroRate, ReactionRates};

    const EXPECTED_ORDER: [&str; 9] = [
        "I135", "Xe135", "Xe136", "Cs135", "Gd157", "Gd156", "U234", "U235", "U238",
    ];

    /// Test 1 — chain identity & indexing.
    ///
    /// Methodology: `simple()` must expose the 9 chain_simple.xml nuclides in a
    /// stable, documented row order, and `index_of` must round-trip against
    /// `nuclide_names`.
    /// Result (2026-07-15): 9 nuclides in order
    /// `[I135, Xe135, Xe136, Cs135, Gd157, Gd156, U234, U235, U238]`; every
    /// name round-trips; an unknown name returns `None`.
    #[test]
    fn simple_chain_has_nine_nuclides_in_stable_order() {
        let chain = DepletionChain::simple();
        assert_eq!(chain.len(), 9);
        assert!(!chain.is_empty());
        assert_eq!(chain.nuclide_names(), EXPECTED_ORDER.to_vec());
        for (i, name) in EXPECTED_ORDER.iter().enumerate() {
            assert_eq!(chain.index_of(name), Some(i), "index round-trip for {name}");
        }
        assert_eq!(chain.index_of("Pu239"), None);
    }

    /// Test 2 — decay constants from half-lives (and vs the live decay libs).
    ///
    /// Methodology: `lambda = ln(2) / T_half`. For I135 (T = 2.36520e4 s) the
    /// reference is `0.6931471805599453 / 2.36520e4`, for Xe135 (3.29040e4 s)
    /// `0.6931471805599453 / 3.29040e4`. Also assert `simple_from_data()` — which
    /// sources the half-lives from `openmc-endf-8-depletion-lib-b` — reproduces
    /// the same constants (its internal cross-check already panics on a mismatch
    /// beyond relative tol 1e-6).
    /// Result (2026-07-15): lambda(I135) = 2.9306071e-5 1/s,
    /// lambda(Xe135) = 2.1065742e-5 1/s; decay-lib half-lives 23652.0 s / 32904.0 s
    /// match chain_simple.xml exactly (rel. diff 0).
    #[test]
    fn decay_constants_match_half_lives() {
        let chain = DepletionChain::simple();
        let lam_i135 = LN_2 / 2.36520e4;
        let lam_xe135 = LN_2 / 3.29040e4;
        assert!((chain.decay_constant_of("I135").unwrap() - lam_i135).abs() < 1e-15);
        assert!((chain.decay_constant_of("Xe135").unwrap() - lam_xe135).abs() < 1e-15);
        // Sanity on the magnitudes quoted in the docs.
        assert!(
            (lam_i135 - 2.9306071e-5).abs() < 1e-11,
            "lambda I135 = {lam_i135}"
        );
        assert!(
            (lam_xe135 - 2.1065742e-5).abs() < 1e-11,
            "lambda Xe135 = {lam_xe135}"
        );
        // Stable nuclide -> zero decay constant.
        assert_eq!(chain.decay_constant_of("U235").unwrap(), 0.0);

        // Live decay-lib path: half-lives pulled from lib-b must reproduce the
        // same decay constants (constructor's internal assert guards tolerance).
        let from_data = DepletionChain::simple_from_data();
        assert!((from_data.decay_constant_of("I135").unwrap() - lam_i135).abs() < 1e-9);
        assert!((from_data.decay_constant_of("Xe135").unwrap() - lam_xe135).abs() < 1e-9);
    }

    /// Test 3 — pure-decay matrix conserves atoms and places the right rate.
    ///
    /// Methodology: for the pure-decay chain I135 -> Xe135 -> Cs135 (all tracked),
    /// each source column must sum to zero (atom conservation: what leaves the
    /// parent appears in the daughter), and the off-diagonal Xe135<-I135 entry
    /// must equal +lambda(I135).
    /// Result (2026-07-15): columns for I135 and Xe135 sum to 0 (|sum| < 1e-18);
    /// A[Xe135][I135] = +2.9306071e-5 1/s = +lambda(I135); A[Cs135][Xe135] =
    /// +lambda(Xe135); diagonals are the negated decay constants.
    #[test]
    fn decay_matrix_conserves_atoms() {
        let chain = DepletionChain::simple();
        let a = chain.decay_matrix();
        let idx = |name: &str| chain.index_of(name).unwrap();

        let lam_i135 = LN_2 / 2.36520e4;
        let lam_xe135 = LN_2 / 3.29040e4;

        // Column sums (over all rows) for the decaying source nuclides == 0.
        let col_sum = |col: usize| (0..a.order()).map(|row| a.get(row, col)).sum::<f64>();
        assert!(col_sum(idx("I135")).abs() < 1e-18, "I135 column sum");
        assert!(col_sum(idx("Xe135")).abs() < 1e-18, "Xe135 column sum");

        // Off-diagonal production terms and diagonals.
        assert!((a.get(idx("Xe135"), idx("I135")) - lam_i135).abs() < 1e-15);
        assert!((a.get(idx("I135"), idx("I135")) + lam_i135).abs() < 1e-15);
        assert!((a.get(idx("Cs135"), idx("Xe135")) - lam_xe135).abs() < 1e-15);
        assert!((a.get(idx("Xe135"), idx("Xe135")) + lam_xe135).abs() < 1e-15);

        // Stable nuclides contribute nothing in pure decay.
        assert_eq!(a.get(idx("U235"), idx("U235")), 0.0);
    }

    /// Test 4 — full burnup matrix with nonzero reaction rates.
    ///
    /// Methodology: set a fission rate on U235 and an `(n,gamma)` rate on Xe135,
    /// build the matrix, and verify each affected entry against the hand-derived
    /// value:
    /// * U235 diagonal = -(fission_rate)   [U235 has no decay];
    /// * Xe135 diagonal = -(gamma_rate + lambda(Xe135));
    /// * A[Xe135][U235] = fission_rate * yield(U235->Xe135) with the
    ///   chain_simple.xml yield 0.002566345;
    /// * A[Xe136][Xe135] = gamma_rate     [Xe135 (n,gamma) -> Xe136].
    /// Result (2026-07-15): with fission_rate = 1.5e-9 1/s, gamma_rate = 3.0e-6
    /// 1/s: U235 diag = -1.5e-9; Xe135 diag = -(3.0e-6 + 2.1065742e-5) =
    /// -2.4065742e-5; A[Xe135][U235] = 1.5e-9 * 0.002566345 = 3.8495175e-12;
    /// A[Xe136][Xe135] = 3.0e-6. All matched to < 1e-20 / rel-1e-12.
    #[test]
    fn build_matrix_with_reaction_rates() {
        let chain = DepletionChain::simple();
        let idx = |name: &str| chain.index_of(name).unwrap();

        let fission_rate = 1.5e-9_f64;
        let gamma_rate = 3.0e-6_f64;
        let mut rates = ReactionRates::zero();
        rates.flux = 1.0e14;
        rates.set(
            "U235",
            MicroRate {
                gamma: 0.0,
                fission: fission_rate,
                n2n: 0.0,
            },
        );
        rates.set(
            "Xe135",
            MicroRate {
                gamma: gamma_rate,
                fission: 0.0,
                n2n: 0.0,
            },
        );

        let a = chain.build_matrix(&rates);
        let lam_xe135 = LN_2 / 3.29040e4;
        let yield_u235_xe135 = 0.002566345;

        assert!((a.get(idx("U235"), idx("U235")) - (-fission_rate)).abs() < 1e-20);
        assert!(
            (a.get(idx("Xe135"), idx("Xe135")) - (-(gamma_rate + lam_xe135))).abs() < 1e-18,
            "Xe135 diagonal = {}",
            a.get(idx("Xe135"), idx("Xe135"))
        );
        let expect_xe_from_u = fission_rate * yield_u235_xe135;
        assert!(
            (a.get(idx("Xe135"), idx("U235")) - expect_xe_from_u).abs() < 1e-24,
            "A[Xe135][U235] = {}",
            a.get(idx("Xe135"), idx("U235"))
        );
        assert!((a.get(idx("Xe136"), idx("Xe135")) - gamma_rate).abs() < 1e-20);

        // U235 also feeds its other fission products (spot-check I135).
        let yield_u235_i135 = 0.0292737;
        assert!((a.get(idx("I135"), idx("U235")) - fission_rate * yield_u235_i135).abs() < 1e-22);
    }

    /// Test 5 — live consumption of `fission-yields-data`.
    ///
    /// Methodology: parse "Xe135" to a `fission_yields_data::Nuclide` and read
    /// `u232_thermal_fission_yield(Xe135).value` (the raw f64, avoiding the
    /// uom-0.37/0.38 clash). Assert it is finite and positive. The U-235 thermal
    /// accessor is *not* publicly reachable in 0.1.4 (see the module note), so the
    /// U-235 -> Xe135 chain yield used by `build_matrix` is the chain_simple.xml
    /// value 0.002566345, cross-checked here against the transcribed table.
    /// Result (2026-07-15, fission-yields-data 0.1.4):
    /// `u232_thermal_fission_yield(Xe135).value` = 0.00645867 (finite, > 0);
    /// chain_simple.xml U235->Xe135 thermal yield = 0.002566345 (the two are
    /// different fissioning parents — U232 vs U235 — so they are not expected to
    /// be equal; the U232 read only proves the crate is consumed live).
    #[test]
    fn consume_fission_yields_data() {
        let xe135 = fission_yields_data::prelude::parse_nuclide_allow_underscore_isomer("Xe135")
            .expect("Xe135 parses");
        let y: f64 = fission_yields_data::prelude::u232_thermal_fission_yield(xe135).value;
        assert!(y.is_finite() && y > 0.0, "U232->Xe135 thermal yield = {y}");
        assert!(
            (y - 0.00645867).abs() < 1e-8,
            "measured U232->Xe135 yield = {y}"
        );

        // The chain's own U235->Xe135 yield is the chain_simple.xml value.
        let chain = DepletionChain::simple();
        let u235 = &chain.nuclides[chain.index_of("U235").unwrap()];
        let (_, u235_xe135) = u235
            .fission_yields
            .iter()
            .find(|(p, _)| p == "Xe135")
            .expect("U235 has an Xe135 yield");
        assert!((u235_xe135 - 0.002566345).abs() < 1e-12);
    }

    /// `simple_from_data()` reproduces the same chain structure as `simple()`.
    ///
    /// Methodology: both constructors must produce identical nuclide ordering and
    /// (since the decay-lib half-lives match chain_simple.xml exactly) identical
    /// decay constants. Result (2026-07-15): identical order and decay constants.
    #[test]
    fn simple_from_data_matches_simple() {
        let a = DepletionChain::simple();
        let b = DepletionChain::simple_from_data();
        assert_eq!(a.nuclide_names(), b.nuclide_names());
        for name in EXPECTED_ORDER {
            assert!(
                (a.decay_constant_of(name).unwrap() - b.decay_constant_of(name).unwrap()).abs()
                    < 1e-12,
                "decay constant mismatch for {name}"
            );
        }
    }
}
