#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Scaffold a new bite-sized LaTeX doc series under docs/latex/, following the
# theory -> numerical methods -> implementation -> walkthrough convention
# documented in docs/README.md.
#
# This is a SKELETON GENERATOR, not a source-to-docs auto-writer: it emits
# four correctly-structured, compilable .tex files with section headings and
# TODO placeholders (so `latexmk -pdf` succeeds immediately and a human/AI
# fills in the physics), not derived content. Full auto-generation from the
# Rust source is a larger, separate undertaking than this scaffold -- see the
# "What's not here yet" section of docs/README.md.
#
# Usage (run from the crate root):
#   python3 dev/gen_latex_doc.py "Some New Topic" some_new_topic
#
# The second argument is the file-name stem; output goes to
# docs/latex/{01,02,03,04}_<stem>.tex. Existing files are never overwritten
# without --force.
import argparse, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
CRATE = os.path.dirname(HERE)
LATEX_DIR = os.path.join(CRATE, "docs", "latex")

PARTS = [
    ("theory", "Theory",
     "The physics/mathematics on its own terms, independent of any "
     "particular implementation. What is being modelled, and why the "
     "governing equations take the form they do. No Rust code in this file."),
    ("numerical_methods", "Numerical Methods",
     "How the theory from Part 1 becomes a computable algorithm: what "
     "singularities/instabilities need guarding against, what iterative "
     "solves are needed and why, what tolerances are chosen and on what "
     "basis. Still no Rust-specific code -- this is language-agnostic "
     "numerical analysis."),
    ("implementation", "Rust Implementation",
     "How this crate specifically structures the code for the algorithm "
     "from Part 2: the relevant enum(s)/struct(s), where the const data "
     "comes from, and why this design (cross-reference the workspace's "
     "no-trait-objects / no-Box / human-interface-layer rules where "
     "relevant)."),
    ("walkthrough", "Function-by-Function Walkthrough",
     "A concrete worked example, traced from the public API entry point "
     "down to the verified numerical result, naming exact files and "
     "functions at each step. End with the actual printed/measured result "
     "and a pointer to the corresponding verification_and_validation/ "
     "write-up, if one exists."),
]

TEMPLATE = r"""\documentclass[11pt]{{article}}
\input{{preamble}}

\title{{{title} \\ \large Part {part_num}: {part_title}}}
\author{{{crate_name}}}
\date{{{date}}}

\begin{{document}}
\maketitle

\begin{{abstract}}
TODO: one-paragraph summary of what this part covers and how it connects to
the part before/after it in the series.
\end{{abstract}}

% {part_title} guidance for this part:
% {guidance}

\section{{TODO: first section heading}}

TODO. Cite references from \texttt{{references.bib}} with
\verb|\cite{{...}}|; equations get \verb|\begin{{equation}}...\end{{equation}}|
with a \verb|\label{{}}| if referenced later.

\printbibliography
\end{{document}}
"""


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("title", help="Human-readable topic title, e.g. 'The Foo Correction'")
    ap.add_argument("stem", help="File-name stem, e.g. foo_correction")
    ap.add_argument("--crate-name", default=os.path.basename(CRATE))
    ap.add_argument("--date", default="YYYY-MM-DD")
    ap.add_argument("--force", action="store_true", help="overwrite existing files")
    args = ap.parse_args()

    os.makedirs(LATEX_DIR, exist_ok=True)
    if not os.path.exists(os.path.join(LATEX_DIR, "preamble.tex")):
        sys.exit(f"error: {LATEX_DIR}/preamble.tex not found -- copy it from an existing "
                 "docs/latex/ series (or another crate's) before scaffolding a new one.")

    for i, (suffix, part_title, guidance) in enumerate(PARTS, start=1):
        path = os.path.join(LATEX_DIR, f"{i:02d}_{args.stem}.tex")
        if os.path.exists(path) and not args.force:
            sys.exit(f"error: {path} already exists (use --force to overwrite)")
        with open(path, "w") as fh:
            fh.write(TEMPLATE.format(
                title=args.title, part_num=i, part_title=part_title,
                crate_name=args.crate_name, date=args.date, guidance=guidance,
            ))
        print("wrote", path)

    print(f"\nNext: fill in each file's TODOs in order (theory -> numerics -> "
          f"implementation -> walkthrough), then:\n"
          f"  cd docs/latex && latexmk -pdf {1:02d}_{args.stem}.tex   # repeat per file\n"
          f"See docs/README.md and the {args.crate_name} worked example "
          f"(01_theory_non_analytic_term.tex etc., if present) for the expected depth/style.")


if __name__ == "__main__":
    main()
