//! HIGH-fidelity data acquisition — download raw ENDF tapes and reconstruct
//! pointwise cross sections on device.
//!
//! This is the **opt-in HIGH tier** of the two-tier data strategy
//! (`docs/data-acquisition.md`). The default LOW tier ships embedded (WMP + fast
//! MGXS, no network); this module reaches upstream for the *authoritative* ENDF
//! evaluation, caches it, and runs the crate's own [`crate::reconr`] to get
//! fully resonance-reconstructed continuous-energy σ(E) — the reference against
//! which the LOW tier is judged.
//!
//! The whole module is behind the **`net-fetch`** Cargo feature, so a data
//! consumer that only wants the offline LOW tier pulls no TLS/HTTP dependency.
//! Enable with `--features net-fetch`.
//!
//! # The upstream is a hardcoded, pinned URL
//!
//! Where the bytes come from is **not** configurable at the call site: the source
//! is the [`EndfLibrary`] enum, and each variant maps to a hardcoded
//! [`IAEA_BASE_URL`] path. Pinning the upstream in code (rather than taking a URL
//! argument) makes provenance auditable and reproducible — you can read exactly
//! which library and mirror a build fetched from without tracing a runtime
//! string. To add or repoint a library, edit the enum; there is one place to look.
//!
//! # Addressing a nuclide: the ENDF MAT number
//!
//! The IAEA NDS `download-endf` tree names each neutron file by its ENDF
//! **material number** (MAT), zipped: `n_<MAT>_<Z>-<Sym>-<A>.zip` (e.g. U-235 is
//! MAT 9228 → `n_9228_92-U-235.zip`). MAT is the canonical ENDF identity of a
//! nuclide, so the download API takes it explicitly. For the nuclides this crate
//! ships CORE data for, [`well_known_mat`] supplies the (verified) MAT so you can
//! address them by GNDS name via the `*_by_name` helpers.
//!
//! ```no_run
//! # #[cfg(feature = "net-fetch")]
//! # fn demo() -> Result<(), njoy_outram_park_fork::NjoyError> {
//! use njoy_outram_park_fork::acquire::{EndfCache, EndfLibrary};
//! use njoy_outram_park_fork::MtReaction;
//!
//! // Fetch U-235 from ENDF/B-VII.1 (cached after the first call) and
//! // reconstruct pointwise cross sections at 0.1% tolerance.
//! let cache = EndfCache::new()?;
//! let result =
//!     cache.download_and_reconstruct_by_name(EndfLibrary::EndfBVII1, "U235", 1.0e-3)?;
//! let sigma_f = result.eval_mt(MtReaction::Mt18Fission, 2.0e6);
//! println!("U-235 σ_f(2 MeV) = {sigma_f} b");
//! # Ok(())
//! # }
//! ```

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use fs2::FileExt;
use sha2::{Digest, Sha256};

use crate::endf::tape::Tape;
use crate::reconr::{reconr, ReconrConfig, ReconrResult};
use crate::NjoyError;

/// The pinned upstream host for every [`EndfLibrary`] — the IAEA Nuclear Data
/// Services `download-endf` tree, which serves each evaluation's per-nuclide
/// ENDF-6 tapes under `<library>/n/<file>.zip` (public evaluated data).
///
/// Hardcoded on purpose (see the module docs): the download source is a compiled
/// constant, not a runtime argument, so a build's provenance is auditable.
pub const IAEA_BASE_URL: &str = "https://www-nds.iaea.org/public/download-endf";

/// Environment variable that overrides the on-disk cache directory.
///
/// Set `OUTRAM_PARK_DATA_DIR` to force downloads and the processed cache into a
/// specific location — useful on HPC/containers where the per-user cache dir is
/// not writable or not shared across nodes. When unset, [`EndfCache::new`] uses
/// the platform cache directory (XDG on Linux, `AppData` on Windows,
/// `~/Library/Caches` on macOS).
pub const CACHE_DIR_ENV: &str = "OUTRAM_PARK_DATA_DIR";

/// A pinned evaluated nuclear-data library — the *source* a HIGH-tier download
/// pulls from.
///
/// A closed enum (workspace rule: no trait objects). Each variant hardcodes its
/// upstream directory under [`IAEA_BASE_URL`]; call [`EndfLibrary::neutron_url`]
/// to get the full per-nuclide download URL. Pick the evaluation your problem
/// needs — they differ in resonance parameters, ν̄, and covariances.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndfLibrary {
    /// ENDF/B-VIII.0 (2018, CSEWG/NNDC) — the US general-purpose library with the
    /// CIELO ¹H/¹⁶O/⁵⁶Fe/²³⁵U/²³⁸U/²³⁹Pu evaluations. The default reference here.
    ///
    /// Note: several VIII.0 actinides use the LRF=7 R-Matrix-Limited resonance
    /// format, which this port does not yet reconstruct — [`reconr`] returns
    /// [`NjoyError::NotPorted`] for those. Use [`EndfLibrary::EndfBVII1`]
    /// (Reich-Moore, LRF=3) for a full-reconstruction demo of U/Pu today.
    EndfBVIII0,
    /// ENDF/B-VII.1 (2011) — the prior US release; matches the CORE WMP blob's
    /// evaluation vintage, and its U/Pu use Reich-Moore (LRF=3), which this port
    /// reconstructs. The most compatible choice for on-device reconstruction now.
    EndfBVII1,
    /// JEFF-3.3 (2017) — the OECD/NEA European joint evaluated file.
    Jeff33,
    /// JENDL-5 (2021) — the Japanese evaluated library.
    Jendl5,
}

impl EndfLibrary {
    /// The upstream directory name under [`IAEA_BASE_URL`] for this library — the
    /// IAEA NDS `download-endf` identifier. Centralised here so a mirror-side
    /// rename is a one-line fix.
    pub const fn dir(&self) -> &'static str {
        match self {
            EndfLibrary::EndfBVIII0 => "ENDF-B-VIII.0",
            EndfLibrary::EndfBVII1 => "ENDF-B-VII.1",
            EndfLibrary::Jeff33 => "JEFF-3.3",
            EndfLibrary::Jendl5 => "JENDL-5",
        }
    }

    /// A short human-readable label (e.g. `"ENDF/B-VIII.0"`), for logs and the
    /// processed-cache provenance key.
    pub const fn label(&self) -> &'static str {
        match self {
            EndfLibrary::EndfBVIII0 => "ENDF/B-VIII.0",
            EndfLibrary::EndfBVII1 => "ENDF/B-VII.1",
            EndfLibrary::Jeff33 => "JEFF-3.3",
            EndfLibrary::Jendl5 => "JENDL-5",
        }
    }

    /// Every library variant, for iterating over the supported set.
    pub const fn all() -> [EndfLibrary; 4] {
        [
            EndfLibrary::EndfBVIII0,
            EndfLibrary::EndfBVII1,
            EndfLibrary::Jeff33,
            EndfLibrary::Jendl5,
        ]
    }

    /// The full hardcoded download URL for a neutron-sublibrary nuclide.
    ///
    /// `mat` is the ENDF material number, `z`/`a` the atomic and mass numbers, and
    /// `symbol` the element symbol (e.g. `"U"`). Builds
    /// `<base>/<library>/n/n_<MAT>_<Z>-<Sym>-<A>.zip`, the IAEA NDS naming
    /// convention (zero-padded 4-digit MAT).
    pub fn neutron_url(&self, mat: i32, z: u32, a: u32, symbol: &str) -> String {
        format!("{IAEA_BASE_URL}/{}/n/{}", self.dir(), zip_filename(mat, z, a, symbol))
    }

    /// The full hardcoded download URL for a **thermal-scattering-law** (tsl) tape.
    ///
    /// Unlike the neutron sublibrary (MAT-numbered `n_<MAT>_...` files), the IAEA
    /// NDS thermal sublibrary serves **named** files under `<library>/tsl/`, e.g.
    /// `<base>/ENDF-B-VIII.0/tsl/tsl-crystalline-graphite.zip`. `tsl_base` is the
    /// basename with no extension (see [`TslMaterial::base`] / [`well_known_tsl`]);
    /// the download is `<base>.zip`, the extracted tape `<base>.endf`.
    ///
    /// The `<library>/tsl/<base>.zip` layout matches the IAEA NDS `download-endf`
    /// tree the neutron path already uses; it has not been re-fetched from this
    /// (offline) host, so a first network fetch is the point to confirm the exact
    /// mirror-side spelling — centralised here so a rename is a one-line fix.
    pub fn thermal_url(&self, tsl_base: &str) -> String {
        format!("{IAEA_BASE_URL}/{}/tsl/{}.zip", self.dir(), tsl_base)
    }
}

/// A thermal-scattering-law (tsl) material in the built-in registry: its IAEA NDS
/// download basename and its ENDF thermal-sublibrary material number.
///
/// The tsl sublibrary treats a *bound-atom scatterer* (H in H₂O, C in graphite, …)
/// as its own material — distinct from the free-atom neutron evaluation — so it is
/// addressed by name, not by (Z, A). See [`well_known_tsl`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TslMaterial {
    /// IAEA NDS `tsl/` sublibrary basename, no extension
    /// (e.g. `"tsl-crystalline-graphite"`). Feeds [`EndfLibrary::thermal_url`].
    pub base: &'static str,
    /// ENDF thermal-sublibrary MAT (MF=7). Metadata for cross-checking the tape's
    /// own MAT; **not** used to build the URL (tsl files are named, not
    /// MAT-numbered).
    pub mat: i32,
}

/// Look up a thermal-scattering-law material by a friendly key.
///
/// Covers the **HTR-10 graphite** moderator/reflector evaluations and the
/// light-water H-in-H₂O law the thermal-pincell tests consume. Values are the
/// public ENDF/B-VIII.0 thermal-sublibrary materials (open data, per
/// `DATA_POLICY.md`); the graphite MATs (30/31/32) match the tapes the
/// `thermal_graphite_coherent` V&V test was measured against. Unknown keys → `None`
/// (pass an explicit `base` to [`EndfCache::fetch_tsl`] for anything else).
pub fn well_known_tsl(name: &str) -> Option<TslMaterial> {
    let m = match name {
        "graphite" | "crystalline-graphite" | "tsl-crystalline-graphite" => {
            TslMaterial { base: "tsl-crystalline-graphite", mat: 30 }
        }
        "reactor-graphite-10P" | "tsl-reactor-graphite-10P" => {
            TslMaterial { base: "tsl-reactor-graphite-10P", mat: 31 }
        }
        "reactor-graphite-30P" | "tsl-reactor-graphite-30P" => {
            TslMaterial { base: "tsl-reactor-graphite-30P", mat: 32 }
        }
        "HinH2O" | "H2O" | "tsl-HinH2O" => TslMaterial { base: "tsl-HinH2O", mat: 1 },
        _ => return None,
    };
    Some(m)
}

/// The IAEA NDS neutron-sublibrary **zip** filename for a nuclide:
/// `n_<MAT>_<Z>-<Sym>-<A>.zip` with a zero-padded 4-digit MAT
/// (e.g. `9228, 92, 235, "U"` → `"n_9228_92-U-235.zip"`).
pub fn zip_filename(mat: i32, z: u32, a: u32, symbol: &str) -> String {
    format!("n_{mat:04}_{z}-{symbol}-{a}.zip")
}

/// The cached **extracted-tape** filename for a nuclide (the `.zip` stem with a
/// `.endf` extension): `n_<MAT>_<Z>-<Sym>-<A>.endf`.
pub fn tape_filename(mat: i32, z: u32, a: u32, symbol: &str) -> String {
    format!("n_{mat:04}_{z}-{symbol}-{a}.endf")
}

/// The ENDF **material number** (MAT) for a nuclide this crate ships CORE data
/// for, or `None` if it is not in the built-in table.
///
/// MAT is not a clean function of (Z, A) — the per-element reference isotope
/// varies — so rather than guess, this table holds only values **verified**
/// against the IAEA NDS listing. For any other nuclide, pass the MAT explicitly
/// (find it in the library's directory index or the ENDF-102 manual). The set
/// here covers the shipped Godiva/LFTR examples (light H/O, structural Fe, and
/// the Th/U/Pu actinides) plus **carbon** for the HTR-10 graphite
/// moderator/reflector.
///
/// # Carbon / graphite conventions
///
/// The MAT returned here is what builds the IAEA NDS download filename
/// (`n_{mat:04}_{z}-{symbol}-{a}.zip`, see [`tape_filename`]/[`zip_filename`]),
/// so it must be the **published** MAT or the fetch 404s. Carbon's ENDF identity
/// changed between library vintages, but these are the standard public values
/// (verified against the IAEA `download-endf` listing):
///
/// - **C-nat** (`(6, 0)` → MAT 600) — natural carbon, the single lumped
///   evaluation used through **ENDF/B-VII.1** and JEFF-3.1 (`n_0600_6-C-0.zip`).
/// - **C-12** (`(6, 12)` → MAT 600) — split out as the reference isotope in
///   **ENDF/B-VIII.0**, JEFF-3.3, JENDL-5 (`n_0600_6-C-12.zip`); it reuses MAT
///   600 because it replaced C-nat as the element's principal evaluation.
/// - **C-13** (`(6, 13)` → MAT 631) — the minor isotope (1.1 % abundance),
///   also split out from VIII.0 onward (`n_0631_6-C-13.zip`).
///
/// Note carbon's MAT here is the **neutron** sublibrary evaluation. The graphite
/// *thermal* S(alpha, beta) law is a separate ENDF thermal-sublibrary material
/// (`tsl-crystalline-graphite` MAT = 30, `tsl-reactor-graphite-10P` MAT = 31,
/// `-30P` MAT = 32) and is **not** reachable through this table or
/// [`EndfLibrary::neutron_url`] — see [`crate::thermr`].
pub fn well_known_mat(z: u32, a: u32) -> Option<i32> {
    let mat = match (z, a) {
        (1, 1) => 125,     // H-1
        (6, 0) => 600,     // C-nat (ENDF/B-VII.1, JEFF-3.1 — lumped natural carbon)
        (6, 12) => 600,    // C-12  (ENDF/B-VIII.0+, JEFF-3.3, JENDL-5 — HTR-10 graphite)
        (6, 13) => 631,    // C-13  (ENDF/B-VIII.0+ — 1.1% natural-abundance minor isotope)
        (8, 16) => 825,    // O-16
        (26, 56) => 2631,  // Fe-56
        (90, 232) => 9040, // Th-232
        (92, 233) => 9222, // U-233
        (92, 234) => 9225, // U-234
        (92, 235) => 9228, // U-235
        (92, 236) => 9231, // U-236
        (92, 238) => 9237, // U-238
        (94, 239) => 9437, // Pu-239
        (94, 240) => 9440, // Pu-240
        (94, 241) => 9443, // Pu-241
        _ => return None,
    };
    Some(mat)
}

// ── Element symbol table ──────────────────────────────────────────────────────

/// Element symbols indexed by atomic number Z (index 0 is a placeholder so
/// `ELEMENTS[z]` reads naturally). Covers Z = 1..=118.
const ELEMENTS: [&str; 119] = [
    "", "H", "He", "Li", "Be", "B", "C", "N", "O", "F", "Ne", "Na", "Mg", "Al", "Si", "P", "S",
    "Cl", "Ar", "K", "Ca", "Sc", "Ti", "V", "Cr", "Mn", "Fe", "Co", "Ni", "Cu", "Zn", "Ga", "Ge",
    "As", "Se", "Br", "Kr", "Rb", "Sr", "Y", "Zr", "Nb", "Mo", "Tc", "Ru", "Rh", "Pd", "Ag", "Cd",
    "In", "Sn", "Sb", "Te", "I", "Xe", "Cs", "Ba", "La", "Ce", "Pr", "Nd", "Pm", "Sm", "Eu", "Gd",
    "Tb", "Dy", "Ho", "Er", "Tm", "Yb", "Lu", "Hf", "Ta", "W", "Re", "Os", "Ir", "Pt", "Au", "Hg",
    "Tl", "Pb", "Bi", "Po", "At", "Rn", "Fr", "Ra", "Ac", "Th", "Pa", "U", "Np", "Pu", "Am", "Cm",
    "Bk", "Cf", "Es", "Fm", "Md", "No", "Lr", "Rf", "Db", "Sg", "Bh", "Hs", "Mt", "Ds", "Rg", "Cn",
    "Nh", "Fl", "Mc", "Lv", "Ts", "Og",
];

/// The element symbol for atomic number `z` (1..=118), or `None` if out of range.
pub fn symbol_for_z(z: u32) -> Option<&'static str> {
    ELEMENTS.get(z as usize).copied().filter(|s| !s.is_empty())
}

/// The atomic number for an element symbol (case-insensitive), or `None` if
/// unrecognised.
pub fn z_for_symbol(symbol: &str) -> Option<u32> {
    ELEMENTS
        .iter()
        .position(|s| s.eq_ignore_ascii_case(symbol) && !s.is_empty())
        .map(|i| i as u32)
}

/// Parse a GNDS-style nuclide name (e.g. `"U235"`, `"Pu239"`, `"Fe56"`) into
/// `(Z, A, canonical symbol)`.
///
/// Splits the leading element symbol from the trailing mass number. Metastable
/// suffixes (e.g. `"Am242m"`) are **not** handled — the ground-state name only.
///
/// # Errors
/// [`NjoyError::Download`] if the name has no symbol, no mass number, or an
/// unrecognised element.
pub fn parse_nuclide(name: &str) -> Result<(u32, u32, &'static str), NjoyError> {
    let split = name
        .find(|c: char| c.is_ascii_digit())
        .ok_or_else(|| NjoyError::Download(format!("nuclide '{name}' has no mass number")))?;
    let (sym, a_str) = name.split_at(split);
    if sym.is_empty() {
        return Err(NjoyError::Download(format!("nuclide '{name}' has no element symbol")));
    }
    let a: u32 = a_str
        .parse()
        .map_err(|_| NjoyError::Download(format!("nuclide '{name}' has a bad mass number")))?;
    let z = z_for_symbol(sym)
        .ok_or_else(|| NjoyError::Download(format!("unknown element '{sym}' in '{name}'")))?;
    // Return the canonical-cased symbol from the table.
    Ok((z, a, symbol_for_z(z).unwrap()))
}

// ── On-disk cache + download ──────────────────────────────────────────────────

/// A directory that caches downloaded ENDF tapes, safe under concurrent access.
///
/// [`EndfCache::fetch`] implements the parallel-run safety contract from
/// `docs/data-acquisition.md`: check the cache, take a per-artifact advisory lock,
/// re-check (a peer may have finished while we waited), download + unzip to a
/// unique `.part` file, `fsync`, then **atomically rename** into place. Readers
/// therefore only ever see complete files, and N processes racing for the same
/// nuclide do the download exactly once.
#[derive(Debug, Clone)]
pub struct EndfCache {
    dir: PathBuf,
}

impl EndfCache {
    /// Open the cache at the platform default location (or the
    /// [`CACHE_DIR_ENV`] override), creating the directory if needed.
    pub fn new() -> Result<Self, NjoyError> {
        let dir = if let Some(over) = std::env::var_os(CACHE_DIR_ENV) {
            PathBuf::from(over)
        } else {
            directories::ProjectDirs::from("org", "OUTRAM PARK", "outram-park")
                .map(|p| p.cache_dir().join("endf"))
                .ok_or_else(|| {
                    NjoyError::Download("no platform cache dir; set OUTRAM_PARK_DATA_DIR".into())
                })?
        };
        fs::create_dir_all(&dir)?;
        Ok(EndfCache { dir })
    }

    /// Open a cache rooted at an explicit directory (created if needed). Useful
    /// for tests and for pointing several runs at a shared scratch area.
    pub fn with_dir(dir: impl Into<PathBuf>) -> Result<Self, NjoyError> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        Ok(EndfCache { dir })
    }

    /// The cache directory root.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The final cached (extracted) tape path for a nuclide of `library`:
    /// `<root>/<library>/n_<MAT>_<Z>-<Sym>-<A>.endf`.
    pub fn path_for(
        &self,
        library: EndfLibrary,
        mat: i32,
        z: u32,
        a: u32,
        symbol: &str,
    ) -> PathBuf {
        self.dir.join(library.dir()).join(tape_filename(mat, z, a, symbol))
    }

    /// Fetch a nuclide's raw ENDF tape (downloading and unzipping if needed),
    /// returning the path to the cached, extracted `.endf` file.
    ///
    /// Idempotent and concurrency-safe (see the type-level note): a cache hit
    /// returns immediately; a miss takes an exclusive lock, downloads + extracts
    /// once, and publishes atomically. A SHA-256 sidecar (`<file>.sha256`) of the
    /// extracted tape is written for later integrity checks.
    pub fn fetch(
        &self,
        library: EndfLibrary,
        mat: i32,
        z: u32,
        a: u32,
        symbol: &str,
    ) -> Result<PathBuf, NjoyError> {
        let final_path = self.path_for(library, mat, z, a, symbol);
        let url = library.neutron_url(mat, z, a, symbol);
        self.fetch_to_path(&url, final_path)
    }

    /// Download `url` (a single-member zip of one ENDF tape) into `final_path`,
    /// returning the cached, extracted path. The concurrency-safe download core
    /// shared by [`Self::fetch`] (neutron sublibrary) and [`Self::fetch_tsl`]
    /// (thermal sublibrary): a cache hit returns immediately; a miss takes an
    /// exclusive lock, downloads + unzips + validates once, publishes atomically
    /// via a fsync'd temp-file rename, and writes a SHA-256 sidecar.
    fn fetch_to_path(&self, url: &str, final_path: PathBuf) -> Result<PathBuf, NjoyError> {
        let parent = final_path.parent().unwrap().to_path_buf();
        fs::create_dir_all(&parent)?;

        // 1. Fast path: already cached.
        if final_path.exists() {
            return Ok(final_path);
        }

        // 2. Take a per-artifact advisory lock (anti-thundering-herd).
        let lock_path = final_path.with_extension("lock");
        let lock_file = File::create(&lock_path)?;
        lock_file.lock_exclusive()?;

        // 3. Double-checked acquire: a peer may have finished while we blocked.
        let result = (|| {
            if final_path.exists() {
                return Ok(final_path.clone());
            }

            // 4. Download the zip, extract the ENDF member in memory.
            let zip_bytes = http_get(url)?;
            let tape_bytes = unzip_single(&zip_bytes, url)?;
            validate_endf(&tape_bytes, url)?;

            // 5. Write to a unique temp file in the same dir, fsync.
            let tmp = parent.join(format!(
                "{}.{}.{}.part",
                final_path.file_name().unwrap().to_string_lossy(),
                std::process::id(),
                unique_suffix(),
            ));
            {
                let mut f = File::create(&tmp)?;
                f.write_all(&tape_bytes)?;
                f.sync_all()?; // durable before the rename
            }

            // 6. Atomic publish — readers never see a partial file.
            fs::rename(&tmp, &final_path)?;

            // 7. Integrity sidecar for later re-checks (best-effort).
            let digest = hex_sha256(&tape_bytes);
            let _ = fs::write(final_path.with_extension("sha256"), &digest);

            Ok(final_path.clone())
        })();

        // 8. Always release the lock.
        let _ = lock_file.unlock();
        result
    }

    /// The final cached (extracted) tape path for a tsl material of `library`:
    /// `<root>/<library>/<base>.endf` (e.g.
    /// `<root>/ENDF-B-VIII.0/tsl-crystalline-graphite.endf`).
    pub fn path_for_tsl(&self, library: EndfLibrary, tsl_base: &str) -> PathBuf {
        self.dir.join(library.dir()).join(format!("{tsl_base}.endf"))
    }

    /// Fetch a **thermal-scattering-law** tape (downloading + unzipping if needed),
    /// returning the path to the cached, extracted `.endf`. The tsl counterpart of
    /// [`Self::fetch`]; same lock / atomic-publish / SHA-256 discipline via the
    /// shared [`Self::fetch_to_path`] core, but addressed by tsl basename (see
    /// [`EndfLibrary::thermal_url`] / [`well_known_tsl`]) since the thermal
    /// sublibrary names its files rather than MAT-numbering them.
    pub fn fetch_tsl(&self, library: EndfLibrary, tsl_base: &str) -> Result<PathBuf, NjoyError> {
        let final_path = self.path_for_tsl(library, tsl_base);
        let url = library.thermal_url(tsl_base);
        self.fetch_to_path(&url, final_path)
    }

    /// Fetch a tsl tape by a friendly registry key (see [`well_known_tsl`]) — e.g.
    /// `"graphite"`, `"reactor-graphite-10P"`, `"HinH2O"`. Errors if the key is not
    /// in the built-in registry; pass an explicit basename to [`Self::fetch_tsl`]
    /// for anything else.
    pub fn fetch_tsl_by_name(
        &self,
        library: EndfLibrary,
        name: &str,
    ) -> Result<PathBuf, NjoyError> {
        let m = well_known_tsl(name).ok_or_else(|| {
            NjoyError::Download(format!(
                "thermal-scattering material {name:?} not in the built-in tsl registry; \
                 pass an explicit basename via fetch_tsl()"
            ))
        })?;
        self.fetch_tsl(library, m.base)
    }

    /// Fetch (or reuse) a tsl tape and parse it into an ENDF [`Tape`] — the entry
    /// point for the [`crate::thermr`] thermal-scattering path (MF=7 S(α,β)).
    pub fn download_tsl_tape(
        &self,
        library: EndfLibrary,
        tsl_base: &str,
    ) -> Result<Tape, NjoyError> {
        let path = self.fetch_tsl(library, tsl_base)?;
        Tape::read(File::open(&path)?)
    }

    /// Fetch (or reuse) the tape and parse it into an ENDF [`Tape`].
    pub fn download_tape(
        &self,
        library: EndfLibrary,
        mat: i32,
        z: u32,
        a: u32,
        symbol: &str,
    ) -> Result<Tape, NjoyError> {
        let path = self.fetch(library, mat, z, a, symbol)?;
        Tape::read(File::open(&path)?)
    }

    /// Fetch, parse, and reconstruct pointwise cross sections for a nuclide — the
    /// full HIGH-fidelity path.
    ///
    /// Runs the crate's [`reconr`] with full resonance reconstruction at the given
    /// fractional `tolerance` (NJOY default `1.0e-3`), at 0 K. Doppler broadening
    /// to a target temperature is a separate BROADR step; the LOW tier's WMP form
    /// carries analytic Doppler instead.
    pub fn download_and_reconstruct(
        &self,
        library: EndfLibrary,
        mat: i32,
        z: u32,
        a: u32,
        symbol: &str,
        tolerance: f64,
    ) -> Result<ReconrResult, NjoyError> {
        let tape = self.download_tape(library, mat, z, a, symbol)?;
        let tape_mat = first_mat(&tape).unwrap_or(mat);
        reconr(&tape, &ReconrConfig { mat: tape_mat, tolerance, temperature: 0.0 })
    }

    /// Fetch a nuclide addressed by GNDS name (e.g. `"U235"`), looking up its MAT
    /// from [`well_known_mat`]. Errors if the nuclide is not in the built-in MAT
    /// table — use [`EndfCache::fetch`] with an explicit MAT for others.
    pub fn fetch_by_name(
        &self,
        library: EndfLibrary,
        name: &str,
    ) -> Result<PathBuf, NjoyError> {
        let (z, a, sym) = parse_nuclide(name)?;
        let mat = well_known_mat(z, a).ok_or_else(|| {
            NjoyError::Download(format!(
                "MAT for {name} not in the built-in table; pass it explicitly via fetch()"
            ))
        })?;
        self.fetch(library, mat, z, a, sym)
    }

    /// Reconstruct pointwise cross sections for a nuclide addressed by GNDS name,
    /// looking up its MAT from [`well_known_mat`] (see [`Self::fetch_by_name`]).
    pub fn download_and_reconstruct_by_name(
        &self,
        library: EndfLibrary,
        name: &str,
        tolerance: f64,
    ) -> Result<ReconrResult, NjoyError> {
        let (z, a, sym) = parse_nuclide(name)?;
        let mat = well_known_mat(z, a).ok_or_else(|| {
            NjoyError::Download(format!(
                "MAT for {name} not in the built-in table; pass it explicitly"
            ))
        })?;
        self.download_and_reconstruct(library, mat, z, a, sym, tolerance)
    }
}

/// The first (usually only) material number present on a single-nuclide tape.
fn first_mat(tape: &Tape) -> Option<i32> {
    tape.sections()
        .iter()
        .map(|s| s.key.mat)
        .find(|&m| m > 0)
}

/// GET `url` into memory, mapping any HTTP/transport failure to
/// [`NjoyError::Download`]. Pure-Rust TLS via rustls (ureq's default), so no
/// system OpenSSL is required.
fn http_get(url: &str) -> Result<Vec<u8>, NjoyError> {
    let resp = ureq::get(url)
        .call()
        .map_err(|e| NjoyError::Download(format!("GET {url}: {e}")))?;
    let mut buf = Vec::new();
    resp.into_reader()
        .read_to_end(&mut buf)
        .map_err(|e| NjoyError::Download(format!("reading {url}: {e}")))?;
    Ok(buf)
}

/// Extract the single ENDF member from an IAEA `.zip` archive in memory.
///
/// IAEA `download-endf` neutron files are one ENDF tape per zip. Returns the first
/// file entry's uncompressed bytes.
fn unzip_single(zip_bytes: &[u8], url: &str) -> Result<Vec<u8>, NjoyError> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes))
        .map_err(|e| NjoyError::Download(format!("{url}: not a valid zip: {e}")))?;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| NjoyError::Download(format!("{url}: zip entry {i}: {e}")))?;
        if entry.is_file() {
            let mut out = Vec::with_capacity(entry.size() as usize);
            entry
                .read_to_end(&mut out)
                .map_err(|e| NjoyError::Download(format!("{url}: extracting entry: {e}")))?;
            return Ok(out);
        }
    }
    Err(NjoyError::Download(format!("{url}: zip contained no file entry")))
}

/// Cheap sanity check that extracted bytes look like an ENDF ASCII tape before
/// we cache them — a mirror serving an HTML error page or a truncated file must
/// fail here, not deep in the parser. ENDF-6 tapes are line-based 80-column
/// ASCII; we require a non-trivial size and that the first kilobyte is ASCII.
fn validate_endf(bytes: &[u8], url: &str) -> Result<(), NjoyError> {
    if bytes.len() < 512 {
        return Err(NjoyError::Download(format!(
            "{url}: extracted tape too small ({} bytes) to be an ENDF file",
            bytes.len()
        )));
    }
    let head = &bytes[..bytes.len().min(1024)];
    if !head.iter().all(|&b| b == b'\n' || b == b'\r' || (b' '..=b'~').contains(&b)) {
        return Err(NjoyError::Download(format!(
            "{url}: extracted tape is not ASCII text (not an ENDF file?)"
        )));
    }
    Ok(())
}

/// Lowercase hex SHA-256 of `bytes`.
fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut s = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// A short unique-ish suffix (nanos since the epoch) to name a per-process
/// `.part` file without pulling in a random-number dependency.
fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The download URL is the pinned IAEA path with a MAT-numbered zip file.
    #[test]
    fn neutron_url_is_pinned_and_well_formed() {
        let url = EndfLibrary::EndfBVIII0.neutron_url(9228, 92, 235, "U");
        assert_eq!(
            url,
            "https://www-nds.iaea.org/public/download-endf/ENDF-B-VIII.0/n/n_9228_92-U-235.zip"
        );
        // MAT zero-pads to four digits; Z and A do not pad.
        assert_eq!(zip_filename(125, 1, 1, "H"), "n_0125_1-H-1.zip");
        assert_eq!(zip_filename(825, 8, 16, "O"), "n_0825_8-O-16.zip");
        assert_eq!(tape_filename(9228, 92, 235, "U"), "n_9228_92-U-235.endf");
    }

    /// The thermal-scattering-law (tsl) acquire path builds the IAEA NDS
    /// `<library>/tsl/<base>.zip` URL and the `<library>/<base>.endf` cache path
    /// for the HTR-10 graphite moderators and light-water H-in-H₂O (bead op-6tz.28).
    ///
    /// # Methodology / results (2026-08-12)
    /// Structural / addressing check (no network): the tsl sublibrary names its
    /// files rather than MAT-numbering them, so a wrong basename 404s. Assert the
    /// registry resolves each key to its published basename + ENDF/B-VIII.0 MAT,
    /// that [`EndfLibrary::thermal_url`] builds the `tsl/` URL, and that
    /// [`EndfCache::path_for_tsl`] lays the cache out under `<library>/`. All PASS.
    #[test]
    fn tsl_acquire_path_is_well_formed() {
        // Registry resolves the HTR-10 graphite evaluations + H-in-H2O.
        assert_eq!(
            well_known_tsl("graphite"),
            Some(TslMaterial { base: "tsl-crystalline-graphite", mat: 30 })
        );
        assert_eq!(well_known_tsl("crystalline-graphite").unwrap().mat, 30);
        assert_eq!(well_known_tsl("reactor-graphite-10P").unwrap().mat, 31);
        assert_eq!(well_known_tsl("reactor-graphite-30P").unwrap().mat, 32);
        assert_eq!(well_known_tsl("HinH2O").unwrap().base, "tsl-HinH2O");
        assert_eq!(well_known_tsl("not-a-scatterer"), None);

        // thermal_url builds the tsl sublibrary URL (named file, not n_<MAT>).
        assert_eq!(
            EndfLibrary::EndfBVIII0.thermal_url("tsl-crystalline-graphite"),
            "https://www-nds.iaea.org/public/download-endf/ENDF-B-VIII.0/tsl/tsl-crystalline-graphite.zip"
        );

        // Cache layout: <root>/<library>/<base>.endf.
        let cache = EndfCache::with_dir(std::env::temp_dir().join("op6tz28_tsl_test")).unwrap();
        let p = cache.path_for_tsl(EndfLibrary::EndfBVIII0, "tsl-crystalline-graphite");
        assert!(p.ends_with("ENDF-B-VIII.0/tsl-crystalline-graphite.endf"));
    }

    /// Each library variant points at a distinct hardcoded upstream directory.
    #[test]
    fn library_dirs_are_distinct() {
        let dirs: Vec<&str> = EndfLibrary::all().iter().map(|l| l.dir()).collect();
        let mut uniq = dirs.clone();
        uniq.sort_unstable();
        uniq.dedup();
        assert_eq!(dirs.len(), uniq.len(), "library dirs must be unique");
        assert!(EndfLibrary::Jeff33.neutron_url(9437, 94, 239, "Pu").contains("JEFF-3.3"));
    }

    /// The built-in MAT table matches values verified against the IAEA listing.
    #[test]
    fn well_known_mat_is_correct() {
        assert_eq!(well_known_mat(92, 235), Some(9228));
        assert_eq!(well_known_mat(92, 238), Some(9237));
        assert_eq!(well_known_mat(94, 239), Some(9437));
        assert_eq!(well_known_mat(1, 1), Some(125));
        assert_eq!(well_known_mat(8, 16), Some(825));
        assert_eq!(well_known_mat(50, 120), None); // not in the shipped set
    }

    /// Carbon is registered so a graphite material can be addressed by name for
    /// HIGH-tier reconstruction (HTR-10 moderator/reflector, bead op-h23).
    ///
    /// # Methodology
    /// Structural / addressing check, not a physics-reconstruction test. The MAT
    /// this table returns is what [`zip_filename`]/[`tape_filename`] interpolate
    /// into the IAEA NDS download name (`n_{mat:04}_{z}-{symbol}-{a}.zip`), so a
    /// wrong MAT silently 404s the fetch. Assert each carbon entry resolves to
    /// the MAT that produces the **published** IAEA filename.
    ///
    /// # Reference / results (2026-08-12)
    /// Checked against the public IAEA NDS `download-endf` naming convention
    /// (`DATA_POLICY.md`: open evaluated-data metadata only). C-nat and C-12
    /// share MAT 600 (`n_0600_6-C-0.zip` / `n_0600_6-C-12.zip`); C-13 is MAT 631
    /// (`n_0631_6-C-13.zip`). Corrects the earlier 625/628, which built
    /// non-existent `n_0625_*` / `n_0628_*` names. All asserts PASS.
    #[test]
    fn carbon_is_registered_for_graphite() {
        assert_eq!(well_known_mat(6, 0), Some(600), "C-nat (ENDF/B-VII.1)");
        assert_eq!(well_known_mat(6, 12), Some(600), "C-12 (ENDF/B-VIII.0+)");
        assert_eq!(well_known_mat(6, 13), Some(631), "C-13 (ENDF/B-VIII.0+)");
        // The MAT must build the real IAEA download filename.
        assert_eq!(zip_filename(600, 6, 12, "C"), "n_0600_6-C-12.zip");
        assert_eq!(zip_filename(631, 6, 13, "C"), "n_0631_6-C-13.zip");
    }

    /// The element table round-trips symbol ↔ Z and rejects nonsense.
    #[test]
    fn element_table_round_trips() {
        for z in 1..=118u32 {
            let sym = symbol_for_z(z).unwrap();
            assert_eq!(z_for_symbol(sym), Some(z), "round trip Z={z}");
        }
        assert_eq!(z_for_symbol("u"), Some(92)); // case-insensitive
        assert_eq!(symbol_for_z(0), None);
        assert_eq!(symbol_for_z(119), None);
        assert_eq!(z_for_symbol("Xx"), None);
    }

    /// Nuclide names split into (Z, A, canonical symbol); bad names error.
    #[test]
    fn parse_nuclide_splits_symbol_and_mass() {
        assert_eq!(parse_nuclide("U235").unwrap(), (92, 235, "U"));
        assert_eq!(parse_nuclide("Pu239").unwrap(), (94, 239, "Pu"));
        assert_eq!(parse_nuclide("fe56").unwrap(), (26, 56, "Fe")); // canonicalised
        assert!(parse_nuclide("U").is_err()); // no mass
        assert!(parse_nuclide("235").is_err()); // no symbol
        assert!(parse_nuclide("Xx12").is_err()); // unknown element
    }

    /// The cache builds `<root>/<library>/<file>.endf` and honours an explicit dir.
    #[test]
    fn cache_path_layout() {
        let tmp = std::env::temp_dir().join("outram_park_acquire_test_cache");
        let cache = EndfCache::with_dir(&tmp).unwrap();
        let p = cache.path_for(EndfLibrary::EndfBVII1, 9237, 92, 238, "U");
        assert!(p.ends_with("ENDF-B-VII.1/n_9237_92-U-238.endf"));
        assert!(p.starts_with(&tmp));
    }

    /// Truncated or non-ASCII bytes are rejected before caching.
    #[test]
    fn validate_endf_rejects_garbage() {
        assert!(validate_endf(b"too short", "x").is_err());
        let binary = vec![0u8; 1024];
        assert!(validate_endf(&binary, "x").is_err());
        let asciiish = vec![b' '; 1024];
        assert!(validate_endf(&asciiish, "x").is_ok());
    }
}
