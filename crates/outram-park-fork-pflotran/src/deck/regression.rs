//! Reader for the subset of genuine PFLOTRAN regression decks that the
//! smallest RICHARDS decks use (gh:#1537, rows 3-5 of gh:#1422).
//!
//! `regression_tests/default/infiltration1d/infiltration1d.in` at PFLOTRAN
//! 669fbce is the target deck. Every card the deck uses is read; **any card or
//! block this reader does not understand is an error with its line number**
//! (the same policy as [`super::parse_pflotran_deck`]), so a successfully read
//! deck is exactly what the file said.
//!
//! # What is read
//!
//! | Block | Cards |
//! |---|---|
//! | `SIMULATION` | `SIMULATION_TYPE SUBSURFACE`, `PROCESS_MODELS` > `SUBSURFACE_FLOW <name>` > `MODE`. An `OPTIONS` sub-block is an error (it can change the physics: `STEADY_STATE`, `DISABLE_CAPILLARITY`) |
//! | `GRID` | `TYPE STRUCTURED`, `NXYZ`, `DXYZ` (with `n*value` repeats), `ORIGIN`, `BOUNDS` |
//! | `MATERIAL_PROPERTY` | `ID`, `POROSITY`, `TORTUOSITY`, `CHARACTERISTIC_CURVES`, `PERMEABILITY` > `PERM_ISO`, `PERM_X/Y/Z` |
//! | `CHARACTERISTIC_CURVES` | `SATURATION_FUNCTION LINEAR` / `VAN_GENUCHTEN`, `PERMEABILITY_FUNCTION BURDINE_LINEAR_LIQ`, `BURDINE_LINEAR_GAS`, `MUALEM_VG_LIQ` |
//! | `EOS WATER` | `DENSITY CONSTANT`, `ENTHALPY CONSTANT`, `VISCOSITY CONSTANT` |
//! | `OUTPUT` | `FORMAT <word...>`, `TIMES <unit> <values...>` |
//! | `TIME` | `FINAL_TIME`, `INITIAL_TIMESTEP_SIZE`, `MAXIMUM_TIMESTEP_SIZE` (value and unit) |
//! | `REGION` | `COORDINATES`, `FACE` |
//! | `FLOW_CONDITION` | `TYPE` > `LIQUID_PRESSURE` / `LIQUID_SATURATION` `DIRICHLET`, and the value |
//! | `INITIAL_CONDITION`, `BOUNDARY_CONDITION [name]` | `FLOW_CONDITION`, `REGION` |
//! | `STRATA` | `REGION`, `MATERIAL` |
//! | `REGRESSION` | `CELL_IDS`, `CELLS_PER_PROCESS` |
//! | `SKIP` / `NOSKIP` | tokens between them are skipped, as upstream does |
//!
//! Provenance of the card semantics: PFLOTRAN 669fbce `src/pflotran/` —
//! `characteristic_curves.F90` (block reading), `eos_water.F90`, `region.F90`,
//! `condition.F90`, `regression.F90`, `timestepper_base.F90` (`TIME` cards).
//! The deck text itself is PFLOTRAN's (LGPL-3.0); see
//! `verification_and_validation/upstream_differential_gh1422/regression_decks/PROVENANCE.md`.
//!
//! > Untrusted AI-generated draft until a human reviews it. The reader is
//! > exercised only by `infiltration1d.in` (tests in `tests/regression_decks_gh1537.rs`).

use super::{
    err_at, is_terminator, next_float, next_usize, parse_fortran_float, time_to_seconds, Cursor,
    FlowMode, Token,
};
use crate::error::PflotranError;

/// A structured grid with explicit cell widths (`DXYZ`).
#[derive(Debug, Clone, PartialEq)]
pub struct RegressionGrid {
    /// Cells along x.
    pub nx: usize,
    /// Cells along y.
    pub ny: usize,
    /// Cells along z.
    pub nz: usize,
    /// Cell widths along x (metres), length `nx`.
    pub dx: Vec<f64>,
    /// Cell widths along y (metres), length `ny`.
    pub dy: Vec<f64>,
    /// Cell widths along z (metres), length `nz`.
    pub dz: Vec<f64>,
    /// Origin `[x, y, z]` (metres); `ORIGIN` card, default zero.
    pub origin: [f64; 3],
}

/// A `MATERIAL_PROPERTY` block.
#[derive(Debug, Clone, PartialEq)]
pub struct RegressionMaterial {
    /// Name after `MATERIAL_PROPERTY`.
    pub name: String,
    /// `ID`.
    pub id: usize,
    /// `POROSITY`.
    pub porosity: f64,
    /// `TORTUOSITY` (read, not used by RICHARDS flow).
    pub tortuosity: Option<f64>,
    /// `CHARACTERISTIC_CURVES` name.
    pub characteristic_curves: String,
    /// Permeability `[kx, ky, kz]` in m^2 (`PERM_ISO` repeats into all three).
    pub permeability: [f64; 3],
}

/// A `SATURATION_FUNCTION` sub-block.
#[derive(Debug, Clone, PartialEq)]
pub enum RegressionSaturationFunction {
    /// `LINEAR`: `LIQUID_RESIDUAL_SATURATION`, `ALPHA`, `MAX_CAPILLARY_PRESSURE`.
    Linear {
        /// Residual liquid saturation.
        residual_saturation: f64,
        /// `ALPHA` (1/Pa).
        alpha: f64,
        /// `MAX_CAPILLARY_PRESSURE` (Pa).
        max_capillary_pressure: f64,
    },
    /// `VAN_GENUCHTEN`: `M`, `ALPHA`, `LIQUID_RESIDUAL_SATURATION`,
    /// `MAX_CAPILLARY_PRESSURE` (read only).
    VanGenuchten {
        /// `M`.
        m: f64,
        /// `ALPHA` (1/Pa).
        alpha: f64,
        /// Residual liquid saturation.
        residual_saturation: f64,
    },
}

/// A `PERMEABILITY_FUNCTION` sub-block.
#[derive(Debug, Clone, PartialEq)]
pub enum RegressionPermeabilityFunction {
    /// `BURDINE_LINEAR_LIQ`.
    BurdineLinearLiquid {
        /// `LIQUID_RESIDUAL_SATURATION`.
        residual_saturation: f64,
    },
    /// `BURDINE_LINEAR_GAS` (unused by RICHARDS, read so the deck is accepted).
    BurdineLinearGas {
        /// `LIQUID_RESIDUAL_SATURATION`.
        liquid_residual_saturation: f64,
        /// `GAS_RESIDUAL_SATURATION`.
        gas_residual_saturation: f64,
    },
    /// `MUALEM_VG_LIQ`.
    MualemVgLiquid {
        /// `M`.
        m: f64,
        /// `LIQUID_RESIDUAL_SATURATION`.
        residual_saturation: f64,
    },
}

/// A `CHARACTERISTIC_CURVES <name>` block.
#[derive(Debug, Clone, PartialEq)]
pub struct RegressionCurves {
    /// Name.
    pub name: String,
    /// The saturation function.
    pub saturation_function: RegressionSaturationFunction,
    /// The liquid relative-permeability function.
    pub liquid_permeability: RegressionPermeabilityFunction,
    /// The gas relative-permeability function, if given.
    pub gas_permeability: Option<RegressionPermeabilityFunction>,
}

/// The `EOS WATER` block: constant values only.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RegressionEosWater {
    /// `DENSITY CONSTANT` (kg/m^3).
    pub density: Option<f64>,
    /// `ENTHALPY CONSTANT` (MJ/kmol upstream; read, unused by RICHARDS).
    pub enthalpy: Option<f64>,
    /// `VISCOSITY CONSTANT` (Pa s).
    pub viscosity: Option<f64>,
}

/// A `FACE` of a region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegressionFace {
    /// `WEST` (x minimum).
    West,
    /// `EAST` (x maximum).
    East,
    /// `SOUTH` (y minimum).
    South,
    /// `NORTH` (y maximum).
    North,
    /// `BOTTOM` (z minimum).
    Bottom,
    /// `TOP` (z maximum).
    Top,
}

/// A `REGION` block.
#[derive(Debug, Clone, PartialEq)]
pub struct RegressionRegion {
    /// Name.
    pub name: String,
    /// `COORDINATES` lower and upper corner.
    pub coordinates: [[f64; 3]; 2],
    /// `FACE`, if given.
    pub face: Option<RegressionFace>,
}

/// What a `FLOW_CONDITION` prescribes (Dirichlet only).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RegressionFlowValue {
    /// `LIQUID_PRESSURE DIRICHLET` (Pa).
    LiquidPressure(f64),
    /// `LIQUID_SATURATION DIRICHLET` (converted to a pressure through the
    /// saturation function, `PatchUpdateCouplerSaturation`).
    LiquidSaturation(f64),
}

/// A `FLOW_CONDITION` block.
#[derive(Debug, Clone, PartialEq)]
pub struct RegressionFlowCondition {
    /// Name.
    pub name: String,
    /// The Dirichlet value.
    pub value: RegressionFlowValue,
}

/// A condition coupler: `INITIAL_CONDITION`, `BOUNDARY_CONDITION`.
#[derive(Debug, Clone, PartialEq)]
pub struct RegressionCoupler {
    /// Optional name.
    pub name: Option<String>,
    /// `FLOW_CONDITION` name.
    pub flow_condition: String,
    /// `REGION` name.
    pub region: String,
}

/// A `STRATA` coupler.
#[derive(Debug, Clone, PartialEq)]
pub struct RegressionStrata {
    /// `REGION` name.
    pub region: String,
    /// `MATERIAL` name.
    pub material: String,
}

/// The `TIME` block (seconds). Unset cards stay `None` so the stepper applies
/// upstream's defaults itself.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegressionTime {
    /// `FINAL_TIME` (s).
    pub final_time_s: f64,
    /// `INITIAL_TIMESTEP_SIZE` (s).
    pub initial_dt_s: Option<f64>,
    /// `MAXIMUM_TIMESTEP_SIZE` (s).
    pub max_dt_s: Option<f64>,
}

/// A genuine PFLOTRAN regression deck (the supported subset).
#[derive(Debug, Clone, PartialEq)]
pub struct RegressionDeck {
    /// `MODE`.
    pub mode: FlowMode,
    /// `REGRESSION` > `CELL_IDS` (1-based natural ids), the cells the gold file lists.
    pub regression_cell_ids: Vec<usize>,
    /// `GRID`.
    pub grid: RegressionGrid,
    /// `MATERIAL_PROPERTY` blocks.
    pub materials: Vec<RegressionMaterial>,
    /// `CHARACTERISTIC_CURVES` blocks.
    pub curves: Vec<RegressionCurves>,
    /// `EOS WATER`.
    pub eos_water: RegressionEosWater,
    /// `OUTPUT` > `TIMES` (s).
    pub output_times_s: Vec<f64>,
    /// `TIME`.
    pub time: RegressionTime,
    /// `REGION` blocks.
    pub regions: Vec<RegressionRegion>,
    /// `FLOW_CONDITION` blocks.
    pub flow_conditions: Vec<RegressionFlowCondition>,
    /// `INITIAL_CONDITION` couplers.
    pub initial_conditions: Vec<RegressionCoupler>,
    /// `BOUNDARY_CONDITION` couplers.
    pub boundary_conditions: Vec<RegressionCoupler>,
    /// `STRATA` couplers.
    pub strata: Vec<RegressionStrata>,
}

fn next_upper(cur: &mut Cursor, ctx: &str) -> Result<(String, Token), PflotranError> {
    let tok = cur.expect(ctx)?;
    Ok((tok.text.to_uppercase(), tok))
}

fn peek_text(cur: &Cursor) -> Option<String> {
    cur.tokens.get(cur.pos).map(|t| t.text.clone())
}

fn unsupported(tok: &Token, block: &str) -> PflotranError {
    err_at(
        tok.line,
        &format!("unsupported card `{}` in {block}", tok.text),
    )
}

/// A time value followed by its unit (`9.504d3 s`), in seconds.
fn next_time(cur: &mut Cursor, ctx: &str) -> Result<f64, PflotranError> {
    let v = next_float(cur, ctx)?;
    let unit = cur.expect(&format!("a time unit after {ctx}"))?;
    time_to_seconds(v, &unit.text).map_err(|_| err_at(unit.line, "unknown time unit"))
}

/// Expand `40*0.005` (repeat) or a plain number into the output vector.
fn push_repeat_value(out: &mut Vec<f64>, tok: &Token) -> Result<(), PflotranError> {
    match tok.text.split_once('*') {
        Some((n, v)) => {
            let n: usize = n
                .parse()
                .map_err(|_| err_at(tok.line, &format!("bad repeat count in `{}`", tok.text)))?;
            let v = parse_fortran_float(v)?;
            for _ in 0..n {
                out.push(v);
            }
        }
        None => out.push(parse_fortran_float(&tok.text)?),
    }
    Ok(())
}

/// Parse a genuine PFLOTRAN regression deck.
///
/// # Errors
///
/// [`PflotranError::InvalidInput`] with a line number for an unsupported or
/// malformed card, or for a missing required block (`SIMULATION`, `GRID`,
/// `TIME`).
pub fn parse_regression_deck(input: &str) -> Result<RegressionDeck, PflotranError> {
    let mut cur = Cursor::new(input);
    let mut mode: Option<FlowMode> = None;
    let mut cell_ids: Vec<usize> = Vec::new();
    let mut grid: Option<RegressionGrid> = None;
    let mut materials = Vec::new();
    let mut curves = Vec::new();
    let mut eos = RegressionEosWater::default();
    let mut output_times = Vec::new();
    let mut time: Option<RegressionTime> = None;
    let mut regions = Vec::new();
    let mut flow_conditions = Vec::new();
    let mut initial_conditions = Vec::new();
    let mut boundary_conditions = Vec::new();
    let mut strata = Vec::new();

    while let Some(tok) = cur.next() {
        match tok.text.to_uppercase().as_str() {
            "SIMULATION" => mode = Some(parse_simulation_strict(&mut cur)?),
            "SUBSURFACE" | "END_SUBSURFACE" => {}
            "SKIP" => skip_to_noskip(&mut cur)?,
            "NOSKIP" => {}
            "REGRESSION" => cell_ids = parse_regression(&mut cur)?,
            "GRID" => grid = Some(parse_grid_dxyz(&mut cur)?),
            "MATERIAL_PROPERTY" => materials.push(parse_material_full(&mut cur)?),
            "CHARACTERISTIC_CURVES" => curves.push(parse_curves_full(&mut cur)?),
            "EOS" => eos = parse_eos(&mut cur)?,
            "OUTPUT" => output_times = parse_output(&mut cur)?,
            "TIME" => time = Some(parse_time_full(&mut cur)?),
            "REGION" => regions.push(parse_region(&mut cur)?),
            "FLOW_CONDITION" => flow_conditions.push(parse_flow_condition(&mut cur)?),
            "INITIAL_CONDITION" => initial_conditions.push(parse_coupler(&mut cur, "INITIAL_CONDITION")?),
            "BOUNDARY_CONDITION" => {
                boundary_conditions.push(parse_coupler(&mut cur, "BOUNDARY_CONDITION")?)
            }
            "STRATA" => strata.push(parse_strata(&mut cur)?),
            other => {
                return Err(err_at(
                    tok.line,
                    &format!("unknown/unsupported top-level keyword `{other}`"),
                ))
            }
        }
    }
    Ok(RegressionDeck {
        mode: mode.ok_or_else(|| {
            PflotranError::InvalidInput("missing required SIMULATION block".to_string())
        })?,
        regression_cell_ids: cell_ids,
        grid: grid
            .ok_or_else(|| PflotranError::InvalidInput("missing required GRID block".to_string()))?,
        materials,
        curves,
        eos_water: eos,
        output_times_s: output_times,
        time: time
            .ok_or_else(|| PflotranError::InvalidInput("missing required TIME block".to_string()))?,
        regions,
        flow_conditions,
        initial_conditions,
        boundary_conditions,
        strata,
    })
}

/// `SKIP` ... `NOSKIP`: upstream (`input_aux.F90`) discards every line between
/// the two cards.
fn skip_to_noskip(cur: &mut Cursor) -> Result<(), PflotranError> {
    loop {
        let t = cur
            .next()
            .ok_or_else(|| PflotranError::InvalidInput("SKIP without NOSKIP".to_string()))?;
        if t.text.eq_ignore_ascii_case("NOSKIP") {
            return Ok(());
        }
    }
}

fn parse_simulation_strict(cur: &mut Cursor) -> Result<FlowMode, PflotranError> {
    let mut mode: Option<FlowMode> = None;
    loop {
        let (up, tok) = next_upper(cur, "a SIMULATION card or END")?;
        if up == "END" {
            break;
        }
        match up.as_str() {
            "SIMULATION_TYPE" => {
                let v = cur.expect("a simulation type")?;
                if !v.text.eq_ignore_ascii_case("SUBSURFACE") {
                    return Err(err_at(v.line, "only SIMULATION_TYPE SUBSURFACE is supported"));
                }
            }
            "PROCESS_MODELS" | "/" => {}
            "SUBSURFACE_FLOW" => {
                cur.expect("a process-model name")?;
            }
            "MODE" => {
                let v = cur.expect("a flow mode")?;
                mode = Some(super::flow_mode_from_str(&v.text));
            }
            _ => return Err(unsupported(&tok, "SIMULATION")),
        }
    }
    mode.ok_or_else(|| PflotranError::InvalidInput("SIMULATION block has no MODE card".into()))
}

fn parse_regression(cur: &mut Cursor) -> Result<Vec<usize>, PflotranError> {
    let mut ids = Vec::new();
    loop {
        let (up, tok) = next_upper(cur, "a REGRESSION card or END")?;
        if is_terminator(&tok.text) {
            break;
        }
        match up.as_str() {
            "CELL_IDS" => loop {
                let t = cur.expect("a cell id or `/`")?;
                if is_terminator(&t.text) {
                    break;
                }
                ids.push(
                    t.text
                        .parse::<usize>()
                        .map_err(|_| err_at(t.line, "expected a cell id"))?,
                );
            },
            "CELLS_PER_PROCESS" => {
                next_usize(cur, "CELLS_PER_PROCESS")?;
            }
            _ => return Err(unsupported(&tok, "REGRESSION")),
        }
    }
    Ok(ids)
}

fn parse_grid_dxyz(cur: &mut Cursor) -> Result<RegressionGrid, PflotranError> {
    let mut nxyz: Option<[usize; 3]> = None;
    let mut dxyz: Option<[Vec<f64>; 3]> = None;
    let mut bounds: Option<[[f64; 3]; 2]> = None;
    let mut origin = [0.0; 3];
    loop {
        let (up, tok) = next_upper(cur, "a GRID card or END")?;
        if is_terminator(&tok.text) {
            break;
        }
        match up.as_str() {
            "TYPE" => {
                let v = cur.expect("a grid type")?;
                if !v.text.eq_ignore_ascii_case("STRUCTURED") {
                    return Err(err_at(v.line, "only TYPE STRUCTURED is supported"));
                }
            }
            "ORIGIN" => {
                for o in origin.iter_mut() {
                    *o = next_float(cur, "ORIGIN")?;
                }
            }
            "NXYZ" => {
                nxyz = Some([
                    next_usize(cur, "NXYZ nx")?,
                    next_usize(cur, "NXYZ ny")?,
                    next_usize(cur, "NXYZ nz")?,
                ])
            }
            "DXYZ" => {
                let n = nxyz.ok_or_else(|| err_at(tok.line, "DXYZ before NXYZ"))?;
                let mut groups: [Vec<f64>; 3] = [Vec::new(), Vec::new(), Vec::new()];
                for (g, count) in groups.iter_mut().zip(n.iter()) {
                    while g.len() < *count {
                        let t = cur.expect("a DXYZ value")?;
                        push_repeat_value(g, &t)?;
                    }
                    if g.len() != *count {
                        return Err(err_at(tok.line, "DXYZ group length does not match NXYZ"));
                    }
                }
                let t = cur.expect("`/` to close DXYZ")?;
                if !is_terminator(&t.text) {
                    return Err(err_at(t.line, "expected `/` to close DXYZ"));
                }
                dxyz = Some(groups);
            }
            "BOUNDS" => {
                let mut b = [[0.0; 3]; 2];
                for row in b.iter_mut() {
                    for v in row.iter_mut() {
                        *v = next_float(cur, "BOUNDS")?;
                    }
                }
                let t = cur.expect("`/` to close BOUNDS")?;
                if !is_terminator(&t.text) {
                    return Err(err_at(t.line, "expected `/` to close BOUNDS"));
                }
                bounds = Some(b);
            }
            _ => return Err(unsupported(&tok, "GRID")),
        }
    }
    let n = nxyz.ok_or_else(|| PflotranError::InvalidInput("GRID missing NXYZ".into()))?;
    let [dx, dy, dz] = match (dxyz, bounds) {
        (Some(d), _) => d,
        (None, Some(b)) => {
            let w = |i: usize| vec![(b[1][i] - b[0][i]) / n[i] as f64; n[i]];
            [w(0), w(1), w(2)]
        }
        (None, None) => {
            return Err(PflotranError::InvalidInput(
                "GRID needs DXYZ or BOUNDS".into(),
            ))
        }
    };
    Ok(RegressionGrid {
        nx: n[0],
        ny: n[1],
        nz: n[2],
        dx,
        dy,
        dz,
        origin,
    })
}

fn parse_material_full(cur: &mut Cursor) -> Result<RegressionMaterial, PflotranError> {
    let name = cur.expect("a material name")?.text;
    let mut id = None;
    let mut porosity = None;
    let mut tortuosity = None;
    let mut cc = None;
    let mut perm = None;
    loop {
        let (up, tok) = next_upper(cur, "a MATERIAL_PROPERTY card or END")?;
        if is_terminator(&tok.text) {
            break;
        }
        match up.as_str() {
            "ID" => id = Some(next_usize(cur, "ID")?),
            "POROSITY" => porosity = Some(next_float(cur, "POROSITY")?),
            "TORTUOSITY" => tortuosity = Some(next_float(cur, "TORTUOSITY")?),
            "CHARACTERISTIC_CURVES" => cc = Some(cur.expect("a curves name")?.text),
            "PERMEABILITY" => {
                let mut k = [None; 3];
                loop {
                    let (u2, t2) = next_upper(cur, "a PERMEABILITY card or `/`")?;
                    if is_terminator(&t2.text) {
                        break;
                    }
                    match u2.as_str() {
                        "PERM_ISO" => {
                            let v = next_float(cur, "PERM_ISO")?;
                            k = [Some(v); 3];
                        }
                        "PERM_X" => k[0] = Some(next_float(cur, "PERM_X")?),
                        "PERM_Y" => k[1] = Some(next_float(cur, "PERM_Y")?),
                        "PERM_Z" => k[2] = Some(next_float(cur, "PERM_Z")?),
                        _ => return Err(unsupported(&t2, "PERMEABILITY")),
                    }
                }
                perm = match k {
                    [Some(a), Some(b), Some(c)] => Some([a, b, c]),
                    _ => return Err(err_at(tok.line, "PERMEABILITY needs PERM_ISO or all of PERM_X/Y/Z")),
                };
            }
            _ => return Err(unsupported(&tok, "MATERIAL_PROPERTY")),
        }
    }
    let miss = |what: &str| PflotranError::InvalidInput(format!("MATERIAL_PROPERTY `{name}` missing {what}"));
    Ok(RegressionMaterial {
        id: id.ok_or_else(|| miss("ID"))?,
        porosity: porosity.ok_or_else(|| miss("POROSITY"))?,
        tortuosity,
        characteristic_curves: cc.ok_or_else(|| miss("CHARACTERISTIC_CURVES"))?,
        permeability: perm.ok_or_else(|| miss("PERMEABILITY"))?,
        name,
    })
}

fn parse_curves_full(cur: &mut Cursor) -> Result<RegressionCurves, PflotranError> {
    let name = cur.expect("a curves name")?.text;
    let mut sf = None;
    let mut liq = None;
    let mut gas = None;
    loop {
        let (up, tok) = next_upper(cur, "a CHARACTERISTIC_CURVES card or END")?;
        if is_terminator(&tok.text) {
            break;
        }
        match up.as_str() {
            "SATURATION_FUNCTION" => {
                let (ty, tytok) = next_upper(cur, "a saturation function type")?;
                let mut sr = None;
                let mut alpha = None;
                let mut pcmax = None;
                let mut m = None;
                loop {
                    let (u2, t2) = next_upper(cur, "a SATURATION_FUNCTION card or `/`")?;
                    if is_terminator(&t2.text) {
                        break;
                    }
                    match u2.as_str() {
                        "LIQUID_RESIDUAL_SATURATION" => sr = Some(next_float(cur, &u2)?),
                        "ALPHA" => alpha = Some(next_float(cur, &u2)?),
                        "MAX_CAPILLARY_PRESSURE" => pcmax = Some(next_float(cur, &u2)?),
                        "M" if ty == "VAN_GENUCHTEN" => m = Some(next_float(cur, &u2)?),
                        _ => return Err(unsupported(&t2, &format!("SATURATION_FUNCTION {ty}"))),
                    }
                }
                let miss = |w: &str| err_at(tytok.line, &format!("SATURATION_FUNCTION {ty} missing {w}"));
                sf = Some(match ty.as_str() {
                    "LINEAR" => RegressionSaturationFunction::Linear {
                        residual_saturation: sr.ok_or_else(|| miss("LIQUID_RESIDUAL_SATURATION"))?,
                        alpha: alpha.ok_or_else(|| miss("ALPHA"))?,
                        max_capillary_pressure: pcmax.ok_or_else(|| miss("MAX_CAPILLARY_PRESSURE"))?,
                    },
                    "VAN_GENUCHTEN" => RegressionSaturationFunction::VanGenuchten {
                        m: m.ok_or_else(|| miss("M"))?,
                        alpha: alpha.ok_or_else(|| miss("ALPHA"))?,
                        residual_saturation: sr.ok_or_else(|| miss("LIQUID_RESIDUAL_SATURATION"))?,
                    },
                    _ => return Err(unsupported(&tytok, "SATURATION_FUNCTION type")),
                });
            }
            "PERMEABILITY_FUNCTION" => {
                let (ty, tytok) = next_upper(cur, "a permeability function type")?;
                let mut sr = None;
                let mut srg = None;
                let mut m = None;
                loop {
                    let (u2, t2) = next_upper(cur, "a PERMEABILITY_FUNCTION card or `/`")?;
                    if is_terminator(&t2.text) {
                        break;
                    }
                    match u2.as_str() {
                        "LIQUID_RESIDUAL_SATURATION" => sr = Some(next_float(cur, &u2)?),
                        "GAS_RESIDUAL_SATURATION" if ty == "BURDINE_LINEAR_GAS" => {
                            srg = Some(next_float(cur, &u2)?)
                        }
                        "M" if ty == "MUALEM_VG_LIQ" => m = Some(next_float(cur, &u2)?),
                        _ => return Err(unsupported(&t2, &format!("PERMEABILITY_FUNCTION {ty}"))),
                    }
                }
                let miss = |w: &str| err_at(tytok.line, &format!("PERMEABILITY_FUNCTION {ty} missing {w}"));
                let f = match ty.as_str() {
                    "BURDINE_LINEAR_LIQ" => RegressionPermeabilityFunction::BurdineLinearLiquid {
                        residual_saturation: sr.ok_or_else(|| miss("LIQUID_RESIDUAL_SATURATION"))?,
                    },
                    "BURDINE_LINEAR_GAS" => RegressionPermeabilityFunction::BurdineLinearGas {
                        liquid_residual_saturation: sr.ok_or_else(|| miss("LIQUID_RESIDUAL_SATURATION"))?,
                        gas_residual_saturation: srg.ok_or_else(|| miss("GAS_RESIDUAL_SATURATION"))?,
                    },
                    "MUALEM_VG_LIQ" => RegressionPermeabilityFunction::MualemVgLiquid {
                        m: m.ok_or_else(|| miss("M"))?,
                        residual_saturation: sr.ok_or_else(|| miss("LIQUID_RESIDUAL_SATURATION"))?,
                    },
                    _ => return Err(unsupported(&tytok, "PERMEABILITY_FUNCTION type")),
                };
                if ty.ends_with("_GAS") {
                    gas = Some(f);
                } else {
                    liq = Some(f);
                }
            }
            _ => return Err(unsupported(&tok, "CHARACTERISTIC_CURVES")),
        }
    }
    let miss = |w: &str| PflotranError::InvalidInput(format!("CHARACTERISTIC_CURVES `{name}` missing {w}"));
    Ok(RegressionCurves {
        saturation_function: sf.ok_or_else(|| miss("SATURATION_FUNCTION"))?,
        liquid_permeability: liq.ok_or_else(|| miss("liquid PERMEABILITY_FUNCTION"))?,
        gas_permeability: gas,
        name,
    })
}

fn parse_eos(cur: &mut Cursor) -> Result<RegressionEosWater, PflotranError> {
    let fluid = cur.expect("an EOS fluid (WATER)")?;
    if !fluid.text.eq_ignore_ascii_case("WATER") {
        return Err(err_at(fluid.line, "only EOS WATER is supported"));
    }
    let mut eos = RegressionEosWater::default();
    loop {
        let (up, tok) = next_upper(cur, "an EOS WATER card or END")?;
        if is_terminator(&tok.text) {
            break;
        }
        let kind = cur.expect("an EOS kind")?;
        if !kind.text.eq_ignore_ascii_case("CONSTANT") {
            return Err(err_at(kind.line, &format!("only {up} CONSTANT is supported")));
        }
        let v = next_float(cur, &up)?;
        match up.as_str() {
            "DENSITY" => eos.density = Some(v),
            "ENTHALPY" => eos.enthalpy = Some(v),
            "VISCOSITY" => eos.viscosity = Some(v),
            _ => return Err(unsupported(&tok, "EOS WATER")),
        }
    }
    Ok(eos)
}

fn parse_output(cur: &mut Cursor) -> Result<Vec<f64>, PflotranError> {
    let mut times = Vec::new();
    loop {
        let (up, tok) = next_upper(cur, "an OUTPUT card or END")?;
        if is_terminator(&tok.text) {
            break;
        }
        match up.as_str() {
            "FORMAT" => {
                // A format is a word, optionally followed by a second word
                // (`TECPLOT POINT`); stop at the next known card.
                cur.expect("an output format")?;
                while let Some(p) = peek_text(cur) {
                    let pu = p.to_uppercase();
                    if matches!(pu.as_str(), "POINT" | "BLOCK" | "FEPOINT") {
                        cur.next();
                    } else {
                        break;
                    }
                }
            }
            "TIMES" => {
                let unit = cur.expect("a TIMES unit")?;
                while let Some(p) = peek_text(cur) {
                    if parse_fortran_float(&p).is_err() {
                        break;
                    }
                    let v = next_float(cur, "TIMES value")?;
                    times.push(time_to_seconds(v, &unit.text)?);
                }
            }
            _ => return Err(unsupported(&tok, "OUTPUT")),
        }
    }
    Ok(times)
}

fn parse_time_full(cur: &mut Cursor) -> Result<RegressionTime, PflotranError> {
    let mut fin = None;
    let mut init = None;
    let mut max = None;
    loop {
        let (up, tok) = next_upper(cur, "a TIME card or END")?;
        if is_terminator(&tok.text) {
            break;
        }
        match up.as_str() {
            "FINAL_TIME" => fin = Some(next_time(cur, "FINAL_TIME")?),
            "INITIAL_TIMESTEP_SIZE" => init = Some(next_time(cur, "INITIAL_TIMESTEP_SIZE")?),
            "MAXIMUM_TIMESTEP_SIZE" => max = Some(next_time(cur, "MAXIMUM_TIMESTEP_SIZE")?),
            _ => return Err(unsupported(&tok, "TIME")),
        }
    }
    Ok(RegressionTime {
        final_time_s: fin.ok_or_else(|| PflotranError::InvalidInput("TIME missing FINAL_TIME".into()))?,
        initial_dt_s: init,
        max_dt_s: max,
    })
}

fn parse_region(cur: &mut Cursor) -> Result<RegressionRegion, PflotranError> {
    let name = cur.expect("a region name")?.text;
    let mut coords = None;
    let mut face = None;
    loop {
        let (up, tok) = next_upper(cur, "a REGION card or END")?;
        if is_terminator(&tok.text) {
            break;
        }
        match up.as_str() {
            "COORDINATES" => {
                let mut c = [[0.0; 3]; 2];
                for row in c.iter_mut() {
                    for v in row.iter_mut() {
                        *v = next_float(cur, "COORDINATES")?;
                    }
                }
                let t = cur.expect("`/` to close COORDINATES")?;
                if !is_terminator(&t.text) {
                    return Err(err_at(t.line, "expected `/` to close COORDINATES"));
                }
                coords = Some(c);
            }
            "FACE" => {
                let (f, ft) = next_upper(cur, "a face")?;
                face = Some(match f.as_str() {
                    "WEST" => RegressionFace::West,
                    "EAST" => RegressionFace::East,
                    "SOUTH" => RegressionFace::South,
                    "NORTH" => RegressionFace::North,
                    "BOTTOM" => RegressionFace::Bottom,
                    "TOP" => RegressionFace::Top,
                    _ => return Err(unsupported(&ft, "FACE")),
                });
            }
            _ => return Err(unsupported(&tok, "REGION")),
        }
    }
    Ok(RegressionRegion {
        coordinates: coords
            .ok_or_else(|| PflotranError::InvalidInput(format!("REGION `{name}` missing COORDINATES")))?,
        face,
        name,
    })
}

fn parse_flow_condition(cur: &mut Cursor) -> Result<RegressionFlowCondition, PflotranError> {
    let name = cur.expect("a flow-condition name")?.text;
    let mut kind: Option<String> = None;
    let mut value: Option<(String, f64)> = None;
    loop {
        let (up, tok) = next_upper(cur, "a FLOW_CONDITION card or END")?;
        if is_terminator(&tok.text) {
            break;
        }
        match up.as_str() {
            "TYPE" => loop {
                let (u2, t2) = next_upper(cur, "a TYPE entry or `/`")?;
                if is_terminator(&t2.text) {
                    break;
                }
                let (bc, bct) = next_upper(cur, "a boundary-condition type")?;
                if bc != "DIRICHLET" || !matches!(u2.as_str(), "LIQUID_PRESSURE" | "LIQUID_SATURATION") {
                    return Err(unsupported(&bct, &format!("FLOW_CONDITION TYPE {u2}")));
                }
                kind = Some(u2);
            },
            "LIQUID_PRESSURE" | "LIQUID_SATURATION" => value = Some((up.clone(), next_float(cur, &up)?)),
            _ => return Err(unsupported(&tok, "FLOW_CONDITION")),
        }
    }
    let miss = |w: &str| PflotranError::InvalidInput(format!("FLOW_CONDITION `{name}` missing {w}"));
    let kind = kind.ok_or_else(|| miss("TYPE"))?;
    let (vname, v) = value.ok_or_else(|| miss("a value"))?;
    if vname != kind {
        return Err(miss("a value matching its TYPE"));
    }
    Ok(RegressionFlowCondition {
        value: if kind == "LIQUID_PRESSURE" {
            RegressionFlowValue::LiquidPressure(v)
        } else {
            RegressionFlowValue::LiquidSaturation(v)
        },
        name,
    })
}

fn parse_coupler(cur: &mut Cursor, block: &str) -> Result<RegressionCoupler, PflotranError> {
    let mut name = None;
    if let Some(p) = peek_text(cur) {
        let pu = p.to_uppercase();
        if !matches!(pu.as_str(), "FLOW_CONDITION" | "REGION" | "END" | "/") {
            name = Some(p);
            cur.next();
        }
    }
    let mut fc = None;
    let mut region = None;
    loop {
        let (up, tok) = next_upper(cur, "a coupler card or END")?;
        if is_terminator(&tok.text) {
            break;
        }
        match up.as_str() {
            "FLOW_CONDITION" => fc = Some(cur.expect("a flow-condition name")?.text),
            "REGION" => region = Some(cur.expect("a region name")?.text),
            _ => return Err(unsupported(&tok, block)),
        }
    }
    let miss = |w: &str| PflotranError::InvalidInput(format!("{block} missing {w}"));
    Ok(RegressionCoupler {
        name,
        flow_condition: fc.ok_or_else(|| miss("FLOW_CONDITION"))?,
        region: region.ok_or_else(|| miss("REGION"))?,
    })
}

fn parse_strata(cur: &mut Cursor) -> Result<RegressionStrata, PflotranError> {
    let mut region = None;
    let mut material = None;
    loop {
        let (up, tok) = next_upper(cur, "a STRATA card or END")?;
        if is_terminator(&tok.text) {
            break;
        }
        match up.as_str() {
            "REGION" => region = Some(cur.expect("a region name")?.text),
            "MATERIAL" => material = Some(cur.expect("a material name")?.text),
            _ => return Err(unsupported(&tok, "STRATA")),
        }
    }
    let miss = |w: &str| PflotranError::InvalidInput(format!("STRATA missing {w}"));
    Ok(RegressionStrata {
        region: region.ok_or_else(|| miss("REGION"))?,
        material: material.ok_or_else(|| miss("MATERIAL"))?,
    })
}
