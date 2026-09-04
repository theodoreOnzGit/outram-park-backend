// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Numeric transform input and axis/plane constraints. Follows the published
// behaviour of Blender's transform-input layer (source/blender/editors/
// transform/transform_input.cc, transform_constraints.cc and transform_convert
// numeric entry, github.com/blender/blender, GPL-2.0-or-later): resolve a raw
// pointer delta into a constrained, optionally exactly-typed transform in a
// chosen coordinate space, with grid-increment snapping and expression entry.
// Concepts only — no upstream source copied.
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

//! **Numeric transform input + axis/plane constraints** (`op-hzs.54.23`, GH
//! issue #37 §D — the precision/CAD core).
//!
//! The parameter model every transform operator ([`crate::transform_ops`], the
//! upcoming snapping engine, PDT) consumes:
//!
//! - [`Constraint`] — free, locked to one axis, or locked to a plane.
//! - [`TransformBasis`] — three orthonormal vectors giving the coordinate space
//!   (global / local / normal / view). [`TransformBasis::global`] is the
//!   identity.
//! - [`NumericEntry`] — a per-component optional exact value, each parsed from
//!   a string with [`eval_expr`] (so `"1+1"`, `"pi/2"`, `"-tau"` all work).
//! - [`resolve_translation`] — combine a raw delta, a constraint, a basis,
//!   numeric overrides and a grid increment into the delta actually applied.
//! - [`apply_translation`] — move a vertex selection by a delta.

use crate::math::Vec3;
use crate::mesh::{Mesh, VertexId};

/// Which components of a transform are free to move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Constraint {
    /// No constraint — all three components free.
    Free,
    /// Locked to one basis axis (`0 = X`, `1 = Y`, `2 = Z` of the basis).
    Axis(u8),
    /// Locked to the plane **orthogonal** to one basis axis (that component is
    /// zeroed; the other two are free).
    Plane(u8),
}

/// Three orthonormal basis vectors defining a transform space.
#[derive(Debug, Clone, Copy)]
pub struct TransformBasis {
    pub x: Vec3,
    pub y: Vec3,
    pub z: Vec3,
}

impl TransformBasis {
    /// The world axes — Blender's *Global* orientation.
    pub fn global() -> Self {
        TransformBasis {
            x: Vec3::new(1.0, 0.0, 0.0),
            y: Vec3::new(0.0, 1.0, 0.0),
            z: Vec3::new(0.0, 0.0, 1.0),
        }
    }

    /// A basis whose `z` is `normal` (Blender's *Normal* orientation), with
    /// `x`/`y` an arbitrary orthonormal completion.
    pub fn from_normal(normal: Vec3) -> Self {
        let z = if normal.length() > 1e-9 {
            normal.normalize()
        } else {
            Vec3::new(0.0, 0.0, 1.0)
        };
        let up = if z.z.abs() < 0.9 {
            Vec3::new(0.0, 0.0, 1.0)
        } else {
            Vec3::new(1.0, 0.0, 0.0)
        };
        let x = up.cross(z).normalize();
        let y = z.cross(x);
        TransformBasis { x, y, z }
    }

    /// Express `v` in this basis (dot with each axis).
    fn to_local(self, v: Vec3) -> [f64; 3] {
        [v.dot(self.x), v.dot(self.y), v.dot(self.z)]
    }

    /// Reconstruct a world vector from basis-space components.
    fn to_world(self, c: [f64; 3]) -> Vec3 {
        self.x
            .scale(c[0])
            .add(self.y.scale(c[1]))
            .add(self.z.scale(c[2]))
    }
}

/// Per-component optional exact value (already parsed). `None` = take the value
/// from the raw delta.
#[derive(Debug, Clone, Copy, Default)]
pub struct NumericEntry {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub z: Option<f64>,
}

impl NumericEntry {
    /// Parse a `"x, y, z"` style string (each field optional, blank = `None`)
    /// with [`eval_expr`]. Returns `None` on a parse error in any field.
    pub fn parse(s: &str) -> Option<Self> {
        let mut e = NumericEntry::default();
        for (i, field) in s.split(',').enumerate().take(3) {
            let f = field.trim();
            if f.is_empty() {
                continue;
            }
            let v = eval_expr(f)?;
            match i {
                0 => e.x = Some(v),
                1 => e.y = Some(v),
                _ => e.z = Some(v),
            }
        }
        Some(e)
    }

    fn get(&self, i: usize) -> Option<f64> {
        match i {
            0 => self.x,
            1 => self.y,
            _ => self.z,
        }
    }
}

/// Resolve the delta actually applied.
///
/// 1. Express `raw_delta` in `basis`.
/// 2. Apply `constraint` (zero the locked components).
/// 3. Override any component that has a [`NumericEntry`] value.
/// 4. Snap each free component to a multiple of `increment` if `Some`.
/// 5. Map back to world space.
pub fn resolve_translation(
    raw_delta: Vec3,
    constraint: Constraint,
    basis: TransformBasis,
    numeric: NumericEntry,
    increment: Option<f64>,
) -> Vec3 {
    let mut c = basis.to_local(raw_delta);

    let free = |i: usize| match constraint {
        Constraint::Free => true,
        Constraint::Axis(a) => i == a as usize % 3,
        Constraint::Plane(a) => i != a as usize % 3,
    };

    for (i, ci) in c.iter_mut().enumerate() {
        if !free(i) {
            *ci = 0.0;
            continue;
        }
        if let Some(v) = numeric.get(i) {
            *ci = v;
        } else if let Some(step) = increment {
            if step > 0.0 {
                *ci = (*ci / step).round() * step;
            }
        }
    }
    basis.to_world(c)
}

/// Move `verts` (empty = whole mesh) by `delta`. Positions only.
pub fn apply_translation(mesh: &Mesh, verts: &[VertexId], delta: Vec3) -> Mesh {
    let mut positions = mesh.positions();
    let idx: Vec<usize> = if verts.is_empty() {
        (0..positions.len()).collect()
    } else {
        verts
            .iter()
            .map(|v| v.0)
            .filter(|&i| i < positions.len())
            .collect()
    };
    for &i in &idx {
        positions[i] = positions[i].add(delta);
    }
    Mesh::from_polygons(
        &positions,
        &mesh
            .polygons()
            .iter()
            .map(|f| f.iter().map(|v| v.0).collect())
            .collect::<Vec<_>>(),
    )
}

/// Evaluate a small arithmetic expression to `f64`. Supports `+ - * / ^`,
/// parentheses, unary minus, and the constants `pi`, `tau`, `e`. Whitespace is
/// ignored. Returns `None` on any syntax error.
pub fn eval_expr(s: &str) -> Option<f64> {
    let toks = tokenize(s)?;
    let mut p = Parser { toks: &toks, i: 0 };
    let v = p.expr()?;
    if p.i == p.toks.len() {
        Some(v)
    } else {
        None
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    LParen,
    RParen,
}

fn tokenize(s: &str) -> Option<Vec<Tok>> {
    let mut out = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' => i += 1,
            '+' => {
                out.push(Tok::Plus);
                i += 1;
            }
            '-' => {
                out.push(Tok::Minus);
                i += 1;
            }
            '*' => {
                out.push(Tok::Star);
                i += 1;
            }
            '/' => {
                out.push(Tok::Slash);
                i += 1;
            }
            '^' => {
                out.push(Tok::Caret);
                i += 1;
            }
            '(' => {
                out.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                out.push(Tok::RParen);
                i += 1;
            }
            '0'..='9' | '.' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let n: f64 = chars[start..i].iter().collect::<String>().parse().ok()?;
                out.push(Tok::Num(n));
            }
            'a'..='z' | 'A'..='Z' => {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_alphabetic() {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect::<String>().to_lowercase();
                let v = match word.as_str() {
                    "pi" => std::f64::consts::PI,
                    "tau" => std::f64::consts::TAU,
                    "e" => std::f64::consts::E,
                    _ => return None,
                };
                out.push(Tok::Num(v));
            }
            _ => return None,
        }
    }
    Some(out)
}

struct Parser<'a> {
    toks: &'a [Tok],
    i: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.i)
    }
    fn eat(&mut self) -> Option<&Tok> {
        let t = self.toks.get(self.i);
        if t.is_some() {
            self.i += 1;
        }
        t
    }

    // expr := term (('+' | '-') term)*
    fn expr(&mut self) -> Option<f64> {
        let mut v = self.term()?;
        while let Some(op) = self.peek().cloned() {
            match op {
                Tok::Plus => {
                    self.eat();
                    v += self.term()?;
                }
                Tok::Minus => {
                    self.eat();
                    v -= self.term()?;
                }
                _ => break,
            }
        }
        Some(v)
    }

    // term := power (('*' | '/') power)*
    fn term(&mut self) -> Option<f64> {
        let mut v = self.power()?;
        while let Some(op) = self.peek().cloned() {
            match op {
                Tok::Star => {
                    self.eat();
                    v *= self.power()?;
                }
                Tok::Slash => {
                    self.eat();
                    let d = self.power()?;
                    if d == 0.0 {
                        return None;
                    }
                    v /= d;
                }
                _ => break,
            }
        }
        Some(v)
    }

    // power := unary ('^' power)?    (right-assoc)
    fn power(&mut self) -> Option<f64> {
        let base = self.unary()?;
        if self.peek() == Some(&Tok::Caret) {
            self.eat();
            let exp = self.power()?;
            Some(base.powf(exp))
        } else {
            Some(base)
        }
    }

    // unary := '-' unary | atom
    fn unary(&mut self) -> Option<f64> {
        if self.peek() == Some(&Tok::Minus) {
            self.eat();
            return Some(-self.unary()?);
        }
        if self.peek() == Some(&Tok::Plus) {
            self.eat();
            return self.unary();
        }
        self.atom()
    }

    // atom := Num | '(' expr ')'
    fn atom(&mut self) -> Option<f64> {
        match self.eat()? {
            Tok::Num(n) => Some(*n),
            Tok::LParen => {
                let v = self.expr()?;
                if self.eat() == Some(&Tok::RParen) {
                    Some(v)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn eval_expr_basic_arithmetic() {
        assert_eq!(eval_expr("1+1"), Some(2.0));
        assert_eq!(eval_expr("2 * 3 + 4"), Some(10.0));
        assert_eq!(eval_expr("2 + 3 * 4"), Some(14.0));
        assert_eq!(eval_expr("(2 + 3) * 4"), Some(20.0));
        assert_eq!(eval_expr("-5"), Some(-5.0));
        assert_eq!(eval_expr("2 ^ 10"), Some(1024.0));
        assert_eq!(eval_expr("2 ^ 3 ^ 2"), Some(512.0)); // right-assoc
        assert!((eval_expr("pi").unwrap() - std::f64::consts::PI).abs() < 1e-12);
        assert!((eval_expr("tau / 4").unwrap() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert_eq!(eval_expr("1 /"), None);
        assert_eq!(eval_expr("1 + x"), None);
        assert_eq!(eval_expr("1 / 0"), None);
    }

    #[test]
    fn numeric_entry_parses_fields() {
        let e = NumericEntry::parse("1+1, , pi").unwrap();
        assert_eq!(e.x, Some(2.0));
        assert_eq!(e.y, None);
        assert!((e.z.unwrap() - std::f64::consts::PI).abs() < 1e-12);
    }

    #[test]
    fn axis_constraint_keeps_only_one_component() {
        let d = resolve_translation(
            Vec3::new(3.0, 4.0, 5.0),
            Constraint::Axis(1),
            TransformBasis::global(),
            NumericEntry::default(),
            None,
        );
        assert_eq!(d, Vec3::new(0.0, 4.0, 0.0));
    }

    #[test]
    fn plane_constraint_zeros_one_component() {
        let d = resolve_translation(
            Vec3::new(3.0, 4.0, 5.0),
            Constraint::Plane(2),
            TransformBasis::global(),
            NumericEntry::default(),
            None,
        );
        assert_eq!(d, Vec3::new(3.0, 4.0, 0.0));
    }

    #[test]
    fn numeric_override_wins_over_the_raw_delta() {
        let n = NumericEntry {
            x: Some(10.0),
            ..Default::default()
        };
        let d = resolve_translation(
            Vec3::new(3.0, 4.0, 5.0),
            Constraint::Free,
            TransformBasis::global(),
            n,
            None,
        );
        assert_eq!(d, Vec3::new(10.0, 4.0, 5.0));
    }

    #[test]
    fn increment_snaps_free_components() {
        let d = resolve_translation(
            Vec3::new(1.2, 2.7, -0.4),
            Constraint::Free,
            TransformBasis::global(),
            NumericEntry::default(),
            Some(1.0),
        );
        assert_eq!(d, Vec3::new(1.0, 3.0, 0.0));
    }

    #[test]
    fn normal_basis_constrains_along_the_surface_normal() {
        // Basis z = (0,0,1); Axis(2) keeps only the z-component in that basis.
        let basis = TransformBasis::from_normal(Vec3::new(0.0, 0.0, 1.0));
        let d = resolve_translation(
            Vec3::new(2.0, 2.0, 5.0),
            Constraint::Axis(2),
            basis,
            NumericEntry::default(),
            None,
        );
        assert!((d.x).abs() < 1e-9 && (d.y).abs() < 1e-9);
        assert!((d.z - 5.0).abs() < 1e-9);
    }

    #[test]
    fn apply_translation_moves_the_selection() {
        let m = primitives::cube(2.0);
        let t = apply_translation(&m, &[VertexId(0)], Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(
            t.vertex(VertexId(0)).unwrap().position,
            m.vertex(VertexId(0))
                .unwrap()
                .position
                .add(Vec3::new(1.0, 0.0, 0.0))
        );
    }
}
