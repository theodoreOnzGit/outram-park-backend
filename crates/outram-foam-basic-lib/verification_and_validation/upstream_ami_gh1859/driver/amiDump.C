// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// amiDump (gh:#1859): differential driver for
// outram_foam_basic_lib::mesh::face_area_weight_ami. Calls ONLY unmodified
// OpenFOAM ESI code (gitlab.com/openfoam/core/openfoam commit
// 6e2595746a396d688edd9890f4aebaaa7ff4ecfd): Foam::faceAreaWeightAMI built
// from a dictionary, calculate(srcPatch, tgtPatch), then the accessors.
// It generates the input geometry itself and writes it, as IEEE-754 bit
// patterns, beside the outputs, so the Rust test runs on identical bits.
//
// Usage: amiDump <output file>
// Research, education and V&V only (RESPONSIBLE_USE.md).

#include "faceAreaWeightAMI.H"
#include "primitivePatch.H"
#include "dictionary.H"
#include <cstdint>
#include <cstring>
#include <cstdio>
#include <cmath>
#include <vector>
#include <string>
#include <algorithm>

using namespace Foam;

static FILE* out = nullptr;

static unsigned long long bits(double x)
{
    std::uint64_t u;
    std::memcpy(&u, &x, sizeof u);
    return static_cast<unsigned long long>(u);
}

struct Geom
{
    std::vector<point> pts;
    std::vector<std::vector<label>> faces;
};

struct Opts
{
    bool requireMatch = true;
    bool reverseTarget = false;
    double lowWeightCorrection = -1;
    std::string triMode = "mesh";
    bool restart = true;
    double maxDistance2 = -1;
    double minCosAngle = -1;
    std::string areaNorm = "project";
};

// Structured grid in z = 0, normal +z (or -z when flip).
static Geom grid(const std::vector<double>& xs, const std::vector<double>& ys, bool flip)
{
    Geom g;
    const label nx = xs.size();
    for (double y : ys) for (double x : xs) g.pts.push_back(point(x, y, 0));
    for (size_t j = 0; j + 1 < ys.size(); ++j)
    {
        for (label i = 0; i + 1 < nx; ++i)
        {
            auto v = [&](label a, label b) { return label(b*nx + a); };
            std::vector<label> f{v(i, j), v(i + 1, j), v(i + 1, j + 1), v(i, j + 1)};
            if (flip) std::reverse(f.begin(), f.end());
            g.faces.push_back(f);
        }
    }
    return g;
}

static std::uint64_t lcgState;
static double lcg()
{
    lcgState = lcgState*6364136223846793005ULL + 1442695040888963407ULL;
    return double(lcgState >> 11)/double(1ULL << 53) - 0.5;
}

static void perturb(Geom& g, const std::vector<double>& xs, const std::vector<double>& ys, double amp, std::uint64_t seed)
{
    lcgState = seed;
    const size_t nx = xs.size();
    for (size_t j = 1; j + 1 < ys.size(); ++j)
    {
        for (size_t i = 1; i + 1 < nx; ++i)
        {
            const double hx = (xs[i + 1] - xs[i - 1])*0.5;
            const double hy = (ys[j + 1] - ys[j - 1])*0.5;
            point& p = g.pts[j*nx + i];
            p.x() += amp*hx*lcg();
            p.y() += amp*hy*lcg();
        }
    }
}

static std::vector<double> linspace(double a, double b, int n)
{
    std::vector<double> v;
    for (int i = 0; i <= n; ++i) v.push_back(a + (b - a)*i/n);
    return v;
}

static Geom cylinder(int nTheta, int nZ, double r, double h, double theta0, bool inward)
{
    Geom g;
    for (int k = 0; k <= nZ; ++k)
    {
        const double z = h*k/nZ;
        for (int j = 0; j < nTheta; ++j)
        {
            const double th = theta0 + 2.0*M_PI*j/nTheta;
            g.pts.push_back(point(r*std::cos(th), r*std::sin(th), z));
        }
    }
    auto v = [&](int j, int k) { return label(k*nTheta + (j % nTheta)); };
    for (int k = 0; k < nZ; ++k)
    {
        for (int j = 0; j < nTheta; ++j)
        {
            std::vector<label> f{v(j, k), v(j + 1, k), v(j + 1, k + 1), v(j, k + 1)};
            if (inward) std::reverse(f.begin(), f.end());
            g.faces.push_back(f);
        }
    }
    return g;
}

static void writeGeom(const char* tag, const Geom& g)
{
    std::fprintf(out, "%s_points %zu\n", tag, g.pts.size());
    for (const point& p : g.pts)
        std::fprintf(out, "%016llx %016llx %016llx\n", bits(p.x()), bits(p.y()), bits(p.z()));
    std::fprintf(out, "%s_faces %zu\n", tag, g.faces.size());
    for (const auto& f : g.faces)
    {
        std::fprintf(out, "%zu", f.size());
        for (label i : f) std::fprintf(out, " %d", int(i));
        std::fprintf(out, "\n");
    }
}

static void writeLabels(const char* tag, const labelListList& a)
{
    std::fprintf(out, "%s %d\n", tag, int(a.size()));
    for (const labelList& l : a)
    {
        std::fprintf(out, "%d", int(l.size()));
        for (label i : l) std::fprintf(out, " %d", int(i));
        std::fprintf(out, "\n");
    }
}

static void writeScalars(const char* tag, const scalarListList& a)
{
    std::fprintf(out, "%s %d\n", tag, int(a.size()));
    for (const scalarList& l : a)
    {
        std::fprintf(out, "%d", int(l.size()));
        for (scalar x : l) std::fprintf(out, " %016llx", bits(x));
        std::fprintf(out, "\n");
    }
}

static void writeList(const char* tag, const UList<scalar>& a)
{
    std::fprintf(out, "%s %d\n", tag, int(a.size()));
    for (scalar x : a) std::fprintf(out, "%016llx\n", bits(x));
}

static void runCase(const char* name, const Geom& src, const Geom& tgt, const Opts& o)
{
    std::fprintf(out, "case %s\n", name);
    std::fprintf(out, "opt require_match %d\n", int(o.requireMatch));
    std::fprintf(out, "opt reverse_target %d\n", int(o.reverseTarget));
    std::fprintf(out, "opt low_weight_correction %016llx\n", bits(o.lowWeightCorrection));
    std::fprintf(out, "opt tri_mode %s\n", o.triMode.c_str());
    std::fprintf(out, "opt restart %d\n", int(o.restart));
    std::fprintf(out, "opt max_distance2 %016llx\n", bits(o.maxDistance2));
    std::fprintf(out, "opt min_cos_angle %016llx\n", bits(o.minCosAngle));
    std::fprintf(out, "opt area_norm %s\n", o.areaNorm.c_str());
    writeGeom("src", src);
    writeGeom("tgt", tgt);

    auto toPatchData = [](const Geom& g, pointField& pts, faceList& faces)
    {
        pts.resize(g.pts.size());
        forAll(pts, i) pts[i] = g.pts[i];
        faces.resize(g.faces.size());
        forAll(faces, i)
        {
            faces[i].resize(g.faces[i].size());
            forAll(faces[i], k) faces[i][k] = g.faces[i][k];
        }
    };
    pointField srcPts, tgtPts;
    faceList srcFaces, tgtFaces;
    toPatchData(src, srcPts, srcFaces);
    toPatchData(tgt, tgtPts, tgtFaces);
    primitivePatch srcPatch(SubList<face>(srcFaces), srcPts);
    primitivePatch tgtPatch(SubList<face>(tgtFaces), tgtPts);

    dictionary dict;
    dict.add("requireMatch", o.requireMatch);
    dict.add("reverseTarget", o.reverseTarget);
    dict.add("lowWeightCorrection", o.lowWeightCorrection);
    dict.add("triMode", word(o.triMode));
    dict.add("restartUncoveredSourceFace", o.restart);
    dict.add("maxDistance2", o.maxDistance2);
    dict.add("minCosAngle", o.minCosAngle);
    dict.add("areaNormalisationMode", word(o.areaNorm));

    faceAreaWeightAMI ami(dict, o.reverseTarget);
    ami.calculate(srcPatch, tgtPatch);

    writeLabels("src_address", ami.srcAddress());
    writeScalars("src_weights", ami.srcWeights());
    writeList("src_weights_sum", ami.srcWeightsSum());
    writeList("src_mag_sf", ami.srcMagSf());
    {
        const pointListList& c = ami.srcCentroids();
        std::fprintf(out, "src_centroids %d\n", int(c.size()));
        for (const pointList& l : c)
        {
            std::fprintf(out, "%d", int(l.size()));
            for (const point& p : l)
                std::fprintf(out, " %016llx %016llx %016llx", bits(p.x()), bits(p.y()), bits(p.z()));
            std::fprintf(out, "\n");
        }
    }
    {
        const labelList& n = ami.srcNonOverlap();
        std::fprintf(out, "src_non_overlap %d\n", int(n.size()));
        for (label i : n) std::fprintf(out, " %d", int(i));
        std::fprintf(out, "\n");
    }
    writeLabels("tgt_address", ami.tgtAddress());
    writeScalars("tgt_weights", ami.tgtWeights());
    writeList("tgt_weights_sum", ami.tgtWeightsSum());
    writeList("tgt_mag_sf", ami.tgtMagSf());
    std::fprintf(out, "end\n");
}

int main(int argc, char* argv[])
{
    if (argc != 2)
    {
        std::fprintf(stderr, "usage: amiDump <output file>\n");
        return 1;
    }
    out = std::fopen(argv[1], "w");
    std::fprintf(out, "# amiDump gh:#1859, OpenFOAM ESI 6e2595746a396d688edd9890f4aebaaa7ff4ecfd faceAreaWeightAMI; values are IEEE-754 bit patterns\n");

    // 1. identical non-uniform grid, facing each other
    {
        const std::vector<double> xs{0.0, 0.2, 0.45, 0.8, 1.0}, ys{0.0, 0.3, 0.55, 1.0};
        Opts o;
        runCase("planar_identity", grid(xs, ys, false), grid(xs, ys, true), o);
        Opts r; r.reverseTarget = true; r.triMode = "fan"; r.requireMatch = false;
        runCase("planar_identity_reverse_target_fan", grid(xs, ys, false), grid(xs, ys, false), r);
    }
    // 2. 1-D strips, 2:1 and partial
    {
        const std::vector<double> ys{0.0, 0.7}, sx{0.0, 0.25, 0.5, 0.75, 1.0};
        Opts o; o.requireMatch = false;
        runCase("strips_2to1", grid(sx, ys, false), grid({0.0, 0.5, 1.0}, ys, true), o);
        runCase("strips_partial", grid(sx, ys, false), grid({0.0, 0.3, 1.0}, ys, true), o);
    }
    // 3. unstructured planar seam: perturbed grids, one hexagon (collinear points)
    {
        const auto sx = linspace(0.0, 1.3, 5), sy = linspace(0.0, 0.9, 4);
        Geom src = grid(sx, sy, false);
        perturb(src, sx, sy, 0.4, 7);
        const auto tx = linspace(0.0, 1.3, 3), ty = linspace(0.0, 0.9, 3);
        Geom tgt = grid(tx, ty, true);
        perturb(tgt, tx, ty, 0.4, 11);
        const label nx = tx.size();
        auto v = [&](label a, label b) { return label(b*nx + a); };
        tgt.faces[0] = {v(0, 0), v(0, 1), v(1, 1), v(2, 1), v(2, 0), v(1, 0)};
        tgt.faces.erase(tgt.faces.begin() + 1);
        for (const char* tm : {"mesh", "fan"})
        {
            for (const char* an : {"project", "mag"})
            {
                Opts o; o.triMode = tm; o.areaNorm = an;
                const std::string nm = std::string("unstructured_") + tm + "_" + an;
                runCase(nm.c_str(), src, tgt, o);
            }
        }
        Geom rev = tgt;
        std::reverse(rev.faces.begin(), rev.faces.end());
        Opts o;
        runCase("unstructured_reversed_target_order", src, rev, o);
        Opts nr; nr.restart = false;
        runCase("unstructured_reversed_no_restart", src, rev, nr);
    }
    // 4. concave source faces: an L-shaped hexagon and the rectangle filling its notch
    {
        Geom src;
        src.pts = {point(0,0,0), point(1,0,0), point(1,0.4,0), point(0.4,0.4,0), point(0.4,1,0), point(0,1,0), point(1,1,0)};
        src.faces = {{0,1,2,3,4,5}, {3,2,6,4}};
        const auto tx = linspace(0.0, 1.0, 3), ty = linspace(0.0, 1.0, 3);
        Geom tgt = grid(tx, ty, true);
        perturb(tgt, tx, ty, 0.3, 5);
        Opts o;
        runCase("concave_l_mesh", src, tgt, o);
        Opts f; f.triMode = "fan";
        runCase("concave_l_fan", src, tgt, f);
    }
    // 5. cylindrical seams
    {
        Opts o;
        runCase("cylinder_24x3_36x5", cylinder(24, 3, 0.5, 0.3, 0.0, false), cylinder(36, 5, 0.5, 0.3, 0.013, true), o);
        runCase("cylinder_coincident_lines", cylinder(24, 3, 0.5, 0.3, 0.0, false), cylinder(36, 4, 0.5, 0.3, 0.0, true), o);
        Opts c; c.minCosAngle = 0.99;
        runCase("cylinder_min_cos_angle", cylinder(24, 3, 0.5, 0.3, 0.0, false), cylinder(36, 5, 0.5, 0.3, 0.013, true), c);
        runCase("cylinder_identity", cylinder(16, 2, 0.4, 0.25, 0.1, false), cylinder(16, 2, 0.4, 0.25, 0.1, true), o);
    }
    // 6. a source face off the seam, rejected by maxDistance2, lowWeightCorrection on
    {
        Geom tgt = grid({0.0, 1.0}, {0.0, 1.0}, true);
        Geom src = grid({0.0, 1.0}, {0.0, 1.0}, false);
        const label base = src.pts.size();
        src.pts.push_back(point(5, 0, 1)); src.pts.push_back(point(6, 0, 1));
        src.pts.push_back(point(6, 1, 1)); src.pts.push_back(point(5, 1, 1));
        src.faces.push_back({base, base + 1, base + 2, base + 3});
        Opts o; o.maxDistance2 = 0.01; o.lowWeightCorrection = 0.2;
        runCase("off_seam_low_weight", src, tgt, o);
    }
    std::fclose(out);
    return 0;
}
