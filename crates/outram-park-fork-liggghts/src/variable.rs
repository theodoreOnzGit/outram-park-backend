// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! Equal-style `variable` formulas (`variable.cpp` of LIGGGHTS-PUBLIC
//! `3d5c00f2`: `Variable::evaluate`, `Variable::math_function`,
//! `Variable::special_function`), the subset the `fix move/mesh`
//! `linear/variable` and `rotate/variable` movers need (gh:#1416).
//!
//! No equal-style evaluator existed in the workspace (searched `crates/`
//! for expression evaluation: only `outram-blender`'s unrelated
//! transform-input parser).
//!
//! Supported: numeric literals, `PI`, `step`, `time`, `dt`, `v_name`
//! (another equal variable, resolved at parse time), the operators
//! `+ - * / % ^ == != < <= > >= && ||`, `!` and unary minus with upstream's
//! precedence table (`OR 1, AND 2, EQ/NE 3, LT/LE/GT/GE 4, ADD/SUB 5,
//! MUL/DIV/MOD 6, CARAT 7, UNARY/NOT 8`, left-associative because upstream
//! pops while `precedence[top] >= precedence[op]`), and the functions `sqrt
//! exp ln log abs sin cos tan asin acos atan atan2 ceil floor round ramp
//! swiggle cwiggle vdisplace` with upstream's argument guards. Not supported
//! (an `Err` at parse time, never silently wrong): `random`, `normal`,
//! `stagger`, `logfreq`, group/compute/fix references, atom-style variables.

use std::collections::HashMap;

/// Run state a formula is evaluated in (`update->ntimestep`,
/// `update->beginstep`, `update->endstep`, `update->dt`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VarContext {
    /// `update->ntimestep`.
    pub step: i64,
    /// `update->beginstep` (first step of the run).
    pub begin_step: i64,
    /// `update->endstep` (last step of the run).
    pub end_step: i64,
    /// `update->dt` `[s]`.
    pub dt: f64,
}

/// Operators of `Variable::evaluate`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    /// `||`
    Or,
    /// `&&`
    And,
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `%`
    Mod,
    /// `^`
    Pow,
    /// unary `-`
    Unary,
    /// `!`
    Not,
}

fn precedence(op: Op) -> u8 {
    match op {
        Op::Or => 1,
        Op::And => 2,
        Op::Eq | Op::Ne => 3,
        Op::Lt | Op::Le | Op::Gt | Op::Ge => 4,
        Op::Add | Op::Sub => 5,
        Op::Mul | Op::Div | Op::Mod => 6,
        Op::Pow => 7,
        Op::Unary | Op::Not => 8,
    }
}

/// A parsed equal-style formula.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A number or `PI`.
    Const(f64),
    /// Thermo keyword `step`.
    Step,
    /// Thermo keyword `time` (`update->get_cur_time()`).
    Time,
    /// Thermo keyword `dt`.
    Dt,
    /// Unary operator.
    Unary(Op, Box<Expr>),
    /// Binary operator.
    Binary(Op, Box<Expr>, Box<Expr>),
    /// Math or special function and its arguments.
    Func(String, Vec<Expr>),
}

/// A table of named equal-style variables (`variable name equal formula`).
#[derive(Debug, Default, Clone)]
pub struct VariableTable {
    formulas: HashMap<String, Expr>,
}

impl VariableTable {
    /// An empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `variable <name> equal <formula>`.
    ///
    /// # Errors
    /// On a formula the evaluator does not support or that is malformed.
    pub fn define_equal(&mut self, name: &str, formula: &str) -> Result<(), String> {
        let e = self.parse(formula)?;
        self.formulas.insert(name.to_string(), e);
        Ok(())
    }

    /// The formula of a defined variable.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Expr> {
        self.formulas.get(name)
    }

    /// Parse `formula` (`Variable::evaluate`'s operator-stack algorithm).
    ///
    /// # Errors
    /// On unsupported or malformed syntax.
    pub fn parse(&self, formula: &str) -> Result<Expr, String> {
        let s: Vec<char> = formula.chars().collect();
        let mut i = 0;
        let mut args: Vec<Expr> = Vec::new();
        let mut ops: Vec<Op> = Vec::new();
        let mut expect_arg = true;
        loop {
            while i < s.len() && s[i].is_whitespace() {
                i += 1;
            }
            if expect_arg {
                if i >= s.len() {
                    return Err("formula ends where an argument is expected".into());
                }
                let c = s[i];
                if c == '(' {
                    let (inner, next) = balanced(&s, i)?;
                    args.push(self.parse(&inner)?);
                    i = next;
                    expect_arg = false;
                } else if c.is_ascii_digit() || c == '.' {
                    let start = i;
                    while i < s.len() && (s[i].is_ascii_digit() || s[i] == '.') {
                        i += 1;
                    }
                    if i < s.len() && (s[i] == 'e' || s[i] == 'E') {
                        i += 1;
                        if i < s.len() && (s[i] == '+' || s[i] == '-') {
                            i += 1;
                        }
                        while i < s.len() && s[i].is_ascii_digit() {
                            i += 1;
                        }
                    }
                    let txt: String = s[start..i].iter().collect();
                    args.push(Expr::Const(txt.parse::<f64>().map_err(|_| format!("bad number {txt}"))?));
                    expect_arg = false;
                } else if c.is_ascii_alphabetic() || c == '_' {
                    let start = i;
                    while i < s.len() && (s[i].is_ascii_alphanumeric() || s[i] == '_') {
                        i += 1;
                    }
                    let word: String = s[start..i].iter().collect();
                    if i < s.len() && s[i] == '(' {
                        let (inner, next) = balanced(&s, i)?;
                        let mut list = Vec::new();
                        for a in split_args(&inner) {
                            list.push(self.parse(&a)?);
                        }
                        check_function(&word, list.len())?;
                        args.push(Expr::Func(word, list));
                        i = next;
                    } else if let Some(name) = word.strip_prefix("v_") {
                        args.push(
                            self.formulas
                                .get(name)
                                .cloned()
                                .ok_or_else(|| format!("unknown variable {name}"))?,
                        );
                    } else {
                        args.push(match word.as_str() {
                            "PI" => Expr::Const(std::f64::consts::PI),
                            "step" => Expr::Step,
                            "time" => Expr::Time,
                            "dt" => Expr::Dt,
                            _ => return Err(format!("unsupported term {word}")),
                        });
                    }
                    expect_arg = false;
                } else if c == '-' {
                    ops.push(Op::Unary);
                    i += 1;
                } else if c == '!' && s.get(i + 1) != Some(&'=') {
                    ops.push(Op::Not);
                    i += 1;
                } else {
                    return Err(format!("invalid syntax at '{c}'"));
                }
                continue;
            }
            // an operator, or the end of the formula
            let op = if i >= s.len() {
                None
            } else {
                let c = s[i];
                let n = s.get(i + 1).copied();
                i += 1;
                let two = |i: &mut usize, op: Op| {
                    *i += 1;
                    op
                };
                Some(match (c, n) {
                    ('+', _) => Op::Add,
                    ('-', _) => Op::Sub,
                    ('*', _) => Op::Mul,
                    ('/', _) => Op::Div,
                    ('%', _) => Op::Mod,
                    ('^', _) => Op::Pow,
                    ('=', Some('=')) => two(&mut i, Op::Eq),
                    ('!', Some('=')) => two(&mut i, Op::Ne),
                    ('<', Some('=')) => two(&mut i, Op::Le),
                    ('<', _) => Op::Lt,
                    ('>', Some('=')) => two(&mut i, Op::Ge),
                    ('>', _) => Op::Gt,
                    ('&', Some('&')) => two(&mut i, Op::And),
                    ('|', Some('|')) => two(&mut i, Op::Or),
                    _ => return Err(format!("invalid operator '{c}'")),
                })
            };
            expect_arg = true;
            // evaluate the stack as deep as precedence allows (DONE = 0)
            let p = op.map_or(0, precedence);
            while let Some(&top) = ops.last() {
                if precedence(top) < p {
                    break;
                }
                ops.pop();
                let b = args.pop().ok_or("stack underflow")?;
                if matches!(top, Op::Unary | Op::Not) {
                    args.push(Expr::Unary(top, Box::new(b)));
                } else {
                    let a = args.pop().ok_or("stack underflow")?;
                    args.push(Expr::Binary(top, Box::new(a), Box::new(b)));
                }
            }
            match op {
                Some(o) => ops.push(o),
                None => break,
            }
        }
        if args.len() != 1 {
            return Err("malformed formula".into());
        }
        Ok(args.pop().expect("one argument"))
    }
}

/// Content of the parenthesis group opening at `s[open]`, and the index after
/// its closing parenthesis.
fn balanced(s: &[char], open: usize) -> Result<(String, usize), String> {
    let mut depth = 0;
    for (k, &c) in s.iter().enumerate().skip(open) {
        if c == '(' {
            depth += 1;
        } else if c == ')' {
            depth -= 1;
            if depth == 0 {
                return Ok((s[open + 1..k].iter().collect(), k + 1));
            }
        }
    }
    Err("unbalanced parentheses".into())
}

fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '(' => {
                depth += 1;
                cur.push(c);
            }
            ')' => {
                depth -= 1;
                cur.push(c);
            }
            ',' if depth == 0 => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

fn check_function(name: &str, n: usize) -> Result<(), String> {
    let want = match name {
        "sqrt" | "exp" | "ln" | "log" | "abs" | "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "ceil"
        | "floor" | "round" => 1,
        "atan2" | "ramp" | "vdisplace" => 2,
        "swiggle" | "cwiggle" => 3,
        _ => return Err(format!("unsupported function {name}")),
    };
    if n == want {
        Ok(())
    } else {
        Err(format!("{name} takes {want} arguments"))
    }
}

fn flag(c: bool) -> f64 {
    if c {
        1.0
    } else {
        0.0
    }
}

impl Expr {
    /// `Variable::compute_equal`: evaluate in `ctx`.
    ///
    /// # Errors
    /// On a guarded domain error (divide by 0, `sqrt` of a negative, ...),
    /// which upstream raises as `error->all`.
    pub fn eval(&self, ctx: &VarContext) -> Result<f64, String> {
        Ok(match self {
            Expr::Const(v) => *v,
            Expr::Step => ctx.step as f64,
            // `Update::get_cur_time`: atime + (ntimestep - atimestep)*dt with atime = 0
            Expr::Time => ctx.step as f64 * ctx.dt,
            Expr::Dt => ctx.dt,
            Expr::Unary(Op::Unary, a) => -a.eval(ctx)?,
            Expr::Unary(_, a) => flag(a.eval(ctx)? == 0.0),
            Expr::Binary(op, a, b) => {
                let (v1, v2) = (a.eval(ctx)?, b.eval(ctx)?);
                match op {
                    Op::Add => v1 + v2,
                    Op::Sub => v1 - v2,
                    Op::Mul => v1 * v2,
                    Op::Div => {
                        if v2 == 0.0 {
                            return Err("Divide by 0 in variable formula".into());
                        }
                        v1 / v2
                    }
                    Op::Mod => {
                        if v2 == 0.0 {
                            return Err("Modulo 0 in variable formula".into());
                        }
                        v1 % v2
                    }
                    Op::Pow => {
                        if v2 == 0.0 {
                            return Err("Power by 0 in variable formula".into());
                        }
                        v1.powf(v2)
                    }
                    Op::Eq => flag(v1 == v2),
                    Op::Ne => flag(v1 != v2),
                    Op::Lt => flag(v1 < v2),
                    Op::Le => flag(v1 <= v2),
                    Op::Gt => flag(v1 > v2),
                    Op::Ge => flag(v1 >= v2),
                    Op::And => flag(v1 != 0.0 && v2 != 0.0),
                    Op::Or => flag(v1 != 0.0 || v2 != 0.0),
                    Op::Unary | Op::Not => return Err("unary operator used as binary".into()),
                }
            }
            Expr::Func(name, a) => {
                let mut v = [0.0; 3];
                for (k, e) in a.iter().enumerate() {
                    v[k] = e.eval(ctx)?;
                }
                let delta = (ctx.step - ctx.begin_step) as f64;
                match name.as_str() {
                    "sqrt" => {
                        if v[0] < 0.0 {
                            return Err("Sqrt of negative in variable formula".into());
                        }
                        v[0].sqrt()
                    }
                    "exp" => v[0].exp(),
                    "ln" | "log" => {
                        if v[0] <= 0.0 {
                            return Err("Log of zero/negative in variable formula".into());
                        }
                        if name == "ln" {
                            v[0].ln()
                        } else {
                            v[0].log10()
                        }
                    }
                    "abs" => v[0].abs(),
                    "sin" => v[0].sin(),
                    "cos" => v[0].cos(),
                    "tan" => v[0].tan(),
                    "asin" | "acos" => {
                        if !(-1.0..=1.0).contains(&v[0]) {
                            return Err("Arcsin/arccos of invalid value in variable formula".into());
                        }
                        if name == "asin" {
                            v[0].asin()
                        } else {
                            v[0].acos()
                        }
                    }
                    "atan" => v[0].atan(),
                    "atan2" => v[0].atan2(v[1]),
                    "ceil" => v[0].ceil(),
                    "floor" => v[0].floor(),
                    // MYROUND(a) = (a-floor(a) >= 0.5) ? ceil(a) : floor(a)
                    "round" => {
                        if v[0] - v[0].floor() >= 0.5 {
                            v[0].ceil()
                        } else {
                            v[0].floor()
                        }
                    }
                    "ramp" => {
                        if ctx.end_step == ctx.begin_step {
                            return Err("Cannot use ramp in variable formula with zero-length run".into());
                        }
                        let d = (ctx.step - ctx.begin_step) as f64 / (ctx.end_step - ctx.begin_step) as f64;
                        v[0] + d * (v[1] - v[0])
                    }
                    "vdisplace" => v[0] + v[1] * delta * ctx.dt,
                    "swiggle" | "cwiggle" => {
                        if v[2] == 0.0 {
                            return Err("Invalid math function in variable formula".into());
                        }
                        let omega = 2.0 * std::f64::consts::PI / v[2];
                        if name == "swiggle" {
                            v[0] + v[1] * (omega * delta * ctx.dt).sin()
                        } else {
                            v[0] + v[1] * (1.0 - (omega * delta * ctx.dt).cos())
                        }
                    }
                    other => return Err(format!("unsupported function {other}")),
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(step: i64) -> VarContext {
        VarContext { step, begin_step: 0, end_step: 1000, dt: 1e-5 }
    }

    /// Precedence and associativity follow upstream's table: `-2^2` is
    /// `(-2)^2 = 4` (UNARY above CARAT), `8/2/2 = 2`, `1+2*3 = 7`.
    #[test]
    fn precedence_matches_upstream() {
        let t = VariableTable::new();
        let c = ctx(1);
        assert_eq!(t.parse("-2^2").unwrap().eval(&c).unwrap(), 4.0);
        assert_eq!(t.parse("8/2/2").unwrap().eval(&c).unwrap(), 2.0);
        assert_eq!(t.parse("1+2*3").unwrap().eval(&c).unwrap(), 7.0);
        assert_eq!(t.parse("2^3^2").unwrap().eval(&c).unwrap(), 64.0);
        assert_eq!(t.parse("(1+2)*3 > 8 && 1").unwrap().eval(&c).unwrap(), 1.0);
    }

    /// Keywords, named variables, functions and every guard.
    #[test]
    fn keywords_functions_and_guards() {
        let mut t = VariableTable::new();
        t.define_equal("w", "2*PI/0.1").unwrap();
        let e = t.parse("0.3*sin(v_w*time)").unwrap();
        let c = ctx(250);
        let want = 0.3 * ((2.0 * std::f64::consts::PI / 0.1) * (250.0 * 1e-5)).sin();
        assert_eq!(e.eval(&c).unwrap().to_bits(), want.to_bits());
        assert_eq!(t.parse("step").unwrap().eval(&c).unwrap(), 250.0);
        assert_eq!(t.parse("dt").unwrap().eval(&c).unwrap(), 1e-5);
        let w = 2.0 * std::f64::consts::PI / 0.5;
        let sw = t.parse("swiggle(1,2,0.5)").unwrap().eval(&c).unwrap();
        assert_eq!(sw, 1.0 + 2.0 * (w * 250.0 * 1e-5).sin());
        let cw = t.parse("cwiggle(1,2,0.5)").unwrap().eval(&c).unwrap();
        assert_eq!(cw, 1.0 + 2.0 * (1.0 - (w * 250.0 * 1e-5).cos()));
        assert_eq!(t.parse("vdisplace(1,2)").unwrap().eval(&c).unwrap(), 1.0 + 2.0 * 250.0 * 1e-5);
        assert_eq!(t.parse("ramp(0,10)").unwrap().eval(&ctx(500)).unwrap(), 5.0);
        assert_eq!(t.parse("round(2.5)").unwrap().eval(&c).unwrap(), 3.0);
        assert_eq!(
            t.parse("floor(2.9)+ceil(2.1)+abs(-1)+sqrt(4)+exp(0)").unwrap().eval(&c).unwrap(),
            9.0
        );
        let m = t
            .parse("ln(2)+log(100)+tan(0)+atan(0)+atan2(0,1)+asin(0)+acos(1)+cos(0)")
            .unwrap()
            .eval(&c)
            .unwrap();
        assert!((m - (2f64.ln() + 3.0)).abs() < 1e-15);
        let l = t
            .parse("5 % 3 + !0 + (1 < 2) + (2 <= 1) + (1 == 1) + (1 != 1) + (1 || 0) + (3 >= 3) + (3 > 4)")
            .unwrap()
            .eval(&c)
            .unwrap();
        assert_eq!(l, 7.0);
        for bad in ["1/0", "5 % 0", "2^0", "sqrt(0-1)", "ln(0)", "log(0)", "asin(2)", "swiggle(1,2,0)"] {
            assert!(t.parse(bad).unwrap().eval(&c).is_err(), "{bad}");
        }
        assert!(t.parse("ramp(0,1)").unwrap().eval(&VarContext { end_step: 0, ..c }).is_err());
        for bad in ["random(0,1,1)", "v_nope", "1 +", "(1", "sin(1,2)", "1 $ 2", "foo"] {
            assert!(t.parse(bad).is_err(), "{bad}");
        }
    }
}
