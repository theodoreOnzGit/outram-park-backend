#!/usr/bin/env python3
r"""
kloc_accounting.py
==================

Reproduce the productivity accounting tables and figure in the Outram Park
introductory paper:

    Table  tab:preagentic_baseline   pre-agentic repositories (the baseline)
    Table  tab:agentic_crates        agentic lines in outram-park-backend
    Figure fig_kloc_productivity     lines per active day, and what the
                                     agentic output is made of

The tables in the manuscript were originally compiled with AI assistance from
repository measurements. This script exists so that those numbers can be
re-derived from the repositories themselves, by anyone, without trusting that
summary -- which is what Annals of Nuclear Energy asks for when a table or
figure is "directly derived from underlying data using reproducible
analytical, computational, or statistical methods".

Where this file lives
---------------------
Two copies, kept byte-identical so that ``diff`` is a sufficient check:

  * ``outram-park-backend/scripts/kloc_accounting.py`` -- shipped with the code
    it measures, so a reader who has the repository can reproduce the paper's
    numbers without also having the manuscript sources.
  * the paper's ``src/results_and_discussion/kloc_accounting/`` -- where the
    manuscript's tables and figure are generated and \input from.

Edit one, copy to the other. If they ever diverge, the backend copy wins: it
travels with the code, and it is the one a reader will find.

Source of truth
---------------
The git repositories themselves. Nothing is estimated and nothing is
hard-coded from the manuscript. Every number this script prints comes from
either

  * a walk over the ``.rs`` files in a checkout, or
  * ``git log`` over that checkout's history.

The one editorial input is CRATE_PROVENANCE below -- which crate counts as a
translation, which as original work, which as an extension of vendored
pre-agentic code. That mapping is stated explicitly rather than inferred, and
each crate's own ``Cargo.toml`` ``description`` is carried into the CSV beside
it so the classification can be audited against what the manifest declares.

Method notes
------------
* **Code lines exclude blank and comment-only lines.** Comments are removed
  with a real Rust scanner (``strip_rust_comments``) that understands nested
  block comments, raw strings (``r#"..."#``), byte strings and char literals,
  so a ``//`` inside a string is not mistaken for a comment. Total lines
  include comments, so Rust doc comments -- which carry executable doctests --
  appear only in the total.
* **An active day is any calendar date carrying at least one commit.** Totals
  across repositories are a union of distinct dates, not a column sum, because
  these projects overlapped in time. Summing the columns would double-count.
* **TUAS is reported net** of the code it inherited from
  ``thermal_hydraulics_rs`` at spin-out, so that the rows sum without double
  counting. Both the net and the head figures are printed.
* **Only ``.rs`` files are counted.** ``target/``, ``.git/`` and vendored
  third-party source trees are skipped (SKIP_DIRS).

Usage
-----
    python3 kloc_accounting.py                 # measure, print, write outputs
    python3 kloc_accounting.py --clone         # git clone anything missing
    python3 kloc_accounting.py --check         # compare against the manuscript
    python3 kloc_accounting.py --no-figure     # skip the PNG

Repositories are looked for in REPO_SEARCH_DIRS first, then in ``vendor/``
next to this script. ``--clone`` fetches whatever is still missing from GitHub
into ``vendor/``, which is gitignored -- the checkouts are large and are not
part of the manuscript.

Outputs (written next to this script)
-------------------------------------
    baseline_repositories.csv     one row per pre-agentic repository
    agentic_crates.csv            one row per crate in outram-park-backend
    summary.txt                   the console report, saved
    fig_kloc_productivity.png     the figure
"""

from __future__ import annotations

import argparse
import contextlib
import csv
import os
import re
import subprocess
import sys
import tarfile
import tempfile
from dataclasses import dataclass, field
from pathlib import Path

HERE = Path(__file__).resolve().parent
VENDOR = HERE / "vendor"

# Where to look for an already-present checkout before falling back to vendor/.
REPO_SEARCH_DIRS = [
    Path.home() / "Documents" / "research",
    Path.home() / "Desktop",
]

GITHUB_USER = "theodoreOnzGit"

# Which ref to measure, in order of preference.
#
# This matters more than it looks. These repositories squash-merge into `main`,
# so `main` carries a handful of release commits while the actual development
# history -- and therefore every active-day count -- lives on `develop`.
# Measuring `main` understates thermal_hydraulics_rs by 135 active days.
PREFERRED_REFS = ["develop", "origin/develop", "main", "origin/main",
                  "master", "origin/master", "HEAD"]

# Directories never descended into when counting source.
SKIP_DIRS = {
    ".git", "target", "node_modules", "vendor", ".venv", "venv",
    "build", "__pycache__", ".cargo", "third_party",
}


# --------------------------------------------------------------------------
# Repository configuration
# --------------------------------------------------------------------------

@dataclass
class Repo:
    """A checkout to measure."""
    key: str                     # directory / GitHub repo name
    label: str                   # how the manuscript names it, as LaTeX
    note: str = ""               # footnote text
    marker: str = ""             # footnote marker (a, b, c) in the table
    cite: str = ""               # bib key, cited in the table row
    # Count lines at this commit instead of the branch tip. Needed only where
    # a repository was emptied after its code moved elsewhere -- see below.
    measure_ref: str | None = None
    path: Path | None = field(default=None, init=False)


# The pre-agentic baseline, in the order the manuscript's table lists them.
BASELINE_REPOS = [
    # Measured at the last commit before spin-out, NOT at the branch tip.
    # On 2024-10-11 this repository was emptied -- 260 .rs files down to 9 --
    # when its code moved into TUAS. Its tip therefore holds ~1.6 KLOC, which
    # would understate the predecessor by ~58 KLOC and, worse, would make the
    # "TUAS net of what it inherited" subtraction meaningless. 4d534af is the
    # last commit at full extent (2024-10-08, 260 .rs files); verify with
    # --peak, which reports the maximum-extent commit on the measured ref.
    Repo("thermal_hydraulics_rs",
         r"\texttt{thermal\_hydraulics\_rs} (predecessor to TUAS)",
         "none", marker="a", cite="ong2024thermalhydraulicsrs",
         measure_ref="4d534af51eca256f318462234c0c8592930f764b"),
    Repo("chem-eng-real-time-process-control-simulator",
         r"\texttt{chem-eng-\ldots-simulator}",
         "none", marker="a", cite="ong2024chemengprocesscontrol"),
    Repo("teh-o-prke",
         r"\texttt{teh-o-prke}",
         "none", marker="a", cite="ong2025tehoprke"),
    Repo("tuas_boussinesq_solver",
         r"\texttt{tuas\_boussinesq\_solver} (net-new)",
         "none", marker="a", cite="ong2024tuasgithubrepo"),
    Repo("tampines-steam-tables",
         r"\texttt{tampines-steam-tables}",
         "root-finding solvers only", marker="b",
         cite="ong2026tampinessteamtables"),
    Repo("boon-lay",
         r"\texttt{boon-lay}",
         "code generation", marker="c", cite="ong2026boonlay"),
]

# The lettered footnotes under the baseline tables. `note` above is the short
# form used in the rate table's "AI?" column; these are the full statements.
BASELINE_FOOTNOTES = [
    ("a", "No AI assistance of any kind."),
    ("b", "Minimal AI assistance, confined to root-finding solvers, "
          "via NUS AI-know."),
    ("c", "Non-agentic AI code generation for user interfaces and some "
          "Monte Carlo solvers, via NUS AI-know."),
]

# TUAS is reported net of the code it inherited from thermal_hydraulics_rs at
# spin-out.
#
# What is subtracted is the code that ACTUALLY CAME ACROSS -- the tree at TUAS's
# second commit, which imported the predecessor wholesale 42 minutes after the
# initial commit -- and not the predecessor's own full extent.
#
# The two differ. The predecessor held 260 .rs files at 4d534af; 236 were
# imported. Subtracting its full extent would remove 7,754 code lines that were
# written in the predecessor and never carried forward, erasing real
# pre-agentic work from the baseline and, because the baseline is the
# denominator of the productivity claim, flattering the agentic figure. It
# would also contradict the table caption, which says "net of the code it
# inherited": inherited means what arrived, not what the predecessor happened
# to contain.
TUAS_KEY = "tuas_boussinesq_solver"
TUAS_PREDECESSOR_KEY = "thermal_hydraulics_rs"
TUAS_IMPORT_REF = "c451c8e203d5772955c2c9f3c6739e92b8180c78"   # 2024-10-10

# Pinned to a commit, not to the branch tip.
#
# `develop` is under active daily development -- it moved four commits during a
# single afternoon of preparing these tables, changing the translated subtotal
# by 462 lines. A manuscript that quotes the tip quotes a number nobody can
# reproduce afterwards. Set this to None to measure the tip instead; --check
# will then tell you how far the repository has moved since the pin.
AGENTIC_MEASURE_REF = "3130b38b65edc94288fb2715bbcab868e6846f79"   # 2026-07-23

AGENTIC_REPO = Repo("outram-park-backend", r"\texttt{outram-park-backend}",
                    measure_ref=AGENTIC_MEASURE_REF)

# The agentic window reported in the manuscript.
AGENTIC_SINCE = "2026-06-19"
AGENTIC_UNTIL = "2026-07-23"

# Provenance of each crate in outram-park-backend/crates.
#   ("translated", upstream)  -- pure-Rust fork or port of a named upstream
#   ("original",   None)      -- newly written for Outram Park
#   ("extension",  repo_key)  -- vendored pre-agentic crate; only the excess
#                                over the standalone original is agentic
CRATE_PROVENANCE = {
    "njoy-outram-park-fork":           ("translated", "NJOY2016"),
    "outram-park-fork-coolprop":       ("translated", "CoolProp"),
    "outram-foam-appbuilder-lib":      ("translated", "OpenFOAM"),
    "outram-foam-basic-lib":           ("translated", "OpenFOAM"),
    "outram-mc-libs":                  ("translated", "OpenMC"),
    "outram-park-fork-pflotran":       ("translated", "PFLOTRAN"),
    "outram-foam-mesh":                ("translated", "OpenFOAM"),
    "outram-blender":                  ("translated", "Blender"),
    "outram-park-fork-dwsim-libs":     ("translated", "DWSIM"),
    "outram-foam-turbulence-lib":      ("translated", "OpenFOAM"),
    "outram-foam-cli":                 ("translated", "OpenFOAM"),

    "outram-park-digital-twin-engine": ("original", None),
    "tampines":                        ("original", None),
    "kovan-tui":                       ("original", None),
    "kovan-cli":                       ("original", None),
    "kovan-codegen":                   ("original", None),
    "kovan-semantics":                 ("original", None),
    "kovan-literature":                ("original", None),
    "kovan-discovery":                 ("original", None),
    "kovan-common":                    ("original", None),
    "nee_soon":                        ("original", None),

    "tampines-steam-tables":           ("extension", "tampines-steam-tables"),
    "boon-lay":                        ("extension", "boon-lay"),
    "tuas_boussinesq_solver":          ("extension", "tuas_boussinesq_solver"),
    "teh-o-prke":                      ("extension", "teh-o-prke"),
    "chem-eng-real-time-process-control-simulator":
        ("extension", "chem-eng-real-time-process-control-simulator"),
}

# The discussion compares repositories written with no AI help at all against
# those that had some non-agentic help from NUS AI-know. Grouped here so the
# rates quoted in the prose come out of the same run as the tables.
ASSISTANCE_GROUPS = {
    "no AI assistance at all": [
        "thermal_hydraulics_rs",
        "chem-eng-real-time-process-control-simulator",
        "teh-o-prke",
        "tuas_boussinesq_solver",
    ],
    "some non-agentic NUS AI-know help": [
        "tampines-steam-tables",
        "boon-lay",
    ],
}

# Parts of a crate that are classified differently from the crate as a whole.
#
# On 2026-07-23 the two terminal interfaces stopped being workspace crates and
# became feature-gated binaries inside the libraries they drive, so that a
# library consumer no longer sees them as separate packages. Their code is
# newly written, but it now lives inside crates that are ports of an existing
# upstream. Counting them with their host crate would credit ~2.5 KLOC of
# original interface work as translation and overstate the translated share.
# They are therefore split out at the path boundary and reported separately.
CRATE_SUBPATH_PROVENANCE = {
    "njoy-outram-park-fork": [
        ("src/bin/njoy-tui", "original", "njoy-tui"),
    ],
    "outram-mc-libs": [
        ("src/bin/outram-mc-tui", "original", "outram-mc-tui"),
    ],
}

CLASS_ORDER = ["translated", "original", "extension"]
CLASS_LABEL = {
    "translated": "Translated or ported",
    "original":   "Originally written",
    "extension":  "Agentic extensions to vendored crates",
}

# Values as printed in the manuscript, for --check only. These are NOT used in
# any computation; they exist so the script can report drift.
#
# The tables themselves are now emitted by this script, so these cannot drift
# through transcription error any more. What they still catch is the case that
# matters: the repositories moving after the manuscript text -- the prose
# figures, the percentages, the 12.8x headline -- was written against them.
# Recorded from the run of 2026-07-23 on develop.
MANUSCRIPT = {
    "baseline_total_lines": 303_463,
    "baseline_code_lines": 181_298,
    "baseline_active_days": 367,
    "agentic_total_rust": 349_541,
    "agentic_vendored_preagentic": 173_544,
    "agentic_code_lines": 175_997,
    "subtotal_translated": 136_462,
    "subtotal_original": 27_177,
    "subtotal_extension": 12_358,
    "n_crates": 26,
}


# --------------------------------------------------------------------------
# Rust source scanning
# --------------------------------------------------------------------------

# r"..." / r#"..."# / br"..." / br#"..."#
_RAW_STRING = re.compile(r'b?r(?P<hashes>#*)"')
# 'a'  '\n'  '\x41'  '\u{1F600}'   -- but NOT the lifetime in &'a str
_CHAR_LIT = re.compile(r"'(?:\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F]{1,6}\}|.)|[^\\'\n])'")
_IDENT_CHAR = re.compile(r"[A-Za-z0-9_]")


def strip_rust_comments(src: str) -> str:
    """Remove Rust comments, preserving line structure.

    Handles nested block comments, line comments, ordinary and raw strings,
    byte strings and char literals. Newlines inside removed comments are kept
    so that line numbering -- and therefore the blank/comment-only test done
    by the caller -- stays aligned with the original file.
    """
    out: list[str] = []
    i, n = 0, len(src)
    depth = 0

    while i < n:
        if depth > 0:
            if src.startswith("/*", i):
                depth += 1
                i += 2
                continue
            if src.startswith("*/", i):
                depth -= 1
                i += 2
                continue
            if src[i] == "\n":
                out.append("\n")
            i += 1
            continue

        if src.startswith("//", i):
            while i < n and src[i] != "\n":
                i += 1
            continue

        if src.startswith("/*", i):
            depth = 1
            i += 2
            continue

        ch = src[i]

        # Raw string: only at a token boundary, so `for` / `expr` don't match.
        if ch in "rb":
            at_boundary = i == 0 or not _IDENT_CHAR.match(src[i - 1])
            m = _RAW_STRING.match(src, i) if at_boundary else None
            if m:
                terminator = '"' + m.group("hashes")
                end = src.find(terminator, m.end())
                end = n if end == -1 else end + len(terminator)
                out.append(src[i:end])
                i = end
                continue

        if ch == '"':
            j = i + 1
            while j < n:
                if src[j] == "\\":
                    j += 2
                    continue
                if src[j] == '"':
                    j += 1
                    break
                j += 1
            out.append(src[i:j])
            i = j
            continue

        if ch == "'":
            m = _CHAR_LIT.match(src, i)
            if m:
                out.append(m.group(0))
                i = m.end()
                continue
            # Otherwise a lifetime -- ordinary code, fall through.

        out.append(ch)
        i += 1

    return "".join(out)


def rust_files(root: Path):
    """Yield every ``.rs`` file under root, skipping SKIP_DIRS."""
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
        for name in filenames:
            if name.endswith(".rs"):
                yield Path(dirpath) / name


def count_lines(root: Path, exclude: list[Path] | None = None) -> tuple[int, int, int]:
    """Return (total_lines, code_lines, n_files) for the Rust source under root.

    ``exclude`` lists subtrees to leave out, so that a part of a crate which
    is classified differently from the crate as a whole can be counted on its
    own without being double counted here.
    """
    total = code = files = 0
    excluded = [e.resolve() for e in (exclude or [])]
    for path in rust_files(root):
        if any(path.resolve().is_relative_to(e) for e in excluded):
            continue
        try:
            src = path.read_text(encoding="utf-8", errors="replace")
        except OSError as exc:            # unreadable file: report, don't guess
            print(f"  ! could not read {path}: {exc}", file=sys.stderr)
            continue
        files += 1
        total += len(src.splitlines())
        for line in strip_rust_comments(src).splitlines():
            if line.strip():
                code += 1
    return total, code, files


# --------------------------------------------------------------------------
# Git
# --------------------------------------------------------------------------

def git(repo: Path, *args: str) -> str:
    """Run git in repo and return stdout, or "" if the command fails."""
    try:
        done = subprocess.run(
            ["git", "-C", str(repo), *args],
            capture_output=True, text=True, check=True,
        )
    except (subprocess.CalledProcessError, FileNotFoundError):
        return ""
    return done.stdout


def ref_exists(repo: Path, ref: str) -> bool:
    try:
        subprocess.run(["git", "-C", str(repo), "rev-parse", "--verify",
                        "--quiet", f"{ref}^{{commit}}"],
                       capture_output=True, check=True)
        return True
    except (subprocess.CalledProcessError, FileNotFoundError):
        return False


def select_ref(repo: Path) -> str:
    """The first of PREFERRED_REFS that resolves in this checkout."""
    for ref in PREFERRED_REFS:
        if ref_exists(repo, ref):
            return ref
    return "HEAD"


def active_days(repo: Path, ref: str = "HEAD", since: str | None = None,
                until: str | None = None) -> set[str]:
    """The set of calendar dates (YYYY-MM-DD) carrying at least one commit."""
    args = ["log", "--format=%ad", "--date=short", ref]
    if since:
        args.append(f"--since={since}")
    if until:
        args.append(f"--until={until}")
    return {d for d in git(repo, *args).split() if d}


def head_date(repo: Path, ref: str = "HEAD") -> str:
    return git(repo, "log", "-1", "--format=%ad", "--date=short", ref).strip()


@contextlib.contextmanager
def materialize(repo: Path, ref: str):
    """Yield a temp directory holding the tree at ``ref``.

    Read via ``git archive`` rather than by checking the ref out, so that a
    working repository the author has open -- possibly with uncommitted work --
    is never touched. The measurement is therefore of committed state at a
    named ref, which is what "reproducible" has to mean here.
    """
    with tempfile.TemporaryDirectory(prefix="kloc_accounting_") as td:
        proc = subprocess.Popen(
            ["git", "-C", str(repo), "archive", "--format=tar", ref],
            stdout=subprocess.PIPE,
        )
        try:
            with tarfile.open(fileobj=proc.stdout, mode="r|") as tar:
                tar.extractall(td, filter="data")
        finally:
            if proc.stdout:
                proc.stdout.close()
            proc.wait()
        if proc.returncode != 0:
            raise RuntimeError(f"git archive failed for {repo} at {ref}")
        yield Path(td)


# --------------------------------------------------------------------------
# Locating repositories
# --------------------------------------------------------------------------

def repo_url(key: str) -> str:
    return f"https://github.com/{GITHUB_USER}/{key}.git"


def find_repo(key: str, github_only: bool = False) -> Path | None:
    bases = [VENDOR] if github_only else [*REPO_SEARCH_DIRS, VENDOR]
    for base in bases:
        candidate = base / key
        if (candidate / ".git").exists():
            return candidate
    return None


def ensure_vendor() -> None:
    VENDOR.mkdir(exist_ok=True)
    gitignore = VENDOR / ".gitignore"
    if not gitignore.exists():
        gitignore.write_text(
            "# Source repositories cloned from GitHub by kloc_accounting.py.\n"
            "# These are measurement inputs, not manuscript content, and they are\n"
            "# large. Do not commit them.\n"
            "*\n"
        )


def clone_missing(repos: list[Repo], fetch: bool = False) -> None:
    """Clone any repo not found locally into vendor/ (gitignored).

    ``git clone`` here is deliberately a full clone, not ``--depth 1``: every
    active-day count comes from the commit history, so a shallow clone would
    silently report a handful of days for a repository with years of work.
    """
    ensure_vendor()
    for repo in repos:
        target = VENDOR / repo.key
        if repo.path is None:
            url = repo_url(repo.key)
            print(f"  cloning {url}")
            try:
                subprocess.run(["git", "clone", "--quiet", url, str(target)],
                               check=True)
            except (subprocess.CalledProcessError, FileNotFoundError) as exc:
                print(f"  ! clone failed for {repo.key}: {exc}", file=sys.stderr)
                continue
            repo.path = target
        elif fetch and repo.path == target:
            # Only ever fetch into our own vendor/ clones. A checkout of the
            # author's that we merely found is never written to.
            print(f"  fetching {repo.key}")
            subprocess.run(["git", "-C", str(target), "fetch", "--quiet",
                            "--all", "--tags"], check=False)


def resolve(repos: list[Repo], do_clone: bool, github_only: bool = False,
            fetch: bool = False) -> None:
    """Locate each repository, optionally cloning or refreshing from GitHub."""
    for repo in repos:
        repo.path = find_repo(repo.key, github_only)
    if do_clone or github_only or fetch:
        clone_missing(repos, fetch=fetch)


# --------------------------------------------------------------------------
# Measurement
# --------------------------------------------------------------------------

@dataclass
class RepoStats:
    repo: Repo
    total_lines: int = 0
    code_lines: int = 0
    # As measured, before the net-of-predecessor adjustment applied to TUAS.
    # Extensions must subtract the standalone original at its head, not the net.
    head_total_lines: int = 0
    head_code_lines: int = 0
    n_files: int = 0
    days: set[str] = field(default_factory=set)
    first: str = ""
    last: str = ""
    ref: str = ""
    missing: bool = False


def measure_repo(repo: Repo) -> RepoStats:
    stats = RepoStats(repo=repo)
    if repo.path is None:
        stats.missing = True
        return stats
    branch = select_ref(repo.path)
    # Lines are counted at measure_ref where one is pinned; active days are
    # always counted over the branch's whole history, because a day the author
    # committed is a day worked whether or not that commit added lines.
    stats.ref = repo.measure_ref or branch
    with materialize(repo.path, stats.ref) as tree:
        stats.total_lines, stats.code_lines, stats.n_files = count_lines(tree)
    stats.head_total_lines, stats.head_code_lines = stats.total_lines, stats.code_lines
    stats.days = active_days(repo.path, branch)
    if stats.days:
        stats.first, stats.last = min(stats.days), max(stats.days)
    return stats


def measure_baseline(repos: list[Repo]) -> tuple[list[RepoStats], dict]:
    stats = [measure_repo(r) for r in repos]
    by_key = {s.repo.key: s for s in stats}

    # TUAS net of what it inherited from thermal_hydraulics_rs at spin-out.
    # The subtrahend is the imported tree itself -- see TUAS_IMPORT_REF above.
    tuas, pred = by_key.get(TUAS_KEY), by_key.get(TUAS_PREDECESSOR_KEY)
    net = {}
    if tuas and pred and not tuas.missing and not pred.missing:
        with materialize(tuas.repo.path, TUAS_IMPORT_REF) as tree:
            imp_total, imp_code, imp_files = count_lines(tree)
        net = {
            "head_total": tuas.total_lines,
            "head_code": tuas.code_lines,
            "import_total": imp_total,
            "import_code": imp_code,
            "import_files": imp_files,
            "net_total": tuas.total_lines - imp_total,
            "net_code": tuas.code_lines - imp_code,
            # Written in the predecessor, not carried across at spin-out.
            "abandoned_code": pred.code_lines - imp_code,
        }
        tuas.total_lines = net["net_total"]
        tuas.code_lines = net["net_code"]

    present = [s for s in stats if not s.missing]
    union_days = set().union(*(s.days for s in present)) if present else set()
    totals = {
        "total_lines": sum(s.total_lines for s in present),
        "code_lines": sum(s.code_lines for s in present),
        "active_days": len(union_days),
        "union_days": union_days,
        "first": min((s.first for s in present if s.first), default=""),
        "last": max((s.last for s in present if s.last), default=""),
        "tuas_net": net,
        "missing": [s.repo.key for s in stats if s.missing],
        # The "measured <date>" stamp in the caption is the newest commit date
        # seen, not today's date -- rerunning the script months later must not
        # silently restamp a table whose inputs have not moved.
        "as_of": max((s.last for s in present if s.last), default=""),
    }
    return stats, totals


def report_peak(repos: list[Repo]) -> None:
    """For each repo, report the commit carrying the most ``.rs`` files.

    A repository that was emptied when its code moved elsewhere has a tip far
    below its peak; that gap is the signal that a ``measure_ref`` is needed.
    This walks every commit on the measured ref, so it is slow -- it is a
    diagnostic, not part of the normal run.
    """
    print(f"{'repository':<46}{'tip':>8}{'peak':>8}  peak commit")
    print("-" * 92)
    for repo in repos:
        if repo.path is None:
            print(f"{repo.key:<46}{'MISSING':>16}")
            continue
        branch = select_ref(repo.path)
        commits = git(repo.path, "log", "--format=%H %ad", "--date=short",
                      branch).splitlines()
        best_n, best = -1, ""
        for line in commits:
            sha, _, date = line.partition(" ")
            names = git(repo.path, "ls-tree", "-r", "--name-only", sha)
            n = sum(1 for f in names.splitlines() if f.endswith(".rs"))
            if n > best_n:
                best_n, best = n, f"{sha[:12]} {date}"
        tip_names = git(repo.path, "ls-tree", "-r", "--name-only", branch)
        tip_n = sum(1 for f in tip_names.splitlines() if f.endswith(".rs"))
        flag = "   <-- tip is far below peak" if tip_n * 2 < best_n else ""
        print(f"{repo.key:<46}{tip_n:>8}{best_n:>8}  {best}{flag}")


def crate_description(crate_dir: Path) -> str:
    """The `description` field a crate declares in its own Cargo.toml."""
    manifest = crate_dir / "Cargo.toml"
    if not manifest.exists():
        return ""
    text = manifest.read_text(encoding="utf-8", errors="replace")
    m = re.search(r'^\s*description\s*=\s*"(.*?)"\s*$', text, re.M | re.S)
    return " ".join(m.group(1).split()) if m else ""


@dataclass
class CrateStats:
    name: str
    display: str                 # how the table names it
    klass: str
    upstream: str | None
    total_lines: int
    code_lines: int          # after subtracting the pre-agentic original
    raw_code_lines: int      # as it stands in outram-park-backend
    baseline_code_lines: int # the standalone pre-agentic original, if any
    description: str


def measure_agentic(agentic: Repo,
                    baseline: dict[str, RepoStats]) -> tuple[list[CrateStats], dict]:
    if agentic.path is None:
        return [], {"missing": True}

    branch = select_ref(agentic.path)
    ref = agentic.measure_ref or branch

    # Materialise the whole backend once at `ref`, then count each crate out of
    # that one tree -- far cheaper than extracting per crate, and it guarantees
    # every crate is measured at the same commit.
    with materialize(agentic.path, ref) as tree:
        crates_dir = tree / "crates"
        if not crates_dir.is_dir():
            return [], {"missing": True}

        found = sorted(d.name for d in crates_dir.iterdir()
                       if (d / "Cargo.toml").exists())
        unclassified = [c for c in found if c not in CRATE_PROVENANCE]
        stale = [c for c in CRATE_PROVENANCE if c not in found]

        rows: list[CrateStats] = []
        for name in found:
            if name not in CRATE_PROVENANCE:
                continue
            klass, upstream = CRATE_PROVENANCE[name]
            crate_root = crates_dir / name

            # Split out any part of the crate classified differently, and
            # exclude it from the crate's own count so nothing is counted twice.
            sub_paths = []
            for rel, sub_klass, sub_name in CRATE_SUBPATH_PROVENANCE.get(name, []):
                sub_root = crate_root / rel
                if not sub_root.is_dir():
                    continue
                sub_paths.append(sub_root)
                s_total, s_code, _ = count_lines(sub_root)
                rows.append(CrateStats(
                    name=sub_name,
                    display=f"{sub_name} (bin in {name})",
                    klass=sub_klass, upstream=None,
                    total_lines=s_total, code_lines=s_code,
                    raw_code_lines=s_code, baseline_code_lines=0,
                    description=f"feature-gated binary inside {name}",
                ))

            total, code, _ = count_lines(crate_root, exclude=sub_paths)

            baseline_code = 0
            if klass == "extension":
                base = baseline.get(upstream)
                # Compare against the standalone repo as measured at its own
                # head, not the net-of-predecessor figure in the baseline table.
                if base is not None and not base.missing:
                    baseline_code = base.head_code_lines
            rows.append(CrateStats(
                name=name, display=name, klass=klass, upstream=upstream,
                total_lines=total,
                code_lines=max(code - baseline_code, 0),
                raw_code_lines=code,
                baseline_code_lines=baseline_code,
                description=crate_description(crates_dir / name),
            ))

    subtotals = {k: sum(r.code_lines for r in rows if r.klass == k) for k in CLASS_ORDER}
    days = active_days(agentic.path, ref, AGENTIC_SINCE, AGENTIC_UNTIL)
    summary = {
        "missing": False,
        "n_crates": len(found),
        "unclassified": unclassified,
        "stale": stale,
        "total_rust_code": sum(r.raw_code_lines for r in rows),
        "subtotals": subtotals,
        "agentic_total": sum(subtotals.values()),
        "active_days": len(days),
        "days": days,
        "head": head_date(agentic.path, ref),
        "branch": ref,
    }
    return rows, summary


# --------------------------------------------------------------------------
# Reporting
# --------------------------------------------------------------------------

def fmt(n: int) -> str:
    return f"{n:,}"


def report(base_stats, base_totals, crate_rows, agentic_summary, check: bool) -> str:
    L: list[str] = []
    def w(s: str = "") -> None:
        L.append(s)

    w("=" * 78)
    w("PRE-AGENTIC BASELINE")
    w("=" * 78)
    w(f"{'repository':<46}{'total':>10}{'code':>10}{'days':>7}")
    w("-" * 78)
    for s in base_stats:
        if s.missing:
            w(f"{s.repo.key:<46}{'MISSING -- run with --clone':>27}")
            continue
        span = f"{s.first} .. {s.last}" if s.first else "no commits"
        w(f"{s.repo.key:<46}{fmt(s.total_lines):>10}{fmt(s.code_lines):>10}"
          f"{len(s.days):>7}")
        w(f"    {span}  [{s.ref}]")
    w("-" * 78)
    w(f"{'BASELINE TOTAL':<46}{fmt(base_totals['total_lines']):>10}"
      f"{fmt(base_totals['code_lines']):>10}{base_totals['active_days']:>7}")
    w("  (days are a union of distinct dates, not a column sum)")

    net = base_totals.get("tuas_net")
    if net:
        w()
        w(f"  TUAS at head       : {fmt(net['head_total'])} total, "
          f"{fmt(net['head_code'])} code")
        w(f"  less imported      : {fmt(net['import_total'])} total, "
          f"{fmt(net['import_code'])} code "
          f"({net['import_files']} files, at {TUAS_IMPORT_REF[:7]})")
        w(f"  TUAS net-new       : {fmt(net['net_total'])} total, "
          f"{fmt(net['net_code'])} code")
        w(f"  predecessor kept {fmt(net['abandoned_code'])} code lines that were "
          f"never imported;")
        w(f"  they stay in the baseline under thermal_hydraulics_rs, and are the "
          f"reason")
        w(f"  the baseline total exceeds the pre-agentic code vendored into the "
          f"backend.")

    if base_totals["missing"]:
        w()
        w("  ! missing repositories, totals are incomplete: "
          + ", ".join(base_totals["missing"]))

    days = base_totals["active_days"]
    if days:
        w()
        w(f"  baseline rate: {base_totals['code_lines'] / days:,.0f} "
          f"code lines per active day")

    by_key = {s.repo.key: s for s in base_stats}
    w()
    w("  Split by how much non-agentic AI help each repository had:")
    for group, keys in ASSISTANCE_GROUPS.items():
        members = [by_key[k] for k in keys if k in by_key and not by_key[k].missing]
        if not members:
            continue
        code = sum(m.code_lines for m in members)
        gdays = set().union(*(m.days for m in members))
        rate = code / len(gdays) if gdays else 0
        w(f"    {group:<36}{fmt(code):>9} code, {len(gdays):>4} days,"
          f" {rate:,.0f} lines/day")
    w("    (this comparison is not clean -- the author also grew more fluent in")
    w("     Rust over the same period, and the crates differ in nature)")

    w()
    w("=" * 78)
    w("AGENTIC OUTPUT -- outram-park-backend")
    w("=" * 78)
    if agentic_summary.get("missing"):
        w("  MISSING -- run with --clone")
        return "\n".join(L)

    w(f"  branch {agentic_summary['branch']} @ {agentic_summary['head']}, "
      f"{agentic_summary['n_crates']} crates")
    w(f"  window {AGENTIC_SINCE} .. {AGENTIC_UNTIL}: "
      f"{agentic_summary['active_days']} active days")
    w()
    for klass in CLASS_ORDER:
        w(f"-- {CLASS_LABEL[klass]} " + "-" * (60 - len(CLASS_LABEL[klass])))
        for r in sorted((r for r in crate_rows if r.klass == klass),
                        key=lambda r: -r.code_lines):
            up = r.upstream or "---"
            extra = ""
            if r.klass == "extension":
                extra = f"  ({fmt(r.raw_code_lines)} - {fmt(r.baseline_code_lines)})"
            w(f"  {r.display:<46}{up:<22}{fmt(r.code_lines):>9}{extra}")
        w(f"  {'SUBTOTAL':<46}{'':<22}"
          f"{fmt(agentic_summary['subtotals'][klass]):>9}")
        w()
    w(f"  {'TOTAL AGENTIC':<46}{'':<22}{fmt(agentic_summary['agentic_total']):>9}")
    w(f"  {'total Rust code in crates/':<46}{'':<22}"
      f"{fmt(agentic_summary['total_rust_code']):>9}")

    if agentic_summary["active_days"]:
        w()
        w(f"  agentic rate: "
          f"{agentic_summary['agentic_total'] / agentic_summary['active_days']:,.0f} "
          f"code lines per active day")

    if agentic_summary["unclassified"]:
        w()
        w("  ! crates present but not classified in CRATE_PROVENANCE "
          "(excluded from all totals):")
        for c in agentic_summary["unclassified"]:
            w(f"      {c}")
    if agentic_summary["stale"]:
        w()
        w("  ! classified but not present in the checkout:")
        for c in agentic_summary["stale"]:
            w(f"      {c}")

    if check:
        w()
        w("=" * 78)
        w("COMPARISON WITH THE MANUSCRIPT (drift check)")
        w("=" * 78)
        measured = {
            "baseline_total_lines": base_totals["total_lines"],
            "baseline_code_lines": base_totals["code_lines"],
            "baseline_active_days": base_totals["active_days"],
            "agentic_code_lines": agentic_summary["agentic_total"],
            "subtotal_translated": agentic_summary["subtotals"]["translated"],
            "subtotal_original": agentic_summary["subtotals"]["original"],
            "subtotal_extension": agentic_summary["subtotals"]["extension"],
            "n_crates": agentic_summary["n_crates"],
        }
        # A missing repository does not just shrink the baseline: because
        # extensions subtract a vendored crate's pre-agentic original, a missing
        # baseline repo silently MOVES those lines into the agentic total. The
        # deltas below would then look like real drift when they are an
        # artefact of an incomplete checkout, so say so before printing them.
        if base_totals["missing"]:
            w("  !! THIS COMPARISON IS NOT VALID.")
            w("  !! Missing: " + ", ".join(base_totals["missing"]))
            w("  !! Their lines are absent from the baseline and are instead")
            w("  !! counted as agentic extensions, which inflates the agentic")
            w("  !! total by exactly the amount the baseline loses.")
            w("  !! Re-run with --from-github to fetch every repository first.")
            w()

        w(f"{'quantity':<28}{'manuscript':>12}{'measured':>12}{'delta':>12}")
        w("-" * 78)
        for key, got in measured.items():
            want = MANUSCRIPT.get(key)
            if want is None:
                continue
            delta = got - want
            flag = "" if delta == 0 else "   <-- differs"
            w(f"{key:<28}{fmt(want):>12}{fmt(got):>12}{delta:>+12,}{flag}")
        w()
        if base_totals["missing"]:
            w("  Deltas above are meaningless until the missing repositories")
            w("  listed at the top of this section are present.")
        else:
            w("  A non-zero delta is not automatically an error: the repositories")
            w("  keep moving. It means the manuscript figure needs re-stating from")
            w("  this run, or the run needs pinning to the commit the table used.")

    return "\n".join(L)


def write_csvs(base_stats, crate_rows) -> None:
    with (HERE / "baseline_repositories.csv").open("w", newline="") as fh:
        wr = csv.writer(fh)
        wr.writerow(["repository", "ref", "first_commit", "last_commit",
                     "total_lines", "code_lines", "active_days",
                     "rust_files", "ai_assistance"])
        for s in base_stats:
            if s.missing:
                wr.writerow([s.repo.key, "", "", "", "", "", "", "", "MISSING"])
                continue
            wr.writerow([s.repo.key, s.ref, s.first, s.last, s.total_lines,
                         s.code_lines, len(s.days), s.n_files, s.repo.note])

    with (HERE / "agentic_crates.csv").open("w", newline="") as fh:
        wr = csv.writer(fh)
        wr.writerow(["crate", "class", "upstream", "total_lines",
                     "code_lines_in_backend", "preagentic_baseline_code_lines",
                     "agentic_code_lines", "cargo_toml_description"])
        for r in crate_rows:
            wr.writerow([r.display, r.klass, r.upstream or "", r.total_lines,
                         r.raw_code_lines, r.baseline_code_lines,
                         r.code_lines, r.description])


# --------------------------------------------------------------------------
# LaTeX tables
# --------------------------------------------------------------------------

# Crates whose names need something other than a plain \texttt{} rendering.
CRATE_TEX_NAME = {
    "chem-eng-real-time-process-control-simulator":
        r"\texttt{chem-eng-\ldots-simulator}",
    "nee_soon": r"\texttt{nee\_soon}$^{\dagger}$",
}

MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun",
          "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]

GENERATED_BY = ("% Generated by kloc_accounting.py -- do not edit by hand.\n"
                "% Re-run the script to update; edits here will be overwritten.\n")


def tex_num(n: int) -> str:
    r"""Format an integer with LaTeX thin-space thousands separators."""
    return f"{n:,}".replace(",", r"\,")


def tex_name(crate: str) -> str:
    if crate in CRATE_TEX_NAME:
        return CRATE_TEX_NAME[crate]
    return r"\texttt{" + crate.replace("_", r"\_") + "}"


def tex_month(date: str) -> str:
    """2023-06-27 -> Jun 2023."""
    if not date:
        return "---"
    year, month, _ = date.split("-")
    return f"{MONTHS[int(month) - 1]} {year}"


def baseline_table_tex(base_stats, base_totals) -> str:
    net = base_totals.get("tuas_net") or {}
    head_total = tex_num(net.get("head_total", 0))
    head_code = tex_num(net.get("head_code", 0))

    rows = []
    for s in base_stats:
        if s.missing:
            continue
        marker = f"$^{{{s.repo.marker}}}$" if s.repo.marker else ""
        cite = r" \cite{" + s.repo.cite + "}" if s.repo.cite else ""
        rows.append(
            f"{s.repo.label}{marker}{cite} & "
            f"{tex_month(s.first)} -- {tex_month(s.last)} & "
            f"{tex_num(s.total_lines)} & {tex_num(s.code_lines)} & "
            f"{len(s.days)} \\\\"
        )

    notes = "\n".join(f"$^{{{m}}}$~{text}" for m, text in BASELINE_FOOTNOTES)

    return GENERATED_BY + r"""\begin{table}[H]
\centering
\caption{Pre-agentic repositories forming the productivity baseline, measured
""" + base_totals.get("as_of", "") + r""". Code lines exclude blank and
comment-only lines; total lines include both, so Rust doc comments, which carry
executable doctests, appear only in the total. An active day is any calendar
date carrying at least one commit; the total is a union of distinct dates
rather than a column sum, because these projects overlapped in time. TUAS is
reported net of the code it inherited from \texttt{thermal\_hydraulics\_rs} at
spin-out --- specifically net of the tree TUAS imported wholesale at its second
commit, \texttt{c451c8e}, rather than net of the predecessor's own full extent,
so that what is removed is what actually came across. At its head TUAS stands at
""" + head_total + " total and " + head_code + r""" code lines, the 96~KLOC
quoted in the text. All figures are measured on each repository's
\texttt{develop} branch, which is where the development history lives ---
these repositories squash-merge into \texttt{main}, so measuring \texttt{main}
would collapse years of work into a handful of commits.
\texttt{thermal\_hydraulics\_rs} is measured at commit \texttt{4d534af}, the
last before its code was moved into TUAS on 2024-10-11 and the repository
emptied. None of these repositories used an agentic coding workflow.
\emph{Every figure in this table is produced by \texttt{kloc\_accounting.py},
which walks the repositories and their git histories directly; see
Section~\ref{sec:reproducing_the_counts}}}
\label{tab:preagentic_baseline}
\footnotesize
\setlength{\tabcolsep}{5pt}
\begin{tabular}{@{}
  >{\raggedright\arraybackslash}p{0.33\linewidth}
  >{\raggedright\arraybackslash}p{0.19\linewidth}
  r r r@{}}
\toprule
\textbf{Repository} & \textbf{Period} & \textbf{Total} & \textbf{Code} &
\textbf{Active} \\
 & & \textbf{lines} & \textbf{lines} & \textbf{days} \\
\midrule
""" + "\n".join(rows) + r"""
\midrule
\textbf{Baseline total} & \textbf{""" + \
        tex_month(base_totals["first"]).split()[-1] + " -- " + \
        tex_month(base_totals["last"]).split()[-1] + r"""} & \textbf{""" + \
        tex_num(base_totals["total_lines"]) + r"""} & \textbf{""" + \
        tex_num(base_totals["code_lines"]) + r"""} & \textbf{""" + \
        str(base_totals["active_days"]) + r"""} \\
\bottomrule
\end{tabular}

\vspace{3pt}
{\footnotesize
\begin{minipage}{\linewidth}
""" + notes + r"""
\end{minipage}}
\end{table}
"""


def rate_table_tex(base_stats, base_totals, agentic_summary) -> str:
    """The per-repository rate table, sorted fastest first."""
    rows = []
    for s in sorted((s for s in base_stats if not s.missing and s.days),
                    key=lambda s: -s.code_lines / len(s.days)):
        rate = s.code_lines / len(s.days)
        rows.append(f"{tex_num(round(rate))} & {s.repo.label} & "
                    f"{s.repo.note} \\\\")

    extra = []
    if base_totals["active_days"]:
        agg = round(base_totals["code_lines"] / base_totals["active_days"])
        extra.append(
            r"\midrule" "\n"
            r"\textbf{" + tex_num(agg)
            + r"} & \textbf{Pre-agentic baseline, all repositories} & "
              r"\textbf{mixed} \\")
    if agentic_summary.get("active_days"):
        rate = round(agentic_summary["agentic_total"]
                     / agentic_summary["active_days"])
        extra.append(
            r"\textbf{" + tex_num(rate) +
            r"} & \textbf{\texttt{outram-park-backend}} & "
            r"\textbf{agentic} \\")

    return GENERATED_BY + r"""\begin{table}[H]
\centering
\caption{Code lines per active day, each pre-agentic repository ranked fastest
first, against the agentic month. The spread among the pre-agentic
repositories --- roughly a factor of three, from 206 to 572 --- is the useful
context for the agentic figure: it is the ordinary variation between projects
of differing nature, and it is far smaller than the gap to the agentic rate.
The \emph{AI?} column records only non-agentic assistance; none of the
pre-agentic repositories used an agentic workflow.
\emph{Produced by \texttt{kloc\_accounting.py}; see
Section~\ref{sec:reproducing_the_counts}}}
\label{tab:baseline_rates}
\footnotesize
\begin{tabular}{@{}r
  >{\raggedright\arraybackslash}p{0.42\linewidth}
  >{\raggedright\arraybackslash}p{0.26\linewidth}@{}}
\toprule
\textbf{Code lines} & \textbf{Repository} & \textbf{Non-agentic} \\
\textbf{per active day} & & \textbf{AI assistance} \\
\midrule
""" + "\n".join(rows) + "\n" + "\n".join(extra) + r"""
\bottomrule
\end{tabular}
\end{table}
"""


def agentic_table_tex(crate_rows, agentic_summary) -> str:
    section_titles = {
        "translated": "Translated or ported from an existing codebase",
        "original": "Originally written",
        "extension": "Agentic extensions to vendored pre-agentic crates",
    }
    subtotal_labels = {
        "translated": r"\textbf{Subtotal, translated}",
        "original": r"\textbf{Subtotal, original}",
        "extension": r"\textbf{Subtotal, extensions}",
    }

    body = []
    for klass in CLASS_ORDER:
        members = sorted((r for r in crate_rows if r.klass == klass),
                         key=lambda r: -r.code_lines)
        if not members:
            continue
        if body:
            body.append(r"\midrule")
        body.append(r"\multicolumn{3}{@{}l}{\textit{" + section_titles[klass]
                    + r"}} \\")
        for r in members:
            upstream = (r.upstream if klass == "translated"
                        else "---" if klass == "original"
                        else "(own, pre-agentic)")
            body.append(f"{tex_name(r.display)} & {upstream} & "
                        f"{tex_num(r.code_lines)} \\\\")
        body.append(r"\cmidrule(r){1-3}")
        body.append(subtotal_labels[klass] + " & & \\textbf{"
                    + tex_num(agentic_summary["subtotals"][klass]) + "} \\\\")

    return GENERATED_BY + r"""\begin{table}[H]
\centering
\caption{Agentic lines of code in \texttt{outram-park-backend} by crate,
""" + AGENTIC_SINCE + " -- " + AGENTIC_UNTIL + r""". Code lines exclude blank
and comment-only lines. Translated crates are pure-Rust forks or ports of the
named upstream project and are independent works, not the official software.
Extensions are the excess of a vendored crate over its standalone pre-agentic
original, and are credited here rather than to the baseline.
\emph{Every figure in this table is produced by \texttt{kloc\_accounting.py}
from the repository pinned at commit \texttt{""" + (AGENTIC_MEASURE_REF or "HEAD")[:7] + r"""}, because
\texttt{develop} moves daily and a tip measurement could not be reproduced
later; the classification of each
crate is the one editorial input, and is carried in the script beside each
crate's own \texttt{Cargo.toml} description so it can be audited. See
Section~\ref{sec:reproducing_the_counts}.}}
\label{tab:agentic_crates}
\footnotesize
\setlength{\tabcolsep}{5pt}
\begin{tabular}{@{}
  >{\raggedright\arraybackslash}p{0.42\linewidth}
  >{\raggedright\arraybackslash}p{0.22\linewidth}
  r@{}}
\toprule
\textbf{Crate} & \textbf{Upstream} & \textbf{Code lines} \\
\midrule
""" + "\n".join(body) + r"""
\midrule
\textbf{Total agentic} & & \textbf{""" + \
        tex_num(agentic_summary["agentic_total"]) + r"""} \\
\bottomrule
\end{tabular}

\vspace{3pt}
{\footnotesize
\begin{minipage}{\linewidth}
$^{\dagger}$~\texttt{nee\_soon} succeeds \texttt{teh-o-prke} in naming --- the
former Teh-O package --- but its code is newly written; the point kinetics
module itself remains vendored as \texttt{teh-o-prke}.
\end{minipage}}
\end{table}
"""


def write_latex_tables(base_stats, base_totals, crate_rows,
                       agentic_summary) -> list[Path]:
    written = []
    targets = [
        ("baseline_table.tex", baseline_table_tex(base_stats, base_totals)),
        ("rate_table.tex", rate_table_tex(base_stats, base_totals,
                                          agentic_summary)),
    ]
    if not agentic_summary.get("missing"):
        targets.append(("agentic_table.tex",
                        agentic_table_tex(crate_rows, agentic_summary)))
    for name, text in targets:
        path = HERE / name
        path.write_text(text)
        written.append(path)
    return written


# --------------------------------------------------------------------------
# Figure
# --------------------------------------------------------------------------

# Categorical slots 1-3 of the validated reference palette (light mode).
C_BLUE, C_ORANGE, C_AQUA = "#2a78d6", "#eb6834", "#1baf7a"
INK, INK_SOFT, GRID = "#0b0b0b", "#52514e", "#d8d7d2"


def make_figure(base_stats, base_totals, agentic_summary, out: Path) -> None:
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    from matplotlib.patches import Patch

    plt.rcParams.update({
        "font.family": "serif",
        "font.serif": ["Nimbus Roman", "Times New Roman", "DejaVu Serif"],
        "font.size": 9,
        "axes.edgecolor": INK_SOFT,
        "axes.labelcolor": INK,
        "text.color": INK,
        "xtick.color": INK_SOFT,
        "ytick.color": INK_SOFT,
        "figure.dpi": 200,
    })

    fig, (ax1, ax2) = plt.subplots(
        2, 1, figsize=(6.6, 6.4),
        gridspec_kw={"height_ratios": [2.9, 1.1], "hspace": 0.46},
    )

    # -- (a) code lines per active day -----------------------------------
    rows = []
    for s in base_stats:
        if s.missing or not s.days:
            continue
        rows.append((s.repo.key, s.code_lines / len(s.days), "baseline"))
    rows.sort(key=lambda r: r[1])

    if base_totals["active_days"]:
        rows.append(("Baseline, all repositories",
                     base_totals["code_lines"] / base_totals["active_days"],
                     "aggregate"))
    if agentic_summary.get("active_days"):
        rows.append(("Agentic (outram-park-backend)",
                     agentic_summary["agentic_total"] / agentic_summary["active_days"],
                     "agentic"))

    colors = {"baseline": C_BLUE, "aggregate": C_BLUE, "agentic": C_ORANGE}
    y = range(len(rows))
    bars = ax1.barh(list(y), [r[1] for r in rows],
                    color=[colors[r[2]] for r in rows],
                    height=0.62, zorder=3)
    # The aggregate row is the same entity as the bars above it; hatch marks it
    # apart without spending a hue on it.
    for bar, row in zip(bars, rows):
        if row[2] == "aggregate":
            bar.set_hatch("///")
            bar.set_edgecolor("white")
            bar.set_linewidth(0.0)

    # Rule between the individual repositories and the two summary rows above
    # them, so the aggregate is not misread as just another repository.
    n_repos = sum(1 for r in rows if r[2] == "baseline")
    if n_repos and n_repos < len(rows):
        ax1.axhline(n_repos - 0.5, color=GRID, linewidth=0.9, zorder=2)

    ax1.set_yticks(list(y))
    ax1.set_yticklabels([r[0] for r in rows], fontsize=8)
    ax1.set_xlabel("Code lines per active day")
    ax1.set_title("(a)  Rate of code production, pre-agentic vs agentic",
                  loc="left", fontsize=10, pad=8)

    span = max((r[1] for r in rows), default=1)
    for bar, row in zip(bars, rows):
        ax1.text(bar.get_width() + span * 0.012,
                 bar.get_y() + bar.get_height() / 2,
                 f"{row[1]:,.0f}", va="center", ha="left",
                 fontsize=8, color=INK)
    ax1.set_xlim(0, span * 1.16)

    ax1.legend(handles=[
        Patch(facecolor=C_BLUE, label="Pre-agentic repository"),
        Patch(facecolor=C_BLUE, hatch="///", edgecolor="white",
              label="Pre-agentic, all repositories"),
        Patch(facecolor=C_ORANGE, label="Agentic"),
    ], loc="lower right", frameon=False, fontsize=8)

    # -- (b) what the agentic output is made of --------------------------
    subtotals = agentic_summary.get("subtotals", {})
    total = sum(subtotals.values())
    if total:
        seg_colors = {"translated": C_BLUE, "original": C_ORANGE,
                      "extension": C_AQUA}
        # One bar per class rather than a single stacked bar: the two small
        # classes are 15% and 7%, far too narrow to hold a legible inline
        # label, and leader lines out of adjacent thin segments cross each
        # other. Separate bars carry the same split with no collision risk,
        # and the share is stated on each label.
        klasses = [k for k in CLASS_ORDER if subtotals.get(k, 0)]
        klasses.reverse()                      # largest at the top
        values = [subtotals[k] for k in klasses]
        yb = range(len(klasses))
        ax2.barh(list(yb), values, height=0.58, zorder=3,
                 color=[seg_colors[k] for k in klasses])
        ax2.set_yticks(list(yb))
        ax2.set_yticklabels([CLASS_LABEL[k] for k in klasses], fontsize=8)
        for i, (k, v) in enumerate(zip(klasses, values)):
            ax2.text(v + total * 0.012, i, f"{v:,.0f}  ({v / total * 100:.0f}%)",
                     va="center", ha="left", fontsize=8, color=INK)
        ax2.set_xlim(0, total * 1.16)
        ax2.set_xlabel("Agentic code lines")
        ax2.set_title(f"(b)  Composition of the {total:,.0f} agentic code lines",
                      loc="left", fontsize=10, pad=8)

    for ax in (ax1, ax2):
        ax.spines["top"].set_visible(False)
        ax.spines["right"].set_visible(False)
        ax.spines["left"].set_visible(False)
        ax.tick_params(length=0)
    ax1.xaxis.grid(True, color=GRID, linewidth=0.6, zorder=0)
    ax1.set_axisbelow(True)
    ax2.xaxis.grid(False)

    fig.savefig(out, bbox_inches="tight", facecolor="white")
    plt.close(fig)
    print(f"  wrote {out}")


# --------------------------------------------------------------------------

def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--clone", action="store_true",
                    help="git clone any repository not found locally into vendor/")
    ap.add_argument("--from-github", action="store_true",
                    help="ignore local checkouts entirely and measure clones of "
                         "the GitHub repositories in vendor/. This is the "
                         "reproduction path: it needs nothing on the machine "
                         "but git and network access")
    ap.add_argument("--fetch", action="store_true",
                    help="git fetch the vendor/ clones before measuring, so a "
                         "previous reproduction run is brought up to date")
    ap.add_argument("--check", action="store_true",
                    help="compare the measurements against the manuscript's figures")
    ap.add_argument("--no-figure", action="store_true", help="skip the PNG")
    ap.add_argument("--peak", action="store_true",
                    help="diagnostic: report each repo's maximum-extent commit, "
                         "which is how a pinned measure_ref is verified (slow)")
    args = ap.parse_args()

    all_repos = [*BASELINE_REPOS, AGENTIC_REPO]
    resolve(all_repos, args.clone, github_only=args.from_github, fetch=args.fetch)
    if args.from_github:
        print(f"Measuring GitHub clones under {VENDOR} "
              f"(github.com/{GITHUB_USER}).\n")

    if args.peak:
        report_peak(all_repos)
        return 0

    missing = [r.key for r in all_repos if r.path is None]
    if missing:
        print("Repositories not found locally: " + ", ".join(missing))
        if not args.clone:
            print("Re-run with --clone to fetch them into vendor/ "
                  "(gitignored), then measure.\n")

    print("Measuring. This walks every .rs file in each checkout and may take "
          "a minute.\n")
    base_stats, base_totals = measure_baseline(BASELINE_REPOS)
    by_key = {s.repo.key: s for s in base_stats}
    crate_rows, agentic_summary = measure_agentic(AGENTIC_REPO, by_key)

    text = report(base_stats, base_totals, crate_rows, agentic_summary, args.check)
    print(text)
    (HERE / "summary.txt").write_text(text + "\n")

    write_csvs(base_stats, crate_rows)
    print(f"\n  wrote {HERE / 'baseline_repositories.csv'}")
    print(f"  wrote {HERE / 'agentic_crates.csv'}")
    print(f"  wrote {HERE / 'summary.txt'}")

    for path in write_latex_tables(base_stats, base_totals, crate_rows,
                                   agentic_summary):
        print(f"  wrote {path}")

    if not args.no_figure and not agentic_summary.get("missing"):
        make_figure(base_stats, base_totals, agentic_summary,
                    HERE / "fig_kloc_productivity.png")

    return 0


if __name__ == "__main__":
    sys.exit(main())
