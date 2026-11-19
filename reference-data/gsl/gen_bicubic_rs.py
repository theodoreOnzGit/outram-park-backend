#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Generate crates/petir/src/interp2d_bicubic.rs from GSL 2.8 bicubic.c.

The six bicubic evaluators are 13 to 16 statements of the form
    v = <polynomial in the 16 cell values>;   *z += <k>*v*t<a>*u<b>;
whose coefficients are the whole point. Typing them would be writing from
memory; this script copies them from the vendored file, changing only the
syntax C and Rust do not share:
  * an integer literal N becomes N.0 (the C arithmetic converts it to double
    exactly, so the value is the same);
  * `*z += ...` / `*z_p += ...` / `*z_pp += ...` becomes `z += ...`;
  * the accumulator declaration and the return.
Operation order, grouping and association are untouched.

usage: gen_bicubic_rs.py <path to bicubic.c> <output .rs>
"""
import re
import sys

src, out = sys.argv[1], sys.argv[2]
text = open(src).read().split("\n")

# function name in C -> name in Rust
funcs = [
    ("bicubic_eval", "eval"),
    ("bicubic_deriv_x", "deriv_x"),
    ("bicubic_deriv_y", "deriv_y"),
    ("bicubic_deriv_xx", "deriv_xx"),
    ("bicubic_deriv_xy", "deriv_xy"),
    ("bicubic_deriv_yy", "deriv_yy"),
]


def starts(name):
    for i, l in enumerate(text):
        if l.startswith(name + "(") or l.startswith(name + " ("):
            return i
    raise SystemExit("function not found: " + name)


def floatify(expr):
    # integer literals not part of an identifier or a float
    return re.sub(r"(?<![\w.])(\d+)(?![\w.])", r"\1.0", expr)


body_out = []
for cname, rname in funcs:
    i = starts(cname)
    # accumulator initialisation line
    j = next(k for k in range(i, len(text)) if re.match(r"\s*\*z(_p|_pp)? = 0;", text[k]))
    r = next(k for k in range(j, len(text)) if text[k].strip() == "return GSL_SUCCESS;")
    first = j + 1  # 1-based line of the `*z = 0;` statement is j+1
    lines = []
    for k in range(j + 1, r):
        l = text[k].strip()
        if not l:
            continue
        m = re.match(r"v = (.*);$", l)
        if m:
            lines.append("    v = " + floatify(m.group(1)) + ";")
            continue
        m = re.match(r"\*z(?:_p|_pp)? \+= (.*);$", l)
        if m:
            lines.append("    z += " + floatify(m.group(1)) + ";")
            continue
        m = re.match(r"\*z(?:_p|_pp)? \*= (.*);$", l)
        if m:
            lines.append("    z *= " + m.group(1) + ";")
            continue
        raise SystemExit("unhandled line in %s: %r" % (cname, l))
    body_out.append(
        "/// `%s` (bicubic.c lines %d-%d): the polynomial sum, copied by script.\n"
        "pub(crate) fn %s(c: &BicubicCell) -> f64 {\n"
        "    let BicubicCell { t0, t1, t2, t3, u0, u1, u2, u3, dt, du,\n"
        "        zminmin, zminmax, zmaxmin, zmaxmax, zxminmin, zxminmax, zxmaxmin, zxmaxmax,\n"
        "        zyminmin, zyminmax, zymaxmin, zymaxmax, zxyminmin, zxyminmax, zxymaxmin, zxymaxmax } = *c;\n"
        "    let mut v;\n"
        "    let mut z = 0.0_f64;\n"
        "%s\n"
        "    z\n"
        "}\n" % (cname, first, r, rname, "\n".join(lines))
    )

header = """// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of PETIR, a component of OUTRAM PARK.
//
// PETIR is free software: you can redistribute it and/or modify it under the
// terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 of the License.
//
// PETIR is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along
// with PETIR.  If not, see <https://www.gnu.org/licenses/>.
//
// PORTED (by script, reference-data/gsl/gen_bicubic_rs.py) from the GNU
// Scientific Library (GSL) 2.8, commit cf180cd7fbd06039a577f9c9ff0b428784765ac1,
// interpolation/bicubic.c. Copyright 2012 David Zaslavsky.
// GPL-3.0-or-later (verified from the file header; see NOTICE).
//
// GENERATED: do not edit by hand; rerun the script. Only integer literals
// became floats and `*z += e` became `z += e`; every coefficient, operation
// order and grouping is the C file's.

//! The bicubic polynomial sums of GSL's `gsl_interp2d_bicubic`, generated from
//! `bicubic.c`. See [`crate::interp2d`] for the type that uses them.

#![allow(
    clippy::all,
    clippy::pedantic,
    unused_variables,
    non_snake_case,
    clippy::too_many_lines
)]

use crate::interp2d::BicubicCell;

"""
open(out, "w").write(header + "\n".join(body_out))
