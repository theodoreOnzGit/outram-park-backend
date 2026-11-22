// SPDX-License-Identifier: AGPL-3.0-only
//
// boundary_surface_distance — gh:#1385 / gh:#1299 helper.
//
// Standalone (no OpenFOAM) tool: reads an ASCII OpenFOAM polyMesh
// (points, faces, boundary) and an STL surface, and reports the distance
// from every boundary point to the nearest surface triangle.
//
// Methodology:
//   * Boundary points = every point used by a face of any boundary patch
//     (patch type "empty" skipped, unless the environment sets
//     BSD_INCLUDE_EMPTY=1: cfMesh writes its intermediate meshes, saved by
//     workflowControls' stopAfter, with every patch typed "empty").
//     Also reported per patch.
//   * The STL is read the way OpenFOAM's STL reader reads it: coordinates
//     rounded to float (single precision), then widened to double.
//     ASCII and binary STL are both accepted.
//   * Distance = exact point-triangle distance (closest point on triangle,
//     Ericson, "Real-Time Collision Detection", sec. 5.1.5), brute force over
//     all triangles, so no search structure can hide a miss.
//   * Reported: count, max, p99, p50, mean, and counts above 1e-9/1e-6/1e-3
//     of the bounding-box diagonal.
//
// Build:  g++ -O2 -std=c++17 -o boundary_surface_distance boundary_surface_distance.cpp
// Usage:  boundary_surface_distance <case>/constant/polyMesh <surface.stl>
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <iostream>
#include <map>
#include <sstream>
#include <string>
#include <vector>

struct V { double x, y, z; };
static V sub(V a, V b) { return {a.x - b.x, a.y - b.y, a.z - b.z}; }
static V add(V a, V b) { return {a.x + b.x, a.y + b.y, a.z + b.z}; }
static V mul(V a, double s) { return {a.x * s, a.y * s, a.z * s}; }
static double dot(V a, V b) { return a.x * b.x + a.y * b.y + a.z * b.z; }

// Ericson closest point on triangle abc to p.
static V closest(V p, V a, V b, V c) {
    V ab = sub(b, a), ac = sub(c, a), ap = sub(p, a);
    double d1 = dot(ab, ap), d2 = dot(ac, ap);
    if (d1 <= 0 && d2 <= 0) return a;
    V bp = sub(p, b);
    double d3 = dot(ab, bp), d4 = dot(ac, bp);
    if (d3 >= 0 && d4 <= d3) return b;
    double vc = d1 * d4 - d3 * d2;
    if (vc <= 0 && d1 >= 0 && d3 <= 0) return add(a, mul(ab, d1 / (d1 - d3)));
    V cp = sub(p, c);
    double d5 = dot(ab, cp), d6 = dot(ac, cp);
    if (d6 >= 0 && d5 <= d6) return c;
    double vb = d5 * d2 - d1 * d6;
    if (vb <= 0 && d2 >= 0 && d6 <= 0) return add(a, mul(ac, d2 / (d2 - d6)));
    double va = d3 * d6 - d5 * d4;
    if (va <= 0 && (d4 - d3) >= 0 && (d5 - d6) >= 0)
        return add(b, mul(sub(c, b), (d4 - d3) / ((d4 - d3) + (d5 - d6))));
    double denom = 1.0 / (va + vb + vc);
    double v = vb * denom, w = vc * denom;
    return add(a, add(mul(ab, v), mul(ac, w)));
}

static std::string slurp(const std::string& path) {
    std::ifstream f(path, std::ios::binary);
    if (!f) { std::cerr << "cannot open " << path << "\n"; std::exit(1); }
    std::stringstream ss; ss << f.rdbuf(); return ss.str();
}

// Strip the FoamFile header and C/C++ comments; return the body.
static std::string body(const std::string& s) {
    std::string out; out.reserve(s.size());
    for (size_t i = 0; i < s.size(); ++i) {
        if (s[i] == '/' && i + 1 < s.size() && s[i + 1] == '/') {
            while (i < s.size() && s[i] != '\n') ++i;
        } else if (s[i] == '/' && i + 1 < s.size() && s[i + 1] == '*') {
            i += 2; while (i + 1 < s.size() && !(s[i] == '*' && s[i + 1] == '/')) ++i; ++i;
        } else out += s[i];
    }
    size_t h = out.find("FoamFile");
    if (h != std::string::npos) {
        size_t e = out.find('}', h);
        out = out.substr(e + 1);
    }
    return out;
}

static std::vector<V> readPoints(const std::string& dir) {
    std::string b = body(slurp(dir + "/points"));
    for (char& c : b) if (c == '(' || c == ')') c = ' ';
    std::istringstream is(b);
    long n; is >> n;
    std::vector<V> p(n);
    for (long i = 0; i < n; ++i) is >> p[i].x >> p[i].y >> p[i].z;
    if (!is) { std::cerr << "points parse error (binary format?)\n"; std::exit(1); }
    return p;
}

static std::vector<std::vector<long>> readFaces(const std::string& dir) {
    std::string raw = slurp(dir + "/faces");
    std::string b = body(raw);
    std::vector<std::vector<long>> faces;
    if (raw.find("faceCompactList") != std::string::npos) {
        for (char& c : b) if (c == '(' || c == ')') c = ' ';
        std::istringstream is(b);
        long no; is >> no; std::vector<long> off(no); for (auto& o : off) is >> o;
        long nc; is >> nc; std::vector<long> con(nc); for (auto& c : con) is >> c;
        for (long i = 0; i + 1 < no; ++i)
            faces.emplace_back(con.begin() + off[i], con.begin() + off[i + 1]);
    } else {
        // n ( k(a b c) k(...) ... )
        size_t i = 0;
        auto skipws = [&]() { while (i < b.size() && std::isspace((unsigned char)b[i])) ++i; };
        auto readInt = [&]() { skipws(); size_t j = i; while (j < b.size() && (std::isdigit((unsigned char)b[j]) || b[j] == '-')) ++j; long v = std::stol(b.substr(i, j - i)); i = j; return v; };
        long n = readInt(); skipws(); ++i; // '('
        faces.reserve(n);
        for (long f = 0; f < n; ++f) {
            long k = readInt(); skipws(); ++i; // '('
            std::vector<long> fv(k); for (auto& v : fv) v = readInt();
            skipws(); ++i; // ')'
            faces.push_back(std::move(fv));
        }
    }
    return faces;
}

struct Patch { std::string name, type; long start, n; };
static std::vector<Patch> readBoundary(const std::string& dir) {
    std::string b = body(slurp(dir + "/boundary"));
    std::vector<Patch> ps;
    size_t i = b.find('(');
    size_t end = b.rfind(')');
    while (true) {
        size_t open = b.find('{', i);
        if (open == std::string::npos || open > end) break;
        size_t ns = open; // patch name precedes '{'
        size_t ne = ns; while (ne > 0 && std::isspace((unsigned char)b[ne - 1])) --ne;
        size_t nb = ne; while (nb > 0 && !std::isspace((unsigned char)b[nb - 1]) && b[nb - 1] != '(') --nb;
        size_t close = b.find('}', open);
        std::string d = b.substr(open + 1, close - open - 1);
        auto get = [&](const std::string& key) {
            size_t k = d.find(key + " "); if (k == std::string::npos) k = d.find(key + "\t");
            if (k == std::string::npos) return std::string();
            size_t s = k + key.size(); size_t e = d.find(';', s);
            std::string v = d.substr(s, e - s);
            v.erase(0, v.find_first_not_of(" \t\n")); v.erase(v.find_last_not_of(" \t\n") + 1);
            return v; };
        ps.push_back({b.substr(nb, ne - nb), get("type"), std::stol(get("startFace")), std::stol(get("nFaces"))});
        i = close + 1;
    }
    return ps;
}

static std::vector<std::array<V, 3>> readStl(const std::string& path) {
    std::string s = slurp(path);
    std::vector<std::array<V, 3>> tris;
    auto f2d = [](double v) { return (double)(float)v; };
    bool ascii = s.compare(0, 5, "solid") == 0 && s.find("facet") != std::string::npos;
    if (ascii) {
        std::istringstream is(s); std::string tok; std::array<V, 3> t; int k = 0;
        while (is >> tok) {
            if (tok == "vertex") {
                double x, y, z; is >> x >> y >> z;
                // OpenFOAM's STL reader holds coordinates as float.
                t[k++] = {f2d((float)x), f2d((float)y), f2d((float)z)};
                if (k == 3) { tris.push_back(t); k = 0; }
            }
        }
    } else {
        uint32_t n; std::memcpy(&n, s.data() + 80, 4);
        for (uint32_t i = 0; i < n; ++i) {
            const char* r = s.data() + 84 + 50 * i + 12;
            std::array<V, 3> t; float c[9]; std::memcpy(c, r, 36);
            for (int k = 0; k < 3; ++k) t[k] = {c[3 * k], c[3 * k + 1], c[3 * k + 2]};
            tris.push_back(t);
        }
    }
    return tris;
}

static void report(const std::string& label, std::vector<double> d, double diag) {
    if (d.empty()) { std::printf("%-28s n=0\n", label.c_str()); return; }
    std::sort(d.begin(), d.end());
    double sum = 0; for (double v : d) sum += v;
    auto pct = [&](double q) { size_t k = (size_t)std::ceil(q * d.size()) - 1; return d[std::min(k, d.size() - 1)]; };
    size_t a = 0, b = 0, c = 0;
    for (double v : d) { if (v > 1e-9 * diag) ++a; if (v > 1e-6 * diag) ++b; if (v > 1e-3 * diag) ++c; }
    std::printf("%-28s n=%zu max=%.6e p99=%.6e p50=%.6e mean=%.6e  >1e-9D:%zu >1e-6D:%zu >1e-3D:%zu\n",
                label.c_str(), d.size(), d.back(), pct(0.99), pct(0.50), sum / d.size(), a, b, c);
}

int main(int argc, char** argv) {
    if (argc < 3) { std::cerr << "usage: " << argv[0] << " <polyMesh dir> <surface.stl>\n"; return 2; }
    std::string dir = argv[1];
    auto pts = readPoints(dir);
    auto faces = readFaces(dir);
    auto patches = readBoundary(dir);
    auto tris = readStl(argv[2]);
    V lo{1e300, 1e300, 1e300}, hi{-1e300, -1e300, -1e300};
    for (auto& t : tris) for (auto& v : t) {
        lo = {std::min(lo.x, v.x), std::min(lo.y, v.y), std::min(lo.z, v.z)};
        hi = {std::max(hi.x, v.x), std::max(hi.y, v.y), std::max(hi.z, v.z)}; }
    double diag = std::sqrt(dot(sub(hi, lo), sub(hi, lo)));
    std::printf("mesh points=%zu faces=%zu patches=%zu; surface triangles=%zu, bbox diagonal D=%.9g\n",
                pts.size(), faces.size(), patches.size(), tris.size(), diag);

    std::vector<double> dist(pts.size(), -1.0);
    std::vector<double> all;
    const char* inc = std::getenv("BSD_INCLUDE_EMPTY");
    const bool includeEmpty = inc && std::string(inc) == "1";
    for (auto& p : patches) {
        if (p.type == "empty" && !includeEmpty) continue;
        std::vector<double> pd; std::vector<char> mark(pts.size(), 0);
        for (long f = p.start; f < p.start + p.n; ++f)
            for (long v : faces[f]) {
                if (mark[v]) continue;
                mark[v] = 1;
                if (dist[v] < 0) {
                    double best = 1e300; V q = pts[v];
                    for (auto& t : tris) {
                        V c = closest(q, t[0], t[1], t[2]);
                        V e = sub(q, c); double dd = dot(e, e);
                        if (dd < best) best = dd;
                    }
                    dist[v] = std::sqrt(best);
                    all.push_back(dist[v]);
                }
                pd.push_back(dist[v]);
            }
        report("patch " + p.name, pd, diag);
    }
    report("ALL boundary points", all, diag);
    if (argc > 3) { // optional: dump per-point distances
        std::FILE* o = std::fopen(argv[3], "w");
        for (size_t i = 0; i < pts.size(); ++i) if (dist[i] >= 0)
            std::fprintf(o, "%zu %.17g %.17g %.17g %.9e\n", i, pts[i].x, pts[i].y, pts[i].z, dist[i]);
        std::fclose(o);
    }
    return 0;
}
