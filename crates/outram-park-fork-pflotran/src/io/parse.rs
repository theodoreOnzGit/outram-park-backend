//! Card-based ASCII input-deck parser for the v1 RICHARDS slice.
//!
//! # AI-designed subset — needs human review
//!
//! This grammar is a small, **AI-designed subset** invented for this crate's
//! vertical slice. It is *inspired by* PFLOTRAN's card/block input style but is
//! **not** the real PFLOTRAN input-deck syntax and is not compatible with it.
//! It must be reviewed by a human against genuine PFLOTRAN input decks before
//! any claim of fidelity is made (per the workspace `RESPONSIBLE_USE.md` rule
//! that AI-generated artifacts are untrusted drafts until reviewed).
//!
//! # Grammar
//!
//! The deck is line-oriented and case-insensitive for keywords:
//!
//! - A `#` or `!` starts a comment that runs to end of line (anywhere on the
//!   line; text before it is still parsed).
//! - Blank lines and whitespace-only lines are ignored.
//! - Tokens on a line are whitespace-separated.
//! - A **block** opens with a block keyword on its own line and closes with a
//!   line whose only token is `END` or `/`.
//! - Numbers accept Rust `f64` / `usize` syntax (e.g. `1.0e-12`, `86400.0`).
//!
//! Recognised top-level cards:
//!
//! ```text
//! GRID
//!   STRUCTURED nx ny nz         # cell counts (usize > 0)
//!   DXYZ dx dy dz               # uniform cell sizes, metres (f64 > 0)
//! END
//!
//! MATERIAL
//!   POROSITY phi                # (-), 0 < phi < 1
//!   PERMEABILITY k              # m^2, k > 0
//! END
//!
//! CHARACTERISTIC_CURVES
//!   MODEL VAN_GENUCHTEN         # or BROOKS_COREY
//!   ALPHA a                     # 1/Pa, a > 0
//!   N n                         # van-Genuchten n  (or LAMBDA for Brooks-Corey)
//!   RESIDUAL_SATURATION sr      # (-), 0 <= sr < 1
//! END
//!
//! INITIAL_PRESSURE p            # single-line card, Pa
//!
//! BOUNDARY_CONDITION            # zero or more of these blocks
//!   LOCATION XMIN               # XMIN|XMAX|YMIN|YMAX|ZMIN|ZMAX
//!   DIRICHLET_PRESSURE p        # Pa  (mutually exclusive with NEUMANN_FLUX)
//!   NEUMANN_FLUX q              # m/s
//! END
//!
//! TIME
//!   FINAL_TIME t                # s, t > 0
//!   INITIAL_DT dt0              # s, dt0 > 0
//!   MAX_DT dtmax                # s, dtmax > 0
//! END
//! ```
//!
//! Required blocks: `GRID`, `MATERIAL`, `CHARACTERISTIC_CURVES`, `TIME`, and the
//! `INITIAL_PRESSURE` card. `BOUNDARY_CONDITION` blocks are optional and may
//! repeat. Any unknown keyword, missing required block, or out-of-range value
//! is a [`PflotranError::InvalidInput`] carrying the offending line number.

use crate::error::PflotranError;

use super::spec::{
    BoundaryConditionSpec, BoundaryKindSpec, BoundaryLocationSpec, CurveModel, CurveSpec, GridSpec,
    InputDeck, MaterialSpec, TimeSpec,
};

/// One non-empty, comment-stripped source line: its 1-based line number and its
/// whitespace-separated tokens (original case preserved for value tokens).
struct Line {
    /// 1-based line number in the original source (for error messages).
    number: usize,
    /// Whitespace-separated tokens on the line (at least one).
    tokens: Vec<String>,
}

/// Strip a `#`/`!` comment, split into tokens, and drop blank lines.
fn tokenize(input: &str) -> Vec<Line> {
    let mut out = Vec::new();
    for (idx, raw) in input.lines().enumerate() {
        // Cut everything from the first comment marker onward.
        let code = match raw.find(|c| c == '#' || c == '!') {
            Some(pos) => &raw[..pos],
            None => raw,
        };
        let tokens: Vec<String> = code.split_whitespace().map(|s| s.to_string()).collect();
        if !tokens.is_empty() {
            out.push(Line {
                number: idx + 1,
                tokens,
            });
        }
    }
    out
}

/// Build an `InvalidInput` error tagged with a line number.
fn err(line: usize, msg: impl AsRef<str>) -> PflotranError {
    PflotranError::InvalidInput(format!("line {}: {}", line, msg.as_ref()))
}

/// Parse a token as `f64`, attaching line context on failure.
fn parse_f64(line: usize, tok: &str) -> Result<f64, PflotranError> {
    tok.parse::<f64>()
        .map_err(|_| err(line, format!("expected a real number, found `{}`", tok)))
        .and_then(|v| {
            if v.is_finite() {
                Ok(v)
            } else {
                Err(err(line, format!("value `{}` is not finite", tok)))
            }
        })
}

/// Parse a token as `usize`, attaching line context on failure.
fn parse_usize(line: usize, tok: &str) -> Result<usize, PflotranError> {
    tok.parse::<usize>().map_err(|_| {
        err(
            line,
            format!("expected a non-negative integer, found `{}`", tok),
        )
    })
}

/// Require exactly `n` value tokens after the keyword; error otherwise.
fn expect_values<'a>(
    line: &'a Line,
    keyword: &str,
    n: usize,
) -> Result<&'a [String], PflotranError> {
    let vals = &line.tokens[1..];
    if vals.len() == n {
        Ok(vals)
    } else {
        Err(err(
            line.number,
            format!(
                "keyword `{}` expects {} value(s), found {}",
                keyword,
                n,
                vals.len()
            ),
        ))
    }
}

/// True if a line is a block terminator (`END` or `/`, case-insensitive).
fn is_terminator(line: &Line) -> bool {
    line.tokens.len() == 1 && (line.tokens[0].eq_ignore_ascii_case("END") || line.tokens[0] == "/")
}

/// Parse a v1 input deck from ASCII text.
///
/// See the [module documentation](self) for the full card grammar. On success
/// returns a fully populated, validated [`InputDeck`]; on any lexical, missing
/// block, or range error returns [`PflotranError::InvalidInput`] whose message
/// begins with the offending 1-based line number.
///
/// # Validation
///
/// Enforces: grid counts `> 0`, cell sizes `> 0`, porosity in `(0, 1)`,
/// permeability `> 0`, `alpha > 0`, `n_or_lambda > 0`, residual saturation in
/// `[0, 1)`, all times `> 0`, and a finite initial pressure.
pub fn parse_deck(input: &str) -> Result<InputDeck, PflotranError> {
    let lines = tokenize(input);

    let mut grid: Option<GridSpec> = None;
    let mut material: Option<MaterialSpec> = None;
    let mut curves: Option<CurveSpec> = None;
    let mut time: Option<TimeSpec> = None;
    let mut initial_pressure: Option<f64> = None;
    let mut boundary_conditions: Vec<BoundaryConditionSpec> = Vec::new();

    let mut i = 0;
    while i < lines.len() {
        let head = &lines[i];
        let keyword = head.tokens[0].to_ascii_uppercase();
        match keyword.as_str() {
            "GRID" => {
                let (spec, next) = parse_grid(&lines, i)?;
                grid = Some(spec);
                i = next;
            }
            "MATERIAL" => {
                let (spec, next) = parse_material(&lines, i)?;
                material = Some(spec);
                i = next;
            }
            "CHARACTERISTIC_CURVES" => {
                let (spec, next) = parse_curves(&lines, i)?;
                curves = Some(spec);
                i = next;
            }
            "BOUNDARY_CONDITION" => {
                let (spec, next) = parse_boundary(&lines, i)?;
                boundary_conditions.push(spec);
                i = next;
            }
            "TIME" => {
                let (spec, next) = parse_time(&lines, i)?;
                time = Some(spec);
                i = next;
            }
            "INITIAL_PRESSURE" => {
                let vals = expect_values(head, "INITIAL_PRESSURE", 1)?;
                initial_pressure = Some(parse_f64(head.number, &vals[0])?);
                i += 1;
            }
            other => {
                return Err(err(
                    head.number,
                    format!("unknown top-level keyword `{}`", other),
                ));
            }
        }
    }

    let grid =
        grid.ok_or_else(|| PflotranError::InvalidInput("missing required GRID block".into()))?;
    let material = material
        .ok_or_else(|| PflotranError::InvalidInput("missing required MATERIAL block".into()))?;
    let curves = curves.ok_or_else(|| {
        PflotranError::InvalidInput("missing required CHARACTERISTIC_CURVES block".into())
    })?;
    let time =
        time.ok_or_else(|| PflotranError::InvalidInput("missing required TIME block".into()))?;
    let initial_pressure = initial_pressure.ok_or_else(|| {
        PflotranError::InvalidInput("missing required INITIAL_PRESSURE card".into())
    })?;

    Ok(InputDeck {
        grid,
        material,
        curves,
        boundary_conditions,
        time,
        initial_pressure,
    })
}

/// Iterate the body lines of the block that opens at `open`, returning each
/// inner line, until a terminator (or end of input, which is an error).
///
/// Returns `Ok(index_after_terminator)` via the `on_line` accumulation; the
/// closure is called for every non-terminator body line.
fn walk_block(
    lines: &[Line],
    open: usize,
    block_name: &str,
    mut on_line: impl FnMut(&Line) -> Result<(), PflotranError>,
) -> Result<usize, PflotranError> {
    let mut i = open + 1;
    while i < lines.len() {
        if is_terminator(&lines[i]) {
            return Ok(i + 1);
        }
        on_line(&lines[i])?;
        i += 1;
    }
    Err(err(
        lines[open].number,
        format!("`{}` block is not closed with END or `/`", block_name),
    ))
}

/// Parse a `GRID` block. Returns the spec and the index just past its `END`.
fn parse_grid(lines: &[Line], open: usize) -> Result<(GridSpec, usize), PflotranError> {
    let mut counts: Option<(usize, usize, usize)> = None;
    let mut sizes: Option<(f64, f64, f64)> = None;

    let next = walk_block(lines, open, "GRID", |line| {
        let kw = line.tokens[0].to_ascii_uppercase();
        match kw.as_str() {
            "STRUCTURED" => {
                let v = expect_values(line, "STRUCTURED", 3)?;
                counts = Some((
                    parse_usize(line.number, &v[0])?,
                    parse_usize(line.number, &v[1])?,
                    parse_usize(line.number, &v[2])?,
                ));
                Ok(())
            }
            "DXYZ" => {
                let v = expect_values(line, "DXYZ", 3)?;
                sizes = Some((
                    parse_f64(line.number, &v[0])?,
                    parse_f64(line.number, &v[1])?,
                    parse_f64(line.number, &v[2])?,
                ));
                Ok(())
            }
            other => Err(err(
                line.number,
                format!("unknown GRID keyword `{}`", other),
            )),
        }
    })?;

    let open_line = lines[open].number;
    let (nx, ny, nz) =
        counts.ok_or_else(|| err(open_line, "GRID block missing STRUCTURED card"))?;
    let (dx, dy, dz) = sizes.ok_or_else(|| err(open_line, "GRID block missing DXYZ card"))?;

    if nx == 0 || ny == 0 || nz == 0 {
        return Err(err(open_line, "grid cell counts must all be > 0"));
    }
    if !(dx > 0.0 && dy > 0.0 && dz > 0.0) {
        return Err(err(open_line, "grid cell sizes DXYZ must all be > 0"));
    }

    Ok((
        GridSpec {
            nx,
            ny,
            nz,
            dx,
            dy,
            dz,
        },
        next,
    ))
}

/// Parse a `MATERIAL` block. Returns the spec and the index past its `END`.
fn parse_material(lines: &[Line], open: usize) -> Result<(MaterialSpec, usize), PflotranError> {
    let mut porosity: Option<f64> = None;
    let mut permeability: Option<f64> = None;

    let next = walk_block(lines, open, "MATERIAL", |line| {
        let kw = line.tokens[0].to_ascii_uppercase();
        match kw.as_str() {
            "POROSITY" => {
                let v = expect_values(line, "POROSITY", 1)?;
                porosity = Some(parse_f64(line.number, &v[0])?);
                Ok(())
            }
            "PERMEABILITY" => {
                let v = expect_values(line, "PERMEABILITY", 1)?;
                permeability = Some(parse_f64(line.number, &v[0])?);
                Ok(())
            }
            other => Err(err(
                line.number,
                format!("unknown MATERIAL keyword `{}`", other),
            )),
        }
    })?;

    let open_line = lines[open].number;
    let porosity = porosity.ok_or_else(|| err(open_line, "MATERIAL block missing POROSITY"))?;
    let permeability =
        permeability.ok_or_else(|| err(open_line, "MATERIAL block missing PERMEABILITY"))?;

    if !(porosity > 0.0 && porosity < 1.0) {
        return Err(err(
            open_line,
            format!(
                "porosity must be in the open interval (0, 1), got {}",
                porosity
            ),
        ));
    }
    if !(permeability > 0.0) {
        return Err(err(
            open_line,
            format!("permeability must be > 0, got {}", permeability),
        ));
    }

    Ok((
        MaterialSpec {
            porosity,
            permeability,
        },
        next,
    ))
}

/// Parse a `CHARACTERISTIC_CURVES` block. Returns the spec and index past `END`.
fn parse_curves(lines: &[Line], open: usize) -> Result<(CurveSpec, usize), PflotranError> {
    let mut model: Option<CurveModel> = None;
    let mut alpha: Option<f64> = None;
    let mut n_or_lambda: Option<f64> = None;
    let mut residual: Option<f64> = None;

    let next = walk_block(lines, open, "CHARACTERISTIC_CURVES", |line| {
        let kw = line.tokens[0].to_ascii_uppercase();
        match kw.as_str() {
            "MODEL" => {
                let v = expect_values(line, "MODEL", 1)?;
                model = Some(match v[0].to_ascii_uppercase().as_str() {
                    "VAN_GENUCHTEN" => CurveModel::VanGenuchten,
                    "BROOKS_COREY" => CurveModel::BrooksCorey,
                    other => {
                        return Err(err(
                            line.number,
                            format!(
                                "unknown curve MODEL `{}` (expected VAN_GENUCHTEN or BROOKS_COREY)",
                                other
                            ),
                        ))
                    }
                });
                Ok(())
            }
            "ALPHA" => {
                let v = expect_values(line, "ALPHA", 1)?;
                alpha = Some(parse_f64(line.number, &v[0])?);
                Ok(())
            }
            // `N` (van-Genuchten) and `LAMBDA` (Brooks-Corey) both map to n_or_lambda.
            "N" | "LAMBDA" => {
                let v = expect_values(line, &kw, 1)?;
                n_or_lambda = Some(parse_f64(line.number, &v[0])?);
                Ok(())
            }
            "RESIDUAL_SATURATION" => {
                let v = expect_values(line, "RESIDUAL_SATURATION", 1)?;
                residual = Some(parse_f64(line.number, &v[0])?);
                Ok(())
            }
            other => Err(err(
                line.number,
                format!("unknown CHARACTERISTIC_CURVES keyword `{}`", other),
            )),
        }
    })?;

    let open_line = lines[open].number;
    let model = model.ok_or_else(|| err(open_line, "CHARACTERISTIC_CURVES block missing MODEL"))?;
    let alpha = alpha.ok_or_else(|| err(open_line, "CHARACTERISTIC_CURVES block missing ALPHA"))?;
    let n_or_lambda = n_or_lambda
        .ok_or_else(|| err(open_line, "CHARACTERISTIC_CURVES block missing N or LAMBDA"))?;
    let residual_saturation = residual.ok_or_else(|| {
        err(
            open_line,
            "CHARACTERISTIC_CURVES block missing RESIDUAL_SATURATION",
        )
    })?;

    if !(alpha > 0.0) {
        return Err(err(open_line, format!("ALPHA must be > 0, got {}", alpha)));
    }
    if !(n_or_lambda > 0.0) {
        return Err(err(
            open_line,
            format!("N / LAMBDA must be > 0, got {}", n_or_lambda),
        ));
    }
    if !(residual_saturation >= 0.0 && residual_saturation < 1.0) {
        return Err(err(
            open_line,
            format!(
                "RESIDUAL_SATURATION must be in [0, 1), got {}",
                residual_saturation
            ),
        ));
    }

    Ok((
        CurveSpec {
            model,
            alpha,
            n_or_lambda,
            residual_saturation,
        },
        next,
    ))
}

/// Parse a `BOUNDARY_CONDITION` block. Returns the spec and index past `END`.
fn parse_boundary(
    lines: &[Line],
    open: usize,
) -> Result<(BoundaryConditionSpec, usize), PflotranError> {
    let mut location: Option<BoundaryLocationSpec> = None;
    let mut kind: Option<BoundaryKindSpec> = None;

    let next = walk_block(lines, open, "BOUNDARY_CONDITION", |line| {
        let kw = line.tokens[0].to_ascii_uppercase();
        match kw.as_str() {
            "LOCATION" => {
                let v = expect_values(line, "LOCATION", 1)?;
                location = Some(match v[0].to_ascii_uppercase().as_str() {
                    "XMIN" => BoundaryLocationSpec::XMin,
                    "XMAX" => BoundaryLocationSpec::XMax,
                    "YMIN" => BoundaryLocationSpec::YMin,
                    "YMAX" => BoundaryLocationSpec::YMax,
                    "ZMIN" => BoundaryLocationSpec::ZMin,
                    "ZMAX" => BoundaryLocationSpec::ZMax,
                    other => {
                        return Err(err(
                            line.number,
                            format!(
                                "unknown LOCATION `{}` (expected XMIN/XMAX/YMIN/YMAX/ZMIN/ZMAX)",
                                other
                            ),
                        ))
                    }
                });
                Ok(())
            }
            "DIRICHLET_PRESSURE" => {
                let v = expect_values(line, "DIRICHLET_PRESSURE", 1)?;
                if kind.is_some() {
                    return Err(err(
                        line.number,
                        "boundary condition already has a kind (DIRICHLET_PRESSURE / NEUMANN_FLUX are mutually exclusive)",
                    ));
                }
                kind = Some(BoundaryKindSpec::DirichletPressure(parse_f64(
                    line.number,
                    &v[0],
                )?));
                Ok(())
            }
            "NEUMANN_FLUX" => {
                let v = expect_values(line, "NEUMANN_FLUX", 1)?;
                if kind.is_some() {
                    return Err(err(
                        line.number,
                        "boundary condition already has a kind (DIRICHLET_PRESSURE / NEUMANN_FLUX are mutually exclusive)",
                    ));
                }
                kind = Some(BoundaryKindSpec::NeumannFlux(parse_f64(
                    line.number,
                    &v[0],
                )?));
                Ok(())
            }
            other => Err(err(
                line.number,
                format!("unknown BOUNDARY_CONDITION keyword `{}`", other),
            )),
        }
    })?;

    let open_line = lines[open].number;
    let location =
        location.ok_or_else(|| err(open_line, "BOUNDARY_CONDITION block missing LOCATION"))?;
    let kind = kind.ok_or_else(|| {
        err(
            open_line,
            "BOUNDARY_CONDITION block missing DIRICHLET_PRESSURE or NEUMANN_FLUX",
        )
    })?;

    Ok((BoundaryConditionSpec { location, kind }, next))
}

/// Parse a `TIME` block. Returns the spec and the index past its `END`.
fn parse_time(lines: &[Line], open: usize) -> Result<(TimeSpec, usize), PflotranError> {
    let mut final_time: Option<f64> = None;
    let mut initial_dt: Option<f64> = None;
    let mut max_dt: Option<f64> = None;

    let next = walk_block(lines, open, "TIME", |line| {
        let kw = line.tokens[0].to_ascii_uppercase();
        match kw.as_str() {
            "FINAL_TIME" => {
                let v = expect_values(line, "FINAL_TIME", 1)?;
                final_time = Some(parse_f64(line.number, &v[0])?);
                Ok(())
            }
            "INITIAL_DT" => {
                let v = expect_values(line, "INITIAL_DT", 1)?;
                initial_dt = Some(parse_f64(line.number, &v[0])?);
                Ok(())
            }
            "MAX_DT" => {
                let v = expect_values(line, "MAX_DT", 1)?;
                max_dt = Some(parse_f64(line.number, &v[0])?);
                Ok(())
            }
            other => Err(err(
                line.number,
                format!("unknown TIME keyword `{}`", other),
            )),
        }
    })?;

    let open_line = lines[open].number;
    let final_time = final_time.ok_or_else(|| err(open_line, "TIME block missing FINAL_TIME"))?;
    let initial_dt = initial_dt.ok_or_else(|| err(open_line, "TIME block missing INITIAL_DT"))?;
    let max_dt = max_dt.ok_or_else(|| err(open_line, "TIME block missing MAX_DT"))?;

    if !(final_time > 0.0 && initial_dt > 0.0 && max_dt > 0.0) {
        return Err(err(open_line, "TIME values must all be > 0"));
    }

    Ok((
        TimeSpec {
            final_time,
            initial_dt,
            max_dt,
        },
        next,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = "\
# v1 pflotran-lite deck
GRID
  STRUCTURED 10 1 1
  DXYZ 0.1 1.0 1.0
END
MATERIAL
  POROSITY 0.35
  PERMEABILITY 1.0e-12
END
CHARACTERISTIC_CURVES
  MODEL VAN_GENUCHTEN
  ALPHA 1.0e-4
  N 2.0
  RESIDUAL_SATURATION 0.1
END
INITIAL_PRESSURE 90000.0
BOUNDARY_CONDITION
  LOCATION XMIN
  DIRICHLET_PRESSURE 101325.0
END
TIME
  FINAL_TIME 86400.0
  INITIAL_DT 10.0
  MAX_DT 3600.0
END
";

    #[test]
    fn parses_the_reference_example_field_by_field() {
        let deck = parse_deck(EXAMPLE).expect("reference deck should parse");

        assert_eq!(deck.grid.nx, 10);
        assert_eq!(deck.grid.ny, 1);
        assert_eq!(deck.grid.nz, 1);
        assert_eq!(deck.grid.dx, 0.1);
        assert_eq!(deck.grid.dy, 1.0);
        assert_eq!(deck.grid.dz, 1.0);

        assert_eq!(deck.material.porosity, 0.35);
        assert_eq!(deck.material.permeability, 1.0e-12);

        assert_eq!(deck.curves.model, CurveModel::VanGenuchten);
        assert_eq!(deck.curves.alpha, 1.0e-4);
        assert_eq!(deck.curves.n_or_lambda, 2.0);
        assert_eq!(deck.curves.residual_saturation, 0.1);

        assert_eq!(deck.initial_pressure, 90000.0);

        assert_eq!(deck.boundary_conditions.len(), 1);
        assert_eq!(
            deck.boundary_conditions[0].location,
            BoundaryLocationSpec::XMin
        );
        assert_eq!(
            deck.boundary_conditions[0].kind,
            BoundaryKindSpec::DirichletPressure(101325.0)
        );

        assert_eq!(deck.time.final_time, 86400.0);
        assert_eq!(deck.time.initial_dt, 10.0);
        assert_eq!(deck.time.max_dt, 3600.0);
    }

    #[test]
    fn robust_to_comments_blank_lines_mixed_case_and_whitespace() {
        let src = "\n\n   grid   ! inline comment after keyword
\tstructured   4   2   3
   DxYz  0.5  0.25  2.0    # trailing comment

/
material
   porosity 0.4
   PERMEABILITY   5e-13
/

characteristic_curves
   model brooks_corey
   alpha 2e-4
   lambda 1.5
   residual_saturation 0.05
/
initial_pressure   101325
boundary_condition
   location zmax
   neumann_flux -1.0e-7
/
time
   final_time 3600
   initial_dt 1
   max_dt 60
/
";
        let deck = parse_deck(src).expect("lenient deck should parse");
        assert_eq!((deck.grid.nx, deck.grid.ny, deck.grid.nz), (4, 2, 3));
        assert_eq!(deck.grid.dx, 0.5);
        assert_eq!(deck.material.porosity, 0.4);
        assert_eq!(deck.material.permeability, 5e-13);
        assert_eq!(deck.curves.model, CurveModel::BrooksCorey);
        assert_eq!(deck.curves.n_or_lambda, 1.5);
        assert_eq!(deck.initial_pressure, 101325.0);
        assert_eq!(
            deck.boundary_conditions[0].location,
            BoundaryLocationSpec::ZMax
        );
        assert_eq!(
            deck.boundary_conditions[0].kind,
            BoundaryKindSpec::NeumannFlux(-1.0e-7)
        );
    }

    #[test]
    fn multiple_boundary_conditions_accumulate() {
        let src = "\
GRID
STRUCTURED 2 1 1
DXYZ 1 1 1
END
MATERIAL
POROSITY 0.3
PERMEABILITY 1e-12
END
CHARACTERISTIC_CURVES
MODEL VAN_GENUCHTEN
ALPHA 1e-4
N 2
RESIDUAL_SATURATION 0.1
END
INITIAL_PRESSURE 90000
BOUNDARY_CONDITION
LOCATION XMIN
DIRICHLET_PRESSURE 101325
END
BOUNDARY_CONDITION
LOCATION XMAX
DIRICHLET_PRESSURE 90000
END
TIME
FINAL_TIME 100
INITIAL_DT 1
MAX_DT 10
END
";
        let deck = parse_deck(src).unwrap();
        assert_eq!(deck.boundary_conditions.len(), 2);
        assert_eq!(
            deck.boundary_conditions[1].location,
            BoundaryLocationSpec::XMax
        );
    }

    #[test]
    fn error_on_unknown_keyword_reports_line() {
        let src = "GRID\nSTRUCTURED 1 1 1\nWALDO 3\nDXYZ 1 1 1\nEND\n";
        let e = parse_deck(src).unwrap_err();
        let msg = format!("{}", e);
        assert!(msg.contains("line 3"), "got: {msg}");
        assert!(msg.contains("WALDO"), "got: {msg}");
    }

    #[test]
    fn error_on_out_of_range_porosity() {
        let src = "\
GRID
STRUCTURED 1 1 1
DXYZ 1 1 1
END
MATERIAL
POROSITY 1.5
PERMEABILITY 1e-12
END
CHARACTERISTIC_CURVES
MODEL VAN_GENUCHTEN
ALPHA 1e-4
N 2
RESIDUAL_SATURATION 0.1
END
INITIAL_PRESSURE 90000
TIME
FINAL_TIME 100
INITIAL_DT 1
MAX_DT 10
END
";
        let e = parse_deck(src).unwrap_err();
        let msg = format!("{}", e);
        assert!(msg.contains("porosity"), "got: {msg}");
    }

    #[test]
    fn error_on_missing_grid_block() {
        let src = "\
MATERIAL
POROSITY 0.3
PERMEABILITY 1e-12
END
CHARACTERISTIC_CURVES
MODEL VAN_GENUCHTEN
ALPHA 1e-4
N 2
RESIDUAL_SATURATION 0.1
END
INITIAL_PRESSURE 90000
TIME
FINAL_TIME 100
INITIAL_DT 1
MAX_DT 10
END
";
        let e = parse_deck(src).unwrap_err();
        assert!(format!("{}", e).contains("GRID"));
    }

    #[test]
    fn error_on_unclosed_block() {
        let src = "GRID\nSTRUCTURED 1 1 1\nDXYZ 1 1 1\n";
        let e = parse_deck(src).unwrap_err();
        assert!(format!("{}", e).contains("not closed"));
    }

    #[test]
    fn error_on_wrong_arity() {
        let src = "GRID\nSTRUCTURED 1 1\nDXYZ 1 1 1\nEND\n";
        let e = parse_deck(src).unwrap_err();
        let msg = format!("{}", e);
        assert!(msg.contains("expects 3"), "got: {msg}");
    }
}
