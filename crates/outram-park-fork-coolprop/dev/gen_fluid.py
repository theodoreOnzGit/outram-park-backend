#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Authoring-time codegen: read one CoolProp fluid JSON (from the gitignored
# `reference/CoolProp/dev/fluids/`) and emit a hardcoded Rust `FluidEos` `const`
# for `src/fluids/`. This is a *data-reduction* step: it copies out only the
# Helmholtz-EOS coefficients (residual + ideal terms + reducing/critical
# params) — NOT the JSON's ANCILLARIES / TRANSPORT / metadata — so the shipped
# crate carries only kilobytes per fluid and never reads JSON at runtime.
#
# Usage (run from the crate root, with the reference clone present):
#   python3 dev/gen_fluid.py Water > src/fluids/water.rs
#
# The reference clone is regenerated with:
#   git clone --depth 1 https://github.com/CoolProp/CoolProp.git reference/CoolProp
import json, sys, os

def slice_f64(xs):
    return "&[" + ", ".join(repr(float(x)) for x in xs) + "]"

def rust_const_ident(name):
    """A valid upper-snake Rust identifier for a fluid `const` name.

    Non-alphanumerics become `_`; a digit-leading name (e.g. `1-Butene`) is
    prefixed with `F_` so the result is a legal identifier (`F_1_BUTENE`)."""
    s = "".join(ch if ch.isalnum() else "_" for ch in name).upper()
    if s and s[0].isdigit():
        s = "F_" + s
    return s

def main():
    if len(sys.argv) != 2:
        sys.exit("usage: gen_fluid.py <FluidName>")
    fluid = sys.argv[1]
    here = os.path.dirname(os.path.abspath(__file__))
    path = os.path.join(here, "..", "reference", "CoolProp", "dev", "fluids", f"{fluid}.json")
    d = json.load(open(path))
    eos = d["EOS"][0]
    red = eos["STATES"]["reducing"]
    crit = d["STATES"]["critical"]
    const_name = rust_const_ident(fluid)

    residual = []
    for t in eos["alphar"]:
        ty = t["type"]
        if ty == "ResidualHelmholtzPower":
            residual.append("    ResidualTerm::Power {{ n: {}, t: {}, d: {}, l: {} }},".format(
                slice_f64(t["n"]), slice_f64(t["t"]), slice_f64(t["d"]), slice_f64(t["l"])))
        elif ty == "ResidualHelmholtzGaussian":
            residual.append("    ResidualTerm::Gaussian {{ n: {}, t: {}, d: {}, eta: {}, epsilon: {}, beta: {}, gamma: {} }},".format(
                slice_f64(t["n"]), slice_f64(t["t"]), slice_f64(t["d"]),
                slice_f64(t["eta"]), slice_f64(t["epsilon"]), slice_f64(t["beta"]), slice_f64(t["gamma"])))
        elif ty == "ResidualHelmholtzNonAnalytic":
            residual.append("    ResidualTerm::NonAnalytic {{ n: {}, a: {}, b: {}, beta: {}, big_a: {}, big_b: {}, big_c: {}, big_d: {} }},".format(
                slice_f64(t["n"]), slice_f64(t["a"]), slice_f64(t["b"]), slice_f64(t["beta"]),
                slice_f64(t["A"]), slice_f64(t["B"]), slice_f64(t["C"]), slice_f64(t["D"])))
        elif ty == "ResidualHelmholtzExponential":
            residual.append("    ResidualTerm::Exponential {{ n: {}, t: {}, d: {}, g: {}, l: {} }},".format(
                slice_f64(t["n"]), slice_f64(t["t"]), slice_f64(t["d"]), slice_f64(t["g"]), slice_f64(t["l"])))
        elif ty == "ResidualHelmholtzDoubleExponential":
            residual.append("    ResidualTerm::DoubleExponential {{ n: {}, t: {}, d: {}, gd: {}, ld: {}, gt: {}, lt: {} }},".format(
                slice_f64(t["n"]), slice_f64(t["t"]), slice_f64(t["d"]),
                slice_f64(t["gd"]), slice_f64(t["ld"]), slice_f64(t["gt"]), slice_f64(t["lt"])))
        elif ty == "ResidualHelmholtzLemmon2005":
            # Lemmon–Jacobsen (2005): exp(-δ^l - τ^m); lower to DoubleExponential
            # with gd = gt = 1 (CoolProp add_Lemmon2005).
            nn = t["n"]
            residual.append("    ResidualTerm::DoubleExponential {{ n: {}, t: {}, d: {}, gd: {}, ld: {}, gt: {}, lt: {} }},".format(
                slice_f64(nn), slice_f64(t["t"]), slice_f64(t["d"]),
                slice_f64([1.0] * len(nn)), slice_f64(t["l"]), slice_f64([1.0] * len(nn)), slice_f64(t["m"])))
        elif ty == "ResidualHelmholtzGaoB":
            residual.append("    ResidualTerm::GaoB {{ n: {}, t: {}, d: {}, eta: {}, beta: {}, gamma: {}, epsilon: {}, b: {} }},".format(
                slice_f64(t["n"]), slice_f64(t["t"]), slice_f64(t["d"]), slice_f64(t["eta"]),
                slice_f64(t["beta"]), slice_f64(t["gamma"]), slice_f64(t["epsilon"]), slice_f64(t["b"])))
        else:
            sys.exit(f"unsupported residual term type: {ty} (extend the engine + codegen)")

    ideal = []
    for t in eos["alpha0"]:
        ty = t["type"]
        if ty == "IdealGasHelmholtzLead":
            ideal.append("    IdealTerm::Lead {{ a1: {!r}, a2: {!r} }},".format(float(t["a1"]), float(t["a2"])))
        elif ty == "IdealGasHelmholtzLogTau":
            ideal.append("    IdealTerm::LogTau {{ a: {!r} }},".format(float(t["a"])))
        elif ty == "IdealGasHelmholtzPlanckEinstein":
            ideal.append("    IdealTerm::PlanckEinstein {{ n: {}, t: {} }},".format(
                slice_f64(t["n"]), slice_f64(t["t"])))
        elif ty == "IdealGasHelmholtzEnthalpyEntropyOffset":
            ideal.append("    IdealTerm::EnthalpyEntropyOffset {{ a1: {!r}, a2: {!r} }},".format(
                float(t["a1"]), float(t["a2"])))
        elif ty == "IdealGasHelmholtzPower":
            ideal.append("    IdealTerm::Power {{ n: {}, t: {} }},".format(
                slice_f64(t["n"]), slice_f64(t["t"])))
        elif ty == "IdealGasHelmholtzPlanckEinsteinGeneralized":
            # theta = t directly (no sign flip, unlike the plain PlanckEinstein).
            ideal.append("    IdealTerm::PlanckEinsteinGeneralized {{ n: {}, theta: {}, c: {}, d: {} }},".format(
                slice_f64(t["n"]), slice_f64(t["t"]), slice_f64(t["c"]), slice_f64(t["d"])))
        elif ty == "IdealGasHelmholtzPlanckEinsteinFunctionT":
            # theta_i = -v_i / Tcrit; then a generalized PE with c=1, d=-1
            # (CoolProp FluidLibrary.h: PlanckEinsteinFunctionT -> Generalized).
            tcrit = float(t["Tcrit"])
            theta = [-float(v) / tcrit for v in t["v"]]
            nn = [float(x) for x in t["n"]]
            ideal.append("    IdealTerm::PlanckEinsteinGeneralized {{ n: {}, theta: {}, c: {}, d: {} }},".format(
                slice_f64(nn), slice_f64(theta), slice_f64([1.0] * len(nn)), slice_f64([-1.0] * len(nn))))
        elif ty == "IdealGasHelmholtzCP0Constant":
            tau0 = float(t["Tc"]) / float(t["T0"])
            ideal.append("    IdealTerm::CP0Constant {{ cp_over_r: {!r}, tau0: {!r} }},".format(
                float(t["cp_over_R"]), tau0))
        elif ty == "IdealGasHelmholtzCP0PolyT":
            ideal.append("    IdealTerm::CP0PolyT {{ c: {}, t: {}, tc: {!r}, t0: {!r} }},".format(
                slice_f64(t["c"]), slice_f64(t["t"]), float(t["Tc"]), float(t["T0"])))
        elif ty == "IdealGasHelmholtzCP0AlyLee":
            # CoolProp FluidLibrary.h lowers AlyLee's 5 constants into a
            # CP0PolyT constant term + PlanckEinsteinGeneralized sinh/cosh terms.
            consts = [float(x) for x in t["c"]]
            tc = float(t["Tc"]); t0 = float(t["T0"])
            if abs(consts[0]) > 1e-14:
                ideal.append("    IdealTerm::CP0PolyT {{ c: {}, t: {}, tc: {!r}, t0: {!r} }},".format(
                    slice_f64([consts[0]]), slice_f64([0.0]), tc, t0))
            n_pe, th_pe, c_pe, d_pe = [], [], [], []
            if abs(consts[1]) > 1e-14:  # sinh term: c_k=1, d_k=-1
                n_pe.append(consts[1]); th_pe.append(-2.0 * consts[2] / tc); c_pe.append(1.0); d_pe.append(-1.0)
            if abs(consts[3]) > 1e-14:  # cosh term: c_k=1, d_k=+1
                n_pe.append(-consts[3]); th_pe.append(-2.0 * consts[4] / tc); c_pe.append(1.0); d_pe.append(1.0)
            if n_pe:
                ideal.append("    IdealTerm::PlanckEinsteinGeneralized {{ n: {}, theta: {}, c: {}, d: {} }},".format(
                    slice_f64(n_pe), slice_f64(th_pe), slice_f64(c_pe), slice_f64(d_pe)))
        else:
            sys.exit(f"unsupported ideal term type: {ty} (extend the engine + codegen)")

    out = []
    out.append("// SPDX-License-Identifier: MIT")
    out.append("// GENERATED by dev/gen_fluid.py from CoolProp's {}.json (MIT).".format(fluid))
    out.append("// Do not edit by hand — re-run the generator. See dev/gen_fluid.py.")
    out.append("//! CoolProp `{}` Helmholtz EOS, as hardcoded `const` data.".format(fluid))
    # EOS fit coefficients occasionally land near pi, 1/pi, etc.; they are data,
    # not approximations of those constants.
    out.append("#![allow(clippy::approx_constant)]")
    out.append("")
    out.append("use crate::eos::{FluidEos, IdealTerm, ResidualTerm};")
    out.append("")
    out.append("/// {} Helmholtz equation of state (from CoolProp).".format(fluid))
    out.append("pub static {}: FluidEos = FluidEos {{".format(const_name))
    out.append('    name: "{}",'.format(fluid))
    out.append("    molar_mass: {!r},".format(float(eos["molar_mass"])))
    out.append("    gas_constant: {!r},".format(float(eos["gas_constant"])))
    out.append("    t_reducing: {!r},".format(float(red["T"])))
    out.append("    rho_reducing: {!r},".format(float(red["rhomolar"])))
    out.append("    t_critical: {!r},".format(float(crit["T"])))
    out.append("    rho_critical: {!r},".format(float(crit["rhomolar"])))
    out.append("    p_critical: {!r},".format(float(crit["p"])))
    out.append("    t_triple: {!r},".format(float(eos["Ttriple"])))
    out.append("    t_max: {!r},".format(float(eos["T_max"])))
    out.append("    p_max: {!r},".format(float(eos["p_max"])))
    out.append("    acentric: {!r},".format(float(eos["acentric"])))
    out.append("    residual: &[")
    out.extend(residual)
    out.append("    ],")
    out.append("    ideal: &[")
    out.extend(ideal)
    out.append("    ],")
    out.append("};")
    out.append("")
    print("\n".join(out))

if __name__ == "__main__":
    main()
