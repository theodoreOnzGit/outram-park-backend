#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Authoring-time codegen: read one CoolProp incompressible-liquid JSON (from
# the gitignored `upstream_source/CoolProp/dev/incompressible_liquids/json/`) and
# emit a hardcoded Rust `IncompressibleFluid` `const` for
# `src/incompressibles/fluids/`. Mirrors `dev/gen_fluid.py` for the pure-fluid
# Helmholtz EOS side: a data-reduction step, not a runtime JSON reader.
#
# Usage (run from the crate root, with the upstream_source clone present):
#   python3 dev/gen_incompressible.py T66 > src/incompressibles/fluids/t66.rs
#
# The upstream_source clone is regenerated with:
#   git clone --depth 1 https://github.com/CoolProp/CoolProp.git upstream_source/CoolProp
#
# Coverage: this crate implements 4 of CoolProp's 5 IncompressibleData fit
# forms (polynomial, exppolynomial, exponential, logexponential) -- covering
# all 126 fluids surveyed in upstream_source/CoolProp/dev/incompressible_liquids/json
# (density/specific_heat/conductivity are always `polynomial`; viscosity is
# one of the four, or absent). `polyoffset` is not used by any of them, so it
# is not implemented; a fluid whose viscosity uses it (there are none today)
# would abort here with a clear message rather than emit a wrong number.
#
# A handful of fluids' JSON also carries an all-*zero* coefficient block for
# one property (typed as if real, e.g. `polynomial`, but never actually
# fit by CoolProp upstream -- Acetone's `conductivity`, LiBr's `conductivity`
# and `viscosity`). Emitting that would be a knowingly-wrong 0.0, not a
# missing-data `None` -- `is_all_zero` below detects it and treats it the
# same as `notdefined`.
import json, sys, os

def slice_f64(xs):
    return "&[" + ", ".join(repr(float(x)) for x in xs) + "]"

def is_all_zero(coeffs):
    flat = coeffs if not (coeffs and isinstance(coeffs[0], list)) else [c for row in coeffs for c in row]
    return all(float(c) == 0.0 for c in flat)

def rust_const_ident(name):
    """A valid upper-snake Rust identifier for a fluid `const` name.

    Non-alphanumerics become `_`; a digit-leading name is prefixed with `F_`
    so the result is a legal identifier."""
    s = "".join(ch if ch.isalnum() else "_" for ch in name).upper()
    if s and s[0].isdigit():
        s = "F_" + s
    return s

KIND = {"pure": "IncompressibleKind::Pure", "mass": "IncompressibleKind::MassBased", "volume": "IncompressibleKind::VolumeBased"}

def build_fit(block, prop_name, fluid_name):
    """A `PropertyFit {...}` literal, or None if the block is absent/unsupported
    (`notdefined`, `polyoffset`) or an all-zero never-fit placeholder -- the
    caller decides whether that's fatal."""
    if not isinstance(block, dict):
        return None
    t = block.get("type")
    coeffs = block.get("coeffs")
    if t in ("polynomial", "exppolynomial", "exponential", "logexponential") and is_all_zero(coeffs):
        return None
    if t == "polynomial":
        rows = "&[" + ", ".join(slice_f64(row) for row in coeffs) + "]"
        return "PropertyFit {{ form: PropertyForm::Polynomial, coeffs: {} }}".format(rows)
    if t == "exppolynomial":
        rows = "&[" + ", ".join(slice_f64(row) for row in coeffs) + "]"
        return "PropertyFit {{ form: PropertyForm::ExpPolynomial, coeffs: {} }}".format(rows)
    if t == "exponential":
        return "PropertyFit {{ form: PropertyForm::Exponential, coeffs: &[{}] }}".format(slice_f64(coeffs))
    if t == "logexponential":
        return "PropertyFit {{ form: PropertyForm::LogExponential, coeffs: &[{}] }}".format(slice_f64(coeffs))
    if t in ("notdefined", None):
        return None
    sys.exit("{}: {} has unsupported fit type {!r} (only polyoffset is unhandled; "
             "add it to gen_incompressible.py + incompressibles/mod.rs if this fires)"
             .format(fluid_name, prop_name, t))

def build(name, d):
    xid = d.get("xid", "pure")
    if xid not in KIND:
        sys.exit("{}: unknown xid {!r}".format(name, xid))
    kind = KIND[xid]
    tmin, tmax = float(d["Tmin"]), float(d["Tmax"])
    if xid == "pure":
        xmin, xmax = 0.0, 0.0
    else:
        xmin, xmax = float(d["xmin"]), float(d["xmax"])
    tbase = float(d.get("Tbase", 0.0))
    xbase = float(d.get("xbase", 0.0))

    density = build_fit(d.get("density"), "density", name)
    if density is None:
        sys.exit("{}: density fit missing/unsupported".format(name))
    cp = build_fit(d.get("specific_heat"), "specific_heat", name)
    if cp is None:
        sys.exit("{}: specific_heat fit missing/unsupported".format(name))
    cond = build_fit(d.get("conductivity"), "conductivity", name)
    cond_lit = "None" if cond is None else "Some({})".format(cond)
    visc = build_fit(d.get("viscosity"), "viscosity", name)
    visc_lit = "None" if visc is None else "Some({})".format(visc)

    ident = rust_const_ident(name) + "_INCOMP"
    return ident, (
        "/// CoolProp `{name}` ({xid}), `T ∈ [{tmin!r}, {tmax!r}] K`.\n"
        "/// `upstream_source/CoolProp/dev/incompressible_liquids/json/{name}.json`.\n"
        "pub const {ident}: IncompressibleFluid = IncompressibleFluid {{\n"
        "    name: \"{name}\",\n"
        "    kind: {kind},\n"
        "    t_range: ({tmin!r}, {tmax!r}),\n"
        "    x_range: ({xmin!r}, {xmax!r}),\n"
        "    t_base: {tbase!r},\n"
        "    x_base: {xbase!r},\n"
        "    density: {density},\n"
        "    heat_capacity: {cp},\n"
        "    conductivity: {cond_lit},\n"
        "    viscosity: {visc_lit},\n"
        "}};\n"
    ).format(name=name, xid=xid, kind=kind, tmin=tmin, tmax=tmax, xmin=xmin, xmax=xmax, tbase=tbase, xbase=xbase,
             ident=ident, density=density, cp=cp, cond_lit=cond_lit, visc_lit=visc_lit)

def main():
    if len(sys.argv) != 2:
        sys.exit("usage: gen_incompressible.py <FluidName>")
    name = sys.argv[1]
    here = os.path.dirname(os.path.abspath(__file__))
    crate = os.path.dirname(here)
    path = os.path.join(crate, "upstream_source", "CoolProp", "dev", "incompressible_liquids", "json", name + ".json")
    with open(path) as fh:
        d = json.load(fh)
    _, body = build(name, d)
    sys.stdout.write(
        "// SPDX-License-Identifier: MIT\n"
        "// GENERATED by dev/gen_incompressible.py from CoolProp's {}.json (MIT).\n"
        "// Do not edit by hand — re-run the generator. See dev/gen_incompressible.py.\n"
        "//! CoolProp `{}` incompressible-fluid fit, as hardcoded `const` data.\n\n"
        "use crate::incompressibles::{{IncompressibleFluid, IncompressibleKind, PropertyFit, PropertyForm}};\n\n"
        .format(name, name))
    sys.stdout.write(body)

if __name__ == "__main__":
    main()
