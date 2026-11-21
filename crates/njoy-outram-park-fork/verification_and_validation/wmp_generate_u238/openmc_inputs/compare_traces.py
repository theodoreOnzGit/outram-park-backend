"""Compare an OpenMC trace (trace_openmc.py) with a port trace (generate/trace.rs), order by order (gh:#1390).

Usage: python compare_traces.py <openmc prefix> <port prefix> [--rows]
Prints: both summaries; the agreement at iteration 1 of each order (initial poles are the same
deterministic linspace, so no chaos has accumulated yet); the per-order best (min points outside on the
test grid, max quality); the iteration at which the two codes' pole sets first differ by more than 1e-6
(relative) at each order; the orders each code visited.
"""
import sys, json
import numpy as np


def load(prefix):
    keys = None
    raw = []
    for line in open(prefix + ".log.tsv"):
        f = line.rstrip("\n").split("\t")
        if keys is None:
            keys = f
            continue
        raw.append(dict(zip(keys, f)))
    summ = json.load(open(prefix + ".summary.txt"))
    # The OpenMC trace logs `order` = the size of new_poles, which is order + n_real (a real pole is split
    # into two); every order runs n_vf_iter = 30 iterations (the last one may stop early at `found_ideal`),
    # so the true order of row r is first_order + 2 * (r // 30). The port trace logs the true order.
    openmc = summ["code"] == "openmc"
    rows = {}
    sizes = []
    for r, d in enumerate(raw):
        o = summ["first_order"] + 2 * (r // 30) if openmc else int(d["order"])
        it = r % 30 + 1 if openmc else int(d["it"])
        sizes.append(int(d["order"]) if openmc else int(d["npoles"]))
        rows[(o, it)] = dict(
            ratio=float(d["ratio"]), ratio2=float(d["ratio2"]), maxre=float(d["maxre"]), quality=float(d["quality"]),
            n_out=int(d["n_out"]), accepted=int(d["accepted"]), n_real=int(d["n_real"]))
    poles = np.fromfile(prefix + ".poles.f64", dtype="<f8")
    off = 0
    P = {}
    for (k, n) in zip(rows, sizes):
        z = poles[off:off + 2 * n].reshape(-1, 2)
        zz = z[:, 0] + 1j * z[:, 1]
        raw, i = [], 0  # collapse each split pair p(1 +- 0.01j) back to the real pole (the raw relocation output)
        while i < zz.size:
            if i + 1 < zz.size and zz[i].imag != 0 and zz[i].imag == zz[i].real * 0.01 and zz[i + 1].real == zz[i].real \
                    and zz[i + 1].imag == -(zz[i].real * 0.01):
                raw.append(complex(zz[i].real, 0.0))
                i += 2
            else:
                raw.append(zz[i])
                i += 1
        P[k] = np.array(raw)
        off += 2 * n
    assert off == poles.size, (off, poles.size)
    return rows, P, summ


def key(z):
    return np.lexsort((z.imag, z.real))


a_rows, a_poles, a_sum = load(sys.argv[1])
b_rows, b_poles, b_sum = load(sys.argv[2])
show_rows = "--rows" in sys.argv
for name, s in (("OpenMC", a_sum), ("port", b_sum)):
    print(f"{name}: " + ", ".join(f"{k}={s[k]}" for k in ("n_peaks", "first_order", "last_order_visited", "orders_visited", "stop", "final_poles", "grid_points_outside", "seconds")))
orders_a = sorted({o for o, _ in a_rows})
orders_b = sorted({o for o, _ in b_rows})
common = [o for o in orders_a if o in set(orders_b)]
print(f"orders visited: OpenMC {orders_a[0]}..{orders_a[-1]} ({len(orders_a)}), port {orders_b[0]}..{orders_b[-1]} ({len(orders_b)}), common {len(common)}")


def its(rows, o):
    return sorted(i for (oo, i) in rows if oo == o)


print("\nIteration 1 of each common order (same deterministic initial poles):")
dq, dpole, same_n_out, same_acc = [], [], 0, 0
for o in common:
    ra, rb = a_rows[(o, 1)], b_rows[(o, 1)]
    dq.append(abs(ra["quality"] - rb["quality"]) if np.isfinite(ra["quality"]) and np.isfinite(rb["quality"]) else (0.0 if ra["quality"] == rb["quality"] else np.inf))
    pa, pb = a_poles[(o, 1)], b_poles[(o, 1)]
    ia, ib = key(pa), key(pb)
    dpole.append(float(np.max(np.abs(pa[ia] - pb[ib]) / np.abs(pa[ia]))))
    same_n_out += int(ra["n_out"] == rb["n_out"])
    if show_rows:
        print(f"  order {o}: quality {ra['quality']:.6f} vs {rb['quality']:.6f}, n_out {ra['n_out']} vs {rb['n_out']}, poles max rel diff {dpole[-1]:.2e}")
dq, dpole = np.array(dq), np.array(dpole)
print(f"  |dquality| <= 1e-9: {int(np.sum(dq <= 1e-9))}/{len(dq)}, <= 1e-6: {int(np.sum(dq <= 1e-6))}/{len(dq)}, <= 1e-3: {int(np.sum(dq <= 1e-3))}/{len(dq)}; max {dq.max():.3e}, median {np.median(dq):.3e}")
print(f"  poles max rel diff <= 1e-9: {int(np.sum(dpole <= 1e-9))}/{len(dpole)}, <= 1e-6: {int(np.sum(dpole <= 1e-6))}/{len(dpole)}, <= 1e-3: {int(np.sum(dpole <= 1e-3))}/{len(dpole)}; median {np.median(dpole):.3e}, max {dpole.max():.3e}")
print(f"  identical test-grid points outside (n_out): {same_n_out}/{len(common)}")

print("\nDrift inside an order: first iteration at which the pole sets differ by more than 1e-6 (relative); '-' = never over the common iterations")
firsts = []
for o in common:
    ni = min(len(its(a_rows, o)), len(its(b_rows, o)))
    first = None
    for i in range(1, ni + 1):
        pa, pb = a_poles[(o, i)], b_poles[(o, i)]
        ia, ib = key(pa), key(pb)
        if float(np.max(np.abs(pa[ia] - pb[ib]) / np.abs(pa[ia]))) > 1e-6:
            first = i
            break
    firsts.append(first)
vals = [f for f in firsts if f is not None]
print(f"  orders whose pole sets drift past 1e-6: {len(vals)}/{len(common)}; first-drift iteration median {np.median(vals) if vals else '-'}, min {min(vals) if vals else '-'}, max {max(vals) if vals else '-'}")

print("\nPer order: best (fewest test-grid points outside) over the iterations of that order, and the accepted-so-far quality")
print("order | OpenMC iters, best n_out, best quality | port iters, best n_out, best quality")
for o in common:
    ia, ib = its(a_rows, o), its(b_rows, o)
    ba = min(a_rows[(o, i)]["n_out"] for i in ia)
    bb = min(b_rows[(o, i)]["n_out"] for i in ib)
    qa = max(a_rows[(o, i)]["quality"] for i in ia)
    qb = max(b_rows[(o, i)]["quality"] for i in ib)
    print(f"{o:4d} | {len(ia):2d} {ba:6d} {qa:9.5f} | {len(ib):2d} {bb:6d} {qb:9.5f}")
