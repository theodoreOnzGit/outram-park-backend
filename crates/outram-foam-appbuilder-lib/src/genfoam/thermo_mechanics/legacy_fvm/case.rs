// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! Reader of a `legacyThermoMechanics` region case (gh:#1781, round 23): the `thermoMechanicalProperties`,
//! `fvSchemes`, `fvSolution`, `controlDict`, the `0/` fields and the `polyMesh` of one region, into the typed
//! inputs of [`LegacyFvmThermoMechanics`]. `std` only (it reads files).
//!
//! Every selection the port does not implement is REFUSED by name ([`AppBuilderError::UnsupportedScheme`] or
//! [`AppBuilderError::Case`]): extend the port, never the refusal. A silent fall-back to another scheme would
//! make a comparison with the real binary meaningless.

use alloc::sync::Arc;
use std::path::Path;

use outram_foam_basic_lib::io::dict::{FoamDict, FoamEntry, FoamFile, FoamValue};
use outram_foam_basic_lib::mesh::FvMesh;
use outram_foam_basic_lib::primitives::Vector3;

use super::{
    traction::GfTractionDisplacement, D2dt2Scheme, DispPatch, LegacyFvmSettings, LegacyFvmThermoMechanics, LinearSolverSettings, ScalarPatch, TStructSolver, ZoneCard,
};
use crate::error::AppBuilderError;
use crate::io::fv_schemes::GradScheme;
use crate::io::poly_mesh::{read_cell_zones, read_poly_mesh};
use crate::solvers::non_orthogonal::LaplacianCorrection;

/// What to do when the case selects `GAMG` for `TStruct` (the shipped ESFR case does).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TStructGamgPolicy {
    /// Refuse `GAMG` by name.
    Refuse,
    /// Run `PBiCG`/`DILU` at the entry's tolerances (the named deviation, [`TStructSolver::GamgSubstitutedByPbicgDilu`]).
    #[default]
    SubstitutePbicgDilu,
}

/// A region case read from disk.
#[derive(Debug, Clone)]
pub struct LegacyFvmCase {
    /// The region's mesh.
    pub mesh: Arc<FvMesh>,
    /// The solver settings.
    pub settings: LegacyFvmSettings,
    /// `cellZones` as `(name, cells)`.
    pub zones: Vec<(String, Vec<usize>)>,
    /// The `materials` cards in dictionary order.
    pub cards: Vec<ZoneCard>,
    /// `disp` patch conditions.
    pub disp_patches: Vec<DispPatch>,
    /// `fuelDisp` patch conditions.
    pub fuel_patches: Vec<ScalarPatch>,
    /// `CRDisp` patch conditions.
    pub cr_patches: Vec<ScalarPatch>,
    /// Initial `TStruct` per cell.
    pub t_struct0: Vec<f64>,
    /// `controlDict.deltaT`.
    pub delta_t: f64,
    /// `fvSchemes.d2dt2Schemes.default` (`steadyState` or `Euler`; gh:#1845).
    pub d2dt2: D2dt2Scheme,
}

fn err(root: &Path, msg: impl Into<String>) -> AppBuilderError {
    AppBuilderError::Case { case: root.to_path_buf().into(), msg: msg.into() }
}

fn unsupported(family: &'static str, scheme: String, reason: &'static str) -> AppBuilderError {
    AppBuilderError::UnsupportedScheme { family, scheme, reason }
}

fn strip(k: &str) -> &str {
    k.trim_matches('"')
}

/// The words of an entry (`Gauss linear corrected`), whatever token shape the parser gave it.
fn words(e: &FoamEntry) -> Vec<String> {
    fn val(v: &FoamValue, out: &mut Vec<String>) {
        match v {
            FoamValue::Word(w) | FoamValue::Str(w) => out.push(w.clone()),
            FoamValue::Scalar(x) => out.push(format!("{x}")),
            FoamValue::List(l) => l.iter().for_each(|x| val(x, out)),
            FoamValue::Dict(_) => {}
        }
    }
    let mut out = Vec::new();
    match e {
        FoamEntry::Word(w) | FoamEntry::Str(w) => out.push(w.clone()),
        FoamEntry::Scalar(x) => out.push(format!("{x}")),
        FoamEntry::Tokens(t) | FoamEntry::List(t) => t.iter().for_each(|x| val(x, &mut out)),
        FoamEntry::Dimensioned(d) => d.value.iter().for_each(|x| val(x, &mut out)),
        FoamEntry::SubDict(_) => {}
    }
    out
}

/// Every number of an entry, flattened (dimension sets included).
fn numbers(e: &FoamEntry) -> Vec<f64> {
    fn val(v: &FoamValue, out: &mut Vec<f64>) {
        match v {
            FoamValue::Scalar(x) => out.push(*x),
            FoamValue::List(l) => l.iter().for_each(|x| val(x, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    match e {
        FoamEntry::Scalar(x) => out.push(*x),
        FoamEntry::Tokens(t) | FoamEntry::List(t) => t.iter().for_each(|x| val(x, &mut out)),
        FoamEntry::Dimensioned(d) => d.value.iter().for_each(|x| val(x, &mut out)),
        _ => {}
    }
    out
}

/// `getOrDefault<dimensionedScalar>(key, default).value()`: the last number of `rho rho [dims] value;`.
fn dimensioned_value(d: &FoamDict, key: &str, default: f64) -> f64 {
    d.get(key).and_then(|e| numbers(e).last().copied()).unwrap_or(default)
}

fn switch(d: &FoamDict, key: &str, default: bool) -> Result<bool, String> {
    match d.get(key).map(words) {
        None => Ok(default),
        Some(w) => match w.first().map(String::as_str) {
            Some("on" | "yes" | "true" | "y" | "t") => Ok(true),
            Some("off" | "no" | "false" | "n" | "f" | "none") => Ok(false),
            other => Err(format!("`{key}`: not a Switch: {other:?}")),
        },
    }
}

fn read_dict(root: &Path, rel: &str) -> Result<FoamDict, AppBuilderError> {
    FoamFile::read(root.join(rel)).map(|f| f.dict).map_err(|e| err(root, format!("{rel}: {e}")))
}

/// `dict.lookup(name)` with OpenFOAM's pattern rule: an exact key, else the LAST matching pattern. Only `".*"` is
/// understood as a pattern; any other pattern key is ignored (the cases the port reads use nothing else).
fn lookup_pattern<'a>(d: &'a FoamDict, name: &str) -> Option<&'a FoamEntry> {
    if let Some(e) = d.iter().find(|(k, _)| strip(k) == name && !k.contains('*')).map(|(_, e)| e) {
        return Some(e);
    }
    d.iter().filter(|(k, _)| strip(k) == ".*").map(|(_, e)| e).last()
}

fn linear_solver(root: &Path, solvers: &FoamDict, name: &str) -> Result<(String, String, LinearSolverSettings), AppBuilderError> {
    let e = lookup_pattern(solvers, name).ok_or_else(|| err(root, format!("fvSolution.solvers: no entry for `{name}`")))?;
    let FoamEntry::SubDict(d) = e else {
        return Err(err(root, format!("fvSolution.solvers.{name}: not a dictionary")));
    };
    let solver = d.get("solver").map(words).and_then(|w| w.first().cloned()).unwrap_or_default();
    let precond = d.get("preconditioner").map(words).and_then(|w| w.first().cloned()).unwrap_or_default();
    let num = |k: &str, default: f64| d.get(k).and_then(|e| numbers(e).first().copied()).unwrap_or(default);
    let s = LinearSolverSettings {
        tolerance: num("tolerance", 1e-6),
        rel_tol: num("relTol", 0.0),
        max_iter: num("maxIter", 1000.0) as usize,
        min_iter: num("minIter", 0.0) as usize,
    };
    Ok((solver, precond, s))
}

/// A `fixedValue`/`zeroGradient` patch list from a field file (`boundaryField` keyed by patch name, or `".*"`).
fn patch_entries<'a>(
    root: &Path,
    bf: &'a FoamDict,
    mesh: &FvMesh,
    field: &str,
) -> Result<Vec<(&'a FoamDict, String)>, AppBuilderError> {
    mesh.patches
        .iter()
        .map(|p| {
            let e = lookup_pattern(bf, &p.name)
                .ok_or_else(|| err(root, format!("{field}: no boundaryField entry for patch `{}`", p.name)))?;
            let FoamEntry::SubDict(d) = e else {
                return Err(err(root, format!("{field}: patch `{}` is not a dictionary", p.name)));
            };
            let ty = d.get("type").map(words).and_then(|w| w.first().cloned()).unwrap_or_default();
            Ok((d, ty))
        })
        .collect()
}

fn uniform_numbers(root: &Path, d: &FoamDict, key: &str, field: &str) -> Result<Vec<f64>, AppBuilderError> {
    let e = d.get(key).ok_or_else(|| err(root, format!("{field}: missing `{key}`")))?;
    let w = words(e);
    if w.first().map(String::as_str) != Some("uniform") {
        return Err(err(root, format!("{field}: `{key}` must be `uniform` (non-uniform fields are not read)")));
    }
    Ok(numbers(e))
}

/// A `Field<Type>` entry (`uniform v` or `nonuniform List<Type> N (...)`) of `width` components, as `n` faces of
/// flattened numbers.
fn face_field(root: &Path, d: &FoamDict, key: &str, field: &str, n: usize, width: usize) -> Result<Vec<f64>, AppBuilderError> {
    let e = d.get(key).ok_or_else(|| err(root, format!("{field}: missing `{key}`")))?;
    let nums = numbers(e);
    match words(e).first().map(String::as_str) {
        Some("uniform") if nums.len() == width => Ok((0..n).flat_map(|_| nums.iter().copied()).collect()),
        Some("nonuniform") if nums.len() == n * width + 1 && nums[0] == n as f64 => Ok(nums[1..].to_vec()),
        _ => Err(err(root, format!("{field}: `{key}` must be `uniform` or a `nonuniform List` of {n} entries"))),
    }
}

impl LegacyFvmCase {
    /// Read the region `region` of the case at `root` (holding `0/`, `constant/`, `system/`), with the `polyMesh`
    /// as PLAIN files (gunzip a shipped mesh first).
    ///
    /// # Errors
    /// [`AppBuilderError::UnsupportedScheme`] / [`AppBuilderError::Case`] naming the first selection the port does
    /// not implement; I/O and parse errors.
    #[allow(clippy::too_many_lines)]
    pub fn read(root: &Path, region: &str, gamg: TStructGamgPolicy) -> Result<Self, AppBuilderError> {
        let poly = root.join(format!("constant/{region}/polyMesh"));
        let mesh = read_poly_mesh(&poly)?;
        let zones: Vec<(String, Vec<usize>)> = read_cell_zones(&poly)?.into_iter().map(|z| (z.name, z.cells)).collect();

        // thermoMechanicalProperties
        let tp = read_dict(root, &format!("constant/{region}/thermoMechanicalProperties"))?;
        let solve_displacement = switch(&tp, "mechanicsSolver", true).map_err(|m| err(root, m))?;
        let solve_thermo = switch(&tp, "thermalSolver", true).map_err(|m| err(root, m))?;
        let go = tp.get_dict("globalOptions").ok_or_else(|| err(root, "thermoMechanicalProperties: no globalOptions"))?;
        let dir = go.get("pinDirection").map(numbers).unwrap_or_default();
        if dir.len() != 3 {
            return Err(err(root, "globalOptions.pinDirection must be a vector"));
        }
        let plane_stress = match tp.get_dict("rheologyOption") {
            Some(r) => switch(r, "planeStress", false).map_err(|m| err(root, m))?,
            None => false,
        };
        if tp.get("rheologyOptions").is_some() {
            return Err(unsupported(
                "thermoMechanicalProperties",
                "rheologyOptions".to_string(),
                "upstream's GFTractionDisplacement then looks up the misspelled `rhelogyOptions` and aborts",
            ));
        }
        let mats = tp.get_dict("materials").ok_or_else(|| err(root, "thermoMechanicalProperties: no materials"))?;
        let mut cards = Vec::new();
        for (name, e) in mats.iter() {
            let FoamEntry::SubDict(d) = e else { continue };
            let mut c = ZoneCard::new(strip(name));
            c.rho = dimensioned_value(d, "rho", 1.0);
            c.e = dimensioned_value(d, "E", 1.0);
            c.nu = dimensioned_value(d, "nu", 1.0);
            c.cp = dimensioned_value(d, "Cp", 1.0);
            c.k = dimensioned_value(d, "k", 1.0);
            c.alpha = dimensioned_value(d, "alpha", 1.0);
            c.t_ref = dimensioned_value(d, "Tref", 0.0);
            c.t_fuel_ref = dimensioned_value(d, "TFuelRef", 0.0);
            c.alpha_fuel = dimensioned_value(d, "alphaFuel", 0.0);
            c.t_cr_ref = dimensioned_value(d, "TCRRef", 0.0);
            c.alpha_cr = dimensioned_value(d, "alphaCR", 0.0);
            cards.push(c);
        }

        // fvSchemes
        let fs = read_dict(root, &format!("system/{region}/fvSchemes"))?;
        let need = |sec: &str, key: &str| -> Option<Vec<String>> {
            let d = fs.get_dict(sec)?;
            let e = d.iter().find(|(k, _)| strip(k) == key).map(|(_, e)| e).or_else(|| d.get("default"))?;
            Some(words(e))
        };
        let grad_sec = fs.get_dict("gradSchemes").ok_or_else(|| err(root, "fvSchemes: no gradSchemes"))?;
        for k in ["grad(disp)", "grad(disp.component(0))", "grad(disp.component(1))", "grad(disp.component(2))", "grad(TStruct)"] {
            if grad_sec.get(k).is_some() {
                return Err(unsupported("gradSchemes", k.to_string(), "a per-field gradient entry is not read; use `default`"));
            }
        }
        let grad = match need("gradSchemes", "default").as_deref() {
            Some([a, b]) if a == "Gauss" && b == "linear" => GradScheme::GaussLinear,
            Some([a]) if a == "leastSquares" => GradScheme::LeastSquares,
            other => return Err(unsupported("gradSchemes", format!("{other:?}"), "only `Gauss linear` and `leastSquares`")),
        };
        let lap_of = |key: &str| -> Result<LaplacianCorrection, AppBuilderError> {
            match need("laplacianSchemes", key).as_deref() {
                Some([a, b, c]) if a == "Gauss" && b == "linear" && c == "corrected" => Ok(LaplacianCorrection::Corrected),
                Some([a, b, c]) if a == "Gauss" && b == "linear" && c == "uncorrected" => Ok(LaplacianCorrection::Uncorrected),
                Some([a, b, c]) if a == "Gauss" && b == "linear" && c == "orthogonal" => Ok(LaplacianCorrection::Orthogonal),
                other => Err(unsupported("laplacianSchemes", format!("{key}: {other:?}"), "only Gauss linear corrected|uncorrected|orthogonal")),
            }
        };
        let laplacian = lap_of("laplacian(DD,D)")?;
        if lap_of("laplacian(rhoK,TStruct)")? != laplacian {
            return Err(unsupported(
                "laplacianSchemes",
                "laplacian(DD,D) and laplacian(rhoK,TStruct) differ".to_string(),
                "one Laplacian scheme for both equations",
            ));
        }
        let expect = |sec: &'static str, key: &str, want: &[&str]| -> Result<(), AppBuilderError> {
            match need(sec, key) {
                Some(w) if w.iter().map(String::as_str).eq(want.iter().copied()) => Ok(()),
                other => Err(unsupported(sec, format!("{key}: {other:?}"), "a different scheme is not ported here")),
            }
        };
        expect("divSchemes", "div(fuelDisp)", &["Gauss", "upwind"])?;
        expect("divSchemes", "div(CRDisp)", &["Gauss", "upwind"])?;
        expect("divSchemes", "div(sigmaD)", &["Gauss", "linear"])?;
        expect("ddtSchemes", "default", &["Euler"])?;
        // ESI v2506 has `steadyState` and `Euler` d2dt2 schemes only: any other name (`backward` included) does not exist
        let d2dt2 = match need("d2dt2Schemes", "default").as_deref() {
            Some([a]) if a == "steadyState" => D2dt2Scheme::SteadyState,
            Some([a]) if a == "Euler" => D2dt2Scheme::Euler,
            other => {
                return Err(unsupported(
                    "d2dt2Schemes",
                    format!("default: {other:?}"),
                    "ESI v2506 has only steadyState and Euler d2dt2 schemes (there is no backward d2dt2)",
                ))
            }
        };
        expect("interpolationSchemes", "default", &["linear"])?;

        // fvSolution
        let fv = read_dict(root, &format!("system/{region}/fvSolution"))?;
        let solvers = fv.get_dict("solvers").ok_or_else(|| err(root, "fvSolution: no solvers"))?;
        let (sx, px, d_solver) = linear_solver(root, solvers, "dispx")?;
        for c in ["dispy", "dispz"] {
            let (s2, p2, set2) = linear_solver(root, solvers, c)?;
            if (s2.as_str(), p2.as_str(), set2) != (sx.as_str(), px.as_str(), d_solver) {
                return Err(unsupported("solvers", c.to_string(), "the three displacement components must share one entry"));
            }
        }
        if sx != "PCG" || px != "DIC" {
            return Err(unsupported("solvers", format!("disp: {sx}/{px}"), "only PCG with DIC for the displacement"));
        }
        let pbicg = |name: &str| -> Result<LinearSolverSettings, AppBuilderError> {
            let (s, p, set) = linear_solver(root, solvers, name)?;
            if s == "PBiCG" && p == "DILU" {
                Ok(set)
            } else {
                Err(unsupported("solvers", format!("{name}: {s}/{p}"), "only PBiCG with DILU for the expansion equations"))
            }
        };
        let fuel_solver = pbicg("fuelDisp")?;
        let cr_solver = pbicg("CRDisp")?;
        let (ts, tpc, t_solver) = linear_solver(root, solvers, "TStruct")?;
        let t_solver_kind = match (ts.as_str(), tpc.as_str(), gamg) {
            ("PBiCG", "DILU", _) => TStructSolver::PbicgDilu,
            ("GAMG", _, TStructGamgPolicy::SubstitutePbicgDilu) => TStructSolver::GamgSubstitutedByPbicgDilu,
            _ => {
                return Err(unsupported(
                    "solvers",
                    format!("TStruct: {ts}/{tpc}"),
                    "the TStruct matrix is asymmetric: PBiCG with DILU (or GAMG, substituted by it by name)",
                ))
            }
        };
        let sa = fv.get_dict("stressAnalysis").ok_or_else(|| err(root, "fvSolution: no stressAnalysis"))?;
        let compact_normal_stress = switch(sa, "compactNormalStress", false).map_err(|m| err(root, m))?;
        let n_correctors = sa.get("nCorrectors").and_then(|e| numbers(e).first().copied()).unwrap_or(1.0) as usize;
        let convergence_tolerance = sa
            .get("D")
            .and_then(|e| numbers(e).first().copied())
            .ok_or_else(|| err(root, "fvSolution.stressAnalysis: no D"))?;
        if let Some(r) = fv.get_dict("relaxationFactors") {
            let hits = ["disp", "dispx", "TStruct", "fuelDisp", "CRDisp"];
            let named = |sec: &str| r.get_dict(sec).map_or(false, |d| d.iter().any(|(k, _)| hits.contains(&strip(k))));
            if named("fields") || named("equations") || r.get_dict("equations").is_some() && r.get_dict("equations").map_or(false, |d| d.get("default").is_some()) {
                return Err(unsupported("relaxationFactors", "disp/TStruct".to_string(), "DEqn.relax() is not ported (the shipped `Disp` key does not match `disp`)"));
            }
        }

        // controlDict
        let cd = read_dict(root, "system/controlDict")?;
        let delta_t = cd.get("deltaT").and_then(|e| numbers(e).first().copied()).ok_or_else(|| err(root, "controlDict: no deltaT"))?;

        // 0/ fields
        let zero = format!("0/{region}");
        let disp_f = read_dict(root, &format!("{zero}/disp"))?;
        let internal = uniform_numbers(root, &disp_f, "internalField", "disp")?;
        if internal != [0.0, 0.0, 0.0] {
            return Err(err(root, "disp: only a zero uniform internalField is read"));
        }
        let bf = disp_f.get_dict("boundaryField").ok_or_else(|| err(root, "disp: no boundaryField"))?;
        let mut disp_patches = Vec::new();
        for (d, ty) in patch_entries(root, bf, &mesh, "disp")? {
            disp_patches.push(match ty.as_str() {
                "fixedValue" => {
                    let v = uniform_numbers(root, d, "value", "disp")?;
                    if v.len() != 3 {
                        return Err(err(root, "disp: fixedValue must be a uniform vector"));
                    }
                    DispPatch::FixedValue(Vector3::new(v[0], v[1], v[2]))
                }
                "zeroGradient" => DispPatch::ZeroGradient,
                "GFTractionDisplacement" => {
                    let n = mesh.patches[disp_patches.len()].size;
                    let t = face_field(root, d, "traction", "disp", n, 3)?;
                    let p = face_field(root, d, "pressure", "disp", n, 1)?;
                    let traction = t.chunks(3).map(|c| Vector3::new(c[0], c[1], c[2])).collect();
                    DispPatch::GfTractionDisplacement(GfTractionDisplacement::new(traction, p)?)
                }
                "gapContact" => {
                    return Err(unsupported(
                        "boundaryField",
                        "disp: gapContact".to_string(),
                        "needs the regionCoupled two-region coupling (gh:#1846); the arithmetic is traction::GapContact",
                    ))
                }
                other => {
                    return Err(unsupported(
                        "boundaryField",
                        format!("disp: {other}"),
                        "only fixedValue, zeroGradient and GFTractionDisplacement",
                    ))
                }
            });
        }
        let scalar_patches = |name: &str| -> Result<Vec<ScalarPatch>, AppBuilderError> {
            let f = read_dict(root, &format!("{zero}/{name}"))?;
            if uniform_numbers(root, &f, "internalField", name)? != [0.0] {
                return Err(err(root, format!("{name}: only a zero uniform internalField is read")));
            }
            let bf = f.get_dict("boundaryField").ok_or_else(|| err(root, format!("{name}: no boundaryField")))?;
            patch_entries(root, bf, &mesh, name)?
                .into_iter()
                .map(|(d, ty)| match ty.as_str() {
                    "fixedValue" => {
                        let v = uniform_numbers(root, d, "value", name)?;
                        Ok(ScalarPatch::FixedValue(*v.first().ok_or_else(|| err(root, format!("{name}: fixedValue without a value")))?))
                    }
                    "zeroGradient" => Ok(ScalarPatch::ZeroGradient),
                    other => Err(unsupported("boundaryField", format!("{name}: {other}"), "only fixedValue and zeroGradient")),
                })
                .collect()
        };
        let fuel_patches = scalar_patches("fuelDisp")?;
        let cr_patches = scalar_patches("CRDisp")?;
        let t_struct0 = match read_dict(root, &format!("{zero}/TStruct")) {
            Ok(f) => {
                let v = uniform_numbers(root, &f, "internalField", "TStruct")?;
                if let Some(bf) = f.get_dict("boundaryField") {
                    for (_, ty) in patch_entries(root, bf, &mesh, "TStruct")? {
                        if ty != "zeroGradient" {
                            return Err(unsupported("boundaryField", format!("TStruct: {ty}"), "only zeroGradient"));
                        }
                    }
                }
                vec![v.first().copied().unwrap_or(0.0); mesh.n_cells]
            }
            Err(_) => vec![0.0; mesh.n_cells],
        };
        let settings = LegacyFvmSettings {
            plane_stress,
            solve_displacement,
            solve_thermo,
            pin_direction: Vector3::new(dir[0], dir[1], dir[2]),
            n_correctors,
            convergence_tolerance,
            compact_normal_stress,
            grad,
            laplacian,
            d_solver,
            fuel_solver,
            cr_solver,
            t_solver,
            t_solver_kind,
            traction_bc_honours_plane_stress: false,
        };
        Ok(Self { mesh, settings, zones, cards, disp_patches, fuel_patches, cr_patches, t_struct0, delta_t, d2dt2 })
    }

    /// Build the solver from the case.
    ///
    /// # Errors
    /// As [`LegacyFvmThermoMechanics::new`].
    pub fn build(&self) -> Result<LegacyFvmThermoMechanics, AppBuilderError> {
        LegacyFvmThermoMechanics::new(
            self.mesh.clone(),
            self.settings.clone(),
            &self.zones,
            &self.cards,
            self.disp_patches.clone(),
            self.fuel_patches.clone(),
            self.cr_patches.clone(),
            self.t_struct0.clone(),
            self.delta_t,
        )
        .map(|s| s.with_d2dt2(self.d2dt2))
    }
}
