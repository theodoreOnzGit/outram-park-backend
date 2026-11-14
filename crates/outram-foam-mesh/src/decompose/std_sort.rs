// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Algorithm: libstdc++ `std::sort` (bits/stl_algo.h, bits/stl_heap.h, GCC 11 to 14:
// `__introsort_loop`, `__unguarded_partition_pivot`, `__move_median_to_first`,
// `__final_insertion_sort`, `__partial_sort`/`__heap_select`/`__sort_heap`). GPL-3.0
// with the GCC Runtime Library Exception; the algorithm is reproduced statement by
// statement, no source text is copied.

//! libstdc++ `std::sort`, reproduced so that **ties are ordered as ESI OpenFOAM orders
//! them** (gh:#1381).
//!
//! `Foam::sort(list, comp)` is `std::sort` (`OpenFOAM/containers/Lists/List/UList.C:313`),
//! which is not stable. `simpleGeomDecomp` sorts cell indices by one coordinate; on a
//! structured mesh many cells tie, and which tied cell lands first decides which
//! processor takes it. Rust's `sort_unstable` is another introsort (pdqsort) with another
//! tie order, so the decomposition would differ from ESI's on exactly the meshes
//! decomposition is used on. This is the libstdc++ algorithm instead.

/// Below this many elements the introsort loop stops and insertion sort finishes.
const THRESHOLD: usize = 16;

/// `std::sort(idx.begin(), idx.end(), comp)` where `comp(a, b)` is the strict "less"
/// of the two *elements* (not positions).
pub fn std_sort_by<F: Fn(usize, usize) -> bool>(idx: &mut [usize], comp: F) {
    let n = idx.len();
    if n > 1 {
        introsort_loop(idx, 0, n, 2 * lg(n), &comp);
        final_insertion_sort(idx, &comp);
    }
}

/// `std::sort` with an explicit initial depth limit, so the heap-sort fallback of the
/// introsort can be reached by a test on a small list.
pub fn std_sort_by_with_depth<F: Fn(usize, usize) -> bool>(idx: &mut [usize], depth: usize, comp: F) {
    let n = idx.len();
    if n > 1 {
        introsort_loop(idx, 0, n, depth, &comp);
        final_insertion_sort(idx, &comp);
    }
}

/// `std::__lg`: floor of log2.
fn lg(n: usize) -> usize {
    (usize::BITS - 1 - n.leading_zeros()) as usize
}

fn introsort_loop<F: Fn(usize, usize) -> bool>(
    a: &mut [usize],
    first: usize,
    mut last: usize,
    mut depth: usize,
    comp: &F,
) {
    while last - first > THRESHOLD {
        if depth == 0 {
            partial_sort_all(a, first, last, comp);
            return;
        }
        depth -= 1;
        let cut = unguarded_partition_pivot(a, first, last, comp);
        introsort_loop(a, cut, last, depth, comp);
        last = cut;
    }
}

fn unguarded_partition_pivot<F: Fn(usize, usize) -> bool>(
    a: &mut [usize],
    first: usize,
    last: usize,
    comp: &F,
) -> usize {
    let mid = first + (last - first) / 2;
    move_median_to_first(a, first, first + 1, mid, last - 1, comp);
    unguarded_partition(a, first + 1, last, first, comp)
}

fn move_median_to_first<F: Fn(usize, usize) -> bool>(
    a: &mut [usize],
    result: usize,
    p: usize,
    q: usize,
    r: usize,
    comp: &F,
) {
    if comp(a[p], a[q]) {
        if comp(a[q], a[r]) {
            a.swap(result, q);
        } else if comp(a[p], a[r]) {
            a.swap(result, r);
        } else {
            a.swap(result, p);
        }
    } else if comp(a[p], a[r]) {
        a.swap(result, p);
    } else if comp(a[q], a[r]) {
        a.swap(result, r);
    } else {
        a.swap(result, q);
    }
}

fn unguarded_partition<F: Fn(usize, usize) -> bool>(
    a: &mut [usize],
    mut first: usize,
    mut last: usize,
    pivot: usize,
    comp: &F,
) -> usize {
    loop {
        while comp(a[first], a[pivot]) {
            first += 1;
        }
        last -= 1;
        while comp(a[pivot], a[last]) {
            last -= 1;
        }
        if first >= last {
            return first;
        }
        a.swap(first, last);
        first += 1;
    }
}

fn final_insertion_sort<F: Fn(usize, usize) -> bool>(a: &mut [usize], comp: &F) {
    let n = a.len();
    if n > THRESHOLD {
        insertion_sort(a, 0, THRESHOLD, comp);
        for i in THRESHOLD..n {
            unguarded_linear_insert(a, i, comp);
        }
    } else {
        insertion_sort(a, 0, n, comp);
    }
}

fn insertion_sort<F: Fn(usize, usize) -> bool>(a: &mut [usize], first: usize, last: usize, comp: &F) {
    if first == last {
        return;
    }
    for i in first + 1..last {
        if comp(a[i], a[first]) {
            let val = a[i];
            let mut j = i;
            while j > first {
                a[j] = a[j - 1];
                j -= 1;
            }
            a[first] = val;
        } else {
            unguarded_linear_insert(a, i, comp);
        }
    }
}

fn unguarded_linear_insert<F: Fn(usize, usize) -> bool>(a: &mut [usize], last: usize, comp: &F) {
    let val = a[last];
    let mut hole = last;
    while comp(val, a[hole - 1]) {
        a[hole] = a[hole - 1];
        hole -= 1;
    }
    a[hole] = val;
}

/// `std::__partial_sort(first, last, last)`: `__heap_select` (a `make_heap`, as the middle
/// is the end) then `__sort_heap`.
fn partial_sort_all<F: Fn(usize, usize) -> bool>(a: &mut [usize], first: usize, last: usize, comp: &F) {
    let h = &mut a[first..last];
    make_heap(h, comp);
    let mut end = h.len();
    while end > 1 {
        end -= 1;
        // pop_heap(first, end, end): value = *end; *end = *first; adjust_heap(0, end, value)
        let value = h[end];
        h[end] = h[0];
        adjust_heap(h, 0, end, value, comp);
    }
}

fn make_heap<F: Fn(usize, usize) -> bool>(h: &mut [usize], comp: &F) {
    let len = h.len();
    if len < 2 {
        return;
    }
    let mut parent = (len - 2) / 2;
    loop {
        let value = h[parent];
        adjust_heap(h, parent, len, value, comp);
        if parent == 0 {
            return;
        }
        parent -= 1;
    }
}

fn adjust_heap<F: Fn(usize, usize) -> bool>(
    h: &mut [usize],
    mut hole: usize,
    len: usize,
    value: usize,
    comp: &F,
) {
    let top = hole;
    let mut second = hole;
    while second < len.saturating_sub(1) / 2 {
        second = 2 * (second + 1);
        if comp(h[second], h[second - 1]) {
            second -= 1;
        }
        h[hole] = h[second];
        hole = second;
    }
    if len % 2 == 0 && second == (len - 2) / 2 {
        second = 2 * (second + 1);
        h[hole] = h[second - 1];
        hole = second - 1;
    }
    // push_heap
    while hole > top {
        let parent = (hole - 1) / 2;
        if !comp(h[parent], value) {
            break;
        }
        h[hole] = h[parent];
        hole = parent;
    }
    h[hole] = value;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(n: usize, modulus: usize) -> Vec<usize> {
        // deterministic pseudo-random keys with many ties
        let mut x = 12345usize;
        (0..n)
            .map(|_| {
                x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                (x >> 33) % modulus
            })
            .collect()
    }

    fn check_sorted(k: &[usize], idx: &[usize]) {
        for w in idx.windows(2) {
            assert!(k[w[0]] <= k[w[1]]);
        }
        let mut seen = idx.to_vec();
        seen.sort_unstable();
        assert!(seen.iter().enumerate().all(|(i, &v)| i == v), "a permutation");
    }

    #[test]
    fn sorts_every_size_with_ties_into_a_permutation() {
        for n in [0usize, 1, 2, 3, 15, 16, 17, 40, 200, 1000] {
            for m in [2usize, 5, 1_000_000] {
                let k = keys(n, m);
                let mut idx: Vec<usize> = (0..n).collect();
                std_sort_by(&mut idx, |a, b| k[a] < k[b]);
                check_sorted(&k, &idx);
            }
        }
    }

    /// The heap-sort fallback (depth limit exhausted) also sorts.
    #[test]
    fn heap_fallback_sorts() {
        for n in [17usize, 33, 100, 257] {
            let k = keys(n, 7);
            let mut idx: Vec<usize> = (0..n).collect();
            std_sort_by_with_depth(&mut idx, 0, |a, b| k[a] < k[b]);
            check_sorted(&k, &idx);
            let k = keys(n, 1_000_000);
            let mut idx: Vec<usize> = (0..n).collect();
            std_sort_by_with_depth(&mut idx, 1, |a, b| k[a] < k[b]);
            check_sorted(&k, &idx);
        }
    }

    #[test]
    fn lg_is_floor_log2() {
        assert_eq!(lg(1), 0);
        assert_eq!(lg(2), 1);
        assert_eq!(lg(3), 1);
        assert_eq!(lg(1024), 10);
        assert_eq!(lg(1025), 10);
    }
}
