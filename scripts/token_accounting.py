#!/usr/bin/env python3
"""API-token accounting for git commits (OUTRAM PARK workspace standing rule).

WHAT THIS DOES
--------------
Claude Code writes every assistant turn's real token usage into the session
transcript at ``~/.claude/projects/<project-slug>/<session-id>.jsonl`` (the same
data ``ccusage`` reads). This script reads those transcripts, tracks a running
cumulative, and attributes the **delta since the previous commit** to each new
commit — so every commit carries an honest record of the API tokens spent
producing it.

It is wired into two git hooks (``.githooks/``):

* ``prepare-commit-msg`` -> ``token_accounting.py trailer <msgfile>``
  Appends an ``API-Usage-*`` trailer to the commit message (idempotent: it never
  double-adds, so amend/rebase are safe).
* ``post-commit`` -> ``token_accounting.py record``
  Advances the baseline to the current cumulative and regenerates
  ``docs/token-usage.md`` from ``git log`` (deterministic, self-healing).

HONESTY / LIMITATIONS (read before trusting a number)
-----------------------------------------------------
* Numbers come straight from the session transcripts; nothing is invented. If no
  transcript is available (e.g. a human runs ``git commit`` outside a Claude
  session), the delta is legitimately ``0`` and the trailer says ``source=none``.
* Attribution is *temporal*: "tokens seen in the transcripts between the previous
  commit and this one". It is NOT a per-file or per-diff measurement — a commit
  that bundles a lot of exploratory work will carry those tokens.
* **Cache-read tokens dominate** (prompt-cache re-reads of the growing context).
  They are shown separately from ``in``/``out`` so you can weigh them yourself;
  ``total`` is the sum of all four components.
* Sub-agent / parallel-session usage in the same project dir is included (all
  ``*.jsonl`` in the project slug dir are summed).

Usage: token_accounting.py {trailer <msgfile> | record | report | init | show}
Stdlib only; never exits non-zero from the hook path (failures degrade to a
zero/"source=none" trailer rather than blocking a commit).
"""
from __future__ import annotations

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


# --------------------------------------------------------------------------- #
# git helpers
# --------------------------------------------------------------------------- #
def git(*args: str) -> str:
    try:
        return subprocess.check_output(
            ["git", *args], stderr=subprocess.DEVNULL, text=True
        ).strip()
    except Exception:
        return ""


def repo_root() -> str:
    return git("rev-parse", "--show-toplevel") or os.getcwd()


def git_dir() -> str:
    d = git("rev-parse", "--git-dir") or ".git"
    return d if os.path.isabs(d) else os.path.join(repo_root(), d)


def baseline_path() -> str:
    return os.path.join(git_dir(), "claude-token-baseline.json")


# --------------------------------------------------------------------------- #
# transcript reading
# --------------------------------------------------------------------------- #
def project_transcript_dir() -> str | None:
    """Locate ~/.claude/projects/<slug>/ for the current repo."""
    base = os.path.expanduser("~/.claude/projects")
    if not os.path.isdir(base):
        return None
    root = os.environ.get("CLAUDE_PROJECT_DIR") or repo_root()
    slug = re.sub(r"[^A-Za-z0-9]+", "-", os.path.abspath(root))
    cand = os.path.join(base, slug)
    if os.path.isdir(cand):
        return cand
    # Fallback: unique dir whose name contains the repo basename.
    bn = os.path.basename(os.path.abspath(root))
    matches = [d for d in glob(os.path.join(base, "*")) if bn and bn in os.path.basename(d)]
    return matches[0] if len(matches) == 1 else None


def read_cumulative() -> dict:
    """Sum token usage across every transcript in the project dir."""
    tot = {c: 0 for c in COMPONENTS}
    tot["web_search"] = 0
    tot["web_fetch"] = 0
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
                    stu = u.get("server_tool_use") or {}
                    if isinstance(stu, dict):
                        tot["web_search"] += int(stu.get("web_search_requests", 0) or 0)
                        tot["web_fetch"] += int(stu.get("web_fetch_requests", 0) or 0)
        except Exception:
            continue
    return tot


def total_of(d: dict) -> int:
    return sum(int(d.get(c, 0) or 0) for c in COMPONENTS)


# --------------------------------------------------------------------------- #
# baseline persistence
# --------------------------------------------------------------------------- #
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
# commands
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
    """Append the API-Usage trailer to the commit message (idempotent)."""
    try:
        with open(msgfile, encoding="utf-8") as fh:
            body = fh.read()
    except Exception:
        return
    if TRAILER_KEY in body:
        return  # amend / re-edit — already stamped, do not double-add.

    cum = read_cumulative()
    base = load_baseline()
    if base is None:
        # First stamped commit in this clone: initialise the baseline here so we
        # do NOT dump the whole session-to-date onto one commit. Per-commit
        # deltas begin from the next commit. delta is 0 => safe on abort.
        save_baseline(cum)
        d = {c: 0 for c in COMPONENTS}
        source = f"{cum['source']}:baseline-initialised"
    else:
        d = delta(cum, base)
        source = cum["source"]

    trailer = fmt_trailer(d, cum, source)
    sep = "" if body.endswith("\n\n") else ("\n" if body.endswith("\n") else "\n\n")
    # Comment lines (git status help) must stay after the trailer only if the
    # message is still a template; simplest robust approach: insert before the
    # first comment block if present, else append.
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
    """post-commit: advance baseline + regenerate the ledger."""
    cum = read_cumulative()
    save_baseline(cum)
    cmd_report()


def _parse_trailer(body: str) -> dict | None:
    m = re.search(rf"^{re.escape(TRAILER_KEY)}:\s*(.+)$", body, re.MULTILINE)
    if not m:
        return None
    fields = {}
    for tok in m.group(1).split():
        if "=" in tok:
            k, v = tok.split("=", 1)
            fields[k] = v
    return fields


def cmd_report() -> None:
    """Rebuild docs/token-usage.md deterministically from git log trailers."""
    root = repo_root()
    sep = "\x00"
    raw = git("log", "--reverse", f"--format=%H{sep}%h{sep}%aI{sep}%s{sep}%b{sep}%x01")
    rows = []
    totals = {c: 0 for c in COMPONENTS}
    grand = 0
    for chunk in raw.split("\x01\n"):
        chunk = chunk.strip()
        if not chunk:
            continue
        parts = chunk.split(sep)
        if len(parts) < 5:
            continue
        _full, short, iso, subject, bodyrest = parts[0], parts[1], parts[2], parts[3], sep.join(parts[4:])
        t = _parse_trailer(bodyrest)
        if not t:
            continue
        try:
            tot = int(t.get("total", 0))
            i = int(t.get("in", 0))
            o = int(t.get("out", 0))
            cr = int(t.get("cache_read", 0))
            cw = int(t.get("cache_write", 0))
        except ValueError:
            continue
        date = iso[:10]
        subj = subject.replace("|", "\\|")
        if len(subj) > 60:
            subj = subj[:57] + "..."
        rows.append((date, short, subj, tot, i, o, cr, cw))
        totals["input"] += i
        totals["output"] += o
        totals["cache_read"] += cr
        totals["cache_write"] += cw
        grand += tot

    def g(n: int) -> str:
        return f"{n:,}"

    out = []
    out.append("# API token usage per commit\n")
    out.append(
        "> **Auto-generated — do not hand-edit.** Regenerated on every commit by "
        "`scripts/token_accounting.py` (via the `post-commit` hook) from the "
        "`API-Usage-Since-Last-Commit` trailer that the `prepare-commit-msg` hook "
        "stamps into each commit message. Rebuild manually with "
        "`python3 scripts/token_accounting.py report`.\n"
    )
    out.append("## Methodology & caveats\n")
    out.append(
        "- **Source.** Token counts come from the Claude Code session transcripts "
        "(`~/.claude/projects/<slug>/*.jsonl`) — the same data `ccusage` reads. "
        "Nothing is estimated or invented.\n"
        "- **Attribution is temporal**, not per-diff: each row is the token usage "
        "recorded *between the previous commit and this one*. A commit that bundles "
        "a lot of exploration carries those tokens.\n"
        "- **`total` = `in` + `out` + `cache_read` + `cache_write`.** Cache-read "
        "(prompt-cache re-reads of the growing context) usually dominates; it is "
        "shown separately so you can weigh it.\n"
        "- Commits authored outside a Claude session legitimately show `0` "
        "(`source=none`) and are omitted below.\n"
        "- Sub-agent / parallel-session usage in the same project dir is included.\n"
    )
    out.append("## Per-commit ledger\n")
    out.append("| Date | Commit | Subject | Total | in | out | cache_read | cache_write |")
    out.append("|---|---|---|--:|--:|--:|--:|--:|")
    for (date, short, subj, tot, i, o, cr, cw) in rows:
        out.append(f"| {date} | `{short}` | {subj} | {g(tot)} | {g(i)} | {g(o)} | {g(cr)} | {g(cw)} |")
    out.append(
        f"| **TOTAL** | | **{len(rows)} commits** | **{g(grand)}** | "
        f"**{g(totals['input'])}** | **{g(totals['output'])}** | "
        f"**{g(totals['cache_read'])}** | **{g(totals['cache_write'])}** |"
    )
    out.append("")
    content = "\n".join(out)
    try:
        docs_dir = os.path.join(root, "docs")
        os.makedirs(docs_dir, exist_ok=True)
        with open(os.path.join(root, LEDGER_REL), "w", encoding="utf-8") as fh:
            fh.write(content)
    except Exception:
        pass


def cmd_init() -> None:
    """Set the baseline to the current cumulative (called by the installer)."""
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


def main() -> int:
    args = sys.argv[1:]
    if not args:
        cmd_show()
        return 0
    cmd = args[0]
    try:
        if cmd == "trailer" and len(args) >= 2:
            cmd_trailer(args[1])
        elif cmd == "record":
            cmd_record()
        elif cmd == "report":
            cmd_report()
        elif cmd == "init":
            cmd_init()
        elif cmd == "show":
            cmd_show()
        else:
            sys.stderr.write(__doc__ or "")
            return 0
    except Exception as exc:  # never break a commit
        sys.stderr.write(f"token_accounting: non-fatal error: {exc}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
