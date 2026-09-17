#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Authoring-time codegen: read one CoolProp binary-pair entry (from the
# gitignored `upstream_source/CoolProp/dev/mixtures/mixture_binary_pairs.json`, plus
# `mixture_departure_functions.json` when a departure function is fitted) and
# print a Rust `BinaryPair {...}` literal for `src/mixtures/binary_pairs.rs`.
# Mirrors `dev/gen_fluid.py` for the pure-fluid side: a data-reduction step,
# not a runtime JSON reader.
#
# Usage (run from the crate root, with the upstream_source clone present):
#   python3 dev/gen_mixture.py Nitrogen Oxygen
#
# Fluid names are the crate's own canonical CoolProp pure-fluid names (the
# `upstream_source/CoolProp/dev/fluids/<Name>.json` filename) -- NOT necessarily the
# short/REFPROP-style names `mixture_binary_pairs.json` itself uses (e.g.
# "CYCLOHEX" for Cyclohexane). Matching is by CAS number (every pure-fluid
# JSON's `INFO.CAS`), which is unambiguous across naming conventions.
import json, sys, os, re

HERE = os.path.dirname(os.path.abspath(__file__))
CRATE = os.path.dirname(HERE)
FLUID_DIR = os.path.join(CRATE, "upstream_source", "CoolProp", "dev", "fluids")
MIXTURE_DIR = os.path.join(CRATE, "upstream_source", "CoolProp", "dev", "mixtures")


def variant_name(fluid):
    """The Rust `Fluid` enum variant for a canonical CoolProp fluid name --
    kept identical to dev/regen_all.py's transform."""
    toks = [t for t in re.split(r"[^0-9a-zA-Z]", fluid) if t]
    v = "".join(t[:1].upper() + t[1:] for t in toks)
    return "F" + v if v and v[0].isdigit() else v


def load_cas_map():
    """canonical fluid name -> CAS number, scanned from every pure-fluid JSON."""
    cas_map = {}
    for fname in os.listdir(FLUID_DIR):
        if not fname.endswith(".json"):
            continue
        name = fname[:-5]
        with open(os.path.join(FLUID_DIR, fname)) as fh:
            d = json.load(fh)
        cas = d.get("INFO", {}).get("CAS")
        if cas:
            cas_map[name] = cas
    return cas_map


def find_binary_pair(pairs, cas_a, cas_b):
    for p in pairs:
        if {p["CAS1"], p["CAS2"]} == {cas_a, cas_b}:
            return p
    return None


def find_departure_function(departures, function_name):
    for d in departures:
        if d["Name"] == function_name or function_name in d.get("aliases", []):
            return d
    return None


def build_departure_terms(dep):
    """A `&[DepartureTerm::...]` literal for one departure-function entry."""
    t = dep["type"]
    n, tt, d = [float(x) for x in dep["n"]], [float(x) for x in dep["t"]], [float(x) for x in dep["d"]]
    terms = []
    if t == "Exponential":
        # CoolProp add_Power/add_Exponential: n*delta^d*tau^t*exp(-delta^l).
        l = [float(x) for x in dep["l"]]
        for i in range(len(n)):
            terms.append("DepartureTerm::Power {{ n: {!r}, d: {!r}, t: {!r}, l: {!r} }}".format(n[i], d[i], tt[i], l[i]))
    elif t == "GERG-2008":
        # add_Power (first Npower, eta=beta=0) + add_GERG2008Gaussian (rest) --
        # GergExponential degenerates to pure power when eta=beta=0, so every
        # term is emitted uniformly (see departure.rs's module doc).
        eta = [float(x) for x in dep["eta"]]
        epsilon = [float(x) for x in dep["epsilon"]]
        beta = [float(x) for x in dep["beta"]]
        gamma = [float(x) for x in dep["gamma"]]
        for i in range(len(n)):
            terms.append(
                "DepartureTerm::GergExponential {{ n: {!r}, d: {!r}, t: {!r}, eta: {!r}, epsilon: {!r}, beta: {!r}, gamma: {!r} }}".format(
                    n[i], d[i], tt[i], eta[i], epsilon[i], beta[i], gamma[i]
                )
            )
    elif t == "Gaussian+Exponential":
        # add_Power/add_Exponential (first Npower, l=0 in all data surveyed) +
        # add_Gaussian (rest) -- Gaussian degenerates to pure power when
        # eta=beta=0, so every term is emitted uniformly (see departure.rs).
        eta = [float(x) for x in dep["eta"]]
        epsilon = [float(x) for x in dep["epsilon"]]
        beta = [float(x) for x in dep["beta"]]
        gamma = [float(x) for x in dep["gamma"]]
        for i in range(len(n)):
            terms.append(
                "DepartureTerm::Gaussian {{ n: {!r}, d: {!r}, t: {!r}, eta: {!r}, epsilon: {!r}, beta: {!r}, gamma: {!r} }}".format(
                    n[i], d[i], tt[i], eta[i], epsilon[i], beta[i], gamma[i]
                )
            )
    else:
        sys.exit("departure function {!r}: unsupported type {!r}".format(dep["Name"], t))
    return "&[" + ", ".join(terms) + "]"


def build(fluid_a, fluid_b):
    cas_map = load_cas_map()
    if fluid_a not in cas_map:
        sys.exit("{}: not a known pure fluid (no CAS in upstream_source/CoolProp/dev/fluids/)".format(fluid_a))
    if fluid_b not in cas_map:
        sys.exit("{}: not a known pure fluid (no CAS in upstream_source/CoolProp/dev/fluids/)".format(fluid_b))
    cas_a, cas_b = cas_map[fluid_a], cas_map[fluid_b]

    with open(os.path.join(MIXTURE_DIR, "mixture_binary_pairs.json")) as fh:
        pairs = json.load(fh)
    pair = find_binary_pair(pairs, cas_a, cas_b)
    if pair is None:
        sys.exit("no binary pair found for {} ({}) / {} ({})".format(fluid_a, cas_a, fluid_b, cas_b))

    if "betaT" in pair:
        beta_t, gamma_t = pair["betaT"], pair["gammaT"]
        beta_v, gamma_v = pair["betaV"], pair["gammaV"]
    elif "xi" in pair and "zeta" in pair:
        # LemmonAirHFCReducingFunction::convert_to_GERG (ReducingFunctions.h):
        # beta_T=beta_v=1; gamma_T/gamma_v folded from xi/zeta + the pure
        # components' own critical T/v (looked up from the pure-fluid JSON,
        # not hardcoded here, to stay a single source of truth).
        with open(os.path.join(FLUID_DIR, fluid_a + ".json")) as fh:
            eos_a = json.load(fh)["EOS"][0]
        with open(os.path.join(FLUID_DIR, fluid_b + ".json")) as fh:
            eos_b = json.load(fh)["EOS"][0]
        tc_a, tc_b = eos_a["STATES"]["reducing"]["T"], eos_b["STATES"]["reducing"]["T"]
        rhoc_a, rhoc_b = eos_a["STATES"]["reducing"]["rhomolar"], eos_b["STATES"]["reducing"]["rhomolar"]
        v_a, v_b = 1.0 / rhoc_a, 1.0 / rhoc_b
        xi, zeta = pair["xi"], pair["zeta"]
        beta_t, beta_v = 1.0, 1.0
        gamma_t = (tc_a + tc_b + xi) / (2.0 * (tc_a * tc_b) ** 0.5)
        gamma_v = (v_a + v_b + zeta) / (0.25 * (v_a ** (1.0 / 3.0) + v_b ** (1.0 / 3.0)) ** 3)
    else:
        sys.exit("{}/{}: binary pair has neither betaT nor xi/zeta".format(fluid_a, fluid_b))

    f_departure = pair.get("F", 0.0)
    if f_departure != 0.0:
        with open(os.path.join(MIXTURE_DIR, "mixture_departure_functions.json")) as fh:
            departures = json.load(fh)
        dep = find_departure_function(departures, pair["function"])
        if dep is None:
            sys.exit("{}/{}: departure function {!r} not found".format(fluid_a, fluid_b, pair["function"]))
        departure_terms = build_departure_terms(dep)
    else:
        departure_terms = "&[]"

    return (
        "BinaryPair {{ a: Fluid::{va}, b: Fluid::{vb}, "
        "beta_gamma_t: ({bt!r}, {gt!r}), beta_gamma_v: ({bv!r}, {gv!r}), "
        "f_departure: {f!r}, departure_terms: {terms} }}"
    ).format(
        va=variant_name(fluid_a), vb=variant_name(fluid_b),
        bt=float(beta_t), gt=float(gamma_t), bv=float(beta_v), gv=float(gamma_v),
        f=float(f_departure), terms=departure_terms,
    )


def main():
    if len(sys.argv) != 3:
        sys.exit("usage: gen_mixture.py <FluidA> <FluidB>")
    sys.stdout.write(build(sys.argv[1], sys.argv[2]))


if __name__ == "__main__":
    main()
