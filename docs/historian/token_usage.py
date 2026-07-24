#!/usr/bin/env python3
"""OUTRAM PARK token usage — the single source for per-commit API-token accounting.

This one script does BOTH sides of token accounting:

WRITE / APPEND (used by the git hooks)
--------------------------------------
* ``trailer <msgfile>`` (``prepare-commit-msg``) — read the Claude Code session
  transcripts, compute the token delta since the previous commit, and append an
  ``API-Usage-Since-Last-Commit:`` trailer (+ an ``API-Usage-Session-Cumulative:``
  line) to the commit message. Idempotent (amend/rebase safe).
* ``record`` (``post-commit``) — advance the baseline and regenerate the rolling
  ledger ``docs/token-usage.md``.
* ``report`` — regenerate ``docs/token-usage.md`` from the commit trailers.
* ``init`` — set the baseline to the current cumulative (installer).
* ``show`` — print the live cumulative from the transcripts.

QUERY (recorded token usage, any time period)
---------------------------------------------
* ``query [--from DDMMYY] [--to DDMMYY] [--branch develop] [--per-commit] [--json]``
  — sum the token usage RECORDED IN THE GIT COMMIT TRAILERS over a time period.
  This reads the durable git record (the trailers), not the live transcript, so
  it works for any window of history on any branch.

HONESTY
-------
Numbers come straight from the session transcripts (write side) and the commit
trailers they produced (query side) — nothing is estimated. Commits made before
the hooks existed, or outside a Claude session, carry no trailer / show
``source=none`` and contribute 0; that is correct, not missing data.
``total`` = ``in`` + ``out`` + ``cache_read`` + ``cache_write``; cache-read
usually dominates and is shown separately.

Stdlib only; the hook path never exits non-zero (a failure degrades to a
zero/"source=none" trailer rather than blocking a commit).
"""
from __future__ import annotations

import argparse
import datetime as _dt
import json
import os
import re
import subprocess
import sys
from glob import glob

TRAILER_KEY = "API-Usage-Since-Last-Commit"
CUMULATIVE_KEY = "API-Usage-Session-Cumulative"
LEDGER_REL = "docs/token-usage.md"
COMPONENTS = ("input", "output", "cache_write", "cache_read")
# Field names as they appear in the trailer text:
TRAILER_FIELDS = {"input": "in", "output": "out", "cache_read": "cache_read", "cache_write": "cache_write"}
TRAILER_RE = re.compile(rf"^{re.escape(TRAILER_KEY)}:\s*(.+)$", re.MULTILINE)


# --------------------------------------------------------------------------- #
# git helpers
# --------------------------------------------------------------------------- #
def git(*args: str) -> str:
    try:
        return subprocess.check_output(["git", *args], stderr=subprocess.DEVNULL, text=True)
    except Exception:
        return ""


def repo_root() -> str:
    return git("rev-parse", "--show-toplevel").strip() or os.getcwd()


def git_dir() -> str:
    d = (git("rev-parse", "--git-dir").strip()) or ".git"
    return d if os.path.isabs(d) else os.path.join(repo_root(), d)


def baseline_path() -> str:
    return os.path.join(git_dir(), "claude-token-baseline.json")


def ref_for_branch(branch: str) -> str:
    for cand in (f"origin/{branch}", branch):
        if git("rev-parse", "--verify", "--quiet", cand).strip():
            return cand
    return branch


# --------------------------------------------------------------------------- #
# transcript reading (write side)
# --------------------------------------------------------------------------- #
def project_transcript_dir() -> str | None:
    base = os.path.expanduser("~/.claude/projects")
    if not os.path.isdir(base):
        return None
    root = os.environ.get("CLAUDE_PROJECT_DIR") or repo_root()
    slug = re.sub(r"[^A-Za-z0-9]+", "-", os.path.abspath(root))
    cand = os.path.join(base, slug)
    if os.path.isdir(cand):
        return cand
    bn = os.path.basename(os.path.abspath(root))
    matches = [d for d in glob(os.path.join(base, "*")) if bn and bn in os.path.basename(d)]
    return matches[0] if len(matches) == 1 else None


def read_cumulative() -> dict:
    tot = {c: 0 for c in COMPONENTS}
    tot["records"] = 0
    tot["source"] = "none"
    pd = project_transcript_dir()
    if not pd:
        return tot
    files = sorted(glob(os.path.join(pd, "*.jsonl")))
    if not files:
        return tot
    tot["source"] = "session-transcript"
    for fp in files:
        try:
            with open(fp, encoding="utf-8") as fh:
                for line in fh:
                    line = line.strip()
                    if not line:
                        continue
                    try:
                        obj = json.loads(line)
                    except Exception:
                        continue
                    msg = obj.get("message")
                    if not isinstance(msg, dict):
                        continue
                    u = msg.get("usage")
                    if not isinstance(u, dict):
                        continue
                    tot["records"] += 1
                    tot["input"] += int(u.get("input_tokens", 0) or 0)
                    tot["output"] += int(u.get("output_tokens", 0) or 0)
                    tot["cache_write"] += int(u.get("cache_creation_input_tokens", 0) or 0)
                    tot["cache_read"] += int(u.get("cache_read_input_tokens", 0) or 0)
        except Exception:
            continue
    return tot


def total_of(d: dict) -> int:
    return sum(int(d.get(c, 0) or 0) for c in COMPONENTS)


def load_baseline() -> dict | None:
    try:
        with open(baseline_path(), encoding="utf-8") as fh:
            return json.load(fh)
    except Exception:
        return None


def save_baseline(cum: dict) -> None:
    data = {c: int(cum.get(c, 0) or 0) for c in COMPONENTS}
    data["records"] = int(cum.get("records", 0) or 0)
    try:
        with open(baseline_path(), "w", encoding="utf-8") as fh:
            json.dump(data, fh)
    except Exception:
        pass


def delta(cum: dict, base: dict | None) -> dict:
    base = base or {}
    return {c: max(0, int(cum.get(c, 0) or 0) - int(base.get(c, 0) or 0)) for c in COMPONENTS}


# --------------------------------------------------------------------------- #
# trailer parsing (shared by report + query)
# --------------------------------------------------------------------------- #
def parse_trailer(body: str) -> dict | None:
    m = TRAILER_RE.search(body)
    if not m:
        return None
    fields = {}
    for tok in m.group(1).split():
        if "=" in tok:
            k, v = tok.split("=", 1)
            fields[k] = v
    return fields


# --------------------------------------------------------------------------- #
# write-side commands
# --------------------------------------------------------------------------- #
def fmt_trailer(d: dict, cum: dict, source: str) -> str:
    line1 = (
        f"{TRAILER_KEY}: total={total_of(d)} "
        f"in={d['input']} out={d['output']} "
        f"cache_read={d['cache_read']} cache_write={d['cache_write']} "
        f"source={source}"
    )
    line2 = f"{CUMULATIVE_KEY}: total={total_of(cum)}"
    return line1 + "\n" + line2


def cmd_trailer(msgfile: str) -> None:
    try:
        with open(msgfile, encoding="utf-8") as fh:
            body = fh.read()
    except Exception:
        return
    if TRAILER_KEY in body:
        return
    cum = read_cumulative()
    base = load_baseline()
    if base is None:
        save_baseline(cum)
        d = {c: 0 for c in COMPONENTS}
        source = f"{cum['source']}:baseline-initialised"
    else:
        d = delta(cum, base)
        source = cum["source"]
    trailer = fmt_trailer(d, cum, source)
    lines = body.splitlines(keepends=True)
    insert_at = len(lines)
    for i, ln in enumerate(lines):
        if ln.startswith("#"):
            insert_at = i
            break
    head = "".join(lines[:insert_at]).rstrip("\n")
    tail = "".join(lines[insert_at:])
    new = head + "\n\n" + trailer + "\n"
    if tail:
        new += "\n" + tail
    try:
        with open(msgfile, "w", encoding="utf-8") as fh:
            fh.write(new)
    except Exception:
        pass


def cmd_record() -> None:
    cum = read_cumulative()
    save_baseline(cum)
    cmd_report()


def cmd_report() -> None:
    root = repo_root()
    FS, RS = "\x1f", "\x1e"
    raw = git("log", "--reverse", "--format=%H%x1f%h%x1f%aI%x1f%s%x1f%b%x1e")
    rows, totals, grand = [], {c: 0 for c in COMPONENTS}, 0
    for chunk in raw.split(RS):
        chunk = chunk.strip("\n").strip()
        if not chunk:
            continue
        parts = chunk.split(FS)
        if len(parts) < 5:
            continue
        _full, short, iso, subject, bodyrest = parts[0], parts[1], parts[2], parts[3], FS.join(parts[4:])
        t = parse_trailer(bodyrest)
        if not t:
            continue
        try:
            tot = int(t.get("total", 0)); i = int(t.get("in", 0)); o = int(t.get("out", 0))
            cr = int(t.get("cache_read", 0)); cw = int(t.get("cache_write", 0))
        except ValueError:
            continue
        subj = subject.replace("|", "\\|")
        if len(subj) > 60:
            subj = subj[:57] + "..."
        rows.append((iso[:10], short, subj, tot, i, o, cr, cw))
        totals["input"] += i; totals["output"] += o
        totals["cache_read"] += cr; totals["cache_write"] += cw; grand += tot

    def g(n): return f"{n:,}"
    out = ["# API token usage per commit\n",
           "> **Auto-generated — do not hand-edit.** Regenerated on every commit by "
           "`docs/historian/token_usage.py` (via the `post-commit` hook) from the "
           "`API-Usage-Since-Last-Commit` commit trailers. Rebuild with "
           "`python3 docs/historian/token_usage.py report`; query a period with "
           "`python3 docs/historian/token_usage.py query --from DDMMYY --to DDMMYY`.\n",
           "## Methodology & caveats\n",
           "- **Source.** Counts come from the Claude Code session transcripts "
           "(`~/.claude/projects/<slug>/*.jsonl`, same data `ccusage` reads). Nothing is invented.\n"
           "- **Attribution is temporal**, not per-diff: each row is the usage recorded "
           "*between the previous commit and this one*.\n"
           "- **`total` = `in` + `out` + `cache_read` + `cache_write`.** Cache-read dominates; shown separately.\n"
           "- Commits authored outside a Claude session show `0` (`source=none`) and are omitted below.\n",
           "## Per-commit ledger\n",
           "| Date | Commit | Subject | Total | in | out | cache_read | cache_write |",
           "|---|---|---|--:|--:|--:|--:|--:|"]
    for (date, short, subj, tot, i, o, cr, cw) in rows:
        out.append(f"| {date} | `{short}` | {subj} | {g(tot)} | {g(i)} | {g(o)} | {g(cr)} | {g(cw)} |")
    out.append(f"| **TOTAL** | | **{len(rows)} commits** | **{g(grand)}** | **{g(totals['input'])}** | "
               f"**{g(totals['output'])}** | **{g(totals['cache_read'])}** | **{g(totals['cache_write'])}** |")
    out.append("")
    try:
        os.makedirs(os.path.join(root, "docs"), exist_ok=True)
        with open(os.path.join(root, LEDGER_REL), "w", encoding="utf-8") as fh:
            fh.write("\n".join(out))
    except Exception:
        pass


def cmd_init() -> None:
    save_baseline(read_cumulative())
    print(f"token-accounting baseline initialised at {baseline_path()}")


def cmd_show() -> None:
    cum = read_cumulative()
    base = load_baseline()
    d = delta(cum, base) if base else {c: 0 for c in COMPONENTS}
    print("source:            ", cum["source"])
    print("transcript records:", cum["records"])
    print("cumulative total:  ", f"{total_of(cum):,}")
    for c in COMPONENTS:
        print(f"  cumulative {c:<12}: {cum[c]:>15,}")
    print("since last commit: ", f"{total_of(d):,}")


# --------------------------------------------------------------------------- #
# query side — recorded token usage over a time period
# --------------------------------------------------------------------------- #
def parse_ddmmyy(s: str) -> _dt.date:
    s = s.strip()
    if not re.fullmatch(r"\d{6}", s):
        raise ValueError(f"expected DDMMYY (6 digits), got {s!r}")
    return _dt.date(2000 + int(s[4:6]), int(s[2:4]), int(s[0:2]))


def cmd_query(dfrom_s, dto_s, branch, per_commit, as_json) -> None:
    dfrom = parse_ddmmyy(dfrom_s) if dfrom_s else None
    dto = parse_ddmmyy(dto_s) if dto_s else None
    ref = ref_for_branch(branch)
    args = ["log", "--no-merges", "--reverse", "--format=%h%x1f%aI%x1f%s%x1f%b%x1e"]
    if dfrom:
        args.append(f"--since={dfrom.isoformat()} 00:00:00")
    if dto:
        args.append(f"--until={dto.isoformat()} 23:59:59")
    args.append(ref)
    raw = git(*args)

    FS, RS = "\x1f", "\x1e"
    totals = {c: 0 for c in COMPONENTS}
    grand = 0
    n_total = n_with_data = 0
    commits = []
    for chunk in raw.split(RS):
        chunk = chunk.strip("\n").strip()
        if not chunk:
            continue
        parts = chunk.split(FS)
        if len(parts) < 3:
            continue
        short, iso, subject, body = parts[0], parts[1], parts[2], FS.join(parts[3:])
        n_total += 1
        t = parse_trailer(body)
        if not t or t.get("source", "none") == "none":
            commits.append((iso[:10], short, subject, None))
            continue
        try:
            row = {k: int(t.get(TRAILER_FIELDS[k], 0)) for k in COMPONENTS}
            tot = int(t.get("total", total_of(row)))
        except ValueError:
            commits.append((iso[:10], short, subject, None))
            continue
        n_with_data += 1
        for c in COMPONENTS:
            totals[c] += row[c]
        grand += tot
        commits.append((iso[:10], short, subject, tot))

    result = {
        "branch": ref,
        "from": dfrom.isoformat() if dfrom else None,
        "to": dto.isoformat() if dto else None,
        "commits_total": n_total,
        "commits_with_token_data": n_with_data,
        "total": grand,
        "input": totals["input"],
        "output": totals["output"],
        "cache_read": totals["cache_read"],
        "cache_write": totals["cache_write"],
    }
    if as_json:
        if per_commit:
            result["per_commit"] = [
                {"date": d, "commit": s, "subject": subj, "total": tk} for (d, s, subj, tk) in commits
            ]
        print(json.dumps(result, indent=2))
        return

    def g(n): return f"{n:,}"
    span = f"{dfrom} .. {dto}" if (dfrom or dto) else "full history"
    print(f"Token usage recorded in commits on {ref}  ({span})")
    print(f"  commits:            {n_total}  ({n_with_data} with token trailers)")
    print(f"  total:              {g(grand)}")
    print(f"    input:            {g(totals['input'])}")
    print(f"    output:           {g(totals['output'])}")
    print(f"    cache_read:       {g(totals['cache_read'])}")
    print(f"    cache_write:      {g(totals['cache_write'])}")
    if per_commit:
        print("  per-commit:")
        for (d, s, subj, tk) in commits:
            subj = subj[:60]
            print(f"    {d}  {s}  {('%15s' % g(tk)) if tk is not None else ('%15s' % '—')}  {subj}")


# --------------------------------------------------------------------------- #
def main() -> int:
    ap = argparse.ArgumentParser(description="OUTRAM PARK per-commit API-token accounting (write + query)")
    sub = ap.add_subparsers(dest="cmd")
    sub.add_parser("record"); sub.add_parser("report"); sub.add_parser("init"); sub.add_parser("show")
    pt = sub.add_parser("trailer"); pt.add_argument("msgfile")
    pq = sub.add_parser("query")
    pq.add_argument("--from", dest="dfrom"); pq.add_argument("--to", dest="dto")
    pq.add_argument("--branch", default="develop")
    pq.add_argument("--per-commit", action="store_true"); pq.add_argument("--json", action="store_true")
    args = ap.parse_args()
    try:
        if args.cmd == "trailer":
            cmd_trailer(args.msgfile)
        elif args.cmd == "record":
            cmd_record()
        elif args.cmd == "report":
            cmd_report()
        elif args.cmd == "init":
            cmd_init()
        elif args.cmd == "query":
            cmd_query(args.dfrom, args.dto, args.branch, args.per_commit, args.json)
        elif args.cmd == "show" or args.cmd is None:
            cmd_show()
    except Exception as exc:
        sys.stderr.write(f"token_usage: non-fatal error: {exc}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
