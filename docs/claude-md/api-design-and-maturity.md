<!-- Moved verbatim from the workspace CLAUDE.md on 2026-09-21 (maintainer direction: physics
     rules stay in the root, everything else here behind a pointer). STILL BINDING. -->

## Human interface layer (mandatory design principle)

**Every public API in this workspace must be navigable by a Rust developer using
rust-analyzer alone — no AI assistant, no prior knowledge of the codebase.**

This is a hard rule, not a goal. The human mind cannot hold large amounts of context
simultaneously. If understanding a function requires recalling three other modules at
once, the interface is wrong regardless of how correct the physics is.

### What this requires in practice

**Every public function, type, trait, and module must have a `///` or `//!` doc comment that answers:**
- What physical quantity does this compute or represent?
- What are the valid input ranges and assumptions?
- What units do parameters represent — even when `uom` enforces them, spell it out for human readers.

**Complex `uom` types must have named type aliases.** A user hovering in their editor
should see `SpecificEnthalpy`, not a raw `Quantity<ISQ<...>, SI<f64>, f64>`.

**Each module's `lib.rs` / `mod.rs` must have a `//!` module-level comment** that
explains what belongs in the module and what does not. This is the map a new user
reads first.

**Examples are the primary entry point, not the API docs.** A user must be able to
find an example, read it top-to-bottom without jumping to other files, and understand
what crate they need and how to call it.

### What AI assistants must not do

- Do not add complexity (extra type parameters, trait indirection, macro magic) in
  the name of correctness or generality if it raises the mental context load for a
  human reader.
- Do not leave public items undocumented. If you add or modify a public item, add or
  update its `///` doc comment in the same change.
- Do not write examples that require reading internal modules to understand.

## Crate maturity gate (HARD RULE)

**The dogfooding rule below is a hard rule for every crate the maintainer has
declared mature, and is not enforced on any crate before that.** Both halves
bind: do not skip it on a mature crate, and do not demand it of one still
finding its shape.

**API polish comes after the internals are shown to be reasonably accurate,
never before.** An interface is a commitment to a shape, and shaping an
interface around physics that is still moving means paying for the same
interface twice. Worse, an API that is pleasant to call and quietly wrong is
more dangerous than one that is awkward, because the ergonomics invite trust
the numbers have not earned.

**What "reasonably accurate" means here.** A crate becomes eligible when its
internals are backed by at least one of:

- **Analytical or manufactured solution** — a closed-form result, or an MMS
  convergence study showing the expected order of accuracy. This proves the
  numerics.
- **Cross-code comparison** — agreement with an established code (OpenMC,
  Serpent, MOOSE, OpenFOAM, NJOY) on the same input, where no closed form
  exists.
- **Unit tests and internal consistency** — conservation, symmetry,
  reciprocity and limiting-case invariants holding under test. Necessary
  always, and sufficient on its own only for crates with no physics to get
  wrong (utility, I/O, tooling).

**Published-benchmark agreement is deliberately not on that list.** Matching
HTR-10, MSRE or ICSBEP is the *goal* of the validation pipeline, and that
pipeline is driven through the very API in question — so requiring it first
would deadlock the work. Benchmark agreement is the reward for a good API, not
its entry fee. (Rationale recorded 2026-09-05; revisit if it proves too
lenient.)

**The tolerance is per-crate, and lives in that crate's own `CLAUDE.md`.**
There is no workspace-wide number: 500 pcm means something in `outram-mc-libs`
and nothing in `outram-park-fork-dwsim-libs`. Each crate states its own bar,
the reference it is measured against, and the evidence class above that it
claims. **A crate with no such statement is by definition not yet mature.**

**Who declares it.** The maintainer, and only the maintainer. An agent that
believes a crate has cleared its bar may *propose* maturity — open an issue,
cite the specific runs and numbers, and stop there. **Do not flip the flag**,
and do not begin enforcing the dogfooding rule on the strength of your own
assessment.

**The bar moves, and that is expected.** Record every revision as a dated
entry in the crate's own `CLAUDE.md`, **keeping the superseded ones** — an
agent reading the crate later needs to know not just today's bar but that it
moved, or it will misread older results as failures against a standard that
did not exist when they were produced.

**Twelve crates were declared mature as of 2026-09-15**, plus `petir`; the
Members table marks them. **The bar and its evidence live in each crate's own
`CLAUDE.md`, which is the authority — this file's roster is a pointer.**

**Read the crate's own file before citing any of them as validated.** Several
carry caveats that matter and that a roster row cannot hold: `teh-o-prke`
lacks analytical transient validation; `farrer-park` has a **known uncured
shear-locking defect** and no cross-code or benchmark leg at all;
`outram-park-fork-dwsim-libs` has met its bar **at the EOS layer only**, with
0 of 107 port-coverage rows at `PORTED + VALIDATED`; `petir`'s bar is
cross-code agreement **alone**, against no published benchmark; and
`outram-park-fork-liggghts` reproduces upstream LIGGGHTS essentially exactly
while having **no experimental comparison** of the granular physics at all.
Agreeing on a number is not the same as agreeing on the physics —
`outram-mc-libs` sits well inside its 500 pcm bar on Godiva while a ~69 pcm
*spectral* residual against OpenMC is still open.

> The full roster with each crate's bar and evidence class, the six honest
> notes in full, and the measured API-guessing table:
> [`docs/claude-md-rationale/maturity-and-api-dogfooding.md`](../claude-md-rationale/maturity-and-api-dogfooding.md).

### Verifying it: dogfood the API on a small model (HARD RULE)

> **If it is too complex for Haiku, it is a bad API.**

That is the standing test for **every crate declared mature** (see the gate
directly above — it is not asked of a crate still finding its shape), and it
is a test, not a slogan: a small model has no budget to read the source, so it
can only use what the interface itself makes discoverable — exactly like the
human this file already requires the API to serve. When it cannot get there,
the interface is wrong, however correct the physics underneath.

**Before claiming a public API is usable from Python, run it on a Haiku agent
with a fresh context and the wheel only — no repository access, no `.pyi`
pasted in, no source.** Count the round trips to a working script and log
every exception. That number is the usability measure; the exception log is
the fix list.

**WHY THIS EXISTS: capability masks usability defects.** A strong model is the
*worst* instrument for this, for the same reason the author of a tool is the
worst person to usability-test it — when the API is unguessable it reads the
Rust source and compensates, and the compensation is invisible in the result.
The finished script looks clean and the defect ships. Measured 2026-09-04, an
Opus session **with** full source access and grep still guessed wrong ten
times across six scripts (`mc.Sphere(center, radius)` for
`Sphere(x0, y0, z0, r, bc)`, `.k_eff` for `.k_mean`, `FvSolution()` which is
not constructible at all, and seven more — the table is in the rationale doc).
Every one cost a round trip. A model that *cannot* read the source has no way
to recover from any of them, which is the point.

**What the test requires:**

- **Fixed context.** Wheel installed, nothing else. Handing the agent the
  stubs or the crate source tests something other than the API.
- **A fixed task list** spanning the tiers — a one-call case, a case assembled
  from parts, and one exotic enough to need the low-level types.
- **Every exception logged, not just the count.** Ranked by frequency across
  tasks, that log *is* the backlog, in priority order.

**What it does NOT measure: physical correctness.** A script can run cleanly
and be nonsense. This never substitutes for the V&V tests — it tests whether
the API can be *found and called*, nothing more.

**Reading the result.** A failure the Opus session also hit is an API defect.
A failure unique to the small model is more likely its own priors — the tell
is whether a different model trips on the same call. Fix the first kind.

**The same principle applies one layer down, to Rust callers.** A confusing
trait-bound error is the compiler's version of an undiscoverable API.
`#[diagnostic::on_unimplemented]` (stable since 1.78) replaces "the trait
bound `X: Y` is not satisfied" with a message, a label and a note of your
choosing; `#[diagnostic::do_not_recommend]` (stable since 1.85) suppresses a
blanket impl the compiler would otherwise suggest unhelpfully. This workspace
has 65 public traits and, as of 2026-09-04, not one carries either attribute.
Any trait whose bound a caller can plausibly fail should.

A good message does not restate the bound. It names the concrete types that
*do* implement the trait, or the constructor to call: "implemented by
`PengRobinson`, `Srk`, `PengRobinson1978`" turns a search through the source
into a choice from a list.

**Take the baseline *before* changing anything** — the fixes this suggests
should be measured against it, not asserted.
