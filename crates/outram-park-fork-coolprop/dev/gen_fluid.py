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


# ── Ancillaries (saturation correlations) ─────────────────────────────────────

def _sat(a, exponential):
    return ("SatAncillary {{ reducing_value: {!r}, t_r: {!r}, using_tau_r: {}, "
            "exponential: {}, n: {}, t: {} }}").format(
        float(a["reducing_value"]), float(a["T_r"]),
        "true" if a.get("using_tau_r") else "false",
        "true" if exponential else "false",
        slice_f64(a["n"]), slice_f64(a["t"]))

def build_ancillaries(d, const_name):
    """A `<NAME>_ANCILLARIES` const, or None if the fluid lacks pS/rhoL/rhoV."""
    anc = d.get("ANCILLARIES", {}) or {}
    pS, rhoL, rhoV = anc.get("pS"), anc.get("rhoL"), anc.get("rhoV")
    if not (isinstance(pS, dict) and isinstance(rhoL, dict) and isinstance(rhoV, dict)):
        return None
    if pS.get("type") not in ("pL", "pV"):
        return None
    if rhoL.get("type") != "rhoLnoexp" or rhoV.get("type") != "rhoV":
        return None
    return "\n".join([
        "/// Saturation ancillaries (CoolProp): fast p_sat/rho' /rho'' fits, used",
        "/// as the VLE initial guess and for standalone saturation lookups.",
        "pub static {}_ANCILLARIES: FluidAncillaries = FluidAncillaries {{".format(const_name),
        "    p_sat: {},".format(_sat(pS, True)),
        "    rho_l: {},".format(_sat(rhoL, False)),
        "    rho_v: {},".format(_sat(rhoV, True)),
        "};",
    ])


# ── Transport (viscosity + conductivity), common model subset ─────────────────

def build_viscosity(visc, ttriple, gas_constant, molar_mass):
    """A `ViscosityModel` literal, or None if unsupported/fully-hardcoded."""
    if not isinstance(visc, dict) or visc.get("hardcoded"):
        return None
    dil, ho = visc.get("dilute"), visc.get("higher_order")
    # Dilute: collision_integral, powers_of_T, or the CO2 Laesecke hardcoded form.
    if isinstance(dil, dict) and dil.get("hardcoded") == "CarbonDioxideLaeseckeJPCRD2017":
        dilute = "ViscosityDilute::CO2LaeseckeJPCRD2017"
    elif isinstance(dil, dict) and dil.get("type") == "collision_integral":
        dilute = ("ViscosityDilute::CollisionIntegral {{ c: {!r}, a: {}, t: {}, "
                  "molar_mass: {!r}, epsilon_over_k: {!r}, sigma_eta: {!r} }}").format(
            float(dil["C"]), slice_f64(dil["a"]), slice_f64(dil["t"]),
            float(dil["molar_mass"]), float(visc["epsilon_over_k"]), float(visc["sigma_eta"]))
    elif isinstance(dil, dict) and dil.get("type") == "powers_of_T":
        dilute = "ViscosityDilute::PowersOfT {{ a: {}, t: {} }}".format(
            slice_f64(dil["a"]), slice_f64(dil["t"]))
    else:
        return None
    # Higher order: modified_Batschinski_Hildebrand or the CO2 Laesecke form.
    if isinstance(ho, dict) and ho.get("hardcoded") == "CarbonDioxideLaeseckeJPCRD2017":
        higher = ("ViscosityHigherOrder::CO2LaeseckeJPCRD2017 {{ ttriple: {!r}, "
                  "gas_constant: {!r}, molar_mass: {!r} }}").format(ttriple, gas_constant, molar_mass)
        return _finish_viscosity(dilute, higher, visc)
    if not (isinstance(ho, dict) and ho.get("type") == "modified_Batschinski_Hildebrand"):
        return None
    higher = ("ViscosityHigherOrder::ModifiedBatschinskiHildebrand {{ t_reduce: {!r}, "
              "rhomolar_reduce: {!r}, a: {}, d1: {}, t1: {}, gamma: {}, l: {}, f: {}, "
              "d2: {}, t2: {}, g: {}, h: {}, p: {}, q: {} }}").format(
        float(ho["T_reduce"]), float(ho["rhomolar_reduce"]),
        slice_f64(ho["a"]), slice_f64(ho["d1"]), slice_f64(ho["t1"]), slice_f64(ho["gamma"]),
        slice_f64(ho["l"]), slice_f64(ho["f"]), slice_f64(ho["d2"]), slice_f64(ho["t2"]),
        slice_f64(ho["g"]), slice_f64(ho["h"]), slice_f64(ho["p"]), slice_f64(ho["q"]))
    return _finish_viscosity(dilute, higher, visc)

def _finish_viscosity(dilute, higher, visc):
    """Add the (optional) Rainwater-Friend initial-density term and assemble.
    Returns None if an unsupported initial term is present (no wrong numbers)."""
    initial = "None"
    for k in visc:
        if "initial" in k.lower() and isinstance(visc[k], dict):
            blk = visc[k]
            if blk.get("type") == "Rainwater-Friend":
                initial = ("Some(ViscosityInitial::RainwaterFriend {{ b: {}, t: {}, "
                           "epsilon_over_k: {!r}, sigma_eta: {!r} }})").format(
                    slice_f64(blk["b"]), slice_f64(blk["t"]),
                    float(visc["epsilon_over_k"]), float(visc["sigma_eta"]))
            else:
                return None
    return "ViscosityModel {{ dilute: {}, initial: {}, higher_order: {} }}".format(dilute, initial, higher)

def build_conductivity(cond, has_viscosity):
    """A `ConductivityModel` literal, or None if unsupported/hardcoded."""
    if not isinstance(cond, dict) or cond.get("hardcoded"):
        return None
    dil, res = cond.get("dilute"), cond.get("residual")
    if not isinstance(dil, dict) or not isinstance(res, dict):
        return None
    dt, rt = dil.get("type"), res.get("type")
    if dil.get("hardcoded") == "CarbonDioxideHuberJPCRD2016":
        dilute = "ConductivityDilute::CO2HuberJPCRD2016"
    elif dt == "ratio_of_polynomials":
        dilute = ("ConductivityDilute::RatioPolynomials {{ t_reducing: {!r}, a: {}, "
                  "n: {}, b: {}, m: {} }}").format(
            float(dil["T_reducing"]), slice_f64(dil["A"]), slice_f64(dil["n"]),
            slice_f64(dil["B"]), slice_f64(dil["m"]))
    elif dt == "eta0_and_poly":
        if not has_viscosity:   # needs the dilute viscosity to evaluate
            return None
        dilute = "ConductivityDilute::Eta0AndPoly {{ a: {}, t: {} }}".format(
            slice_f64(dil["A"]), slice_f64(dil["t"]))
    else:
        return None
    if rt == "polynomial":
        residual = ("ConductivityResidual::Polynomial {{ t_reducing: {!r}, "
                    "rhomass_reducing: {!r}, b: {}, t: {}, d: {} }}").format(
            float(res["T_reducing"]), float(res["rhomass_reducing"]),
            slice_f64(res["B"]), slice_f64(res["t"]), slice_f64(res["d"]))
    elif rt == "polynomial_and_exponential":
        residual = ("ConductivityResidual::PolynomialAndExponential {{ a: {}, t: {}, "
                    "d: {}, gamma: {}, l: {} }}").format(
            slice_f64(res["A"]), slice_f64(res["t"]), slice_f64(res["d"]),
            slice_f64(res["gamma"]), slice_f64(res["l"]))
    else:
        return None
    return "ConductivityModel {{ dilute: {}, residual: {} }}".format(dilute, residual)

# Fluids whose viscosity AND conductivity are hardcoded and implemented here.
HARDCODED_TRANSPORT = {
    "Helium": "HardcodedTransport::Helium",
    "Water": "HardcodedTransport::Water",
}

def hardcoded_transport(tr):
    """The `HardcodedTransport::…` variant for a fluid, or None."""
    v, c = tr.get("viscosity"), tr.get("conductivity")
    vh = v.get("hardcoded") if isinstance(v, dict) else None
    ch = c.get("hardcoded") if isinstance(c, dict) else None
    if vh and vh == ch and vh in HARDCODED_TRANSPORT:
        return HARDCODED_TRANSPORT[vh]
    return None

def build_transport(d, const_name):
    """A `<NAME>_TRANSPORT` const, or None if nothing is supported."""
    tr = d.get("TRANSPORT", {}) or {}
    eos = d["EOS"][0]
    hc = hardcoded_transport(tr)
    if hc:
        # A hardcoded formula supplies both μ and λ; skip the correlation fields.
        visc = cond = None
    else:
        visc = build_viscosity(tr.get("viscosity"), float(eos["Ttriple"]),
                               float(eos["gas_constant"]), float(eos["molar_mass"]))
        cond = build_conductivity(tr.get("conductivity"), visc is not None)
    if visc is None and cond is None and hc is None:
        return None
    v = "Some({})".format(visc) if visc else "None"
    c = "Some({})".format(cond) if cond else "None"
    h = "Some({})".format(hc) if hc else "None"
    return "\n".join([
        "/// Transport models (CoolProp): dynamic viscosity and/or thermal",
        "/// conductivity (critical enhancement omitted; see `crate::transport`).",
        "pub static {}_TRANSPORT: FluidTransport = FluidTransport {{".format(const_name),
        "    viscosity: {},".format(v),
        "    conductivity: {},".format(c),
        "    hardcoded: {},".format(h),
        "};",
    ])

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

    anc_block = build_ancillaries(d, const_name)
    trans_block = build_transport(d, const_name)

    out.append("use crate::eos::{FluidEos, IdealTerm, ResidualTerm};")
    if anc_block:
        out.append("use crate::ancillaries::{FluidAncillaries, SatAncillary};")
    if trans_block:
        ttypes = ["FluidTransport"]
        if "ViscosityModel" in trans_block:
            ttypes += ["ViscosityModel", "ViscosityDilute", "ViscosityHigherOrder"]
        if "ViscosityInitial" in trans_block:
            ttypes += ["ViscosityInitial"]
        if "ConductivityModel" in trans_block:
            ttypes += ["ConductivityModel", "ConductivityDilute", "ConductivityResidual"]
        if "HardcodedTransport" in trans_block:
            ttypes += ["HardcodedTransport"]
        out.append("use crate::transport::{{{}}};".format(", ".join(ttypes)))
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
    if anc_block:
        out.append(anc_block)
        out.append("")
    if trans_block:
        out.append(trans_block)
        out.append("")
    print("\n".join(out))

if __name__ == "__main__":
    main()
