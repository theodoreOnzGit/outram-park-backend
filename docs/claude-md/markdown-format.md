<!-- Moved verbatim from the workspace CLAUDE.md on 2026-09-21 (maintainer direction: physics
     rules stay in the root, everything else here behind a pointer). STILL BINDING. -->

## README / Markdown format (mandatory)

**Every `README.md` in this workspace must render correctly on GitHub
(GitHub-Flavored Markdown).** GitHub renders LaTeX math via MathJax (`$...$`
inline, `$$...$$` display), so math *is* allowed — but keep it to a conservative
subset that also survives editor previewers. **No exotic math.** Concretely:

- **No matrix/array environments** (`\begin{bmatrix}`, `pmatrix`, `array`) and
  **no `\begin{cases}`** — write a matrix system or a piecewise definition as
  separate `$$...$$` equations, one per line, labelled in prose or with a
  trailing `\quad (\text{...})`.
- **No** `\boxed`, `\underbrace`, `\displaystyle`, `\tfrac`/`\dfrac` (use
  `\frac`), or negative-space `\!`.
- **No Unicode Greek or operators inside math** — use `\gamma`, `\rho`, `\xi`,
  `-`, `\le`, `\pm`, etc. (Unicode is fine in ordinary prose and in inline
  code spans.)
- Write superscripts/subscripts with explicit braces (`(\hat{u}^*)^2`, not
  `\hat u^{*2}`).

**Check every README before finishing.** Prefer `pandoc` when available — it
validates both markdown structure *and* the LaTeX math (via its texmath engine):

```bash
pandoc -f gfm+tex_math_dollars -t html --mathml README.md > /dev/null
```

Exit 0 with **no warnings** means all math converted (any malformed equation
prints a `[WARNING] Could not convert TeX math …`). Note: without `--mathml`,
pandoc emits harmless "rendering as TeX" warnings for every equation — those are
not errors, so always pass `--mathml` when validating.

If `pandoc` is not installed, fall back to `cmark-gfm` for a structure-only
check (`cmark-gfm -e table -e strikethrough -e tagfilter README.md > /dev/null`,
exit 0, no warnings) — but `cmark-gfm` does not render math, so also eyeball the
math against the subset above.

