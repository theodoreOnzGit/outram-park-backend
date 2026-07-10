#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Regenerate ALL fluids: run dev/gen_fluid.py over every CoolProp fluid JSON in
# the gitignored upstream_source clone, then rewrite src/fluids/mod.rs and
# src/fluid.rs so every fluid is declared and wired into the `Fluid` enum.
#
# Usage (from the crate root, with upstream_source/CoolProp present):
#   python3 dev/regen_all.py
#
# Naming (kept consistent with dev/gen_fluid.py's `rust_const_ident`):
#   module  = lowercase, non-alnum -> '_', digit-leading -> 'f_'   (file name)
#   const   = UPPER,     non-alnum -> '_', digit-leading -> 'F_'    (in the file)
#   variant = CoolProp name, non-alnum removed, each token capitalised,
#             digit-leading -> 'F' prefix  (e.g. n-Heptane -> NHeptane)
import glob, os, re, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
CRATE = os.path.dirname(HERE)


def module_name(fluid):
    s = re.sub(r"[^0-9a-zA-Z]", "_", fluid).lower()
    return "f_" + s if s and s[0].isdigit() else s


def const_name(fluid):
    s = "".join(ch if ch.isalnum() else "_" for ch in fluid).upper()
    return "F_" + s if s and s[0].isdigit() else s


def variant_name(fluid):
    toks = [t for t in re.split(r"[^0-9a-zA-Z]", fluid) if t]
    v = "".join(t[:1].upper() + t[1:] for t in toks)
    return "F" + v if v and v[0].isdigit() else v


# Richer doc comments for the physically notable fluids; concise for the rest.
DOC = {
    "Water": "Water (IAPWS-95). Its residual includes non-analytic critical-region\n"
             "    /// terms (currently a no-op stub), so accuracy is degraded within ~1 % of Tc.",
    "Helium": "Helium (Ortiz-Vega et al.). Power + Gaussian residual only (no\n"
              "    /// non-analytic term), so it is accurate even at the critical point.",
}


def main():
    fluid_dir = os.path.join(CRATE, "upstream_source", "CoolProp", "dev", "fluids")
    names = sorted(os.path.basename(f)[:-5] for f in glob.glob(os.path.join(fluid_dir, "*.json")))
    if not names:
        sys.exit(f"no fluid JSON found under {fluid_dir} (clone upstream_source first)")

    fluids_out = os.path.join(CRATE, "src", "fluids")
    for old in glob.glob(os.path.join(fluids_out, "*.rs")):
        if os.path.basename(old) != "mod.rs":
            os.remove(old)

    rows, failed = [], []
    for n in names:
        r = subprocess.run([sys.executable, os.path.join(HERE, "gen_fluid.py"), n],
                           capture_output=True, text=True)
        if r.returncode != 0:
            failed.append((n, r.stderr.strip()))
            continue
        with open(os.path.join(fluids_out, module_name(n) + ".rs"), "w") as fh:
            fh.write(r.stdout)
        cn = const_name(n)
        has_anc = f"{cn}_ANCILLARIES" in r.stdout
        has_trans = f"{cn}_TRANSPORT" in r.stdout
        rows.append((variant_name(n), module_name(n), cn, n, has_anc, has_trans))

    if failed:
        for n, e in failed:
            print("FAILED", n, e, file=sys.stderr)
        sys.exit(f"{len(failed)} fluid(s) failed to generate")

    with open(os.path.join(fluids_out, "mod.rs"), "w") as fh:
        fh.write(
            "//! Hardcoded per-fluid Helmholtz EOS data (`const FluidEos`), generated from\n"
            "//! CoolProp's fluid JSON by `dev/gen_fluid.py` (one file per fluid) and wired\n"
            "//! into [`super::fluid::Fluid`]. These are the only \"data\" the shipped crate\n"
            "//! carries — a few KB each, no runtime JSON.\n"
            "//!\n"
            "//! Regenerate the whole set with `dev/regen_all.py` (see the crate README).\n\n")
        for row in sorted(rows, key=lambda r: r[1]):
            fh.write(f"pub mod {row[1]};\n")

    with open(os.path.join(CRATE, "src", "fluid.rs"), "w") as fh:
        fh.write(
            "//! The fluid selector — an **enum**, matching each fluid to its hardcoded\n"
            "//! [`FluidEos`]. This is the enum-dispatch replacement for CoolProp's\n"
            "//! string-keyed fluid lookup / backend polymorphism: adding a fluid is a new\n"
            "//! variant, and every `match` on `Fluid` becomes exhaustive.\n"
            "//!\n"
            "//! All of CoolProp's pure fluids are wired in. Variant names follow the\n"
            "//! CoolProp fluid name with non-alphanumerics removed (e.g. `n-Heptane` →\n"
            "//! `NHeptane`, `R1234ze(E)` → `R1234zeE`); [`Fluid::name`] returns the\n"
            "//! original CoolProp name.\n\n"
            "use crate::ancillaries::FluidAncillaries;\n"
            "use crate::eos::FluidEos;\n"
            "use crate::fluids;\n"
            "use crate::transport::FluidTransport;\n\n"
            "/// A supported pure fluid (one per CoolProp pure-fluid EOS). Each variant maps\n"
            "/// to a hardcoded `const` [`FluidEos`] via [`Fluid::eos`].\n"
            "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n"
            "#[non_exhaustive]\n"
            "#[allow(non_camel_case_types)]\n"
            "pub enum Fluid {\n")
        for var, _, _, n, _, _ in sorted(rows):
            fh.write(f"    /// {DOC.get(n, f'CoolProp `{n}`.')}\n    {var},\n")
        fh.write("}\n\nimpl Fluid {\n")
        fh.write(f"    /// Every supported fluid, for enumeration ({len(rows)} in total).\n")
        fh.write("    pub const ALL: &'static [Fluid] = &[\n")
        for var, _, _, _, _, _ in sorted(rows):
            fh.write(f"        Fluid::{var},\n")
        fh.write("    ];\n\n")
        fh.write("    /// The hardcoded Helmholtz EOS for this fluid.\n"
                 "    pub fn eos(self) -> &'static FluidEos {\n"
                 "        match self {\n")
        for var, mod, const, _, _, _ in sorted(rows):
            fh.write(f"            Fluid::{var} => &fluids::{mod}::{const},\n")
        fh.write("        }\n    }\n\n")

        # ancillaries(): Some(&..._ANCILLARIES) for fluids that ship saturation fits.
        n_anc = sum(1 for r in rows if r[4])
        fh.write("    /// Saturation ancillaries (for VLE / saturation lookups), if this fluid\n"
                 f"    /// ships them ({n_anc} of {len(rows)} do).\n"
                 "    pub fn ancillaries(self) -> Option<&'static FluidAncillaries> {\n"
                 "        match self {\n")
        for var, mod, const, _, has_anc, _ in sorted(rows):
            if has_anc:
                fh.write(f"            Fluid::{var} => Some(&fluids::{mod}::{const}_ANCILLARIES),\n")
        fh.write("            #[allow(unreachable_patterns)]\n"
                 "            _ => None,\n"
                 "        }\n    }\n\n")

        # transport(): Some(&..._TRANSPORT) for fluids with a supported μ/λ model.
        n_tr = sum(1 for r in rows if r[5])
        fh.write("    /// Transport models (viscosity/conductivity), if this fluid has a\n"
                 f"    /// supported model ({n_tr} of {len(rows)} do; the rest are hardcoded or\n"
                 "    /// unimplemented — see `crate::transport`).\n"
                 "    pub fn transport(self) -> Option<&'static FluidTransport> {\n"
                 "        match self {\n")
        for var, mod, const, _, _, has_tr in sorted(rows):
            if has_tr:
                fh.write(f"            Fluid::{var} => Some(&fluids::{mod}::{const}_TRANSPORT),\n")
        fh.write("            #[allow(unreachable_patterns)]\n"
                 "            _ => None,\n"
                 "        }\n    }\n\n")

        fh.write("    /// The fluid's name (as in CoolProp).\n"
                 "    pub fn name(self) -> &'static str {\n"
                 "        self.eos().name\n"
                 "    }\n}\n")

    print(f"regenerated {len(rows)} fluids + mod.rs + fluid.rs "
          f"({sum(1 for r in rows if r[4])} anc, {sum(1 for r in rows if r[5])} transport)")


if __name__ == "__main__":
    main()
