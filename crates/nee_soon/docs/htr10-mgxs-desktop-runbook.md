# HTR-10 MGXS cases: runbook for the run machine

> **Research, education and V&V only.** Not for reactor operation, licensing
> or safety decisions (`RESPONSIBLE_USE.md`).

This page is for a session whose job is only to **run** the long HTR-10 MGXS
cases (gh:#845) on a machine with many cores, using as few tokens as
possible. Every decision about the cases (model, temperature, particle
budget, seed) is already fixed in `crates/nee_soon/src/htr10_mgxs/cases.rs`.
**Do not change a case, a budget or a seed.** If a case looks wrong, stop and
report it; do not fix it here.

## Token rules

- **Start the queue detached with `nohup`** (step 2) and end the turn. A
  recorded case takes hours and the queue a day or more, longer than a
  session should sit waiting; the process keeps running after the session
  ends. Come back in a later session for step 3. Do not poll, do not
  `sleep`, and do not `tail` the log while it runs.
- **Read only the last lines** of output: every run ends with one `RESULT`
  or `ERROR` line, and any `FAILED CHECK` lines just before it.
- **Never open `record.json`**: it is a few MB of numbers on one line. The
  numbers to report are in `summary.csv` and `INDEX.md`.
- Do not read the source to "understand" a case. This page is enough.

## One-time setup

```bash
git clone https://github.com/theodoreOnzGit/outram-park-backend.git
cd outram-park-backend
git checkout develop
git submodule update --init --depth 1 reference-data/ace   # 145 MB: the rod nickel tapes
cargo build --release -p nee_soon --example htr10_mgxs_case
```

The ENDF/B-VIII.0 tapes are in the repository (`reference-data/endf/`);
nothing else is downloaded. Peak memory is about 1 to 2 GB.

## Each session

```bash
git pull origin develop
cargo build --release -p nee_soon --example htr10_mgxs_case
./target/release/examples/htr10_mgxs_case --list
```

`--list` prints every case, an estimated wall clock for this machine, and
`[done]` beside cases that already have a record.

**1. Smoke test first, every new machine or after a pull that touched
`nee_soon`** (a few minutes; nothing to commit, it writes to `target/`):

```bash
./target/release/examples/htr10_mgxs_case --case smoke
```

It must end `RESULT smoke ... checks=...`. On `smoke` two checks are
expected to be able to fail, because 500 particles are too few: "the fuel
kernel saw flux in every group" (measured 2026-10-12: 10 of 70 groups empty)
and "k agrees with the 2026-10-07 record". Any other `FAILED CHECK`, or an
`ERROR` line: stop and report it. On a recorded case every check is expected
to pass.

**2. Run the queue in the background:**

```bash
nohup ./target/release/examples/htr10_mgxs_case --queue > mgxs_queue.log 2>&1 &
```

`--queue` runs every recorded case without a `record.json`, in order, and
skips the finished ones, so a stopped queue resumes by running the same
command. Add `--threads N` to leave cores free for other work; it changes the
wall clock, never the result.

**3. When it finishes** (or after each case, if the session must end first):

```bash
grep -E "^(RESULT|ERROR|FAILED CHECK)" mgxs_queue.log
cat crates/nee_soon/verification_and_validation/htr10_mgxs_runs/INDEX.md
```

**4. Commit and push the records** (this page is the maintainer's request to
do so; push to `develop` only, never `main`):

```bash
git add crates/nee_soon/verification_and_validation/htr10_mgxs_runs
git commit -m "nee_soon: HTR-10 MGXS case records (<case names>)" \
           -m "Run with htr10_mgxs_case on <CPU>, <threads> threads. No token accounting on this clone."
git pull --rebase origin develop
git push origin develop
```

If the rebase conflicts in `INDEX.md` only, regenerate it and continue:

```bash
./target/release/examples/htr10_mgxs_case --index
git add crates/nee_soon/verification_and_validation/htr10_mgxs_runs/INDEX.md
git rebase --continue
```

**5. Report** in one short message: the `RESULT` lines, any `FAILED CHECK`
lines verbatim, and the commit pushed. Do not interpret the physics; the
analysis is done elsewhere.

## What a FAILED CHECK means (report it, do not fix it)

A case with a failed check still writes its record; the record is the result.

| check | if it fails |
|---|---|
| the two passes ran identical histories | the matrix pass is not on the same histories as the scalar pass; the constants are suspect |
| no history was lost | a geometry hole; report the count |
| collapse conserves k_inf | a defect in the collapse code |
| the fuel kernel saw flux in every group | too few particles for the 70-group structure |
| k agrees with the 2026-10-07 record | the 300 K cases only; a real disagreement with the earlier `k` |

## Errors

| `ERROR` says | do this |
|---|---|
| a tape is not in this checkout | `git submodule update --init --depth 1 reference-data/ace`, then rerun |
| out of memory, or the process was killed | rerun the same command with fewer `--threads`; `--queue` resumes |
| anything else | stop, report the line verbatim |

## What the cases are

`--list` is authoritative; at the time of writing:

| case | what | feeds |
|---|---|---|
| `smoke` | full core, 500 × \[2 + 3\]: a pipeline check, not a record | — |
| `verify_tallies` | full core, 4000 × \[10 + 40\]: the tally checks at a resolvable budget; already run on the development machine, not queued | — |
| `core_n12_t300` | full core N = 12 at 300.15 K, 10 000 × \[20 + 135\] | #850, #851, #854 |
| `core_n12_t600`, `_t900`, `_t1200` | the same at 600, 900, 1200 K (isothermal) | #852 |
| `core_n12_t300_replica` | `core_n12_t300` at an independent seed | the spread of the constants |

Each recorded case is two transport passes of 1.55 M histories, roughly 13
to 32 CPU-hours (measured 14.4 to 37.5 ms CPU per history on this core).
