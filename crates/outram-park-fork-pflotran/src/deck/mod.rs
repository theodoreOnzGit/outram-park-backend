//! Parser for **genuine PFLOTRAN input-deck syntax** (a supported subset).
//!
//! Unlike the [`crate::io`] module — whose grammar is an AI-invented "lite"
//! format that is *not* compatible with real PFLOTRAN — this module parses the
//! actual keyword/block syntax used by the upstream PFLOTRAN simulator
//! (documentation.pflotran.org). It is a step toward real-deck compatibility.
//!
//! This parser is intentionally self-contained: it depends only on
//! [`crate::error::PflotranError`] and the standard library, and it produces its
//! own plain [`PflotranDeck`] struct (it does **not** reuse the `io` module's
//! [`crate::io::InputDeck`] spec types).
//!
//! # Supported subset — flag clearly what is parsed
//!
//! Real PFLOTRAN decks are large; this v1 parser targets a *representative,
//! honest subset*. It is **not** a full PFLOTRAN reader. Supported blocks and
//! cards:
//!
//! | Block | Cards parsed | Notes |
//! |---|---|---|
//! | `SIMULATION` | `SIMULATION_TYPE`, `PROCESS_MODELS` → `SUBSURFACE_FLOW` → `MODE` | only `MODE` is captured; other cards are skipped leniently |
//! | `GRID` | `TYPE`, `NXYZ`, `BOUNDS` | structured Cartesian only |
//! | `MATERIAL_PROPERTY <name>` | `ID`, `POROSITY`, `PERMEABILITY` → `PERM_ISO` | isotropic permeability only |
//! | `CHARACTERISTIC_CURVES <name>` | `SATURATION_FUNCTION <type>` → `ALPHA`, `M`, `LIQUID_RESIDUAL_SATURATION` | |
//! | `TIME` | `FINAL_TIME`, `INITIAL_TIMESTEP_SIZE`, `MAXIMUM_TIMESTEP_SIZE` | value + unit suffix, normalised to seconds |
//! | `SUBSURFACE` / `END_SUBSURFACE` | (markers) | consumed and ignored |
//! | `SKIP` / `NOSKIP` | (markers) | consumed and ignored (v1: no region skipping) |
//!
//! # NOT yet parsed (human-review flags)
//!
//! Everything else in the real format, including: unstructured / `DXYZ` grids,
//! anisotropic / tensor permeability, `REGION` / `OBSERVATION` /
//! `BOUNDARY_CONDITION` / `INITIAL_CONDITION` / `STRATA` cards, `FLOW_CONDITION`
//! and `TRANSPORT_CONDITION`, `CHEMISTRY`, `DATASET` / `HDF5`, `EXTERNAL_FILE`
//! inclusion, `REFERENCE_*` cards, output/`SNAPSHOT_FILE` controls, and true
//! `SKIP`/`NOSKIP` region skipping. Inside a supported block an *unknown card is
//! rejected* (with line context) rather than silently ignored, so the parser
//! never pretends to understand more than it does. This is untrusted
//! AI-generated draft material until a human reviews it against real PFLOTRAN
//! decks, per the workspace `RESPONSIBLE_USE.md` rule.
//!
//! # Syntax features handled
//!
//! - **Fortran D-exponent floats** — `1.d-12` == `1.0e-12`, `0.d0` == `0.0`,
//!   `-3.5D2` == `-350.0` (see [`parse_fortran_float`]).
//! - **Time unit suffixes** — `s`/`m`/`h`/`d`/`y` on `TIME` cards, normalised to
//!   seconds (see [`time_to_seconds`]); a year is 365.25 days.
//! - **Comments** — `#` and `!` to end of line.
//! - **Named blocks** — `MATERIAL_PROPERTY soil1`, `CHARACTERISTIC_CURVES cc1`.
//! - **Nested blocks** closed by `/`; top-level blocks closed by `END` (either
//!   terminator is accepted where a block ends).
//! - **Case-insensitive keywords**; blank lines; stray `:` tokens ignored.

use crate::error::PflotranError;

/// A parsed (subset of a) real PFLOTRAN input deck.
///
/// Produced by [`parse_pflotran_deck`]. Only the [supported subset](self) of
/// cards is represented; unsupported cards cause a parse error rather than being
/// dropped, so a successfully parsed deck reflects exactly what the file said
/// about the fields below.
#[derive(Debug, Clone, PartialEq)]
pub struct PflotranDeck {
    /// Flow mode from `SIMULATION` > `PROCESS_MODELS` > `SUBSURFACE_FLOW` >
    /// `MODE` (e.g. `RICHARDS`).
    pub simulation_mode: FlowMode,
    /// Structured grid dimensions and physical bounds (`GRID` block).
    pub grid: GridBlock,
    /// Zero or more `MATERIAL_PROPERTY` blocks, in file order.
    pub materials: Vec<MaterialProperty>,
    /// Zero or more `CHARACTERISTIC_CURVES` blocks, in file order.
    pub characteristic_curves: Vec<CharacteristicCurveBlock>,
    /// Time-stepping controls (`TIME` block), all normalised to seconds.
    pub time: TimeBlock,
}

/// The subsurface-flow process model selected by the `MODE` card.
///
/// `RICHARDS`/`TH`/`GENERAL` are the common PFLOTRAN flow modes; any other
/// keyword is preserved verbatim in [`FlowMode::Unknown`] so the caller can see
/// what the deck asked for without the parser silently discarding it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowMode {
    /// `RICHARDS` — variably-saturated single-phase flow.
    Richards,
    /// `TH` — thermo-hydraulic (flow + heat).
    Th,
    /// `GENERAL` — multiphase / multicomponent flow.
    General,
    /// Any other `MODE` keyword, stored verbatim (original case preserved).
    Unknown(String),
}

/// The `GRID` block: a structured Cartesian mesh and its physical extent.
///
/// `nx`/`ny`/`nz` come from the `NXYZ` card (cell counts along each axis).
/// `bounds_min`/`bounds_max` come from the `BOUNDS` sub-block as
/// `[x, y, z]` corner coordinates in **metres**.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridBlock {
    /// `true` when `TYPE STRUCTURED` was given (or defaulted). Only structured
    /// grids are supported in v1.
    pub structured: bool,
    /// Cell count along x, from `NXYZ`.
    pub nx: usize,
    /// Cell count along y, from `NXYZ`.
    pub ny: usize,
    /// Cell count along z, from `NXYZ`.
    pub nz: usize,
    /// Lower `[x, y, z]` corner of the domain, in metres (first `BOUNDS` line).
    pub bounds_min: [f64; 3],
    /// Upper `[x, y, z]` corner of the domain, in metres (second `BOUNDS` line).
    pub bounds_max: [f64; 3],
}

/// A `MATERIAL_PROPERTY <name>` block (isotropic subset).
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialProperty {
    /// The material name that followed the `MATERIAL_PROPERTY` keyword.
    pub name: String,
    /// Integer material `ID`.
    pub id: usize,
    /// `POROSITY`, dimensionless.
    pub porosity: f64,
    /// Isotropic intrinsic permeability from `PERMEABILITY` > `PERM_ISO`, in
    /// m^2.
    pub permeability_iso: f64,
}

/// A `CHARACTERISTIC_CURVES <name>` block (saturation-function subset).
///
/// Captures the `SATURATION_FUNCTION` type keyword (e.g. `VAN_GENUCHTEN`) and
/// its three parameters. Relative-permeability sub-blocks are not parsed in v1.
#[derive(Debug, Clone, PartialEq)]
pub struct CharacteristicCurveBlock {
    /// The curve-set name that followed the `CHARACTERISTIC_CURVES` keyword.
    pub name: String,
    /// The `SATURATION_FUNCTION` model keyword, verbatim (e.g. `VAN_GENUCHTEN`).
    pub saturation_function: String,
    /// `ALPHA`, the van-Genuchten air-entry parameter, in 1/Pa.
    pub alpha: f64,
    /// `M`, the van-Genuchten exponent, dimensionless.
    pub m: f64,
    /// `LIQUID_RESIDUAL_SATURATION`, dimensionless.
    pub liquid_residual_saturation: f64,
}

/// The `TIME` block, with every value normalised to **seconds**.
///
/// Each source card carries a value plus a unit suffix (`s`/`m`/`h`/`d`/`y`);
/// [`time_to_seconds`] converts them, so the fields here are always in SI
/// seconds regardless of the deck's chosen units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeBlock {
    /// `FINAL_TIME`, in seconds.
    pub final_time_s: f64,
    /// `INITIAL_TIMESTEP_SIZE`, in seconds.
    pub initial_dt_s: f64,
    /// `MAXIMUM_TIMESTEP_SIZE`, in seconds.
    pub max_dt_s: f64,
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Parse a real-PFLOTRAN-syntax deck (the [supported subset](self)).
///
/// Returns a fully-populated [`PflotranDeck`] on success. On failure returns
/// [`PflotranError::InvalidInput`] carrying line context and, where useful, the
/// offending keyword.
///
/// Required blocks: `SIMULATION` (with a `MODE` card), `GRID`, and `TIME`; their
/// absence is an error. `MATERIAL_PROPERTY` and `CHARACTERISTIC_CURVES` are
/// optional and may repeat.
pub fn parse_pflotran_deck(input: &str) -> Result<PflotranDeck, PflotranError> {
    let mut cur = Cursor::new(input);

    let mut mode: Option<FlowMode> = None;
    let mut grid: Option<GridBlock> = None;
    let mut materials: Vec<MaterialProperty> = Vec::new();
    let mut curves: Vec<CharacteristicCurveBlock> = Vec::new();
    let mut time: Option<TimeBlock> = None;

    while let Some(tok) = cur.next() {
        match tok.text.to_uppercase().as_str() {
            "SIMULATION" => mode = Some(parse_simulation(&mut cur)?),
            // SUBSURFACE / END_SUBSURFACE wrap the physics blocks; treat as
            // markers. SKIP / NOSKIP toggle region skipping in real PFLOTRAN;
            // v1 ignores them (documented subset limitation).
            "SUBSURFACE" | "END_SUBSURFACE" | "SKIP" | "NOSKIP" => {}
            "GRID" => grid = Some(parse_grid(&mut cur)?),
            "MATERIAL_PROPERTY" => materials.push(parse_material(&mut cur)?),
            "CHARACTERISTIC_CURVES" => curves.push(parse_curves(&mut cur)?),
            "TIME" => time = Some(parse_time(&mut cur)?),
            other => {
                return Err(err_at(
                    tok.line,
                    &format!("unknown/unsupported top-level keyword `{other}`"),
                ))
            }
        }
    }

    let simulation_mode = mode.ok_or_else(|| {
        PflotranError::InvalidInput(
            "missing required SIMULATION > PROCESS_MODELS > MODE card".to_string(),
        )
    })?;
    let grid =
        grid.ok_or_else(|| PflotranError::InvalidInput("missing required GRID block".to_string()))?;
    let time =
        time.ok_or_else(|| PflotranError::InvalidInput("missing required TIME block".to_string()))?;

    Ok(PflotranDeck {
        simulation_mode,
        grid,
        materials,
        characteristic_curves: curves,
        time,
    })
}

/// Parse a single Fortran-style float token.
///
/// PFLOTRAN decks are written by/for Fortran and use the `D`/`d` exponent marker
/// (`1.d-12`, `0.d0`, `-3.5D2`). This accepts that plus ordinary `e`/`E`
/// exponents (`1.0e3`) and plain decimals (`2.5`). The `D`/`d` marker is
/// translated to `e` and the result parsed as an [`f64`].
///
/// Returns [`PflotranError::InvalidInput`] for tokens that are not valid
/// numbers.
///
/// # Examples
///
/// ```
/// use outram_park_fork_pflotran::deck::parse_fortran_float;
/// assert_eq!(parse_fortran_float("1.d-12").unwrap(), 1.0e-12);
/// assert_eq!(parse_fortran_float("0.d0").unwrap(), 0.0);
/// assert_eq!(parse_fortran_float("-3.5D2").unwrap(), -350.0);
/// assert_eq!(parse_fortran_float("1.0e3").unwrap(), 1000.0);
/// assert!(parse_fortran_float("nonsense").is_err());
/// ```
pub fn parse_fortran_float(token: &str) -> Result<f64, PflotranError> {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return Err(PflotranError::InvalidInput(
            "empty token where a number was expected".to_string(),
        ));
    }
    // Translate the Fortran D/d exponent marker to e. Anything that still fails
    // to parse (letters, doubled dots, bare markers) is rejected below.
    let normalized: String = trimmed
        .chars()
        .map(|c| match c {
            'd' | 'D' => 'e',
            other => other,
        })
        .collect();
    normalized
        .parse::<f64>()
        .map_err(|_| PflotranError::InvalidInput(format!("invalid Fortran float `{token}`")))
}

/// Convert a PFLOTRAN time value plus a unit suffix to seconds.
///
/// Recognised units (case-insensitive): `s` (second), `m` (minute), `h` (hour),
/// `d` (day), `y` (year = 365.25 days). Common long forms (`sec`, `min`, `hr`,
/// `day`, `yr`, and plurals) are also accepted. Unknown units are rejected.
///
/// # Examples
///
/// ```
/// use outram_park_fork_pflotran::deck::time_to_seconds;
/// assert_eq!(time_to_seconds(1.0, "d").unwrap(), 86_400.0);
/// assert_eq!(time_to_seconds(2.0, "h").unwrap(), 7_200.0);
/// assert_eq!(time_to_seconds(5.0, "s").unwrap(), 5.0);
/// assert!(time_to_seconds(1.0, "furlong").is_err());
/// ```
pub fn time_to_seconds(value: f64, unit: &str) -> Result<f64, PflotranError> {
    let factor = match unit.trim().to_lowercase().as_str() {
        "s" | "sec" | "second" | "seconds" => 1.0,
        "m" | "min" | "minute" | "minutes" => 60.0,
        "h" | "hr" | "hour" | "hours" => 3600.0,
        "d" | "day" | "days" => 86_400.0,
        "y" | "yr" | "year" | "years" => 365.25 * 86_400.0, // 31_557_600 s
        other => {
            return Err(PflotranError::InvalidInput(format!(
                "unknown time unit `{other}` (expected s/m/h/d/y)"
            )))
        }
    };
    Ok(value * factor)
}

// ---------------------------------------------------------------------------
// Tokenizer + cursor
// ---------------------------------------------------------------------------

/// One whitespace-delimited token plus the 1-based source line it came from.
#[derive(Debug, Clone)]
struct Token {
    text: String,
    line: usize,
}

/// A forward-only cursor over the tokenized deck.
struct Cursor {
    tokens: Vec<Token>,
    pos: usize,
}

impl Cursor {
    /// Tokenize `input`, stripping `#`/`!` comments and stray `:` tokens.
    fn new(input: &str) -> Self {
        let mut tokens = Vec::new();
        for (i, raw) in input.lines().enumerate() {
            let line = match raw.find(|c| c == '#' || c == '!') {
                Some(idx) => &raw[..idx],
                None => raw,
            };
            for tok in line.split_whitespace() {
                if tok == ":" {
                    // A bare card-continuation colon; ignorable in v1.
                    continue;
                }
                tokens.push(Token {
                    text: tok.to_string(),
                    line: i + 1,
                });
            }
        }
        Cursor { tokens, pos: 0 }
    }

    /// Advance and return the next token, or `None` at end of input.
    fn next(&mut self) -> Option<Token> {
        let tok = self.tokens.get(self.pos).cloned();
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    /// Advance and return the next token, erroring with `ctx` at end of input.
    fn expect(&mut self, ctx: &str) -> Result<Token, PflotranError> {
        self.next().ok_or_else(|| {
            PflotranError::InvalidInput(format!("unexpected end of deck: expected {ctx}"))
        })
    }
}

/// True when a token ends the current block (`END` or `/`).
fn is_terminator(text: &str) -> bool {
    text == "/" || text.eq_ignore_ascii_case("END")
}

/// Build an `InvalidInput` error tagged with a 1-based line number.
fn err_at(line: usize, msg: &str) -> PflotranError {
    PflotranError::InvalidInput(format!("line {line}: {msg}"))
}

/// Read the next token and parse it as a Fortran float, with line context.
fn next_float(cur: &mut Cursor, ctx: &str) -> Result<f64, PflotranError> {
    let tok = cur.expect(ctx)?;
    parse_fortran_float(&tok.text).map_err(|_| {
        err_at(
            tok.line,
            &format!("expected a number for {ctx}, got `{}`", tok.text),
        )
    })
}

/// Read the next token and parse it as a non-negative integer, with line
/// context.
fn next_usize(cur: &mut Cursor, ctx: &str) -> Result<usize, PflotranError> {
    let tok = cur.expect(ctx)?;
    tok.text.parse::<usize>().map_err(|_| {
        err_at(
            tok.line,
            &format!("expected an integer for {ctx}, got `{}`", tok.text),
        )
    })
}

// ---------------------------------------------------------------------------
// Block parsers
// ---------------------------------------------------------------------------

/// Parse a `SIMULATION` block, returning the `MODE` it declares.
///
/// The block's nested structure (`PROCESS_MODELS` > `SUBSURFACE_FLOW`) is
/// handled leniently: every card except `MODE` is skipped, and the block is
/// closed by the first `END` (nested sub-blocks close with `/`, never `END`, so
/// this is unambiguous for the supported subset).
fn parse_simulation(cur: &mut Cursor) -> Result<FlowMode, PflotranError> {
    let mut mode: Option<FlowMode> = None;
    loop {
        let tok = cur
            .next()
            .ok_or_else(|| PflotranError::InvalidInput("unclosed SIMULATION block".to_string()))?;
        if tok.text.eq_ignore_ascii_case("END") {
            break;
        }
        if tok.text.eq_ignore_ascii_case("MODE") {
            let value = cur.expect("a value after MODE")?;
            mode = Some(flow_mode_from_str(&value.text));
        }
        // All other tokens (SIMULATION_TYPE, PROCESS_MODELS, SUBSURFACE_FLOW,
        // their values, and `/` terminators) are skipped for v1.
    }
    mode.ok_or_else(|| PflotranError::InvalidInput("SIMULATION block has no MODE card".to_string()))
}

/// Map a `MODE` keyword to [`FlowMode`], preserving unknown values verbatim.
fn flow_mode_from_str(s: &str) -> FlowMode {
    match s.to_uppercase().as_str() {
        "RICHARDS" => FlowMode::Richards,
        "TH" => FlowMode::Th,
        "GENERAL" => FlowMode::General,
        _ => FlowMode::Unknown(s.to_string()),
    }
}

/// Parse a `GRID` block (structured Cartesian subset).
fn parse_grid(cur: &mut Cursor) -> Result<GridBlock, PflotranError> {
    let mut structured = true; // STRUCTURED is the PFLOTRAN default
    let mut nx: Option<usize> = None;
    let mut ny: Option<usize> = None;
    let mut nz: Option<usize> = None;
    let mut bounds_min: Option<[f64; 3]> = None;
    let mut bounds_max: Option<[f64; 3]> = None;

    loop {
        let tok = cur
            .next()
            .ok_or_else(|| PflotranError::InvalidInput("unclosed GRID block".to_string()))?;
        if is_terminator(&tok.text) {
            break;
        }
        match tok.text.to_uppercase().as_str() {
            "TYPE" => {
                let value = cur.expect("a grid type after TYPE")?;
                structured = value.text.eq_ignore_ascii_case("STRUCTURED");
            }
            "NXYZ" => {
                nx = Some(next_usize(cur, "NXYZ nx")?);
                ny = Some(next_usize(cur, "NXYZ ny")?);
                nz = Some(next_usize(cur, "NXYZ nz")?);
            }
            "BOUNDS" => {
                let lo = [
                    next_float(cur, "BOUNDS min x")?,
                    next_float(cur, "BOUNDS min y")?,
                    next_float(cur, "BOUNDS min z")?,
                ];
                let hi = [
                    next_float(cur, "BOUNDS max x")?,
                    next_float(cur, "BOUNDS max y")?,
                    next_float(cur, "BOUNDS max z")?,
                ];
                let term = cur.expect("`/` to close BOUNDS")?;
                if !is_terminator(&term.text) {
                    return Err(err_at(
                        term.line,
                        &format!("expected `/` to close BOUNDS, got `{}`", term.text),
                    ));
                }
                bounds_min = Some(lo);
                bounds_max = Some(hi);
            }
            other => {
                return Err(err_at(
                    tok.line,
                    &format!("unsupported card `{other}` in GRID block"),
                ))
            }
        }
    }

    Ok(GridBlock {
        structured,
        nx: nx.ok_or_else(|| PflotranError::InvalidInput("GRID missing NXYZ".to_string()))?,
        ny: ny.ok_or_else(|| PflotranError::InvalidInput("GRID missing NXYZ".to_string()))?,
        nz: nz.ok_or_else(|| PflotranError::InvalidInput("GRID missing NXYZ".to_string()))?,
        bounds_min: bounds_min
            .ok_or_else(|| PflotranError::InvalidInput("GRID missing BOUNDS".to_string()))?,
        bounds_max: bounds_max
            .ok_or_else(|| PflotranError::InvalidInput("GRID missing BOUNDS".to_string()))?,
    })
}

/// Parse a `MATERIAL_PROPERTY <name>` block (isotropic subset).
fn parse_material(cur: &mut Cursor) -> Result<MaterialProperty, PflotranError> {
    let name = cur.expect("a name after MATERIAL_PROPERTY")?.text;
    let mut id: Option<usize> = None;
    let mut porosity: Option<f64> = None;
    let mut permeability_iso: Option<f64> = None;

    loop {
        let tok = cur.next().ok_or_else(|| {
            PflotranError::InvalidInput(format!("unclosed MATERIAL_PROPERTY `{name}` block"))
        })?;
        if is_terminator(&tok.text) {
            break;
        }
        match tok.text.to_uppercase().as_str() {
            "ID" => id = Some(next_usize(cur, "material ID")?),
            "POROSITY" => porosity = Some(next_float(cur, "POROSITY")?),
            "PERMEABILITY" => {
                // Sub-block closed by `/`; v1 supports only PERM_ISO.
                loop {
                    let inner = cur.next().ok_or_else(|| {
                        PflotranError::InvalidInput("unclosed PERMEABILITY sub-block".to_string())
                    })?;
                    if is_terminator(&inner.text) {
                        break;
                    }
                    match inner.text.to_uppercase().as_str() {
                        "PERM_ISO" => permeability_iso = Some(next_float(cur, "PERM_ISO")?),
                        other => {
                            return Err(err_at(
                                inner.line,
                                &format!("unsupported card `{other}` in PERMEABILITY sub-block"),
                            ))
                        }
                    }
                }
            }
            other => {
                return Err(err_at(
                    tok.line,
                    &format!("unsupported card `{other}` in MATERIAL_PROPERTY block"),
                ))
            }
        }
    }

    Ok(MaterialProperty {
        name: name.clone(),
        id: id.ok_or_else(|| {
            PflotranError::InvalidInput(format!("MATERIAL_PROPERTY `{name}` missing ID"))
        })?,
        porosity: porosity.ok_or_else(|| {
            PflotranError::InvalidInput(format!("MATERIAL_PROPERTY `{name}` missing POROSITY"))
        })?,
        permeability_iso: permeability_iso.ok_or_else(|| {
            PflotranError::InvalidInput(format!(
                "MATERIAL_PROPERTY `{name}` missing PERMEABILITY/PERM_ISO"
            ))
        })?,
    })
}

/// Parse a `CHARACTERISTIC_CURVES <name>` block (saturation-function subset).
fn parse_curves(cur: &mut Cursor) -> Result<CharacteristicCurveBlock, PflotranError> {
    let name = cur.expect("a name after CHARACTERISTIC_CURVES")?.text;
    let mut saturation_function: Option<String> = None;
    let mut alpha: Option<f64> = None;
    let mut m: Option<f64> = None;
    let mut liquid_residual_saturation: Option<f64> = None;

    loop {
        let tok = cur.next().ok_or_else(|| {
            PflotranError::InvalidInput(format!("unclosed CHARACTERISTIC_CURVES `{name}` block"))
        })?;
        if is_terminator(&tok.text) {
            break;
        }
        match tok.text.to_uppercase().as_str() {
            "SATURATION_FUNCTION" => {
                let ty = cur.expect("a saturation-function type")?;
                saturation_function = Some(ty.text);
                // Sub-block closed by `/`.
                loop {
                    let inner = cur.next().ok_or_else(|| {
                        PflotranError::InvalidInput(
                            "unclosed SATURATION_FUNCTION sub-block".to_string(),
                        )
                    })?;
                    if is_terminator(&inner.text) {
                        break;
                    }
                    match inner.text.to_uppercase().as_str() {
                        "ALPHA" => alpha = Some(next_float(cur, "ALPHA")?),
                        "M" => m = Some(next_float(cur, "M")?),
                        "LIQUID_RESIDUAL_SATURATION" => {
                            liquid_residual_saturation =
                                Some(next_float(cur, "LIQUID_RESIDUAL_SATURATION")?)
                        }
                        other => {
                            return Err(err_at(
                                inner.line,
                                &format!(
                                    "unsupported card `{other}` in SATURATION_FUNCTION sub-block"
                                ),
                            ))
                        }
                    }
                }
            }
            other => {
                return Err(err_at(
                    tok.line,
                    &format!("unsupported card `{other}` in CHARACTERISTIC_CURVES block"),
                ))
            }
        }
    }

    Ok(CharacteristicCurveBlock {
        name: name.clone(),
        saturation_function: saturation_function.ok_or_else(|| {
            PflotranError::InvalidInput(format!(
                "CHARACTERISTIC_CURVES `{name}` missing SATURATION_FUNCTION"
            ))
        })?,
        alpha: alpha.ok_or_else(|| {
            PflotranError::InvalidInput(format!("CHARACTERISTIC_CURVES `{name}` missing ALPHA"))
        })?,
        m: m.ok_or_else(|| {
            PflotranError::InvalidInput(format!("CHARACTERISTIC_CURVES `{name}` missing M"))
        })?,
        liquid_residual_saturation: liquid_residual_saturation.ok_or_else(|| {
            PflotranError::InvalidInput(format!(
                "CHARACTERISTIC_CURVES `{name}` missing LIQUID_RESIDUAL_SATURATION"
            ))
        })?,
    })
}

/// Parse a `TIME` block, normalising every value to seconds.
fn parse_time(cur: &mut Cursor) -> Result<TimeBlock, PflotranError> {
    let mut final_time_s: Option<f64> = None;
    let mut initial_dt_s: Option<f64> = None;
    let mut max_dt_s: Option<f64> = None;

    loop {
        let tok = cur
            .next()
            .ok_or_else(|| PflotranError::InvalidInput("unclosed TIME block".to_string()))?;
        if is_terminator(&tok.text) {
            break;
        }
        match tok.text.to_uppercase().as_str() {
            "FINAL_TIME" => final_time_s = Some(read_time_card(cur, "FINAL_TIME")?),
            "INITIAL_TIMESTEP_SIZE" => {
                initial_dt_s = Some(read_time_card(cur, "INITIAL_TIMESTEP_SIZE")?)
            }
            "MAXIMUM_TIMESTEP_SIZE" => {
                max_dt_s = Some(read_time_card(cur, "MAXIMUM_TIMESTEP_SIZE")?)
            }
            other => {
                return Err(err_at(
                    tok.line,
                    &format!("unsupported card `{other}` in TIME block"),
                ))
            }
        }
    }

    Ok(TimeBlock {
        final_time_s: final_time_s
            .ok_or_else(|| PflotranError::InvalidInput("TIME missing FINAL_TIME".to_string()))?,
        initial_dt_s: initial_dt_s.ok_or_else(|| {
            PflotranError::InvalidInput("TIME missing INITIAL_TIMESTEP_SIZE".to_string())
        })?,
        max_dt_s: max_dt_s.ok_or_else(|| {
            PflotranError::InvalidInput("TIME missing MAXIMUM_TIMESTEP_SIZE".to_string())
        })?,
    })
}

/// Read a `<value> <unit>` time card and return the value in seconds.
fn read_time_card(cur: &mut Cursor, card: &str) -> Result<f64, PflotranError> {
    let value = next_float(cur, card)?;
    let unit = cur.expect(&format!("a unit suffix after {card}"))?;
    time_to_seconds(value, &unit.text).map_err(|e| err_at(unit.line, &format!("{card}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The representative real-PFLOTRAN-syntax deck from the module spec.
    const EXAMPLE_DECK: &str = r#"
SIMULATION
  SIMULATION_TYPE SUBSURFACE
  PROCESS_MODELS
    SUBSURFACE_FLOW flow
      MODE RICHARDS
    /
  /
END

SUBSURFACE

GRID
  TYPE STRUCTURED
  NXYZ 10 1 1
  BOUNDS
    0.d0 0.d0 0.d0
    10.d0 1.d0 1.d0
  /
END

MATERIAL_PROPERTY soil1
  ID 1
  POROSITY 0.25d0
  PERMEABILITY
    PERM_ISO 1.d-12
  /
END

CHARACTERISTIC_CURVES cc1
  SATURATION_FUNCTION VAN_GENUCHTEN
    ALPHA 1.d-4
    M 0.5d0
    LIQUID_RESIDUAL_SATURATION 0.1d0
  /
END

TIME
  FINAL_TIME 1.d0 d
  INITIAL_TIMESTEP_SIZE 1.d-2 d
  MAXIMUM_TIMESTEP_SIZE 1.d-1 d
END

END_SUBSURFACE
"#;

    /// Relative/absolute approximate float comparison for parsed physics values.
    fn approx(a: f64, b: f64) {
        let tol = 1e-9 * b.abs().max(1.0);
        assert!((a - b).abs() <= tol, "expected {b}, got {a}");
    }

    #[test]
    fn parses_full_example_deck() {
        let deck = parse_pflotran_deck(EXAMPLE_DECK).expect("example deck should parse");

        // SIMULATION > ... > MODE
        assert_eq!(deck.simulation_mode, FlowMode::Richards);

        // GRID
        assert!(deck.grid.structured);
        assert_eq!(deck.grid.nx, 10);
        assert_eq!(deck.grid.ny, 1);
        assert_eq!(deck.grid.nz, 1);
        assert_eq!(deck.grid.bounds_min, [0.0, 0.0, 0.0]);
        assert_eq!(deck.grid.bounds_max, [10.0, 1.0, 1.0]);

        // MATERIAL_PROPERTY
        assert_eq!(deck.materials.len(), 1);
        let mat = &deck.materials[0];
        assert_eq!(mat.name, "soil1");
        assert_eq!(mat.id, 1);
        approx(mat.porosity, 0.25);
        approx(mat.permeability_iso, 1.0e-12);

        // CHARACTERISTIC_CURVES
        assert_eq!(deck.characteristic_curves.len(), 1);
        let cc = &deck.characteristic_curves[0];
        assert_eq!(cc.name, "cc1");
        assert_eq!(cc.saturation_function, "VAN_GENUCHTEN");
        approx(cc.alpha, 1.0e-4);
        approx(cc.m, 0.5);
        approx(cc.liquid_residual_saturation, 0.1);

        // TIME (normalised to seconds: 1 d = 86400 s)
        approx(deck.time.final_time_s, 86_400.0);
        approx(deck.time.initial_dt_s, 864.0);
        approx(deck.time.max_dt_s, 8_640.0);
    }

    #[test]
    fn parse_fortran_float_variants() {
        approx(parse_fortran_float("1.d-12").unwrap(), 1.0e-12);
        assert_eq!(parse_fortran_float("0.d0").unwrap(), 0.0);
        approx(parse_fortran_float("-3.5D2").unwrap(), -350.0);
        approx(parse_fortran_float("1.0e3").unwrap(), 1000.0);
        approx(parse_fortran_float("2.5").unwrap(), 2.5);

        assert!(parse_fortran_float("abc").is_err());
        assert!(parse_fortran_float("1.2.3").is_err());
        assert!(parse_fortran_float("").is_err());
        assert!(parse_fortran_float("d0").is_err());
    }

    #[test]
    fn time_to_seconds_units() {
        approx(time_to_seconds(1.0, "d").unwrap(), 86_400.0);
        approx(time_to_seconds(1.0, "y").unwrap(), 31_557_600.0); // 365.25 d
        approx(time_to_seconds(2.0, "h").unwrap(), 7_200.0);
        approx(time_to_seconds(30.0, "m").unwrap(), 1_800.0);
        approx(time_to_seconds(5.0, "s").unwrap(), 5.0);

        assert!(time_to_seconds(1.0, "furlong").is_err());
        assert!(time_to_seconds(1.0, "").is_err());
    }

    #[test]
    fn rejects_unknown_top_level_keyword() {
        let deck = "WIDGET foo\nEND\n";
        let err = parse_pflotran_deck(deck).unwrap_err();
        match err {
            PflotranError::InvalidInput(msg) => {
                assert!(
                    msg.contains("WIDGET"),
                    "message should name the keyword: {msg}"
                );
                assert!(
                    msg.contains("line 1"),
                    "message should carry line context: {msg}"
                );
            }
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn rejects_unclosed_block() {
        // GRID opened but never terminated (no END / `/`).
        let deck = "GRID\n  NXYZ 1 1 1\n";
        let err = parse_pflotran_deck(deck).unwrap_err();
        match err {
            PflotranError::InvalidInput(msg) => {
                assert!(msg.to_lowercase().contains("unclosed"), "got: {msg}");
            }
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn rejects_missing_required_block() {
        // Valid SIMULATION + TIME but no GRID.
        let deck = r#"
SIMULATION
  PROCESS_MODELS
    SUBSURFACE_FLOW flow
      MODE RICHARDS
    /
  /
END
TIME
  FINAL_TIME 1.d0 d
  INITIAL_TIMESTEP_SIZE 1.d-2 d
  MAXIMUM_TIMESTEP_SIZE 1.d-1 d
END
"#;
        let err = parse_pflotran_deck(deck).unwrap_err();
        match err {
            PflotranError::InvalidInput(msg) => {
                assert!(msg.contains("GRID"), "should flag missing GRID: {msg}");
            }
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn robust_to_comments_blanks_and_mixed_case() {
        let deck = r#"
# a leading comment line
simulation
  process_models
    subsurface_flow flow
      mode general   ! inline comment after a card
    /
  /
end

grid   # comment on the block opener

  type structured
  nxyz 4 2 3

  bounds
    0.d0 0.d0 0.d0
    4.d0 2.d0 3.d0
  /
end

time
  final_time 2.0 h
  initial_timestep_size 1.d0 s
  maximum_timestep_size 10.0 m
end
"#;
        let deck = parse_pflotran_deck(deck).expect("mixed-case/commented deck should parse");
        assert_eq!(deck.simulation_mode, FlowMode::General);
        assert_eq!((deck.grid.nx, deck.grid.ny, deck.grid.nz), (4, 2, 3));
        approx(deck.time.final_time_s, 7_200.0); // 2 h
        approx(deck.time.initial_dt_s, 1.0); // 1 s
        approx(deck.time.max_dt_s, 600.0); // 10 min
        assert!(deck.materials.is_empty());
        assert!(deck.characteristic_curves.is_empty());
    }

    #[test]
    fn unknown_mode_preserved_verbatim() {
        let deck = r#"
SIMULATION
  PROCESS_MODELS
    SUBSURFACE_FLOW flow
      MODE WIPP_FLOW
    /
  /
END
GRID
  NXYZ 1 1 1
  BOUNDS
    0.d0 0.d0 0.d0
    1.d0 1.d0 1.d0
  /
END
TIME
  FINAL_TIME 1.d0 s
  INITIAL_TIMESTEP_SIZE 1.d0 s
  MAXIMUM_TIMESTEP_SIZE 1.d0 s
END
"#;
        let deck = parse_pflotran_deck(deck).expect("should parse with unknown mode");
        assert_eq!(
            deck.simulation_mode,
            FlowMode::Unknown("WIPP_FLOW".to_string())
        );
    }

    #[test]
    fn rejects_unsupported_card_in_block() {
        let deck = r#"
GRID
  NXYZ 1 1 1
  GRAVITY 0.0 0.0 -9.81
  BOUNDS
    0.d0 0.d0 0.d0
    1.d0 1.d0 1.d0
  /
END
"#;
        let err = parse_pflotran_deck(deck).unwrap_err();
        match err {
            PflotranError::InvalidInput(msg) => {
                assert!(
                    msg.contains("GRAVITY"),
                    "should name the unsupported card: {msg}"
                );
            }
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }
}
