#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Regenerate ALL fluids: run dev/gen_fluid.py over every CoolProp fluid JSON in
# the gitignored reference clone, then rewrite src/fluids/mod.rs and
# src/fluid.rs so every fluid is declared and wired into the `Fluid` enum.
#
# Usage (from the crate root, with reference/CoolProp present):
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
    fluid_dir = os.path.join(CRATE, "reference", "CoolProp", "dev", "fluids")
    names = sorted(os.path.basename(f)[:-5] for f in glob.glob(os.path.join(fluid_dir, "*.json")))
    if not names:
        sys.exit(f"no fluid JSON found under {fluid_dir} (clone the reference first)")

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
        rows.append((variant_name(n), module_name(n), const_name(n), n))

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
        for _, mod, _, _ in sorted(rows, key=lambda r: r[1]):
            fh.write(f"pub mod {mod};\n")

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
            "use crate::eos::FluidEos;\n"
            "use crate::fluids;\n\n"
            "/// A supported pure fluid (one per CoolProp pure-fluid EOS). Each variant maps\n"
            "/// to a hardcoded `const` [`FluidEos`] via [`Fluid::eos`].\n"
            "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n"
            "#[non_exhaustive]\n"
            "#[allow(non_camel_case_types)]\n"
            "pub enum Fluid {\n")
        for var, _, _, n in sorted(rows):
            fh.write(f"    /// {DOC.get(n, f'CoolProp `{n}`.')}\n    {var},\n")
        fh.write("}\n\nimpl Fluid {\n")
        fh.write(f"    /// Every supported fluid, for enumeration ({len(rows)} in total).\n")
        fh.write("    pub const ALL: &'static [Fluid] = &[\n")
        for var, _, _, _ in sorted(rows):
            fh.write(f"        Fluid::{var},\n")
        fh.write("    ];\n\n")
        fh.write("    /// The hardcoded Helmholtz EOS for this fluid.\n"
                 "    pub fn eos(self) -> &'static FluidEos {\n"
                 "        match self {\n")
        for var, mod, const, _ in sorted(rows):
            fh.write(f"            Fluid::{var} => &fluids::{mod}::{const},\n")
        fh.write("        }\n    }\n\n"
                 "    /// The fluid's name (as in CoolProp).\n"
                 "    pub fn name(self) -> &'static str {\n"
                 "        self.eos().name\n"
                 "    }\n}\n")

    print(f"regenerated {len(rows)} fluids + mod.rs + fluid.rs")


if __name__ == "__main__":
    main()
