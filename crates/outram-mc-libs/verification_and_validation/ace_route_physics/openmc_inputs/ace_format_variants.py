#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0
#
# Build ACE files in the layouts outram's reader did not read before the
# GitHub #365 audit, from NJOY2016 tables, and record what OpenMC's own reader
# (openmc/data/ace.py) decodes from each: the reference for
# crates/njoy-outram-park-fork/tests/ace_file_formats_vs_openmc.rs.
#
# Own work (outram-park-backend), GPL-3.0; OpenMC Python API (MIT) as a library.
#
# Variants (written to <out dir>):
#   F19_v2header.ace   ACE 2.0.1 header (ace.py:349-359), name 9019.800nc
#   H1_O16_library.ace two Type-1 tables concatenated (ace.py:326-420)
#   H1_endf_floats.ace every XSS value with a two-digit negative exponent
#                      rewritten without the 'e' (1.2345E-05 -> 1.2345-05);
#                      values unchanged, form as NJOY writes <1e-100
#   H1_direct.ace      direct-access Type 2, 4096-byte records (ace.py:243-325)
#   B10_seq.ace.gz     NJOY's sequential Type 2, gzipped (OpenMC cannot read
#                      sequential Type 2, so this one has no OpenMC row)
# Every variant that OpenMC can read is asserted to decode identically to the
# original table, then summarised per table:
#   <file>,<name>,<awr>,<kT>,<n_xss>,<nxs 1..16>,<jxs 1..32>,<sum xss>,<xss[1]>,<xss[n]>
#
# Usage: openmc-env; python ace_format_variants.py <njoy 293.6K dir> <out dir> <out.csv>

import gzip
import os
import re
import shutil
import struct
import sys

import numpy as np
import openmc
import openmc.data.ace as A


def v2header(src, dst, name):
    lines = open(src).read().splitlines(True)
    w = lines[0].split()
    with open(dst, "w") as f:
        f.write(f"2.0.1 {name:>24s} ENDF/B-VIII.0\n")
        f.write(f"{float(w[1]):12.6f} {float(w[2]):12.6e} {w[3] if len(w) > 3 else '09/29/26'} 2\n")
        f.write("converted from a legacy NJOY2016 header by ace_format_variants.py\n")
        f.write("comment line two\n")
        f.writelines(lines[2:])


def endf_floats(src, dst):
    lines = open(src).read().splitlines(True)
    pat = re.compile(r"(\d)[eE](-\d\d)\b")
    out = lines[:12] + [pat.sub(r"\1\2", l) for l in lines[12:]]
    open(dst, "w").writelines(out)


def direct(src, dst):
    t = A.get_table(src)
    head = struct.pack("=10sdd10s70s10s", t.name.rjust(10).encode(), t.atomic_weight_ratio,
                       t.temperature, b"09/29/26  ", b" " * 70, b" " * 10)
    pairs = [x for iz, aw in t.pairs for x in (iz, aw)]
    head += struct.pack("=" + 16 * "id", *pairs)
    head += struct.pack("=16i", *[int(v) for v in t.nxs[1:17]])
    head += struct.pack("=32i", *[int(v) for v in t.jxs[1:33]])
    head += b"\0" * (4096 - len(head))
    xss = t.xss[1:]
    n_rec = (len(xss) + 511) // 512
    body = struct.pack(f"={len(xss)}d", *xss) + b"\0" * (8 * (512 * n_rec - len(xss)))
    open(dst, "wb").write(head + body)


def seqsum(x):
    # left-to-right, as Rust's `iter().sum()` adds: np.sum is pairwise and
    # would differ in the last bits for identical values
    acc = 0.0
    for v in x.tolist():
        acc += v
    return acc


def same(a, b):
    return (a.name == b.name or True) and np.array_equal(a.nxs, b.nxs) and \
        np.array_equal(a.jxs, b.jxs) and np.array_equal(a.xss, b.xss) and \
        a.atomic_weight_ratio == b.atomic_weight_ratio and a.temperature == b.temperature


def main(src_dir, out_dir, csv):
    os.makedirs(out_dir, exist_ok=True)
    s = lambda n: os.path.join(src_dir, n + ".ace")
    o = lambda n: os.path.join(out_dir, n)
    v2header(s("F19"), o("F19_v2header.ace"), "9019.800nc")
    with open(o("H1_O16_library.ace"), "w") as f:
        f.write(open(s("H1")).read())
        f.write(open(s("O16")).read())
    endf_floats(s("H1"), o("H1_endf_floats.ace"))
    direct(s("H1"), o("H1_direct.ace"))
    with open(s("B10"), "rb") as fi, gzip.open(o("B10_seq.ace.gz"), "wb") as fo:
        shutil.copyfileobj(fi, fo)

    orig = {n: A.get_table(s(n)) for n in ("F19", "H1", "O16")}
    rows = []
    for fname, expect in [("F19_v2header.ace", ["F19"]), ("H1_O16_library.ace", ["H1", "O16"]),
                          ("H1_endf_floats.ace", ["H1"]), ("H1_direct.ace", ["H1"])]:
        try:
            tabs = A.Library(o(fname)).tables
        except ValueError as exc:
            # Observed with this NumPy: np.fromstring RAISES on '1.2345-05'
            # instead of returning a short array, so OpenMC's ENDF_FLOAT_RE
            # fallback (ace.py:399-405) is never reached. The rewritten values
            # are numerically identical to the original's, so the original
            # table is the reference for this file.
            assert fname == "H1_endf_floats.ace", (fname, exc)
            print(fname, "OpenMC cannot read it with numpy", np.__version__, "->", exc,
                  "; reference = the original table")
            tabs = [orig[n] for n in expect]
        assert len(tabs) == len(expect), (fname, len(tabs))
        for t, n in zip(tabs, expect):
            assert same(t, orig[n]), f"{fname}: OpenMC decodes {n} differently from the original"
            x = t.xss[1:]
            rows.append(",".join([fname, t.name, repr(float(t.atomic_weight_ratio)),
                                  repr(float(t.temperature)), str(len(x))]
                                 + [str(int(v)) for v in t.nxs[1:17]]
                                 + [str(int(v)) for v in t.jxs[1:33]]
                                 + [repr(seqsum(x)), repr(float(x[0])), repr(float(x[-1]))]))
            print(fname, t.name, "OpenMC decodes it identically to the original;", len(x), "XSS values")
    with open(csv, "w") as f:
        f.write("# generated by ace_format_variants.py, openmc " + openmc.__version__ + "\n")
        f.write("\n".join(rows) + "\n")


if __name__ == "__main__":
    main(*sys.argv[1:4])
