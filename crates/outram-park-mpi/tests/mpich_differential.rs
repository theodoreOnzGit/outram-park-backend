//! Differential test of `outram-park-mpi` against MPICH 4.2.0 (gh:#1435, epic #1336).
//!
//! # Methodology
//!
//! `verification_and_validation/mpich_differential/upstream_driver/mpich_driver.c`
//! is compiled against an unmodified MPICH 4.2.0 (Ubuntu 24.04 `mpich
//! 4.2.0-5build3`, device `ch4:ucx`) and run with 2..=8 ranks. Every rank prints one
//! line per case; floating-point values are printed as the hex of their IEEE-754 bit
//! pattern. The output is stored in
//! `verification_and_validation/mpich_differential/refs/mpich_4.2.0_default.txt`
//! (regenerate with `scripts/run_mpich.sh`). This test runs the same cases with
//! [`outram_park_mpi::run`] and compares the lines **string for string**, i.e. bit for
//! bit for every floating-point value. Cases: point-to-point (wildcard status,
//! non-overtaking, selective matching), `reduce` (sum f64 at three roots, max/min/prod),
//! `all_reduce` (sum f64/f32, max/min f64), bcast/scatter/gather/allgather, group
//! algebra, `create_from_group`, `split`, `dup`, and 2-D Cartesian topologies. The test
//! values have exponents spread over 2^-78..2^78 so every floating-point sum rounds and
//! its association order is visible in the bits.
//!
//! Predictions were committed before the recorded runs:
//! `verification_and_validation/mpich_differential/PREDICTIONS.md`.
//!
//! # Pass criterion
//!
//! Every line equal. No tolerance exists in this test.
//!
//! # Results
//!
//! See `verification_and_validation/mpich_differential/README.md` for the table, the
//! old to new numbers and the cause analysis.

use outram_park_mpi::{run, Communicator, ReduceOp, ANY_SOURCE, ANY_TAG};

/// MPICH 4.2.0 default algorithms, test values with exponents spread over 2^-78..2^78
/// (a sum is dominated by the largest term: insensitive to association order).
const REF_WIDE: &str = include_str!(
    "../verification_and_validation/mpich_differential/refs/mpich_4.2.0_default_wide.txt"
);

/// MPICH 4.2.0 default algorithms, exponents within +-3 (order-sensitive values).
const REF_NARROW: &str = include_str!(
    "../verification_and_validation/mpich_differential/refs/mpich_4.2.0_default_narrow.txt"
);

/// `2^e` exactly, for |e| well inside the f64 exponent range.
fn pow2(e: i32) -> f64 {
    f64::from_bits(((1023 + e) as u64) << 52)
}

/// The driver's test value `mk(r, j)` (C: `ldexp` of an exact quotient).
fn mk(narrow: bool, r: i32, j: i32) -> f64 {
    let m = 1.0 + f64::from((r * 37 + j * 11) % 101) / 101.0;
    let e = if narrow {
        ((r * 13 + j * 29) % 7) - 3
    } else {
        (((r * 13 + j * 29) % 53) - 26) * 3
    };
    let v = m * pow2(e);
    if (r + j) % 2 != 0 {
        -v
    } else {
        v
    }
}

fn hex64(x: f64) -> String {
    format!("{:016x}", x.to_bits())
}

/// FNV-1a over the little-endian bytes of each value's bit pattern (the driver's `fnv`).
fn fnv(v: &[f64]) -> u64 {
    let mut h: u64 = 14695981039346656037;
    for x in v {
        for b in x.to_bits().to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(1099511628211);
        }
    }
    h
}

fn join<T: ToString>(v: &[T]) -> String {
    v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")
}

fn opt(x: Option<i32>) -> i32 {
    x.unwrap_or(-1)
}

type Slots = Vec<(&'static str, Option<String>)>;

fn case_p2p(comm: &Communicator, out: &mut Slots) {
    let n = comm.size();
    let me = comm.rank();
    let right = (me + 1) % n;
    let sb = [me * 10, me * 10 + 1, me * 10 + 2];
    let req = comm.irecv::<i32>(ANY_SOURCE, ANY_TAG).unwrap();
    comm.send(&sb, right, 40 + me).unwrap();
    let (buf, st) = req.wait().unwrap().unwrap();
    out.push((
        "p2p_ring",
        Some(format!(
            "buf={} src={} tag={} count={}",
            join(&buf),
            st.source,
            st.tag,
            st.count
        )),
    ));
    comm.barrier().unwrap(); // same test-race guard as the C driver
    if me == 1 {
        let tags = [7, 8, 7, 8];
        for (i, t) in tags.iter().enumerate() {
            comm.send(&[100 + i as i32], 0, *t).unwrap();
        }
        out.push(("p2p_order", None));
    } else if me == 0 {
        let (a, _) = comm.recv::<i32>(1, 8).unwrap();
        let (b, _) = comm.recv::<i32>(1, 7).unwrap();
        let (c, sc) = comm.recv::<i32>(1, ANY_TAG).unwrap();
        let (d, sd) = comm.recv::<i32>(1, ANY_TAG).unwrap();
        out.push((
            "p2p_order",
            Some(format!(
                "got={},{},{},{} ctag={} dtag={}",
                a[0], b[0], c[0], d[0], sc.tag, sd.tag
            )),
        ));
    } else {
        out.push(("p2p_order", None));
    }
}

fn case_reduce(comm: &Communicator, nw: bool, out: &mut Slots) {
    let n = comm.size();
    let me = comm.rank();
    for count in [1usize, 3] {
        for root in [0, n - 1, n / 2] {
            let sb: Vec<f64> = (0..count).map(|j| mk(nw, me, j as i32)).collect();
            let r = comm.reduce(&sb, ReduceOp::Sum, root).unwrap();
            let line = r.map(|v| {
                let hex: Vec<String> = v.iter().map(|x| hex64(*x)).collect();
                format!("root={root} count={count} {}", hex.join(" "))
            });
            out.push(("reduce_sum_f64", line));
        }
    }
    let sb40: Vec<f64> = (0..40).map(|j| mk(nw, me, j)).collect();
    let r = comm.reduce(&sb40, ReduceOp::Sum, n - 1).unwrap();
    out.push((
        "reduce_sum_f64_c40",
        r.map(|v| format!("root={} count=40 fnv={:016x}", n - 1, fnv(&v))),
    ));
    let ia = me * 3 - 4;
    let r = comm.reduce(&[ia], ReduceOp::Max, 0).unwrap();
    out.push(("reduce_max_i32", r.map(|v| format!("max_i32={}", v[0]))));
    let r = comm.reduce(&[ia], ReduceOp::Min, 1).unwrap();
    out.push(("reduce_min_i32", r.map(|v| format!("min_i32={}", v[0]))));
    let r = comm
        .reduce(&[2 + i64::from(me)], ReduceOp::Product, 0)
        .unwrap();
    out.push(("reduce_prod_i64", r.map(|v| format!("prod_i64={}", v[0]))));
    let da = mk(nw, me, 5);
    let mx = comm.all_reduce(&[da], ReduceOp::Max).unwrap();
    let mn = comm.all_reduce(&[da], ReduceOp::Min).unwrap();
    out.push((
        "allreduce_maxmin_f64",
        Some(format!("max={} min={}", hex64(mx[0]), hex64(mn[0]))),
    ));
}

fn case_allreduce(comm: &Communicator, nw: bool, out: &mut Slots) {
    let me = comm.rank();
    for count in [1usize, 3] {
        let sb: Vec<f64> = (0..count).map(|j| mk(nw, me, j as i32)).collect();
        let r = comm.all_reduce(&sb, ReduceOp::Sum).unwrap();
        let hex: Vec<String> = r.iter().map(|x| hex64(*x)).collect();
        out.push((
            "allreduce_sum_f64",
            Some(format!("count={count} {}", hex.join(" "))),
        ));
    }
    let sb40: Vec<f64> = (0..40).map(|j| mk(nw, me, j)).collect();
    let r = comm.all_reduce(&sb40, ReduceOp::Sum).unwrap();
    out.push((
        "allreduce_sum_f64_c40",
        Some(format!("count=40 fnv={:016x}", fnv(&r))),
    ));
    let sb: Vec<f32> = (0..2).map(|j| mk(nw, me, j) as f32).collect();
    let r = comm.all_reduce(&sb, ReduceOp::Sum).unwrap();
    out.push((
        "allreduce_sum_f32",
        Some(format!("{:08x} {:08x}", r[0].to_bits(), r[1].to_bits())),
    ));
}

fn case_movement(comm: &Communicator, out: &mut Slots) {
    let n = comm.size();
    let me = comm.rank();
    let root = n - 1;
    let b = if me == root {
        comm.broadcast(Some(&[11i32, 22, 33]), root).unwrap()
    } else {
        comm.broadcast::<i32>(None, root).unwrap()
    };
    out.push(("bcast_i32", Some(join(&b))));

    let all: Vec<i32> = (0..n).flat_map(|i| [1000 + i, 2000 + i]).collect();
    let sroot = 1 % n;
    let mine = if me == sroot {
        comm.scatter(Some(&all), 2, sroot).unwrap()
    } else {
        comm.scatter::<i32>(None, 2, sroot).unwrap()
    };
    out.push(("scatter_i32", Some(join(&mine))));

    let o = [me * 7, me * 7 + 1];
    let g = comm.gather(&o, 2 % n).unwrap();
    out.push(("gather_i32", g.map(|v| join(&v))));
    let ag = comm.all_gather(&o).unwrap();
    out.push(("allgather_i32", Some(join(&ag))));
    comm.barrier().unwrap();
}

fn wl(g: &outram_park_mpi::Group) -> String {
    format!("[{}]", join(g.world_ranks()))
}

fn case_group(comm: &Communicator, out: &mut Slots) {
    let n = comm.size();
    if n < 4 {
        return;
    }
    let wg = comm.group();
    let ga = wg.incl(&[n - 1, 0, 2 % n, 1]);
    let gb = wg.incl(&[1, n - 1, 0]);
    let gu = ga.union(&gb);
    let gi = ga.intersection(&gb);
    let gd = ga.difference(&gb);
    let ge = wg.excl(&[1, n - 1]);
    let gx = gb.union(&ga);
    let line = format!(
        "A={} B={} A|B={} B|A={} A&B={} A-B={} excl={}",
        wl(&ga),
        wl(&gb),
        wl(&gu),
        wl(&gx),
        wl(&gi),
        wl(&gd),
        wl(&ge)
    );
    out.push(("group_algebra", (comm.rank() == 0).then_some(line)));
    let tr = ga.translate_ranks(&[0, 1, 2, 3], &gb);
    let trs: Vec<String> = tr
        .iter()
        .map(|x| x.map_or("-32766".to_string(), |v| v.to_string()))
        .collect();
    out.push((
        "group_translate",
        (comm.rank() == 0).then(|| format!("tr={}", trs.join(","))),
    ));
    let sub = comm.create_from_group(&gu).unwrap();
    let line = match sub {
        Some(c) => {
            let sum = c.all_reduce(&[comm.rank()], ReduceOp::Sum).unwrap();
            format!(
                "newrank={} newsize={} sum_world={}",
                c.rank(),
                c.size(),
                sum[0]
            )
        }
        None => "null".to_string(),
    };
    out.push(("comm_create", Some(line)));
}

fn case_split(comm: &Communicator, out: &mut Slots) {
    let n = comm.size();
    let me = comm.rank();
    let color = if me % 3 == 2 {
        Communicator::UNDEFINED
    } else {
        me % 3
    };
    let key = (n - me) % 3;
    let line = match comm.split(color, key).unwrap() {
        Some(c) => {
            let sum = c.all_reduce(&[me], ReduceOp::Sum).unwrap();
            format!(
                "color={color} key={key} newrank={} newsize={} sum_world={}",
                c.rank(),
                c.size(),
                sum[0]
            )
        }
        None => "null".to_string(),
    };
    out.push(("comm_split", Some(line)));
    let d = comm.dup().unwrap();
    out.push((
        "comm_dup",
        Some(format!("rank={} size={}", d.rank(), d.size())),
    ));
}

fn case_cart(comm: &Communicator, out: &mut Slots) {
    let n = comm.size();
    let shapes = [[2, 3], [3, 2], [2, 2]];
    let pers = [[true, false], [false, true], [true, true]];
    for k in 0..3 {
        let dims = shapes[k];
        let per = pers[k];
        if dims[0] * dims[1] > n {
            out.push(("cart", None));
            continue;
        }
        let line = match comm.cart_create(&dims, &per).unwrap() {
            None => format!("shape={}x{} null", dims[0], dims[1]),
            Some(cc) => {
                let co = cc.my_coords();
                let (s0, d0) = cc.shift(0, 1);
                let (s1, d1) = cc.shift(1, 1);
                let (s2, d2) = cc.shift(1, -2);
                let off = [co[0] - 1, co[1] + 2];
                let ok = (0..2).all(|i| per[i] || (off[i] >= 0 && off[i] < dims[i]));
                let rr = if ok { opt(cc.rank(&off)) } else { -1 };
                format!(
                    "shape={}x{} per={}{} coords={},{} sh0={},{} sh1={},{} sh1m2={},{} rank_off={}",
                    dims[0],
                    dims[1],
                    i32::from(per[0]),
                    i32::from(per[1]),
                    co[0],
                    co[1],
                    opt(s0),
                    opt(d0),
                    opt(s1),
                    opt(d1),
                    opt(s2),
                    opt(d2),
                    rr
                )
            }
        };
        out.push(("cart", Some(line)));
    }
}

/// All lines the port produces for `n` ranks, in the driver's order.
fn port_lines(n: i32, nw: bool) -> Vec<String> {
    let per_rank: Vec<Slots> = run(n, |comm| {
        let mut out: Slots = Vec::new();
        case_p2p(comm, &mut out);
        case_reduce(comm, nw, &mut out);
        case_allreduce(comm, nw, &mut out);
        case_movement(comm, &mut out);
        case_group(comm, &mut out);
        case_split(comm, &mut out);
        case_cart(comm, &mut out);
        out
    })
    .unwrap();
    let slots = per_rank[0].len();
    let mut lines = Vec::new();
    for s in 0..slots {
        for (r, ranks_slots) in per_rank.iter().enumerate() {
            let (name, f) = &ranks_slots[s];
            if let Some(f) = f {
                lines.push(format!("{name} n={n} r={r} {f}"));
            }
        }
    }
    lines
}

fn ref_lines(text: &str, n: i32) -> Vec<String> {
    let tag = format!(" n={n} ");
    text.lines()
        .filter(|l| l.contains(&tag))
        .map(str::to_string)
        .collect()
}

fn case_of(line: &str) -> &str {
    line.split(' ').next().unwrap()
}

/// Per-case comparison of the port against one MPICH reference file.
/// Returns `(identical, total, differing lines)` for every case name.
fn compare(text: &str, nw: bool) -> Vec<(String, usize, usize, Vec<String>)> {
    let mut table: Vec<(String, usize, usize, Vec<String>)> = Vec::new();
    for n in 2..=8 {
        let port = port_lines(n, nw);
        let up = ref_lines(text, n);
        assert_eq!(
            port.len(),
            up.len(),
            "n={n}: the port printed {} lines, MPICH {}",
            port.len(),
            up.len()
        );
        for (p, u) in port.iter().zip(up.iter()) {
            let name = case_of(u).to_string();
            let idx = match table.iter().position(|t| t.0 == name) {
                Some(i) => i,
                None => {
                    table.push((name.clone(), 0, 0, Vec::new()));
                    table.len() - 1
                }
            };
            table[idx].2 += 1;
            if p == u {
                table[idx].1 += 1;
            } else {
                table[idx].3.push(format!("  port: {p}\n  mpich: {u}"));
            }
        }
    }
    table
}

fn assert_port_matches(text: &str, nw: bool) {
    let table = compare(text, nw);
    let mut failed = false;
    for (name, same, total, diffs) in &table {
        println!("{name:<24} {same:>3} / {total:<3} identical");
        if same != total {
            failed = true;
            for d in diffs.iter().take(3) {
                println!("{d}");
            }
        }
    }
    assert!(!failed, "the port differs from MPICH 4.2.0; see the table above");
}

#[test]
fn port_matches_mpich_4_2_0_wide_values() {
    assert_port_matches(REF_WIDE, false);
}

#[test]
fn port_matches_mpich_4_2_0_narrow_values() {
    assert_port_matches(REF_NARROW, true);
}

/// Model of `MPIR_Allreduce_intra_recursive_doubling` (MPICH `src/mpi/coll/allreduce/`,
/// identical algorithm in v3.3 through v4.3.2) reduced to the f64 sum of one lane:
/// returns the value every rank ends with. Pure arithmetic, no MPI, so it is an
/// independent statement of the association order.
fn model_recursive_doubling_sum(x: &[f64]) -> Vec<f64> {
    let n = x.len();
    let mut pof2 = 1;
    while pof2 * 2 <= n {
        pof2 *= 2;
    }
    let rem = n - pof2;
    let mut val = x.to_vec();
    let mut newrank = vec![-1i64; n];
    for rank in 0..n {
        if rank < 2 * rem {
            if rank % 2 == 1 {
                val[rank] += val[rank - 1]; // tmp op recvbuf: addition commutes bitwise
                newrank[rank] = (rank / 2) as i64;
            }
        } else {
            newrank[rank] = (rank - rem) as i64;
        }
    }
    let mut mask = 1;
    while mask < pof2 {
        let before = val.clone();
        for rank in 0..n {
            if newrank[rank] >= 0 {
                let newdst = (newrank[rank] as usize) ^ mask;
                let dst = if newdst < rem { newdst * 2 + 1 } else { newdst + rem };
                val[rank] = before[dst] + before[rank];
            }
        }
        mask <<= 1;
    }
    for rank in 0..2 * rem {
        if rank % 2 == 0 {
            val[rank] = val[rank + 1];
        }
    }
    val
}

#[test]
fn recursive_doubling_model_reproduces_mpich_allreduce_bits() {
    // The cause of any all_reduce difference is the association order: this model of
    // MPICH's recursive doubling must reproduce MPICH's own bits for every rank.
    for n in 2..=8usize {
        for j in [0i32, 1, 2] {
            let x: Vec<f64> = (0..n).map(|r| mk(true, r as i32, j)).collect();
            let model = model_recursive_doubling_sum(&x);
            let count = if j == 0 { 1 } else { 3 };
            let up = ref_lines(REF_NARROW, n as i32);
            for (r, m) in model.iter().enumerate() {
                let prefix = format!("allreduce_sum_f64 n={n} r={r} count={count} ");
                let line = up.iter().find(|l| l.starts_with(&prefix)).unwrap();
                let words: Vec<&str> = line[prefix.len()..].split(' ').collect();
                let lane = if count == 1 { 0 } else { j as usize };
                assert_eq!(words[lane], hex64(*m), "n={n} rank={r} lane={j}");
            }
        }
    }
}

/// Model of the port's original `all_reduce` (binomial `reduce` onto rank 0, then
/// `broadcast`): the value every rank ended with. Used only to count how often the two
/// association orders disagree in the bits; the live port is compared against MPICH above.
fn model_binomial_sum(x: &[f64]) -> f64 {
    let n = x.len();
    let mut val = x.to_vec();
    let mut mask = 1;
    while mask < n {
        for rel in 0..n {
            if rel % (2 * mask) == 0 && rel + mask < n {
                val[rel] = val[rel + mask] + val[rel];
            }
        }
        mask <<= 1;
    }
    val[0]
}

#[test]
fn recursive_doubling_model_reproduces_mpich_40_lane_hash_and_orders_differ() {
    let mut differing_orders = 0;
    let mut total = 0;
    for n in 2..=8usize {
        let up = ref_lines(REF_NARROW, n as i32);
        let mut rd_lanes = vec![0.0; 40];
        for (j, lane) in rd_lanes.iter_mut().enumerate() {
            let x: Vec<f64> = (0..n).map(|r| mk(true, r as i32, j as i32)).collect();
            let rd = model_recursive_doubling_sum(&x);
            assert!(rd.iter().all(|v| v.to_bits() == rd[0].to_bits()));
            *lane = rd[0];
            total += 1;
            if model_binomial_sum(&x).to_bits() != rd[0].to_bits() {
                differing_orders += 1;
            }
        }
        let want = format!("fnv={:016x}", fnv(&rd_lanes));
        for r in 0..n {
            let prefix = format!("allreduce_sum_f64_c40 n={n} r={r} count=40 ");
            let line = up.iter().find(|l| l.starts_with(&prefix)).unwrap();
            assert_eq!(&line[prefix.len()..], want, "n={n} rank={r}");
        }
    }
    println!(
        "binomial vs recursive-doubling association: {differing_orders} of {total} (n, lane) sums differ in the bits"
    );
    assert!(differing_orders > 0, "the test values must be able to expose the order");
}
