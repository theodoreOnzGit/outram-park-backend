# Comment for https://github.com/vorot/roots/issues/35

Issue #35 is open and titled *"find_roots_sturm misses a root of a quartic"*.
This finding generalises it, so it belongs there as a comment rather than as a
new issue. If #35 turns out to be about something narrower, post it as a new
issue instead — the text works either way.

---

Before the report — thank you.

`roots` has been a real dependency of my work, not a passing reference.
`tuas_boussinesq_solver`, the thermal-hydraulics solver I built for my PhD and
the TUAS paper, uses `find_root_brent` and `SimpleConvergency` across 13 source
files, plus `find_root_regula_falsi` and `find_root_inverse_quadratic`. The
bracketing solvers have been completely dependable, and having them available
with no dependencies at all is exactly why I could use them where I did.

I have a lot of respect for the fact that this was built by hand and carefully,
without AI assistance. That was groundwork the ecosystem was thin on, and it
mattered.

Which is also why I want to report this precisely.

## This is not specific to one quartic

`find_roots_sturm` loses roots at **every** degree from 4 upward, and never
returns more than three. On `prod (x - k)` for `k = 1..n`, whose roots are the
integers `1..n` and are as well separated as a test case can be:

| degree | expected | `find_roots_sturm` | `find_roots_eigen` |
|---|---|---|---|
| 2 | 2 | 2 | 2 |
| 3 | 3 | 3 | 3 |
| 4 | 4 | **3** | 4 |
| 5 | 5 | **3** | 5 |
| 6 | 6 | **3** | 6 |
| 7 | 7 | **1** | 7 |
| 8 | 8 | **3** | 8 |
| 9 | 9 | **1** | 9 |
| 10 | 10 | **1** | 10 |

**Every shortfall is silent.** The returned `Vec` contains no `Err` entry, so a
caller has no way to distinguish a complete answer from a partial one — the
count is simply smaller than the degree, which is also what a polynomial with
complex roots legitimately looks like.

`find_roots_eigen`, on identical inputs, returns every root at every degree.

## Reproduction

`roots = "=0.0.8"`, no other dependencies:

```rust
use roots::{find_roots_eigen, find_roots_sturm, SimpleConvergency};

/// prod (x - k) for k = 1..=n, as monic descending coefficients WITHOUT the
/// leading 1 — the convention `find_roots_sturm` documents.
fn monic_descending(n: usize) -> Vec<f64> {
    let mut c = vec![1.0f64];
    for k in 1..=n {
        let mut next = vec![0.0f64; c.len() + 1];
        for (i, &v) in c.iter().enumerate() {
            next[i] += v;
            next[i + 1] -= v * k as f64;
        }
        c = next;
    }
    c[1..].to_vec()
}

fn main() {
    println!("degree | expected | find_roots_sturm | find_roots_eigen");
    for n in 2..=10usize {
        let desc = monic_descending(n);

        let mut conv = SimpleConvergency { eps: 1e-12f64, max_iter: 100usize };
        let sturm = find_roots_sturm(&desc, &mut conv);
        let ok = sturm.iter().filter(|r| r.is_ok()).count();
        let err = sturm.iter().filter(|r| r.is_err()).count();

        // find_roots_eigen takes the ASCENDING form of the same polynomial.
        let mut asc = desc.clone();
        asc.reverse();
        let eigen = find_roots_eigen(asc).len();

        println!("  {n:2}   |    {n:2}    |  {ok:2} ({err} errors)  |  {eigen:2}");
    }
}
```

Output:

```
degree | expected | find_roots_sturm | find_roots_eigen
   2   |     2    |   2 (0 errors)  |   2
   3   |     3    |   3 (0 errors)  |   3
   4   |     4    |   3 (0 errors)  |   4
   5   |     5    |   3 (0 errors)  |   5
   6   |     6    |   3 (0 errors)  |   6
   7   |     7    |   1 (0 errors)  |   7
   8   |     8    |   3 (0 errors)  |   8
   9   |     9    |   1 (0 errors)  |   9
  10   |    10    |   1 (0 errors)  |  10
```

I have not tried to isolate the cause beyond confirming it is not the
convergency settings — the same pattern holds at `eps = 1e-8` and
`max_iter = 1000`. Since `find_root_intervals` recurses into `find_roots_sturm`
on the derivative polynomial, one plausible reading is that the recursion's own
incompleteness propagates upward, but I have not verified that and would not
want to send you down a wrong path.

## Separately, and much more minor: the two functions take opposite conventions

`find_roots_sturm` documents *"the normalized polynomial `x^n + a[0]*x^(n-1) +
... + a[n-1]`"* — descending. `find_roots_eigen` documents *"the normalized
polynomial"* with no ordering stated, and its doctest (`find_roots_eigen(vec![0,
-1, 0])` for `x^3 - x`) shows it is **ascending**.

Both are monic with an implicit leading 1, and the two are reverses of each
other, so passing one function's input to the other produces confident,
completely wrong output rather than an error. I hit this myself and spent a
measurement cycle concluding `find_roots_eigen` was broken before realising it
was my mistake. Looking back through the issue list, #25
(*"find_roots_eigen Not Working for Polynomial of Degree 4 or More"*, since
closed) may well have been the same thing.

A single line in `find_roots_eigen`'s doc comment saying the coefficients are
ascending would close that off permanently.

## Disclosure

I have ported `src/analytical/quartic.rs` and its dependencies, and
`src/numerical/eigen.rs`, into a `no_std` crate in my own workspace — GPL-3.0,
with your BSD-2-Clause notice and copyright retained in the file headers, and
the flow marked one-way.

The `eigen.rs` port carries its full chain in the header: Martin and Wilkinson's
Algol `hqr2`/`orthes`, EISPACK, JAMA, Stepan Yakovenko's hand transpilation —
whose own header asks that he be mentioned in the source code, so he is — and
yourself for taking it in. My port is bit-identical to yours across all 71 real
roots in my reference set, which was a pleasant thing to be able to verify.

I have deliberately **not** ported `find_roots_sturm`, for the reason above, and
the decision is pinned by a test so that if this is ever fixed upstream I will
notice rather than inherit the choice.
