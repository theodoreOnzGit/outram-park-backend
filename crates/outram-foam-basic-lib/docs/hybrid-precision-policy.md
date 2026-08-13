# Hybrid co-execution: the precision policy

**Status: decided, not implemented.** This records a maintainer decision made
on 2026-08-13 for bead `op-yvj.4.8` (hybrid CPU+GPU co-execution), so that the
trade-off is settled *before* anyone writes the split scheduler rather than
discovered afterwards. No `Hybrid` backend exists in `compute.rs` today, and
none can until a GPU kernel exists at all — see "Why this is not implemented"
at the end.

---

## The decision

> **In hybrid co-execution, the CPU half runs `f32`, matching the GPU.**
> Precision is uniform across the output array. `Hybrid` is an explicitly
> `f32`-class throughput mode. `Serial` (`f64`) remains the oracle.

The alternative — CPU at `f64`, GPU at `f32` — means two lanes of a single
output array can differ in accuracy by seven orders of magnitude purely by
which side of the split ratio they landed on. Cell 5 and cell 6 of one
`VolScalarField` would carry different error bars for a reason that has nothing
to do with the physics. That is the thing being ruled out.

## What the decision fixes

**Precision heterogeneity, completely.** Every lane of a hybrid result now has
the same accuracy class regardless of which device computed it. A caller can
state one error bound for the array instead of a bound that depends on a
scheduling parameter. This is the whole point and it is worth the cost below.

**A throughput bonus, incidentally.** `f32` on the CPU is not a slowdown: AVX2
carries 8 `f32` lanes against 4 `f64`, and the memory traffic halves. The CPU
half of a hybrid split is likely *faster* at `f32` than the `f64` path it
replaces, so this is not a case of degrading one side to match the other.

## What the decision does NOT fix

**Reduction reproducibility under a varying split.** Uniform `f32` does not
make a reduction reproducible run to run, because floating-point addition is
non-associative in `f32` exactly as in `f64`. If a dynamic work-stealing split
hands 60% of the lanes to the GPU on one run and 55% on the next, the partial
sums group differently and the total changes in the last bits.

`f32` in fact makes this **more visible, not less**: the mantissa is 24 bits
against 53, so the same reordering produces a relative discrepancy about `2^29`
times larger. A dot product that wandered in the 16th digit at `f64` wanders in
the 7th at `f32`.

So the second caveat from the bead stands unchanged and must still be honoured:
**default to a static, deterministic split**, and offer work-stealing only as
an opt-in for throughput runs where bitwise reproducibility is explicitly not
wanted. Uniform `f32` is not a substitute for that.

## What the decision introduces

**CPU `f32` and GPU `f32` are the same precision class but are not bitwise
identical.** Matching the type does not match the result, for three reasons:

- **Transcendentals are not correctly rounded on the GPU.** IEEE-754 requires
  correct rounding for `+ - * /` and `sqrt`, and Rust delivers it. WGSL
  specifies only *ULP bounds* for `exp`, `log`, `sin`, `pow` and friends, and
  those bounds are met differently by different vendors. Any kernel whose
  objective function contains a transcendental will diverge between the two
  halves of the split.
- **FMA contraction.** GPU shader compilers routinely fuse `a*b + c` into a
  single fused multiply-add with one rounding; Rust does not contract
  automatically (there is no `-ffast-math` equivalent applied here). That is a
  half-ULP difference per fused site, and it compounds.
- **Denormal flush-to-zero.** Many GPUs flush `f32` denormals to zero. CPUs do
  not, unless asked.

**Consequence for the parity gate:** the hybrid parity test in
`tests/hybrid_parity.rs` cannot assert bitwise equality between the CPU and GPU
halves of a split. It must be a tolerance gate sized at `f32` epsilon scale,
and the tolerance must be justified per kernel from the operations that kernel
actually performs — a kernel of pure `+ - * /` can be held far tighter than one
calling `exp`.

**Consequence for the accuracy contract:** hybrid answers will sit roughly
`1e-7` relative from the `Serial` `f64` oracle, not `1e-15`. This must be
stated on every kernel that offers `Hybrid`, in the kernel's own doc comment,
not only here.

## Per-kernel consequence — which kernels may take this deal

The accuracy floors documented in `src/math/differentiate.rs` and
`src/math/minimise.rs` are functions of machine epsilon, so they move bodily
when epsilon changes. Computed 2026-08-13 from
`f64::EPSILON = 2.220446049250313e-16` and
`f32::EPSILON = 1.1920928955078125e-7`:

| Method | Floor | `f64` | `f32` | Decimal digits lost |
|---|---|---|---|---|
| Forward / backward differences | `sqrt(eps)` | `1.49e-8` | `3.45e-4` | 4.4 |
| Central differences | `eps^(2/3)` | `3.67e-11` | `2.42e-5` | 5.8 |
| Richardson-extrapolated | `eps^(4/5)` | `3.00e-13` | `2.89e-6` | 7.0 |
| Golden-section minimisation | `sqrt(2*eps*abs(f)/abs(f''))` | `1.49e-8` scaled | `3.45e-4` scaled | 4.4 |

Read the golden-section row with the warning already in `minimise.rs`: that
floor is a property of the **objective**, through `|f|/|f''|`, not of the
method. A flat objective is already far worse than the headline figure at
`f64`; at `f32` it is worse again by the same 4.4 digits.

This gives a clean split of the kernel families:

**Good candidates for `Hybrid`** — the answer is a value, not a difference of
nearby values, so `f32` costs precision proportionally and nothing more:

- batched root finding (`op-yvj.4.2`) — a bracketed root is located to a
  relative tolerance the caller names; `f32` simply sets a lower bound on what
  they may name
- fixed-rule quadrature (`op-yvj.4.5`) — a weighted sum; the concern is the
  reduction ordering above, not the element precision
- elementwise field algebra and SpMV (`op-yvj.4.4`)

**Poor candidates — do not offer `Hybrid`, or offer it only with the floor
stated loudly in the signature's doc comment:**

- **numerical differentiation** (`op-yvj.4.6`). Central differences fall from
  11 correct digits to 4.6. A finite-difference Jacobian at `f32` is a
  3-to-4-significant-digit object, which is thin for a Newton iteration to feed
  on and very thin for the Rosenbrock stages that consume it.
- **golden-section minimisation** (`op-yvj.4.3`). This crate's own motivating
  application is the choked-flow maximum `G(p) = rho*sqrt(2*(h0 - h))` along an
  isentrope, and that objective was measured earlier in development to locate
  its optimum to order 0.6–3 Pa at `f64` — eight orders worse than the
  `1.49e-8` headline, because it is flat near the maximum. Taking a further 4.4
  digits off that is not a trade-off, it is a failure.

The general rule the table encodes: **a kernel whose accuracy floor is already
governed by cancellation must not be given a smaller epsilon.** Differentiation
and minimisation are both cancellation-limited by construction. Root finding
and quadrature are not.

## Why this is not implemented

`op-yvj.4.8` is a *selection policy over* an existing `CpuMulti` and an
existing `Gpu` path. The `Gpu` path does not exist. As of 2026-08-13 this crate
contains zero `.wgsl` shaders and zero `create_compute_pipeline` or
`dispatch_workgroups` calls; the only use of `wgpu` is `gpu_adapter_present()`
in `src/compute.rs`, which asks for an adapter so that `select_backend` can
decline it. All seven kernel families are `Serial` + `CpuMulti` only.

There is also no GPU adapter in the development container, so a GPU kernel
cannot be developed or checked against a reference here.

Nothing in this note should be read as describing working functionality. It
records a decision so that the decision does not have to be made under
implementation pressure later.
