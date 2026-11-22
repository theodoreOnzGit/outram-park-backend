// Differential-testing driver for gh:#1386 (outram-mc-libs vs OpenMC), prio-1
// rows 1 (surface distance / evaluate / sense) and 2 (lattice distance and
// indices). Calls OpenMC's own surface.cpp / lattice.cpp routines, extracted
// verbatim by extract.sh, on a FIXED list of inputs and prints every input and
// output as a 16-digit hex bit pattern so the Rust tests can compare bit by bit.
//
// Output line grammar (all doubles are 16 hex digits of the IEEE-754 bits):
//   S  kind inst n p0 .. p{n-1}                      surface parameters
//   C  kind inst rx ry rz ux uy uz coinc E D sense  one surface case
//   T  ray step surf d rx ry rz                      multi-surface ray trace
//   LR inst is3d nx ny nz llx lly llz px py pz       rect lattice
//   LH inst orient nrings naxial cx cy cz p0 p1      hex lattice (0 = y, 1 = x)
//   RD inst rx ry rz ux uy uz | d tx ty tz           RectLattice::distance
//   RI inst rx ry rz ux uy uz | i j k                RectLattice::get_indices
//   HO inst i j k | ox oy oz                         HexLattice tile centre
//   HD inst rlx rly rlz ux uy uz i j k | d tx ty tz  HexLattice::distance
//   HI inst rx ry rz ux uy uz | i j k                HexLattice::get_indices
//   W  kind inst ray step i j k d rx ry rz            lattice-crossing walk (step -1 = start)
// INFTY is DBL_MAX in OpenMC and is printed as such.
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <memory>
#include <string>
#include <vector>

#include "surface_stub.h"
#include "lattice_stub.h"
#include "surface_extract.inc"
#include "lattice_extract.inc"

using namespace openmc;

static std::string hx(double v)
{
  uint64_t b;
  std::memcpy(&b, &v, 8);
  char buf[20];
  std::snprintf(buf, sizeof buf, "%016llx", (unsigned long long)b);
  return buf;
}

// Deterministic generator for the "random-looking" probes: a plain 64-bit LCG
// (Knuth MMIX constants). Its draws only choose inputs; they are printed.
static uint64_t g_state = 0x9e3779b97f4a7c15ULL;
static double rnd01()
{
  g_state = g_state * 6364136223846793005ULL + 1442695040888963407ULL;
  return double(g_state >> 11) * (1.0 / 9007199254740992.0);
}
static double rnd(double lo, double hi) { return lo + (hi - lo) * rnd01(); }

static Direction unit(double x, double y, double z)
{
  double n = std::sqrt(x * x + y * y + z * z);
  return {x / n, y / n, z / n};
}

struct Inst {
  std::string kind;
  std::vector<double> p;
  std::unique_ptr<Surface> s;
  std::vector<Position> on_surface; // exact (or nearly exact) points on it
};

static std::vector<Inst> g_insts;

static Position add(Position a, double x, double y, double z)
{
  return {a.x + x, a.y + y, a.z + z};
}

static void add_inst(std::string kind, std::vector<double> p,
  std::unique_ptr<Surface> s, std::vector<Position> on)
{
  Inst i;
  i.kind = kind;
  i.p = p;
  i.s = std::move(s);
  i.on_surface = on;
  g_insts.push_back(std::move(i));
}

static void build_surfaces()
{
  // Axis-aligned planes: two offsets each.
  for (double x0 : {1.5, -3250.25}) {
    auto s = std::make_unique<SurfaceXPlane>();
    s->x0_ = x0;
    add_inst("xplane", {x0}, std::move(s), {{x0, 0.3, -1.2}, {x0, 0.0, 0.0}});
  }
  for (double y0 : {1.5, -3250.25}) {
    auto s = std::make_unique<SurfaceYPlane>();
    s->y0_ = y0;
    add_inst("yplane", {y0}, std::move(s), {{0.3, y0, -1.2}, {0.0, y0, 0.0}});
  }
  for (double z0 : {1.5, -3250.25}) {
    auto s = std::make_unique<SurfaceZPlane>();
    s->z0_ = z0;
    add_inst("zplane", {z0}, std::move(s), {{0.3, -1.2, z0}, {0.0, 0.0, z0}});
  }
  {
    auto s = std::make_unique<SurfacePlane>();
    s->A_ = 1; s->B_ = 2; s->C_ = -2; s->D_ = 3;
    add_inst("plane", {1, 2, -2, 3}, std::move(s),
      {{3, 0, 0}, {1, 1, 0}, {1, 0, -1}, {-1, 2, 0}});
    auto t = std::make_unique<SurfacePlane>();
    t->A_ = 0.3; t->B_ = -0.7; t->C_ = 0.1; t->D_ = 0.25;
    add_inst("plane", {0.3, -0.7, 0.1, 0.25}, std::move(t), {});
  }
  // Cylinders (centre offsets, radius 5; second one tiny and off-centre).
  {
    auto s = std::make_unique<SurfaceXCylinder>();
    s->y0_ = 1; s->z0_ = -2; s->radius_ = 5;
    add_inst("xcyl", {1, -2, 5}, std::move(s),
      {{0, 4, 2}, {2.5, -2, 2}, {-1, 6, -2}, {1, 1, -7}});
    auto t = std::make_unique<SurfaceXCylinder>();
    t->y0_ = 0.1; t->z0_ = 0.2; t->radius_ = 0.3;
    add_inst("xcyl", {0.1, 0.2, 0.3}, std::move(t), {});
  }
  {
    auto s = std::make_unique<SurfaceYCylinder>();
    s->x0_ = 1; s->z0_ = -2; s->radius_ = 5;
    add_inst("ycyl", {1, -2, 5}, std::move(s),
      {{4, 0.7, 2}, {-2, -1, 2}, {6, 3, -2}, {1, 0, -7}});
    auto t = std::make_unique<SurfaceYCylinder>();
    t->x0_ = 0.1; t->z0_ = 0.2; t->radius_ = 0.3;
    add_inst("ycyl", {0.1, 0.2, 0.3}, std::move(t), {});
  }
  {
    auto s = std::make_unique<SurfaceZCylinder>();
    s->x0_ = 1; s->y0_ = -2; s->radius_ = 5;
    add_inst("zcyl", {1, -2, 5}, std::move(s),
      {{4, 2, 0.5}, {-2, 2, -1}, {6, -2, 3}, {1, -7, 0}});
    auto t = std::make_unique<SurfaceZCylinder>();
    t->x0_ = 0.1; t->y0_ = 0.2; t->radius_ = 0.3;
    add_inst("zcyl", {0.1, 0.2, 0.3}, std::move(t), {});
  }
  // Spheres.
  {
    auto s = std::make_unique<SurfaceSphere>();
    s->x0_ = 1; s->y0_ = -2; s->z0_ = 0.5; s->radius_ = 5;
    add_inst("sphere", {1, -2, 0.5, 5}, std::move(s),
      {{4, 2, 0.5}, {1, 1, 4.5}, {-4, -2, 0.5}, {1, -2, -4.5}, {4, -2, -3.5}});
    auto t = std::make_unique<SurfaceSphere>();
    t->x0_ = 0; t->y0_ = 0; t->z0_ = 0; t->radius_ = 0.37;
    add_inst("sphere", {0, 0, 0, 0.37}, std::move(t), {});
  }
  // Cones: x0 y0 z0 r_sq.
  {
    auto s = std::make_unique<SurfaceXCone>();
    s->x0_ = 1; s->y0_ = -2; s->z0_ = 0.5; s->radius_sq_ = 0.25;
    Position c {1, -2, 0.5};
    add_inst("xcone", {1, -2, 0.5, 0.25}, std::move(s),
      {add(c, 4, 2, 0), add(c, 4, 0, -2), add(c, 6, 3, 0), add(c, -8, 0, 4), c});
    auto t = std::make_unique<SurfaceXCone>();
    t->x0_ = 0; t->y0_ = 0; t->z0_ = 0; t->radius_sq_ = 3.0;
    add_inst("xcone", {0, 0, 0, 3.0}, std::move(t), {});
  }
  {
    auto s = std::make_unique<SurfaceYCone>();
    s->x0_ = 1; s->y0_ = -2; s->z0_ = 0.5; s->radius_sq_ = 0.25;
    Position c {1, -2, 0.5};
    add_inst("ycone", {1, -2, 0.5, 0.25}, std::move(s),
      {add(c, 2, 4, 0), add(c, 0, 4, -2), add(c, 0, -6, 3), add(c, -4, -8, 0), c});
    auto t = std::make_unique<SurfaceYCone>();
    t->x0_ = 0; t->y0_ = 0; t->z0_ = 0; t->radius_sq_ = 3.0;
    add_inst("ycone", {0, 0, 0, 3.0}, std::move(t), {});
  }
  {
    auto s = std::make_unique<SurfaceZCone>();
    s->x0_ = 1; s->y0_ = -2; s->z0_ = 0.5; s->radius_sq_ = 0.25;
    Position c {1, -2, 0.5};
    add_inst("zcone", {1, -2, 0.5, 0.25}, std::move(s),
      {add(c, 2, 0, 4), add(c, 0, -3, 6), add(c, -4, 0, -8), add(c, 0, 2, -4), c});
    auto t = std::make_unique<SurfaceZCone>();
    t->x0_ = 0; t->y0_ = 0; t->z0_ = 0; t->radius_sq_ = 3.0;
    add_inst("zcone", {0, 0, 0, 3.0}, std::move(t), {});
  }
  // General quadric: A..K = A B C D E F G H J K.
  {
    auto s = std::make_unique<SurfaceQuadric>();
    s->A_ = 9; s->B_ = 4; s->C_ = 36; s->D_ = 0; s->E_ = 0; s->F_ = 0;
    s->G_ = 0; s->H_ = 0; s->J_ = 0; s->K_ = -36;
    add_inst("quadric", {9, 4, 36, 0, 0, 0, 0, 0, 0, -36}, std::move(s),
      {{2, 0, 0}, {0, 3, 0}, {0, 0, 1}, {-2, 0, 0}, {0, -3, 0}});
    auto t = std::make_unique<SurfaceQuadric>();
    t->A_ = 0.5; t->B_ = 1.25; t->C_ = -0.75; t->D_ = 0.3; t->E_ = -0.2;
    t->F_ = 0.1; t->G_ = 0.4; t->H_ = -0.6; t->J_ = 0.8; t->K_ = -2.0;
    add_inst("quadric", {0.5, 1.25, -0.75, 0.3, -0.2, 0.1, 0.4, -0.6, 0.8, -2.0},
      std::move(t), {});
    // A hyperbolic paraboloid with a=0 along the x axis (the "plane-like" branch).
    auto u = std::make_unique<SurfaceQuadric>();
    u->A_ = 0; u->B_ = 1; u->C_ = -1; u->D_ = 0; u->E_ = 0; u->F_ = 0;
    u->G_ = 1; u->H_ = 0; u->J_ = 0; u->K_ = -1;
    add_inst("quadric", {0, 1, -1, 0, 0, 0, 1, 0, 0, -1}, std::move(u),
      {{1, 0, 0}, {0, 1, 0}, {3, 1, 1}, {-1, 1, 1}});
  }
  // Tori: x0 y0 z0 A B C. B is the half-axis along the axis of revolution, C the
  // tube radius (OpenMC docs). #701 records b and c swapped in the port: the
  // second and third instances have B != C so a swap cannot hide.
  struct T { double x0, y0, z0, A, B, C; };
  std::vector<T> tori = {{0.5, -0.25, 0.75, 3, 1, 1}, {0, 0, 0, 3, 2, 1}, {0, 0, 0, 5, 1.5, 2}};
  // relative on-surface points per instance and axis (z-torus rel; x/y permuted below)
  std::vector<std::vector<std::array<double, 3>>> relz = {
    {{4, 0, 0}, {2, 0, 0}, {3, 0, 1}, {0, 3, 1}, {0, -4, 0}, {0, 3, -1}},
    {{4, 0, 0}, {3, 0, 2}, {2, 0, 0}, {0, -3, -2}},
    {{7, 0, 0}, {3, 0, 0}, {5, 0, 1.5}, {0, 5, -1.5}}};
  for (size_t k = 0; k < tori.size(); ++k) {
    T t = tori[k];
    std::vector<double> p = {t.x0, t.y0, t.z0, t.A, t.B, t.C};
    Position c {t.x0, t.y0, t.z0};
    std::vector<Position> pz, px, py;
    for (auto& q : relz[k]) {
      pz.push_back(add(c, q[0], q[1], q[2]));
      // x-torus: axis x; (x,y,z)_rel <- (z, x, y) of the z-torus point
      px.push_back(add(c, q[2], q[0], q[1]));
      // y-torus: axis y; (x,y,z)_rel <- (x, z, y)
      py.push_back(add(c, q[0], q[2], q[1]));
    }
    auto xs = std::make_unique<SurfaceXTorus>();
    xs->x0_ = t.x0; xs->y0_ = t.y0; xs->z0_ = t.z0; xs->A_ = t.A; xs->B_ = t.B; xs->C_ = t.C;
    add_inst("xtorus", p, std::move(xs), px);
    auto ys = std::make_unique<SurfaceYTorus>();
    ys->x0_ = t.x0; ys->y0_ = t.y0; ys->z0_ = t.z0; ys->A_ = t.A; ys->B_ = t.B; ys->C_ = t.C;
    add_inst("ytorus", p, std::move(ys), py);
    auto zs = std::make_unique<SurfaceZTorus>();
    zs->x0_ = t.x0; zs->y0_ = t.y0; zs->z0_ = t.z0; zs->A_ = t.A; zs->B_ = t.B; zs->C_ = t.C;
    add_inst("ztorus", p, std::move(zs), pz);
  }
}

static void print_case(const Inst& in, int idx, Position r, Direction u, bool coinc)
{
  double e = in.s->evaluate(r);
  double d = in.s->distance(r, u, coinc);
  bool sn = in.s->sense(r, u);
  std::printf("C %s %d %s %s %s %s %s %s %d %s %s %d\n", in.kind.c_str(), idx,
    hx(r.x).c_str(), hx(r.y).c_str(), hx(r.z).c_str(), hx(u.x).c_str(),
    hx(u.y).c_str(), hx(u.z).c_str(), coinc ? 1 : 0, hx(e).c_str(), hx(d).c_str(),
    sn ? 1 : 0);
}

static std::vector<Position> generic_origins()
{
  std::vector<Position> o = {{0, 0, 0}, {1, -2, 0.5}, {0.3, 0.4, -0.2}, {2, 1, 1},
    {-3, 2, 4}, {6, -1, 2}, {10, 10, 10}, {-7.5, 3.25, -6}, {0.5, -0.25, 0.75},
    {2.5, 0.125, -0.5}};
  for (int i = 0; i < 4; ++i)
    o.push_back({rnd(-8, 8), rnd(-8, 8), rnd(-8, 8)});
  return o;
}

static std::vector<Direction> generic_dirs()
{
  std::vector<Direction> d = {{1, 0, 0}, {-1, 0, 0}, {0, 1, 0}, {0, -1, 0},
    {0, 0, 1}, {0, 0, -1}, unit(1, 1, 1), unit(-1, 2, -2), unit(0.6, 0.8, 0),
    unit(0, 0.6, -0.8), unit(0.48, 0.64, 0.6), unit(1, 1e-9, 0)};
  for (int i = 0; i < 2; ++i)
    d.push_back(unit(rnd(-1, 1), rnd(-1, 1), rnd(-1, 1)));
  return d;
}

static void surface_cases()
{
  auto gen_o = generic_origins();
  auto gen_d = generic_dirs();
  for (size_t k = 0; k < g_insts.size(); ++k) {
    const Inst& in = g_insts[k];
    std::printf("S %s %zu %zu", in.kind.c_str(), k, in.p.size());
    for (double v : in.p)
      std::printf(" %s", hx(v).c_str());
    std::printf("\n");
    for (auto& r : gen_o)
      for (auto& u : gen_d)
        print_case(in, (int)k, r, u, false);
    // Each exact on-surface point is also probed 4e-13 / -3e-13 off in x / z, i.e.
    // inside the |f| < FP_COINCIDENT band without being exactly on the surface.
    std::vector<Position> special = in.on_surface;
    for (auto& r : in.on_surface)
      special.push_back({r.x + 4e-13, r.y, r.z - 3e-13});
    for (auto& r : special) {
      std::vector<Direction> ds = gen_d;
      // Along and against the (unnormalised) normal, and two tangents.
      Direction n = in.s->normal(r);
      double nn = std::sqrt(n.x * n.x + n.y * n.y + n.z * n.z);
      if (nn > 0) {
        Direction nu {n.x / nn, n.y / nn, n.z / nn};
        ds.push_back(nu);
        ds.push_back({-nu.x, -nu.y, -nu.z});
        Direction e = std::abs(nu.x) < 0.9 ? Direction {1, 0, 0} : Direction {0, 1, 0};
        Direction t1 = unit(nu.y * e.z - nu.z * e.y, nu.z * e.x - nu.x * e.z,
          nu.x * e.y - nu.y * e.x);
        Direction t2 = unit(nu.y * t1.z - nu.z * t1.y, nu.z * t1.x - nu.x * t1.z,
          nu.x * t1.y - nu.y * t1.x);
        ds.push_back(t1);
        ds.push_back(t2);
      }
      for (auto& u : ds) {
        print_case(in, (int)k, r, u, false);
        print_case(in, (int)k, r, u, true);
      }
    }
  }
}

// Multi-surface ray trace: repeat "find the nearest surface among all of them,
// with the last-crossed one flagged coincident, advance". The selection rule is
// Region::distance_to_nearest_surface (src/cell.cpp:955-995), the advance is
// r += d * u. The loop is written here; only surface.distance is upstream's.
static void ray_trace()
{
  std::vector<std::unique_ptr<Surface>> ss;
  auto zc = [&](double r) {
    auto s = std::make_unique<SurfaceZCylinder>();
    s->x0_ = 0; s->y0_ = 0; s->radius_ = r;
    ss.push_back(std::move(s));
  };
  zc(1.0); zc(2.0); zc(3.5);
  { auto s = std::make_unique<SurfaceSphere>(); s->x0_ = 0.5; s->y0_ = -0.25; s->z0_ = 0.0; s->radius_ = 2.75; ss.push_back(std::move(s)); }
  { auto s = std::make_unique<SurfaceZPlane>(); s->z0_ = -4; ss.push_back(std::move(s)); }
  { auto s = std::make_unique<SurfaceZPlane>(); s->z0_ = 4; ss.push_back(std::move(s)); }
  { auto s = std::make_unique<SurfaceXPlane>(); s->x0_ = 2.5; ss.push_back(std::move(s)); }
  { auto s = std::make_unique<SurfacePlane>(); s->A_ = 1; s->B_ = 2; s->C_ = -2; s->D_ = 3; ss.push_back(std::move(s)); }
  { auto s = std::make_unique<SurfaceXCone>(); s->x0_ = -1; s->y0_ = 0; s->z0_ = 0; s->radius_sq_ = 0.25; ss.push_back(std::move(s)); }
  { auto s = std::make_unique<SurfaceZTorus>(); s->x0_ = 0; s->y0_ = 0; s->z0_ = 0; s->A_ = 3; s->B_ = 2; s->C_ = 1; ss.push_back(std::move(s)); }
  { auto s = std::make_unique<SurfaceQuadric>(); s->A_ = 9; s->B_ = 4; s->C_ = 36; s->D_ = 0; s->E_ = 0; s->F_ = 0; s->G_ = 0; s->H_ = 0; s->J_ = 0; s->K_ = -36; ss.push_back(std::move(s)); }
  struct Ray { Position r; Direction u; };
  std::vector<Ray> rays = {
    {{-6, 0.1, 0.2}, {1, 0, 0}},
    {{-5, -4.5, -3}, unit(1, 1, 0.6)},
    {{0.05, 0.02, -6}, {0, 0, 1}},
    {{4.5, 3.1, 1.7}, unit(-1.3, -0.8, -0.4)},
    {{0.3, -0.2, 0.1}, unit(0.2, 0.3, 0.9)},
    {{-3.0, 2.0, -3.5}, unit(0.7, -0.5, 0.45)},
  };
  for (size_t k = 0; k < rays.size(); ++k) {
    Position r = rays[k].r;
    Direction u = rays[k].u;
    int on = -1;
    for (int step = 0; step < 24; ++step) {
      double min_dist = INFTY;
      int isurf = -1;
      for (size_t i = 0; i < ss.size(); ++i) {
        bool coinc = (int)i == on;
        double d = ss[i]->distance(r, u, coinc);
        if (d < min_dist) {
          if (min_dist - d >= FP_PRECISION * min_dist) {
            min_dist = d;
            isurf = (int)i;
          }
        }
      }
      if (isurf < 0)
        break;
      r += min_dist * u;
      std::printf("T %zu %d %d %s %s %s %s\n", k, step, isurf, hx(min_dist).c_str(),
        hx(r.x).c_str(), hx(r.y).c_str(), hx(r.z).c_str());
      on = isurf;
    }
  }
}

// ---------------------------------------------------------------- lattices --

static std::vector<Direction> lattice_dirs()
{
  std::vector<Direction> d = {{1, 0, 0}, {-1, 0, 0}, {0, 1, 0}, {0, -1, 0},
    {0, 0, 1}, {0, 0, -1}, unit(1, 1, 0), unit(-1, 1, 0), unit(1, 1, 1),
    unit(1, 1 + 1e-13, 0), unit(1, 0.5, 0.25)};
  d.push_back(unit(rnd(-1, 1), rnd(-1, 1), rnd(-1, 1)));
  return d;
}

// Walks: the lattice-crossing loop of OpenMC's transport, one lattice level
// (src/geometry.cpp cross_lattice / distance_to_boundary): indices from
// get_indices, local position from get_local_position, d and the translation from
// distance, then r += d u and i += translation. Written here; only the three
// lattice routines are upstream's. For the hex lattice the position handed to
// distance() is the LATTICE-frame x, y and the tile-local z (geometry.cpp:459-467).
static void walk_print(const char* kind, size_t inst, int ray, int step,
  const array<int, 3>& i, double d, Position r)
{
  std::printf("W %s %zu %d %d %d %d %d %s %s %s %s\n", kind, inst, ray, step, i[0], i[1],
    i[2], hx(d).c_str(), hx(r.x).c_str(), hx(r.y).c_str(), hx(r.z).c_str());
}

static std::vector<Direction> walk_dirs()
{
  return {unit(1, 0.7, 0.3), unit(-0.4, 1, -0.2), unit(1, 1, 0), unit(-1, -1, 0.5),
    unit(0.2, -1, 0.9)};
}

static void rect_walk(size_t inst, RectLattice& lat, Position start)
{
  auto dirs = walk_dirs();
  for (size_t k = 0; k < dirs.size(); ++k) {
    Position r = start;
    Direction u = dirs[k];
    array<int, 3> i {};
    lat.get_indices(r, u, i);
    walk_print("rect", inst, (int)k, -1, i, 0.0, r);
    for (int step = 0; step < 12; ++step) {
      Position rl = lat.get_local_position(r, i);
      auto res = lat.distance(rl, u, i);
      if (res.first == INFTY)
        break;
      r += res.first * u;
      i[0] += res.second[0]; i[1] += res.second[1]; i[2] += res.second[2];
      walk_print("rect", inst, (int)k, step, i, res.first, r);
    }
  }
}

static void hex_walk(size_t inst, HexLattice& lat, Position start)
{
  auto dirs = walk_dirs();
  for (size_t k = 0; k < dirs.size(); ++k) {
    Position r = start;
    Direction u = dirs[k];
    array<int, 3> i {};
    lat.get_indices(r, u, i);
    walk_print("hex", inst, (int)k, -1, i, 0.0, r);
    for (int step = 0; step < 12; ++step) {
      Position rl = lat.get_local_position(r, i);
      Position rh {r.x, r.y, rl.z};
      auto res = lat.distance(rh, u, i);
      if (res.first == INFTY)
        break;
      r += res.first * u;
      i[0] += res.second[0]; i[1] += res.second[1]; i[2] += res.second[2];
      walk_print("hex", inst, (int)k, step, i, res.first, r);
    }
  }
}

static void rect_lattices()
{
  struct R { bool is3d; int n[3]; Position ll; Position p; };
  std::vector<R> rs = {
    {false, {3, 3, 1}, {-3, -3, 0}, {2, 2, 0}},
    {true, {2, 3, 2}, {-1.2, -1.8, -1.0}, {1.2, 1.2, 1.0}},
    {true, {3, 3, 3}, {-1.5e5, -1.5e5, -1.5e5}, {1e5, 1e5, 1e5}},   // large pitch (#3853)
    {false, {4, 4, 1}, {-2.0e6, -2.0e6, 0}, {1e6, 1e6, 0}},
  };
  auto dirs = lattice_dirs();
  for (size_t k = 0; k < rs.size(); ++k) {
    R& L = rs[k];
    RectLattice lat;
    lat.is_3d_ = L.is3d;
    lat.n_cells_ = {L.n[0], L.n[1], L.n[2]};
    lat.lower_left_ = L.ll;
    lat.pitch_ = L.p;
    std::printf("LR %zu %d %d %d %d %s %s %s %s %s %s\n", k, L.is3d ? 1 : 0, L.n[0],
      L.n[1], L.n[2], hx(L.ll.x).c_str(), hx(L.ll.y).c_str(), hx(L.ll.z).c_str(),
      hx(L.p.x).c_str(), hx(L.p.y).c_str(), hx(L.p.z).c_str());
    rect_walk(k, lat, {L.ll.x + 0.37 * L.p.x, L.ll.y + 0.41 * L.p.y,
      L.is3d ? L.ll.z + 0.43 * L.p.z : 0.0});
    // distance: tile-local positions (tile centre at the origin).
    std::vector<Position> loc;
    double hx_ = 0.5 * L.p.x, hy_ = 0.5 * L.p.y, hz_ = 0.5 * L.p.z;
    for (double fx : {0.0, 0.25, -0.5})
      for (double fy : {0.0, -0.25, 0.5})
        loc.push_back({fx * L.p.x, fy * L.p.y, L.is3d ? 0.1 * L.p.z : 0.0});
    loc.push_back({-hx_, -hy_, L.is3d ? -hz_ : 0.0}); // on a corner
    loc.push_back({hx_, hy_, L.is3d ? hz_ : 0.0});
    loc.push_back({-0.5 * hx_, -hy_, 0.0});
    for (int i = 0; i < 3; ++i)
      loc.push_back({rnd(-hx_, hx_), rnd(-hy_, hy_), L.is3d ? rnd(-hz_, hz_) : 0.0});
    for (auto& r : loc)
      for (auto& u : dirs) {
        auto res = lat.distance(r, u, {0, 0, 0});
        std::printf("RD %zu %s %s %s %s %s %s | %s %d %d %d\n", k, hx(r.x).c_str(),
          hx(r.y).c_str(), hx(r.z).c_str(), hx(u.x).c_str(), hx(u.y).c_str(),
          hx(u.z).c_str(), hx(res.first).c_str(), res.second[0], res.second[1],
          res.second[2]);
      }
    // get_indices: global positions on and off the tile boundaries.
    std::vector<Position> glob;
    for (int a = -1; a <= L.n[0] + 1; ++a)
      for (int b = -1; b <= L.n[1] + 1; ++b) {
        for (double fa : {0.0, 0.5})
          for (double fb : {0.0}) {
            double z = L.is3d ? L.ll.z + 0.0 * L.p.z + 0.5 * L.p.z : 0.0;
            glob.push_back({L.ll.x + (a + fa) * L.p.x, L.ll.y + (b + fb) * L.p.y, z});
          }
      }
    if (L.is3d)
      for (int c = -1; c <= L.n[2] + 1; ++c)
        glob.push_back({L.ll.x + 0.5 * L.p.x, L.ll.y + 0.5 * L.p.y, L.ll.z + c * L.p.z});
    for (int i = 0; i < 8; ++i)
      glob.push_back({rnd(L.ll.x - L.p.x, L.ll.x + (L.n[0] + 1) * L.p.x),
        rnd(L.ll.y - L.p.y, L.ll.y + (L.n[1] + 1) * L.p.y),
        L.is3d ? rnd(L.ll.z - L.p.z, L.ll.z + (L.n[2] + 1) * L.p.z) : 0.0});
    for (auto& r : glob)
      for (size_t di = 0; di < 8; ++di) {
        auto& u = dirs[di];
        array<int, 3> res {};
        lat.get_indices(r, u, res);
        std::printf("RI %zu %s %s %s %s %s %s | %d %d %d\n", k, hx(r.x).c_str(),
          hx(r.y).c_str(), hx(r.z).c_str(), hx(u.x).c_str(), hx(u.y).c_str(),
          hx(u.z).c_str(), res[0], res[1], res[2]);
      }
  }
}

static void hex_lattices()
{
  struct H { int orient; int nr; int na; Position c; double p0, p1; };
  std::vector<H> hs = {
    {0, 3, 1, {0.3, -0.2, 0}, 1.8, 0},
    {1, 3, 1, {0.3, -0.2, 0}, 1.8, 0},
    {0, 2, 3, {0.3, -0.2, 1.0}, 1.8, 4.0},
    {1, 2, 3, {-0.7, 0.4, 1.0}, 1.8, 4.0},
    {0, 4, 1, {0, 0, 0}, 1.0e4, 0},    // large pitch
    {1, 4, 1, {5.0e4, -3.0e4, 0}, 1.0e4, 0},
  };
  auto dirs = lattice_dirs();
  for (size_t k = 0; k < hs.size(); ++k) {
    H& h = hs[k];
    HexLattice lat;
    lat.is_3d_ = h.na > 1;
    lat.n_rings_ = h.nr;
    lat.n_axial_ = h.na;
    lat.orientation_ = h.orient == 0 ? HexLattice::Orientation::y : HexLattice::Orientation::x;
    lat.center_ = h.c;
    lat.pitch_ = {h.p0, h.p1};
    std::printf("LH %zu %d %d %d %s %s %s %s %s\n", k, h.orient, h.nr, h.na,
      hx(h.c.x).c_str(), hx(h.c.y).c_str(), hx(h.c.z).c_str(), hx(h.p0).c_str(),
      hx(h.p1).c_str());
    hex_walk(k, lat, {h.c.x + 0.31 * h.p0, h.c.y - 0.17 * h.p0,
      lat.is_3d_ ? h.c.z + 0.2 * h.p1 : 0.0});
    int w = 2 * h.nr - 1;
    // Tile centres in the lattice frame: -get_local_position(0, i) (exact negation).
    auto centre = [&](int i, int j, int kz) {
      Position z0 {0, 0, 0};
      Position l = lat.get_local_position(z0, {i, j, kz});
      return Position {-l.x, -l.y, -l.z};
    };
    std::vector<array<int, 3>> tiles;
    for (int kz = 0; kz < h.na; ++kz) {
      tiles.push_back({h.nr - 1, h.nr - 1, kz});
      tiles.push_back({h.nr - 1 + 1, h.nr - 1, kz});
    }
    for (auto& t : tiles) {
      Position off = centre(t[0], t[1], t[2]);
      std::printf("HO %zu %d %d %d | %s %s %s\n", k, t[0], t[1], t[2], hx(off.x).c_str(),
        hx(off.y).c_str(), hx(off.z).c_str());
      // HexLattice::distance takes the lattice-frame x,y and the TILE-LOCAL z
      // (src/geometry.cpp:459-467). r_local is tile-local; r_lat = r_local + off.
      double ph = 0.5 * h.p0;
      std::vector<Position> loc;
      for (double fx : {0.0, -0.45})
        for (double fy : {0.0, 0.4})
          loc.push_back({fx * h.p0, fy * h.p0, h.na > 1 ? 0.1 * h.p1 : 0.0});
      loc.push_back({ph, 0, 0}); loc.push_back({0, ph, 0}); loc.push_back({-ph, 0, 0});
      loc.push_back({0, -ph, 0});
      if (h.na > 1) loc.push_back({0.2, 0.1, 0.5 * h.p1});
      for (int i = 0; i < 2; ++i)
        loc.push_back({rnd(-ph, ph), rnd(-ph, ph), h.na > 1 ? rnd(-0.5 * h.p1, 0.5 * h.p1) : 0.0});
      for (auto& rl : loc)
        for (auto& u : dirs) {
          Position rlat {rl.x + off.x, rl.y + off.y, rl.z};
          auto res = lat.distance(rlat, u, t);
          std::printf("HD %zu %s %s %s %s %s %s %d %d %d | %s %d %d %d\n", k,
            hx(rl.x).c_str(), hx(rl.y).c_str(), hx(rl.z).c_str(), hx(u.x).c_str(),
            hx(u.y).c_str(), hx(u.z).c_str(), t[0], t[1], t[2], hx(res.first).c_str(),
            res.second[0], res.second[1], res.second[2]);
        }
    }
    // get_indices: tile centres, edge mid-points, vertices, off-lattice, random.
    std::vector<Position> glob;
    for (int i = 0; i < w; ++i)
      for (int j = 0; j < w; ++j) {
        Position c = centre(i, j, 0);
        glob.push_back({c.x, c.y, lat.is_3d_ ? h.c.z + 0.3 * h.p1 : 0.0});
        // mid-point to the +x-index neighbour and +y-index neighbour; a vertex.
        Position cx = centre(i + 1, j, 0), cy = centre(i, j + 1, 0), cd = centre(i + 1, j - 1, 0);
        glob.push_back({0.5 * (c.x + cx.x), 0.5 * (c.y + cx.y), 0.0});
        glob.push_back({0.5 * (c.x + cy.x), 0.5 * (c.y + cy.y), 0.0});
        glob.push_back({(c.x + cx.x + cd.x) / 3.0, (c.y + cx.y + cd.y) / 3.0, 0.0});
      }
    if (lat.is_3d_)
      for (int kz = -1; kz <= h.na + 1; ++kz)
        glob.push_back({h.c.x, h.c.y, h.c.z + (kz - 0.5 * h.na) * h.p1});
    for (int i = 0; i < 10; ++i)
      glob.push_back({h.c.x + rnd(-w, w) * h.p0, h.c.y + rnd(-w, w) * h.p0,
        lat.is_3d_ ? h.c.z + rnd(-0.5 * h.na - 1, 0.5 * h.na + 1) * h.p1 : 0.0});
    for (auto& r : glob) {
      Position rr = r;
      for (size_t di = 0; di < 6; ++di) {
        auto& u = dirs[di];
        array<int, 3> res {};
        lat.get_indices(rr, u, res);
        std::printf("HI %zu %s %s %s %s %s %s | %d %d %d\n", k, hx(rr.x).c_str(),
          hx(rr.y).c_str(), hx(rr.z).c_str(), hx(u.x).c_str(), hx(u.y).c_str(),
          hx(u.z).c_str(), res[0], res[1], res[2]);
      }
    }
  }
}

int main()
{
  build_surfaces();
  surface_cases();
  ray_trace();
  rect_lattices();
  hex_lattices();
  return 0;
}
