# Issue for https://github.com/Axect/Peroxide/issues/new

**Title:** Three incorrect Gauss-Legendre node values in `LEGENDRE_ROOT_11` and `LEGENDRE_ROOT_12`

---

Before the bug report — thank you.

Peroxide has been a real dependency of my work, not a passing reference.
`tuas_boussinesq_solver`, the thermal-hydraulics solver I built for my PhD and
the TUAS paper, uses `CubicSpline`/`Spline` across 26 source files, plus
`erfc` and `Calculus`. Having a solid, well-shaped numerical stack available in
pure Rust is what made it possible to do that work in Rust at all.

I have a lot of respect for the fact that this was built by hand and carefully,
without AI assistance. That was groundwork the ecosystem was thin on, and it
mattered — it is a large part of why "just use the Rust crate" was a sentence I
could say when I started.

Which is also why I want to report this precisely.

## Summary

In `src/numerical/integral.rs`, three tabulated Gauss-Legendre **node** values
are incorrect. Each appears twice (once per sign), so six entries are affected.
All the **weights** are correct, at every order including 11 and 12.

| order | current | correct | absolute error |
|---|---|---|---|
| 11 | `0.519096129110681` | `0.5190961292068118` | 9.6e-11 |
| 12 | `0.36783149891818` | `0.3678314989981802` | 8.0e-11 |
| 12 | `0.125333408511469` | `0.1252334085114689` | **1.0e-04** |

In 0.41.2 these are at lines 754, 758 (`LEGENDRE_ROOT_11`) and 768–771
(`LEGENDRE_ROOT_12`).

The third is the significant one, and it looks like a plain single-digit typo:
a `2` became a `3` in the fourth decimal place, with every remaining digit
correct.

## Impact

`Integral::GaussLegendre(12)` loses roughly ten significant figures, with
nothing in the result to indicate it. Order 11 is affected at the 1e-11 level —
milder, but still far below what an order-11 Gauss rule should deliver.

```rust
use peroxide::fuga::*;

fn main() {
    // Gauss-Legendre(12) is exact to degree 23, so each of these should come
    // back at machine precision.
    for (p, exact) in [(2i32, 2.0 / 3.0), (10, 2.0 / 11.0), (22, 2.0 / 23.0)] {
        let v: f64 = integrate(|x: f64| x.powi(p), (-1f64, 1f64),
                               Integral::GaussLegendre(12));
        println!("x^{p:<2}  rel.err {:.3e}", ((v - exact) / exact).abs());
    }
}
```

Observed, with the current table:

```
x^2   rel.err 1.873e-05
x^10  rel.err 2.059e-11
x^22  rel.err 9.576e-16
```

With the three values corrected, all three drop to ~1e-16.

**The `x^22` line is worth a moment, because it explains how this survived.**
The badly wrong node is at `x ≈ 0.125`, near the centre of the interval. A
high-power test integrand puts almost all its weight on the outer nodes, where
the table is correct, so the error hides completely. Low-degree integrands are
what expose it. I very nearly filed this with `x^22` as the reproduction, which
would have looked like a false alarm.

## How it was found

Evaluate `P_n` and `P_n'` by Bonnet's three-term recurrence — stable at every
order — and Newton-polish each tabulated node. A node that is genuinely a root
of `P_n` does not move; these three move by the amounts above. Independently,
all three corrected values agree digit-for-digit with the standard published
Gauss-Legendre tables.

Every other value in the set — orders 2 through 30, nodes and weights, 928
values — checks out.

One caveat that cost me a detour and may save you one: if you verify the
weights with `w_k = 2 / ((1 - x_k^2) * P_n'(x_k)^2)` evaluated at the
*tabulated* node, the bad nodes make the corresponding weights look wrong too.
They are not. Evaluate at the polished node.

## Disclosure

I have ported these tables, along with `structure/polynomial.rs` and the two
quadrature routines, into a `no_std` crate in my own workspace — GPL-3.0, taken
under your MIT option, with your copyright and the MIT notice retained in the
file headers. The corrections above are applied there and documented as
deliberate deviations from upstream.

The audit is a single self-contained test file with no dependencies. I am happy
to open a PR with the six-value data fix, the test, or both — whichever is more
useful to you.
