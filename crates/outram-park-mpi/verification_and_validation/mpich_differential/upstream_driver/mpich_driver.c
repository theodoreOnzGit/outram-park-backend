/* Differential driver for outram-park-mpi (gh:#1435).
 *
 * Runs against an UNMODIFIED MPICH (mpicc from the MPICH build). Every case
 * prints one line per rank, in rank order, to stdout of rank 0:
 *     <case> n=<N> r=<rank> <fields>
 * The Rust side (tests/mpich_differential.rs) builds the identical lines with
 * outram_park_mpi::run(N, ..) and compares them with this driver's output,
 * stored in refs/. Floating-point values are printed as the 16 hex digits of
 * their IEEE-754 bit pattern so the comparison is bit for bit.
 *
 * Test values: mk(r,j) is built only from integer arithmetic, one IEEE division
 * and an exact power-of-two scale (ldexp), so C and Rust produce the same bits.
 */
#include <mpi.h>
#include <math.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define LINE 240

static int world_n, world_r;

static void emit(const char *name, const char *fields) {
    char line[LINE];
    memset(line, 0, LINE);
    if (fields)
        snprintf(line, LINE, "%s n=%d r=%d %s", name, world_n, world_r, fields);
    char *all = NULL;
    if (world_r == 0) all = (char *)calloc((size_t)world_n, LINE);
    MPI_Gather(line, LINE, MPI_CHAR, all, LINE, MPI_CHAR, 0, MPI_COMM_WORLD);
    if (world_r == 0) {
        for (int i = 0; i < world_n; i++)
            if (all[i * LINE]) puts(all + (size_t)i * LINE);
        free(all);
    }
}

static uint64_t bits(double x) { uint64_t u; memcpy(&u, &x, 8); return u; }

/* order-sensitive test value: sign alternates, magnitude spread over 2^-78..2^78 so that
 * every floating-point sum rounds and its association order shows in the bits */
static double mk(int r, int j) {
    double m = 1.0 + (double)((r * 37 + j * 11) % 101) / 101.0;
    int e = (((r * 13 + j * 29) % 53) - 26) * 3;
    double v = ldexp(m, e);
    return ((r + j) % 2) ? -v : v;
}

static void cat(char *buf, const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    size_t l = strlen(buf);
    vsnprintf(buf + l, LINE - l, fmt, ap);
    va_end(ap);
}

static int pn(int x) { return x == MPI_PROC_NULL ? -1 : x; }

static void case_p2p(void) {
    char f[LINE];
    int right = (world_r + 1) % world_n;
    int sb[3] = {world_r * 10, world_r * 10 + 1, world_r * 10 + 2}, rb[3] = {-1, -1, -1};
    MPI_Request rq;
    MPI_Irecv(rb, 3, MPI_INT, MPI_ANY_SOURCE, MPI_ANY_TAG, MPI_COMM_WORLD, &rq);
    MPI_Send(sb, 3, MPI_INT, right, 40 + world_r, MPI_COMM_WORLD);
    MPI_Status st;
    MPI_Wait(&rq, &st);
    int cnt;
    MPI_Get_count(&st, MPI_INT, &cnt);
    snprintf(f, LINE, "buf=%d,%d,%d src=%d tag=%d count=%d", rb[0], rb[1], rb[2], st.MPI_SOURCE, st.MPI_TAG, cnt);
    emit("p2p_ring", f);

    /* non-overtaking + selective matching: rank 1 -> rank 0 */
    if (world_r == 1) {
        int t[4] = {7, 8, 7, 8}, v[4] = {100, 101, 102, 103};
        for (int i = 0; i < 4; i++) MPI_Send(&v[i], 1, MPI_INT, 0, t[i], MPI_COMM_WORLD);
        emit("p2p_order", NULL);
    } else if (world_r == 0) {
        int a, b, c, d;
        MPI_Status s;
        MPI_Recv(&a, 1, MPI_INT, 1, 8, MPI_COMM_WORLD, &s);
        MPI_Recv(&b, 1, MPI_INT, 1, 7, MPI_COMM_WORLD, &s);
        MPI_Recv(&c, 1, MPI_INT, 1, MPI_ANY_TAG, MPI_COMM_WORLD, &s);
        int ctag = s.MPI_TAG;
        MPI_Recv(&d, 1, MPI_INT, 1, MPI_ANY_TAG, MPI_COMM_WORLD, &s);
        snprintf(f, LINE, "got=%d,%d,%d,%d ctag=%d dtag=%d", a, b, c, d, ctag, s.MPI_TAG);
        emit("p2p_order", f);
    } else {
        emit("p2p_order", NULL);
    }
}

static void case_reduce(void) {
    for (int count = 1; count <= 3; count += 2) {
        int roots[3] = {0, world_n - 1, world_n / 2};
        for (int ri = 0; ri < 3; ri++) {
            int root = roots[ri];
            double sb[3], rb[3] = {0, 0, 0};
            for (int j = 0; j < count; j++) sb[j] = mk(world_r, j);
            MPI_Reduce(sb, rb, count, MPI_DOUBLE, MPI_SUM, root, MPI_COMM_WORLD);
            char f[LINE];
            f[0] = 0;
            if (world_r == root) {
                cat(f, "root=%d count=%d", root, count);
                for (int j = 0; j < count; j++) cat(f, " %016llx", (unsigned long long)bits(rb[j]));
            }
            emit("reduce_sum_f64", f[0] ? f : NULL);
        }
    }
    int ia = world_r * 3 - 4, ib = 0;
    long long pa = 2 + world_r, pb = 0;
    double da = mk(world_r, 5), dmax, dmin;
    char f[LINE];
    MPI_Reduce(&ia, &ib, 1, MPI_INT, MPI_MAX, 0, MPI_COMM_WORLD);
    f[0] = 0;
    if (world_r == 0) snprintf(f, LINE, "max_i32=%d", ib);
    emit("reduce_max_i32", f[0] ? f : NULL);
    MPI_Reduce(&ia, &ib, 1, MPI_INT, MPI_MIN, 1, MPI_COMM_WORLD);
    f[0] = 0;
    if (world_r == 1) snprintf(f, LINE, "min_i32=%d", ib);
    emit("reduce_min_i32", f[0] ? f : NULL);
    MPI_Reduce(&pa, &pb, 1, MPI_LONG_LONG, MPI_PROD, 0, MPI_COMM_WORLD);
    f[0] = 0;
    if (world_r == 0) snprintf(f, LINE, "prod_i64=%lld", pb);
    emit("reduce_prod_i64", f[0] ? f : NULL);
    MPI_Allreduce(&da, &dmax, 1, MPI_DOUBLE, MPI_MAX, MPI_COMM_WORLD);
    MPI_Allreduce(&da, &dmin, 1, MPI_DOUBLE, MPI_MIN, MPI_COMM_WORLD);
    snprintf(f, LINE, "max=%016llx min=%016llx", (unsigned long long)bits(dmax), (unsigned long long)bits(dmin));
    emit("allreduce_maxmin_f64", f);
}

static void case_allreduce(void) {
    for (int count = 1; count <= 3; count += 2) {
        double sb[3], rb[3] = {0, 0, 0};
        for (int j = 0; j < count; j++) sb[j] = mk(world_r, j);
        MPI_Allreduce(sb, rb, count, MPI_DOUBLE, MPI_SUM, MPI_COMM_WORLD);
        char f[LINE];
        snprintf(f, LINE, "count=%d", count);
        for (int j = 0; j < count; j++) cat(f, " %016llx", (unsigned long long)bits(rb[j]));
        emit("allreduce_sum_f64", f);
    }
    float sb[2], rb[2];
    for (int j = 0; j < 2; j++) sb[j] = (float)mk(world_r, j);
    MPI_Allreduce(sb, rb, 2, MPI_FLOAT, MPI_SUM, MPI_COMM_WORLD);
    char f[LINE];
    uint32_t u0, u1;
    memcpy(&u0, &rb[0], 4);
    memcpy(&u1, &rb[1], 4);
    snprintf(f, LINE, "%08x %08x", u0, u1);
    emit("allreduce_sum_f32", f);
}

static void case_movement(void) {
    char f[LINE];
    int buf[3] = {0, 0, 0};
    int root = world_n - 1;
    if (world_r == root) { buf[0] = 11; buf[1] = 22; buf[2] = 33; }
    MPI_Bcast(buf, 3, MPI_INT, root, MPI_COMM_WORLD);
    snprintf(f, LINE, "%d,%d,%d", buf[0], buf[1], buf[2]);
    emit("bcast_i32", f);

    int *all = (int *)malloc(sizeof(int) * 2 * world_n);
    for (int i = 0; i < world_n; i++) { all[2 * i] = 1000 + i; all[2 * i + 1] = 2000 + i; }
    int mine[2];
    MPI_Scatter(all, 2, MPI_INT, mine, 2, MPI_INT, 1 % world_n, MPI_COMM_WORLD);
    snprintf(f, LINE, "%d,%d", mine[0], mine[1]);
    emit("scatter_i32", f);

    int out[2] = {world_r * 7, world_r * 7 + 1};
    MPI_Gather(out, 2, MPI_INT, all, 2, MPI_INT, 2 % world_n, MPI_COMM_WORLD);
    f[0] = 0;
    if (world_r == 2 % world_n) {
        for (int i = 0; i < 2 * world_n; i++) cat(f, "%s%d", i ? "," : "", all[i]);
    }
    emit("gather_i32", f[0] ? f : NULL);

    MPI_Allgather(out, 2, MPI_INT, all, 2, MPI_INT, MPI_COMM_WORLD);
    f[0] = 0;
    for (int i = 0; i < 2 * world_n; i++) cat(f, "%s%d", i ? "," : "", all[i]);
    emit("allgather_i32", f);
    free(all);
    MPI_Barrier(MPI_COMM_WORLD);
}

static void world_list(MPI_Group g, MPI_Group wg, char *f) {
    int sz;
    MPI_Group_size(g, &sz);
    int *in = (int *)malloc(sizeof(int) * (sz + 1)), *outp = (int *)malloc(sizeof(int) * (sz + 1));
    for (int i = 0; i < sz; i++) in[i] = i;
    MPI_Group_translate_ranks(g, sz, in, wg, outp);
    cat(f, "[");
    for (int i = 0; i < sz; i++) cat(f, "%s%d", i ? "," : "", outp[i]);
    cat(f, "]");
    free(in);
    free(outp);
}

static void case_group(void) {
    if (world_n < 4) return; /* the index lists below need 4 distinct ranks */
    MPI_Group wg;
    MPI_Comm_group(MPI_COMM_WORLD, &wg);
    int n = world_n;
    int ia[4] = {n - 1, 0, 2 % n, 1};
    int na = 4;
    int ib[3] = {1, n - 1, 0};
    int nb = 3;
    MPI_Group ga, gb, gu, gi, gd, ge, gx;
    MPI_Group_incl(wg, na, ia, &ga);
    MPI_Group_incl(wg, nb, ib, &gb);
    MPI_Group_union(ga, gb, &gu);
    MPI_Group_intersection(ga, gb, &gi);
    MPI_Group_difference(ga, gb, &gd);
    MPI_Group_excl(wg, 2, ib, &ge);
    MPI_Group_union(gb, ga, &gx);
    char f[LINE];
    f[0] = 0;
    if (world_r == 0) {
        cat(f, "A=");
        world_list(ga, wg, f);
        cat(f, " B=");
        world_list(gb, wg, f);
        cat(f, " A|B=");
        world_list(gu, wg, f);
        cat(f, " B|A=");
        world_list(gx, wg, f);
        cat(f, " A&B=");
        world_list(gi, wg, f);
        cat(f, " A-B=");
        world_list(gd, wg, f);
        cat(f, " excl=");
        world_list(ge, wg, f);
    }
    emit("group_algebra", f[0] ? f : NULL);
    int in[4] = {0, 1, 2, 3}, tr[4];
    MPI_Group_translate_ranks(ga, 4, in, gb, tr);
    f[0] = 0;
    if (world_r == 0) {
        cat(f, "tr=");
        for (int i = 0; i < 4; i++) cat(f, "%s%d", i ? "," : "", tr[i] == MPI_UNDEFINED ? -32766 : tr[i]);
    }
    emit("group_translate", f[0] ? f : NULL);
    MPI_Comm sub;
    MPI_Comm_create(MPI_COMM_WORLD, gu, &sub);
    if (sub != MPI_COMM_NULL) {
        int r, s, sum, me = world_r;
        MPI_Comm_rank(sub, &r);
        MPI_Comm_size(sub, &s);
        MPI_Allreduce(&me, &sum, 1, MPI_INT, MPI_SUM, sub);
        snprintf(f, LINE, "newrank=%d newsize=%d sum_world=%d", r, s, sum);
        MPI_Comm_free(&sub);
    } else {
        snprintf(f, LINE, "null");
    }
    emit("comm_create", f);
}

static void case_split(void) {
    char f[LINE];
    int color = (world_r % 3 == 2) ? MPI_UNDEFINED : world_r % 3;
    int key = (world_n - world_r) % 3;
    MPI_Comm c;
    MPI_Comm_split(MPI_COMM_WORLD, color, key, &c);
    if (c == MPI_COMM_NULL) {
        snprintf(f, LINE, "null");
    } else {
        int r, s, sum, me = world_r;
        MPI_Comm_rank(c, &r);
        MPI_Comm_size(c, &s);
        MPI_Allreduce(&me, &sum, 1, MPI_INT, MPI_SUM, c);
        snprintf(f, LINE, "color=%d key=%d newrank=%d newsize=%d sum_world=%d", color, key, r, s, sum);
        MPI_Comm_free(&c);
    }
    emit("comm_split", f);
    MPI_Comm d;
    MPI_Comm_dup(MPI_COMM_WORLD, &d);
    int r, s;
    MPI_Comm_rank(d, &r);
    MPI_Comm_size(d, &s);
    snprintf(f, LINE, "rank=%d size=%d", r, s);
    emit("comm_dup", f);
    MPI_Comm_free(&d);
}

static void case_cart(void) {
    char f[LINE];
    int dims[2], periods[2];
    int shapes[3][2] = {{2, 3}, {3, 2}, {2, 2}};
    int pers[3][2] = {{1, 0}, {0, 1}, {1, 1}};
    for (int k = 0; k < 3; k++) {
        dims[0] = shapes[k][0];
        dims[1] = shapes[k][1];
        periods[0] = pers[k][0];
        periods[1] = pers[k][1];
        if (dims[0] * dims[1] > world_n) { emit("cart", NULL); continue; }
        MPI_Comm cc;
        MPI_Cart_create(MPI_COMM_WORLD, 2, dims, periods, 0, &cc);
        if (cc == MPI_COMM_NULL) {
            snprintf(f, LINE, "shape=%dx%d null", dims[0], dims[1]);
        } else {
            int co[2], s0, d0, s1, d1, s2, d2, rk, rr;
            MPI_Comm_rank(cc, &rk);
            MPI_Cart_coords(cc, rk, 2, co);
            MPI_Cart_shift(cc, 0, 1, &s0, &d0);
            MPI_Cart_shift(cc, 1, 1, &s1, &d1);
            MPI_Cart_shift(cc, 1, -2, &s2, &d2);
            int off[2] = {co[0] - 1, co[1] + 2};
            int ok = 1;
            for (int i = 0; i < 2; i++)
                if (!periods[i] && (off[i] < 0 || off[i] >= dims[i])) ok = 0;
            rr = -1;
            if (ok) MPI_Cart_rank(cc, off, &rr);
            snprintf(f, LINE, "shape=%dx%d per=%d%d coords=%d,%d sh0=%d,%d sh1=%d,%d sh1m2=%d,%d rank_off=%d",
                     dims[0], dims[1], periods[0], periods[1], co[0], co[1],
                     pn(s0), pn(d0), pn(s1), pn(d1), pn(s2), pn(d2), rr);
            MPI_Comm_free(&cc);
        }
        emit("cart", f);
    }
}

int main(int argc, char **argv) {
    MPI_Init(&argc, &argv);
    MPI_Comm_rank(MPI_COMM_WORLD, &world_r);
    MPI_Comm_size(MPI_COMM_WORLD, &world_n);
    case_p2p();
    case_reduce();
    case_allreduce();
    case_movement();
    case_group();
    case_split();
    case_cart();
    MPI_Finalize();
    return 0;
}
