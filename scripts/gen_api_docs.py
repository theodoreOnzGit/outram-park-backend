#!/usr/bin/env python3
"""Generate crates/<crate>/docs/api.md: a single-file Markdown mirror of that
crate's public API, via rustdoc's (nightly-only) JSON output piped through
the `rustdoc-md` crate (https://github.com/tqwewe/rustdoc-md).

This is the auto-generated third leg of the per-crate docs/ convention (see
docs/docs-ci-spec.md and each crate's docs/README.md): docs/api.md sits
alongside the hand-written docs/code_structure.md ("why the code is laid out
this way") and docs/latex/ (theory -> numerics -> implementation ->
walkthrough) -- docs/api.md is "what every public item's doc comment says",
as one greppable, LLM-ingestible file.

Why rustdoc-md and not a hand-rolled HTML scrape of `cargo doc`'s output:
rustdoc's JSON output is the same structured AST rustdoc itself renders
from, so item lists (enum variants, struct fields, trait methods) come out
complete and correctly typed. An HTML scrape has to fight rendering-only
artifacts instead (collapsed "Show N variants" disclosure widgets meant for
JS, nav-sidebar/search-box chrome) and breaks silently if rustdoc's internal
template markup changes across toolchain versions. An early version of this
script scraped HTML with pandoc; it produced a truncated `Fluid` enum
variant list from the JS-collapsed widget and was replaced with this one
before being committed.

Trade-off knowingly accepted: rustdoc-md emits one monolithic file per
crate, not a directory tree mirroring the module hierarchy (its module
headings are flat -- a submodule gets the same heading level as its parent,
so the tree can't be reconstructed from heading depth alone). For a crate
this size that is still comfortably GitHub-renderable and, per the tool's
own design goal, easier to hand to an AI assistant wholesale than a 800-file
tree would be.

Requires:
  - a nightly Rust toolchain (`rustup toolchain install nightly`) -- the
    JSON output format is nightly-only.
  - the `rustdoc-md` binary (`cargo install rustdoc-md --locked`); this
    script installs it automatically if missing.

Usage (from the workspace root):
    python3 scripts/gen_api_docs.py <crate-dir-name> [--private]

Example:
    python3 scripts/gen_api_docs.py outram-park-fork-coolprop
"""
import argparse
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def read_package_name(crate_dir: Path) -> str:
    cargo_toml = (crate_dir / "Cargo.toml").read_text()
    m = re.search(r'^\s*name\s*=\s*"([^"]+)"', cargo_toml, re.M)
    if not m:
        sys.exit(f"error: could not find [package] name in {crate_dir / 'Cargo.toml'}")
    return m.group(1)


def ensure_nightly() -> None:
    result = subprocess.run(["rustup", "toolchain", "list"], capture_output=True, text=True)
    if "nightly" not in result.stdout:
        sys.exit(
            "error: no nightly toolchain installed -- rustdoc's JSON output is "
            "nightly-only. Run: rustup toolchain install nightly"
        )


def ensure_rustdoc_md() -> None:
    if shutil.which("rustdoc-md"):
        return
    print("rustdoc-md not found on PATH -- installing via `cargo install rustdoc-md --locked`...")
    subprocess.run(["cargo", "install", "rustdoc-md", "--locked"], check=True)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("crate", help="crate directory name under crates/, e.g. outram-park-fork-coolprop")
    ap.add_argument("--private", action="store_true", help="include --document-private-items")
    args = ap.parse_args()

    crate_dir = ROOT / "crates" / args.crate
    if not crate_dir.is_dir():
        sys.exit(f"error: {crate_dir} does not exist")

    package_name = read_package_name(crate_dir)
    snake_name = package_name.replace("-", "_")

    ensure_nightly()
    ensure_rustdoc_md()

    doc_cmd = [
        "cargo", "+nightly", "doc", "-p", package_name, "--no-deps",
        "-Z", "unstable-options", "--output-format", "json",
    ]
    if args.private:
        doc_cmd.append("--document-private-items")
    print("running:", " ".join(doc_cmd))
    subprocess.run(doc_cmd, cwd=ROOT, check=True)

    json_path = ROOT / "target" / "doc" / f"{snake_name}.json"
    if not json_path.is_file():
        sys.exit(f"error: expected rustdoc JSON at {json_path}, not found")

    out_path = crate_dir / "docs" / "api.md"
    out_path.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(["rustdoc-md", "--path", str(json_path), "--output", str(out_path)], check=True)

    json_path.unlink()

    # rustdoc-md 0.2.0 double-escapes the lifetime apostrophe in `&'static`
    # references (emits `&''static`) -- a known upstream serialization quirk,
    # not a workspace-specific issue. Fix it here rather than waiting on an
    # upstream release; safe to drop this once rustdoc-md stops doing it.
    text = out_path.read_text(encoding="utf-8")
    fixed = text.replace("&''", "&'")
    if fixed != text:
        out_path.write_text(fixed, encoding="utf-8")

    print(f"wrote {out_path}")


if __name__ == "__main__":
    main()
