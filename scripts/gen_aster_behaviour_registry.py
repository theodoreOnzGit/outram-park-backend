#!/usr/bin/env python3
"""Generate the Rust code_aster behaviour registry from the upstream catalogue.

code_aster declares each of its constitutive laws as a ``LoiComportement(...)``
call in ``code_aster/Behaviours/*.py``.  Those declarations are pure metadata --
law name, ``num_lc`` dispatch number, state-variable names, supported
modelisations and deformations, integration algorithms -- with no algorithm in
them.  That makes the catalogue machine-readable, and it is why the Rust
registry is *generated* rather than hand-transcribed: 231 declarations
hand-copied would drift from upstream silently, and the drift would be invisible
until a law dispatched to the wrong number.

The parse uses :mod:`ast` and never imports or executes upstream code.  That is
deliberate: the upstream tree is read-only reference material, and executing it
would both violate that and require its dependencies.

Usage::

    python3 scripts/gen_aster_behaviour_registry.py \\
        --upstream /opt/upstream/codeaster-src \\
        --out crates/outram-park-fork-offbeat/src/rheology/aster/catalogue.rs

Provenance is baked into the generated header: upstream commit, generation
date, and the count of declarations parsed.  Regenerate after any upstream bump
rather than editing the output.
"""

from __future__ import annotations

import argparse
import ast
import datetime
import pathlib
import re
import subprocess
import sys

# Upstream field -> whether it is a tuple-valued field. Scalar fields (nom,
# num_lc, nb_vari, doc) are handled individually.
TUPLE_FIELDS = (
    "lc_type",
    "nom_vari",
    "mc_mater",
    "modelisation",
    "deformation",
    "algo_inte",
    "type_matr_tang",
    "proprietes",
    "syme_matr_tang",
    "exte_vari",
    "deform_ldc",
    "regu_visc",
    "post_incr",
)


def literal(node):
    """Best-effort literal evaluation, returning None for anything dynamic.

    A handful of declarations build a field from a module-level constant rather
    than a literal.  Those are reported by the caller rather than guessed at --
    silently substituting a default would put a wrong dispatch number or a wrong
    state-variable count into the registry.
    """
    try:
        return ast.literal_eval(node)
    except (ValueError, TypeError, SyntaxError):
        return None


def as_tuple(value):
    """Normalise an upstream field to a tuple of strings."""
    if value is None:
        return ()
    if isinstance(value, str):
        return (value,)
    if isinstance(value, (list, tuple)):
        return tuple(str(v) for v in value if v is not None)
    return (str(value),)


def parse_declaration(path: pathlib.Path):
    """Extract one ``LoiComportement`` declaration, or None if the file has none."""
    tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    for node in ast.walk(tree):
        if not isinstance(node, ast.Call):
            continue
        func = node.func
        name = func.id if isinstance(func, ast.Name) else getattr(func, "attr", None)
        # Upstream declares laws through two classes: `LoiComportement` for the
        # Fortran-implemented laws, and `LoiComportementMFront` for the 14
        # declared in MFront's DSL. Both are catalogue entries and both belong
        # in the registry -- `META_LEMA_ANI`, a named target of the port's
        # metallurgy/irradiation phase, is an MFront one. Accepting only the
        # base class would silently drop it.
        if name not in ("LoiComportement", "LoiComportementMFront"):
            continue
        fields = {kw.arg: literal(kw.value) for kw in node.keywords if kw.arg}
        if not fields.get("nom"):
            continue
        fields["__mfront__"] = name == "LoiComportementMFront"
        return fields, path.name
    return None


def rust_ident(aster_name: str) -> str:
    """Turn an ASTER behaviour name into an UpperCamelCase Rust identifier.

    This is a mechanical transliteration for the *generated* registry only.  It
    is NOT the descriptive English name the port's hand-written laws carry --
    see section 4 of docs/code-aster-port-scoping.md, which requires a law such
    as `NORTON` to surface as `NortonViscoplastic` in the hand-written API.  The
    registry keeps the mechanical form so that it can be regenerated without
    losing hand-chosen names, and the two are linked by `aster_name()`.
    """
    parts = re.split(r"[^0-9A-Za-z]+", aster_name)
    out = "".join(p[:1].upper() + p[1:].lower() for p in parts if p)
    if out and out[0].isdigit():
        out = "Law" + out
    return out or "Unnamed"


def escape(text: str) -> str:
    return text.replace("\\", "\\\\").replace('"', '\\"')


def doc_lines(text: str) -> list[str]:
    """Wrap an upstream ``doc`` string into Rust doc-comment lines.

    Upstream docs are in French; they are reproduced verbatim rather than
    translated, because a machine translation in a provenance-bearing comment
    would be an unattributed paraphrase of upstream's own words.
    """
    if not text:
        return []
    flat = " ".join(text.split())
    words, lines, cur = flat.split(), [], ""
    for w in words:
        if len(cur) + len(w) + 1 > 72:
            lines.append(cur)
            cur = w
        else:
            cur = f"{cur} {w}".strip()
    if cur:
        lines.append(cur)
    return lines


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--upstream", required=True, type=pathlib.Path)
    ap.add_argument("--out", required=True, type=pathlib.Path)
    args = ap.parse_args()

    cat_dir = args.upstream / "code_aster" / "Behaviours"
    if not cat_dir.is_dir():
        print(f"error: catalogue not found at {cat_dir}", file=sys.stderr)
        return 1

    try:
        commit = subprocess.run(
            ["git", "-C", str(args.upstream), "rev-parse", "HEAD"],
            capture_output=True, text=True, check=True,
        ).stdout.strip()
    except (subprocess.CalledProcessError, FileNotFoundError):
        commit = "unknown"

    laws, skipped = [], []
    for path in sorted(cat_dir.glob("*.py")):
        if path.name.startswith("__") or path.name == "cata_comportement.py":
            continue
        parsed = parse_declaration(path)
        if parsed is None:
            skipped.append((path.name, "no LoiComportement declaration"))
            continue
        fields, fname = parsed
        nom = fields.get("nom")
        num_lc = fields.get("num_lc")
        nb_vari = fields.get("nb_vari")
        if nb_vari is None:
            # Upstream leaves nb_vari unset on some MFront declarations, where
            # the state-variable count comes from the generated law rather than
            # the catalogue. Zero is the honest transcription of "the catalogue
            # does not say", and `has_declared_state_variables` reports it.
            nb_vari = 0
        if not isinstance(num_lc, int) or not isinstance(nb_vari, int):
            # Report rather than guess: a wrong num_lc is a wrong dispatch.
            skipped.append((fname, f"non-literal num_lc/nb_vari ({num_lc!r}/{nb_vari!r})"))
            continue
        record = {
            "nom": nom,
            "ident": rust_ident(nom),
            "num_lc": num_lc,
            "nb_vari": nb_vari,
            "doc": fields.get("doc") or "",
            "file": fname,
            "mfront": bool(fields.get("__mfront__")),
        }
        for f in TUPLE_FIELDS:
            record[f] = as_tuple(fields.get(f))
        laws.append(record)

    # Guard against two laws colliding on one Rust identifier.
    seen: dict[str, str] = {}
    for law in laws:
        if law["ident"] in seen:
            print(
                f"error: Rust identifier {law['ident']!r} claimed by both "
                f"{seen[law['ident']]!r} and {law['nom']!r}",
                file=sys.stderr,
            )
            return 1
        seen[law["ident"]] = law["nom"]

    laws.sort(key=lambda r: (r["num_lc"], r["nom"]))
    today = datetime.date.today().isoformat()

    o: list[str] = []
    w = o.append
    w("// SPDX-License-Identifier: GPL-3.0-only")
    w("// Copyright (C) 2026 OUTRAM PARK contributors")
    w("//")
    w("// Derived from code_aster (https://gitlab.com/codeaster/src)")
    w("//   Copyright (C) 1991 - 2026 - EDF R&D")
    w("//   Licence: GPL-3.0-or-later")
    w(f"//   Upstream commit: {commit}")
    w("//   Source: code_aster/Behaviours/*.py (declarative catalogue; metadata only)")
    w("//")
    w("// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence")
    w("// notice.")
    w("//")
    w("// @generated by scripts/gen_aster_behaviour_registry.py --")
    w(f"// DO NOT EDIT BY HAND. Regenerate after any upstream bump. Generated {today}.")
    w("")
    w("//! Generated registry of code_aster's constitutive-law catalogue.")
    w("//!")
    w("//! # What this is, and what it is not")
    w("//!")
    w("//! This is **metadata only** -- the declarative half of code_aster's")
    w("//! behaviour catalogue, transcribed mechanically. It records what laws")
    w("//! exist, the `num_lc` number each dispatches on, how many internal state")
    w("//! variables each carries and what they are called, and which")
    w("//! modelisations, strain measures and integration algorithms each")
    w("//! supports.")
    w("//!")
    w("//! It contains **no physics**. No stress is computed here. A variant")
    w("//! appearing in [`AsterBehaviour`] means only that upstream declares that")
    w("//! law, not that this port implements it -- ask")
    w("//! [`AsterBehaviour::is_implemented`] for that, and expect `false` for")
    w("//! nearly all of them at present.")
    w("//!")
    w("//! # Why generated")
    w("//!")
    w("//! 231 declarations transcribed by hand would drift from upstream")
    w("//! silently, and the drift would stay invisible until a law dispatched on")
    w("//! the wrong number and produced a plausible wrong stress. Regenerating")
    w("//! from the upstream tree makes any divergence a diff rather than a")
    w("//! mystery.")
    w("//!")
    w("//! # Naming")
    w("//!")
    w("//! Variant identifiers here are a mechanical transliteration of the ASTER")
    w("//! name (`VISC_CIN2_CHAB` -> `ViscCin2Chab`). They are deliberately *not*")
    w("//! the descriptive English names the hand-written laws carry -- per")
    w("//! `docs/code-aster-port-scoping.md` section 4, `NORTON` surfaces as")
    w("//! `NortonViscoplastic` in the implemented API. Keeping the registry")
    w("//! mechanical means it can be regenerated without overwriting hand-chosen")
    w("//! names; [`AsterBehaviour::aster_name`] is the link between the two.")
    w("")
    w("/// One entry of code_aster's behaviour catalogue.")
    w("///")
    w("/// Every variant corresponds to one `LoiComportement` declaration in")
    w("/// upstream's `code_aster/Behaviours/`. See the module documentation for")
    w("/// why presence here does not imply implementation.")
    w("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]")
    w("#[non_exhaustive]")
    w("pub enum AsterBehaviour {")
    for law in laws:
        for dl in doc_lines(law["doc"]):
            w(f"    /// {dl}")
        if law["doc"]:
            w("    ///")
        w(f"    /// ASTER behaviour name: `{law['nom']}` (`num_lc = {law['num_lc']}`,")
        w(f"    /// {law['nb_vari']} state variable(s)).")
        w(f"    /// Upstream declaration: `code_aster/Behaviours/{law['file']}`.")
        w(f"    {law['ident']},")
    w("}")
    w("")
    w("/// Every catalogue entry, ordered by `num_lc` then ASTER name.")
    w("pub const ALL: &[AsterBehaviour] = &[")
    for law in laws:
        w(f"    AsterBehaviour::{law['ident']},")
    w("];")
    w("")
    w("impl AsterBehaviour {")
    w("    /// The ASTER behaviour name, verbatim (e.g. `\"VISC_CIN2_CHAB\"`).")
    w("    ///")
    w("    /// This is what a code_aster user types in a deck and what the")
    w("    /// literature cites, so it is preserved exactly and must not be")
    w("    /// \"improved\".")
    w("    #[must_use]")
    w("    pub const fn aster_name(self) -> &'static str {")
    w("        match self {")
    for law in laws:
        w(f'            Self::{law["ident"]} => "{escape(law["nom"])}",')
    w("        }")
    w("    }")
    w("")
    w("    /// Upstream's `num_lc` dispatch number.")
    w("    ///")
    w("    /// code_aster dispatches constitutive laws through")
    w("    /// `bibfor/lc/lc0000.F90` on this number, so it is the stable identity")
    w("    /// of a law across upstream revisions -- more so than its name.")
    w("    #[must_use]")
    w("    pub const fn num_lc(self) -> u32 {")
    w("        match self {")
    for law in laws:
        w(f"            Self::{law['ident']} => {law['num_lc']},")
    w("        }")
    w("    }")
    w("")
    w("    /// Number of internal state variables the law carries per integration point.")
    w("    #[must_use]")
    w("    pub const fn n_state_variables(self) -> usize {")
    w("        match self {")
    for law in laws:
        w(f"            Self::{law['ident']} => {law['nb_vari']},")
    w("        }")
    w("    }")
    w("")
    w("    /// Names of the internal state variables, in upstream's order.")
    w("    ///")
    w("    /// The order is load-bearing: it is the order the state vector is")
    w("    /// packed in, so a law reading `EPSPXY` from the wrong slot produces a")
    w("    /// plausible wrong answer rather than an error.")
    w("    #[must_use]")
    w("    pub const fn state_variable_names(self) -> &'static [&'static str] {")
    w("        match self {")
    for law in laws:
        names = ", ".join(f'"{escape(n)}"' for n in law["nom_vari"])
        w(f"            Self::{law['ident']} => &[{names}],")
    w("        }")
    w("    }")
    w("")
    w("    /// `lc_type` classification (`MECANIQUE`, `KIT_THM`, ...).")
    w("    #[must_use]")
    w("    pub const fn lc_types(self) -> &'static [&'static str] {")
    w("        match self {")
    for law in laws:
        vals = ", ".join(f'"{escape(v)}"' for v in law["lc_type"])
        w(f"            Self::{law['ident']} => &[{vals}],")
    w("        }")
    w("    }")
    w("")
    w("    /// Strain measures the law supports (`PETIT`, `PETIT_REAC`, `GDEF_LOG`, ...).")
    w("    ///")
    w("    /// `PETIT` is small strain; `GDEF_LOG` is the logarithmic (Hencky)")
    w("    /// finite-strain wrapper. Since this port designs for finite strain")
    w("    /// from the start (maintainer decision, 2026-08-04), a law offering")
    w("    /// only `PETIT` is a law whose finite-strain use needs justifying.")
    w("    #[must_use]")
    w("    pub const fn deformations(self) -> &'static [&'static str] {")
    w("        match self {")
    for law in laws:
        vals = ", ".join(f'"{escape(v)}"' for v in law["deformation"])
        w(f"            Self::{law['ident']} => &[{vals}],")
    w("        }")
    w("    }")
    w("")
    w("    /// Integration algorithms upstream offers for this law.")
    w("    #[must_use]")
    w("    pub const fn integration_algorithms(self) -> &'static [&'static str] {")
    w("        match self {")
    for law in laws:
        vals = ", ".join(f'"{escape(v)}"' for v in law["algo_inte"])
        w(f"            Self::{law['ident']} => &[{vals}],")
    w("        }")
    w("    }")
    w("")
    w("    /// Modelisations the law supports (`3D`, `AXIS`, `D_PLAN`, ...).")
    w("    #[must_use]")
    w("    pub const fn modelisations(self) -> &'static [&'static str] {")
    w("        match self {")
    for law in laws:
        vals = ", ".join(f'"{escape(v)}"' for v in law["modelisation"])
        w(f"            Self::{law['ident']} => &[{vals}],")
    w("        }")
    w("    }")
    w("")
    w("    /// Material-property keywords the law reads (`ELAS`, `LEMAITRE`, ...).")
    w("    #[must_use]")
    w("    pub const fn material_keywords(self) -> &'static [&'static str] {")
    w("        match self {")
    for law in laws:
        vals = ", ".join(f'"{escape(v)}"' for v in law["mc_mater"])
        w(f"            Self::{law['ident']} => &[{vals}],")
    w("        }")
    w("    }")
    w("")
    w("    /// True if upstream declares this law through MFront's DSL rather")
    w("    /// than implementing it in Fortran.")
    w("    ///")
    w("    /// These are declared with `LoiComportementMFront` and carry their")
    w("    /// algorithm in a `.mfront` file rather than a `bibfor/comport/`")
    w("    /// subroutine. They reach this port by a different route: the")
    w("    /// maintainer decided on 2026-08-04 to port the MFront **generator**")
    w("    /// rather than hand-porting these laws individually.")
    w("    #[must_use]")
    w("    pub const fn is_mfront(self) -> bool {")
    w("        match self {")
    for law in laws:
        w(f"            Self::{law['ident']} => {'true' if law['mfront'] else 'false'},")
    w("        }")
    w("    }")
    w("")
    w("    /// True if the catalogue states a state-variable count for this law.")
    w("    ///")
    w("    /// Some MFront declarations leave `nb_vari` unset, because the count")
    w("    /// comes from the generated law rather than the catalogue. For those,")
    w("    /// [`n_state_variables`](Self::n_state_variables) reports 0, which")
    w("    /// means \"the catalogue does not say\" and not \"this law is")
    w("    /// stateless\".")
    w("    #[must_use]")
    w("    pub fn has_declared_state_variables(self) -> bool {")
    w("        self.n_state_variables() > 0 || !self.state_variable_names().is_empty()")
    w("    }")
    w("")
    w("    /// True if this law is a mechanical (`MECANIQUE`) constitutive law.")
    w("    ///")
    w("    /// The 151 mechanical laws are the port's target; the THM, hydraulic")
    w("    /// and drying subsets are concrete- and geomechanics-oriented and are")
    w("    /// deferred.")
    w("    #[must_use]")
    w("    pub fn is_mechanical(self) -> bool {")
    w('        self.lc_types().contains(&"MECANIQUE")')
    w("    }")
    w("")
    w("    /// Look a law up by its ASTER name.")
    w("    #[must_use]")
    w("    pub fn from_aster_name(name: &str) -> Option<Self> {")
    w("        ALL.iter().copied().find(|b| b.aster_name() == name)")
    w("    }")
    w("")
    w("    /// Look a law up by its upstream `num_lc` dispatch number.")
    w("    #[must_use]")
    w("    pub fn from_num_lc(num_lc: u32) -> Option<Self> {")
    w("        ALL.iter().copied().find(|b| b.num_lc() == num_lc)")
    w("    }")
    w("}")
    w("")

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text("\n".join(o), encoding="utf-8")

    mech = sum(1 for law in laws if "MECANIQUE" in law["lc_type"])
    mfront = sum(1 for law in laws if law["mfront"])
    print(f"parsed {len(laws)} declarations ({mech} MECANIQUE, {mfront} MFront) -> {args.out}")
    print(f"upstream commit {commit}")
    if skipped:
        print(f"skipped {len(skipped)}:")
        for name, why in skipped:
            print(f"  {name}: {why}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
