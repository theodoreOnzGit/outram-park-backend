#!/usr/bin/env python3
"""OUTRAM PARK "historian" — pre-merge-to-main release report generator.

WHAT THIS PRODUCES
------------------
A single markdown file summarising a window of `develop` history before it is
merged into `main`: the **API tokens spent** and the **lines / KLOC written**
across every commit in a `DDMMYY..DDMMYY` date range, plus a per-crate breakdown
and a per-commit table.

* **Tokens** come from the `API-Usage-Since-Last-Commit:` trailers that
  `scripts/token_accounting.py` stamps into each commit message (nothing is
  estimated — commits made before the token hooks existed, or outside a Claude
  session, simply contribute 0 and are counted as "no token data").
* **Lines** come from `git log --numstat --no-merges` over the range (added /
  removed / net, with a Rust-only `.rs` figure). Merge commits are excluded so
  lines are not double-counted.

WHERE IT LIVES
--------------
Both this script and the reports it writes live under `docs/historian/` at the
workspace root. Reports are named `historian_<from>_to_<to>.md` (DDMMYY).

STANDING RULE (see workspace CLAUDE.md)
---------------------------------------
Before merging `develop` into `main`, generate the report for the window being
released and commit it alongside the merge.

USAGE
-----
    # explicit window (DDMMYY = day-month-year, 2-digit year)
    python3 docs/historian/historian.py --from 220726 --to 230726

    # default: everything on develop that is not yet on main, up to today
    python3 docs/historian/historian.py

    # a different branch / target
    python3 docs/historian/historian.py --branch develop --base main

Stdlib only.
"""
from __future__ import annotations

import argparse
import datetime as _dt
import os
import re
import subprocess
import sys

TRAILER_RE = re.compile(r"^API-Usage-Since-Last-Commit:\s*(.+)$", re.MULTILINE)
COMPONENTS = ("in", "out", "cache_read", "cache_write")
HERE = os.path.dirname(os.path.abspath(__file__))


def git(*args: str) -> str:
    try:
        return subprocess.check_output(["git", *args], stderr=subprocess.DEVNULL, text=True)
    except Exception:
        return ""


def repo_root() -> str:
    return (git("rev-parse", "--show-toplevel").strip()) or os.getcwd()


def parse_ddmmyy(s: str) -> _dt.date:
    s = s.strip()
    if not re.fullmatch(r"\d{6}", s):
        raise ValueError(f"expected DDMMYY (6 digits), got {s!r}")
    dd, mm, yy = int(s[0:2]), int(s[2:4]), int(s[4:6])
    return _dt.date(2000 + yy, mm, dd)


def ref_for_branch(branch: str) -> str:
    """Prefer the remote-tracking ref so the report reflects what is pushed."""
    for cand in (f"origin/{branch}", branch):
        if git("rev-parse", "--verify", "--quiet", cand).strip():
            return cand
    return branch


def commits_in_range(branch_ref: str, base_ref: str | None,
                     dfrom: _dt.date | None, dto: _dt.date | None) -> list[str]:
    args = ["log", "--no-merges", "--reverse", "--format=%H"]
    if dfrom:
        args.append(f"--since={dfrom.isoformat()} 00:00:00")
    if dto:
        args.append(f"--until={dto.isoformat()} 23:59:59")
    if base_ref:
        args.append(f"{base_ref}..{branch_ref}")
    else:
        args.append(branch_ref)
    out = git(*args).strip()
    return [h for h in out.splitlines() if h]


def commit_meta(sha: str) -> dict:
    FS = "\x1f"
    raw = git("show", "-s", f"--format=%h{FS}%aI{FS}%s{FS}%b", sha)
    parts = raw.split(FS)
    short = parts[0] if len(parts) > 0 else sha[:9]
    iso = parts[1] if len(parts) > 1 else ""
    subject = parts[2] if len(parts) > 2 else ""
    body = FS.join(parts[3:]) if len(parts) > 3 else ""
    toks = {c: 0 for c in COMPONENTS}
    toks["total"] = 0
    toks["has_data"] = False
    m = TRAILER_RE.search(body)
    if m:
        fields = {}
        for tok in m.group(1).split():
            if "=" in tok:
                k, v = tok.split("=", 1)
                fields[k] = v
        if fields.get("source", "none") != "none":
            toks["has_data"] = True
        for c in COMPONENTS:
            try:
                toks[c] = int(fields.get(c, 0))
            except ValueError:
                toks[c] = 0
        try:
            toks["total"] = int(fields.get("total", 0))
        except ValueError:
            toks["total"] = sum(toks[c] for c in COMPONENTS)
    return {"sha": short, "date": iso[:10], "subject": subject, "tok": toks}


def numstat(sha: str) -> tuple[int, int, int, int, dict]:
    """(added, removed, rs_added, rs_removed, per_crate_added) for one commit."""
    raw = git("show", "--numstat", "--format=", sha)
    added = removed = rs_added = rs_removed = 0
    per_crate: dict[str, int] = {}
    for line in raw.splitlines():
        line = line.strip()
        if not line:
            continue
        cols = line.split("\t")
        if len(cols) < 3:
            continue
        a, r, path = cols[0], cols[1], cols[2]
        if a == "-" or r == "-":  # binary
            continue
        # Normalise git's rename compaction to the NEW path:
        #   "crates/{old => new}/f.rs"  -> "crates/new/f.rs"
        #   "old/path/f => new/path/f"   -> "new/path/f"
        path = re.sub(r"\{[^{}]*=>\s*([^{}]*)\}", r"\1", path)
        if " => " in path:
            path = path.split(" => ", 1)[1]
        path = re.sub(r"//+", "/", path).strip()
        a_i, r_i = int(a), int(r)
        added += a_i
        removed += r_i
        if path.endswith(".rs"):
            rs_added += a_i
            rs_removed += r_i
        m = re.match(r"crates/([^/]+)/", path)
        if m:
            per_crate[m.group(1)] = per_crate.get(m.group(1), 0) + a_i
    return added, removed, rs_added, rs_removed, per_crate


def g(n: int) -> str:
    return f"{n:,}"


def render(dfrom, dto, branch_ref, base_ref, commits) -> str:
    rows = []
    tot = {c: 0 for c in COMPONENTS}
    tot["total"] = 0
    tot_added = tot_removed = tot_rs_added = tot_rs_removed = 0
    tok_covered = 0
    per_crate_total: dict[str, int] = {}
    for sha in commits:
        meta = commit_meta(sha)
        added, removed, rs_a, rs_r, pc = numstat(sha)
        for c in COMPONENTS:
            tot[c] += meta["tok"][c]
        tot["total"] += meta["tok"]["total"]
        if meta["tok"]["has_data"]:
            tok_covered += 1
        tot_added += added
        tot_removed += removed
        tot_rs_added += rs_a
        tot_rs_removed += rs_r
        for k, v in pc.items():
            per_crate_total[k] = per_crate_total.get(k, 0) + v
        rows.append((meta, added, removed))

    n = len(commits)
    span = ""
    if dfrom and dto:
        span = f"{dfrom.strftime('%d %b %Y')} → {dto.strftime('%d %b %Y')}"
    elif base_ref:
        span = f"all of `{branch_ref}` not in `{base_ref}`"
    else:
        span = f"full history of `{branch_ref}`"
    stamp = _dt.datetime.now().strftime("%Y-%m-%d %H:%M")

    out = []
    out.append(f"# OUTRAM PARK — historian report ({span})\n")
    out.append(
        "> Pre-merge-to-`main` accounting of the API tokens spent and the lines / "
        "KLOC written across this window of `develop` history. **Auto-generated** "
        f"by `docs/historian/historian.py` on {stamp}; regenerate with "
        "`python3 docs/historian/historian.py --from DDMMYY --to DDMMYY`.\n"
    )
    out.append("## Scope\n")
    out.append(f"- **Branch:** `{branch_ref}`" + (f" (vs base `{base_ref}`)" if base_ref else "") + "\n")
    out.append(f"- **Window:** {span}\n")
    out.append(f"- **Commits (non-merge):** {n}\n")
    out.append(
        f"- **Token coverage:** {tok_covered}/{n} commits carry an "
        "`API-Usage-Since-Last-Commit` trailer. Commits before the token-accounting "
        "hooks existed (or made outside a Claude session) contribute 0 and are "
        "counted here as *no token data* — that is correct, not missing data.\n"
    )
    out.append("\n## Totals\n")
    out.append("### Lines written (git numstat, merges excluded)\n")
    out.append("| Metric | Lines | KLOC |")
    out.append("|---|--:|--:|")
    out.append(f"| Added (all files) | {g(tot_added)} | {tot_added/1000:.1f} |")
    out.append(f"| Removed (all files) | {g(tot_removed)} | {tot_removed/1000:.1f} |")
    out.append(f"| **Net (all files)** | **{g(tot_added - tot_removed)}** | **{(tot_added - tot_removed)/1000:.1f}** |")
    out.append(f"| Added (Rust `.rs`) | {g(tot_rs_added)} | {tot_rs_added/1000:.1f} |")
    out.append(f"| Net (Rust `.rs`) | {g(tot_rs_added - tot_rs_removed)} | {(tot_rs_added - tot_rs_removed)/1000:.1f} |")
    out.append("\n### API tokens spent\n")
    out.append("| Component | Tokens |")
    out.append("|---|--:|")
    out.append(f"| input | {g(tot['in'])} |")
    out.append(f"| output | {g(tot['out'])} |")
    out.append(f"| cache_read | {g(tot['cache_read'])} |")
    out.append(f"| cache_write | {g(tot['cache_write'])} |")
    out.append(f"| **total** | **{g(tot['total'])}** |")
    out.append(
        "\n_`total` = input + output + cache_read + cache_write. Cache-read "
        "(prompt-cache re-reads of the growing context) usually dominates; the "
        "output figure is the closest proxy for net generated content._\n"
    )

    if per_crate_total:
        out.append("## Lines added, by crate (top 20)\n")
        out.append("| Crate | Lines added |")
        out.append("|---|--:|")
        for k, v in sorted(per_crate_total.items(), key=lambda kv: -kv[1])[:20]:
            out.append(f"| `{k}` | {g(v)} |")
        out.append("")

    out.append("## Per-commit ledger\n")
    out.append("| Date | Commit | Subject | +lines | -lines | Tokens |")
    out.append("|---|---|---|--:|--:|--:|")
    for (meta, added, removed) in rows:
        subj = meta["subject"].replace("|", "\\|")
        if len(subj) > 64:
            subj = subj[:61] + "..."
        tk = g(meta["tok"]["total"]) if meta["tok"]["has_data"] else "—"
        out.append(f"| {meta['date']} | `{meta['sha']}` | {subj} | {g(added)} | {g(removed)} | {tk} |")
    out.append("")
    return "\n".join(out)


def main() -> int:
    ap = argparse.ArgumentParser(description="OUTRAM PARK historian — pre-merge-to-main report")
    ap.add_argument("--from", dest="dfrom", help="window start DDMMYY (day-month-year, 2-digit year)")
    ap.add_argument("--to", dest="dto", help="window end DDMMYY (default: today)")
    ap.add_argument("--branch", default="develop", help="branch to report on (default: develop)")
    ap.add_argument("--base", default="main",
                    help="base branch for the default 'not yet in base' window (default: main)")
    ap.add_argument("--outfile", help="explicit output path (default: docs/historian/historian_<from>_to_<to>.md)")
    args = ap.parse_args()

    branch_ref = ref_for_branch(args.branch)

    dfrom = parse_ddmmyy(args.dfrom) if args.dfrom else None
    dto = parse_ddmmyy(args.dto) if args.dto else (_dt.date.today() if args.dfrom else None)
    base_ref = None
    if not args.dfrom:
        # Default window: commits on <branch> not yet on <base>.
        base_ref = ref_for_branch(args.base)

    commits = commits_in_range(branch_ref, base_ref, dfrom, dto)
    md = render(dfrom, dto, branch_ref, base_ref, commits)

    if args.outfile:
        outpath = args.outfile
    else:
        if dfrom and dto:
            tag = f"{dfrom.strftime('%d%m%y')}_to_{dto.strftime('%d%m%y')}"
        else:
            tag = f"since_{args.base}_to_{_dt.date.today().strftime('%d%m%y')}"
        outpath = os.path.join(HERE, f"historian_{tag}.md")

    os.makedirs(os.path.dirname(outpath), exist_ok=True)
    with open(outpath, "w", encoding="utf-8") as fh:
        fh.write(md)
    rel = os.path.relpath(outpath, repo_root())
    print(f"wrote {rel}  ({len(commits)} commits)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
