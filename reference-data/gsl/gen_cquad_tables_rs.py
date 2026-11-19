#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Generate crates/petir/src/integration/cquad_tables.rs from GSL 2.8
integration/cquad_const.c (1 334 lines of constants: the Clenshaw-Curtis nodes,
the inverse Vandermonde-like matrices, the interval-split matrices). Typing
them would be writing from memory; this script copies each literal's text,
changing only the syntax C and Rust do not share:
  * `.5e-1` becomes `0.5e-1`, `-.5` becomes `-0.5`, `1.` becomes `1.0`;
  * a bare integer `0` becomes `0.0`;
  * `5 * 5` in an array length becomes `25`.
No digit is altered; both compilers round the decimal text to the nearest
double.

usage: gen_cquad_tables_rs.py <path to cquad_const.c> <output .rs>
"""
import re
import sys

src, out = sys.argv[1], sys.argv[2]
text = open(src).read()
text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)

names = {
    "xi": "XI", "bee": "BEE", "Lalpha": "LALPHA", "Lgamma": "LGAMMA",
    "V1inv": "V1INV", "V2inv": "V2INV", "V3inv": "V3INV", "V4inv": "V4INV",
    "Tleft": "TLEFT", "Tright": "TRIGHT",
}


def literal(tok):
    sign = ""
    if tok.startswith("-"):
        sign, tok = "-", tok[1:]
    if tok.startswith("."):
        tok = "0" + tok
    m = re.fullmatch(r"(\d+)\.?(\d*)([eE][+-]?\d+)?", tok)
    if not m:
        raise SystemExit("cannot parse literal %r" % tok)
    ip, fp, ex = m.group(1), m.group(2), m.group(3) or ""
    if fp == "":
        fp = "0"
    return "%s%s.%s%s" % (sign, ip, fp, ex)


lines = []
count = {}
for m in re.finditer(
    r"static const double (\w+)\[([^\]]+)\]\s*=\s*\{(.*?)\};", text, flags=re.S
):
    cname, size, body = m.group(1), m.group(2), m.group(3)
    n = 1
    for f in size.split("*"):
        n *= int(f)
    toks = [t for t in re.split(r"[\s,]+", body) if t]
    if len(toks) != n:
        raise SystemExit("%s: %d values, expected %d" % (cname, len(toks), n))
    count[cname] = n
    vals = [literal(t) for t in toks]
    lines.append("pub(super) const %s: [f64; %d] = [" % (names[cname], n))
    for i in range(0, n, 4):
        lines.append("    " + ", ".join(vals[i:i + 4]) + ",")
    lines.append("];")
    lines.append("")

if sorted(count) != sorted(names):
    raise SystemExit("tables found: %s" % sorted(count))

header = """//! Constant tables of GSL 2.8 `integration/cquad_const.c`, commit
//! `cf180cd7`, copied from the file by `reference-data/gsl/gen_cquad_tables_rs.py`
//! (never typed): the 33 Clenshaw-Curtis nodes, the Newton-basis downdating
//! data and the four inverse Vandermonde-like matrices, and the matrices that
//! map the coefficients of an interval onto its left and right halves.
//! Copyright (C) 2010 Pedro Gonnet (GPL-3.0-or-later). gh:#1340.

#![allow(clippy::excessive_precision, clippy::unreadable_literal)]

"""
open(out, "w").write(header + "\n".join(lines))
print("wrote", out, {names[k]: v for k, v in count.items()})
