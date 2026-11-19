/*---------------------------------------------------------------------------*\
  gh:#1361 step 2 driver. NOT part of GeN-Foam: a small program written for
  the OUTRAM PARK differential comparison. It links GeN-Foam 652b3da0cc55's
  own libCommon (unmodified sources) built on OpenFOAM-ESI v2506 and prints,
  at 17 significant digits (round-trips a double), the results of

    radialBasisFunctionInterpolation::{solvePolyharmonicSpline (N-D),
                                       polyharmonicSpline (N-D)}
    scalarInterpolateTableGF  (operator(), integral, getNextPoint)
    timeProfile::value

  for inputs it also prints, so the Rust test reads inputs and reference from
  the one file. Inputs come from a splitmix64 generator (exact doubles).

  Usage:  genfoamCommonDump <caseDir>   (caseDir holds system/controlDict)
  Output: stdout; line format "TAG count v1 v2 ..." (strings as words).

  Licence: GPL-3.0-or-later, as the OUTRAM PARK workspace; written fresh.
\*---------------------------------------------------------------------------*/

#include "Time.H"
#include "dictionary.H"
#include "IStringStream.H"
#include "SquareMatrix.H"
#include "radialBasisFunctionInterpolation.H"
#include "InterpolateTablesGF.H"
#include "timeProfile.H"

#include <cstdio>
#include <cstdint>
#include <string>
#include <vector>

using namespace Foam;
using namespace radialBasisFunctionInterpolation;

// ---------------------------------------------------------------- generator
static uint64_t rngState = 0x1361'0000'0002ULL;

static double u01()
{
    // splitmix64
    uint64_t z = (rngState += 0x9E3779B97F4A7C15ULL);
    z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9ULL;
    z = (z ^ (z >> 27)) * 0x94D049BB133111EBULL;
    z = z ^ (z >> 31);
    return double(z >> 11) * (1.0 / 9007199254740992.0);
}

static double uni(double lo, double hi) { return lo + (hi - lo)*u01(); }

// ------------------------------------------------------------------ output
static void emit(const char* tag, const std::vector<double>& v)
{
    std::printf("%s %zu", tag, v.size());
    for (double x : v) std::printf(" %.17g", x);
    std::printf("\n");
}

static void emit(const char* tag, const scalarList& v)
{
    std::vector<double> w(v.begin(), v.end());
    emit(tag, w);
}

// --------------------------------------------------------------------- RBF
static void rbfCase
(
    const std::string& name,
    const label mode,
    const std::vector<std::vector<double>>& X, // X[param][data]
    const std::vector<double>& V,
    const std::vector<std::vector<double>>& Q, // Q[query][param]
    const bool dumpInverse
)
{
    const label p = X.size();
    const label n = X[0].size();

    List<scalarList> posList(p);
    forAll(posList, i)
    {
        posList[i] = scalarList(n);
        for (label j = 0; j < n; ++j) posList[i][j] = X[i][j];
    }
    scalarList values(n);
    for (label j = 0; j < n; ++j) values[j] = V[j];

    std::printf("RBF %s mode %d p %d n %zu nq %zu\n",
        name.c_str(), int(mode), int(p), X[0].size(), Q.size());
    std::vector<double> flatX;
    for (auto& r : X) flatX.insert(flatX.end(), r.begin(), r.end());
    emit("X", flatX);
    emit("V", V);
    std::vector<double> flatQ;
    for (auto& q : Q) flatQ.insert(flatQ.end(), q.begin(), q.end());
    emit("Q", flatQ);

    SquareMatrix<scalar> invRBFmatrix(0);
    scalarList w = solvePolyharmonicSpline(posList, values, invRBFmatrix, mode);

    if (dumpInverse)
    {
        std::vector<double> inv;
        for (label i = 0; i < invRBFmatrix.m(); ++i)
            for (label j = 0; j < invRBFmatrix.n(); ++j)
                inv.push_back(invRBFmatrix(i, j));
        emit("INV", inv);
    }
    else
    {
        emit("INV", std::vector<double>());
    }
    emit("W", w);

    std::vector<double> f;
    for (auto& q : Q)
    {
        scalarList x(p);
        for (label i = 0; i < p; ++i) x[i] = q[i];
        f.push_back(polyharmonicSpline(w, posList, x, mode));
    }
    emit("F", f);
}

static void rbfRandom
(
    const std::string& name, const label mode, const label p, const label n,
    const double scale, const double offset, const bool dumpInverse
)
{
    std::vector<std::vector<double>> X(p, std::vector<double>(n));
    for (auto& row : X)
        for (auto& x : row) x = offset + scale*u01();
    std::vector<double> V(n);
    for (label j = 0; j < n; ++j)
    {
        double s = 1.0;
        for (label i = 0; i < p; ++i) s += (0.3 + 0.1*i)*X[i][j]/scale;
        V[j] = s*(1.0 + 0.05*u01());
    }
    std::vector<std::vector<double>> Q;
    for (int k = 0; k < 8; ++k)
    {
        std::vector<double> q(p);
        for (auto& x : q) x = offset + scale*uni(-0.2, 1.2);
        Q.push_back(q);
    }
    // two queries sitting exactly on data points (rSquare == 0 branch)
    for (int k = 0; k < 2; ++k)
    {
        std::vector<double> q(p);
        for (label i = 0; i < p; ++i) q[i] = X[i][k];
        Q.push_back(q);
    }
    rbfCase(name, mode, X, V, Q, dumpInverse);
}

static void rbfAll()
{
    for (label mode = 1; mode <= 4; ++mode)
    {
        // upstream Test-radialBasisFunctions: 1-D
        rbfCase("test1D", mode, {{1.0, 2.0}}, {10.0, 20.0},
            {{0.1}, {1.0}, {2.0}, {3.0}, {1.5}}, true);
        // 2-D
        rbfCase("test2D", mode, {{1, 1, 2}, {2, 4, 2}}, {10, 20, 40},
            {{1, 2}, {1, 4}, {2, 2}, {1, 8}, {2, 4}, {1.3, 2.9}}, true);
        // 2-D small values (36 points)
        std::vector<double> xs{371.15}, ys{989.16}, vs{9.019256e-12};
        const double v[] = {
            8.745869e-12, 8.844590e-12, 8.916762e-12, 8.977144e-12, 9.025965e-12,
            8.748343e-12, 8.844299e-12, 8.920262e-12, 8.979478e-12, 9.023146e-12,
            8.748084e-12, 8.842483e-12, 8.921235e-12, 8.977545e-12, 9.028591e-12,
            8.746593e-12, 8.843715e-12, 8.920047e-12, 8.975632e-12, 9.024377e-12,
            8.745491e-12, 8.839599e-12, 8.916838e-12, 8.975146e-12, 9.028591e-12,
            8.743773e-12, 8.837589e-12, 8.921473e-12, 8.977415e-12, 9.026030e-12,
            8.741957e-12, 8.843273e-12, 8.916871e-12, 8.975114e-12, 9.026678e-12};
        for (int i = 0; i < 7; ++i)
            for (int j = 0; j < 5; ++j)
            {
                xs.push_back(300.0 + 100.0*i);
                ys.push_back(600.0 + 100.0*j);
                vs.push_back(v[5*i + j]);
            }
        rbfCase("testSmall", mode, {xs, ys}, vs,
            {{301.0, 600.0}, {350.0, 950.0}, {371.15, 989.16}, {512.5, 733.3}},
            true);
    }
    // random point sets, p parameters, n data points
    const label cfg[][2] = {{1, 6}, {2, 9}, {3, 12}, {4, 15}, {5, 20}};
    for (label mode = 1; mode <= 4; ++mode)
        for (auto& c : cfg)
        {
            rbfRandom
            (
                "rand_p" + std::to_string(c[0]) + "_n" + std::to_string(c[1]),
                mode, c[0], c[1], 1.0, 0.0, mode == 1
            );
        }
    // physical-scale set like a feedback table: Tfuel [K], density [kg/m3]
    rbfRandom("scale1000", 1, 2, 16, 700.0, 300.0, true);
    rbfRandom("scale1000", 2, 2, 16, 700.0, 300.0, false);
}

// ------------------------------------------------------------------- tables
static void tblCase
(
    const std::string& name,
    const InterpolateTableBaseGF::interpolationMethod method,
    const InterpolateTableBaseGF::outofBoundsMehtod ob,
    const scalarField& xs, const scalarField& ys,
    const std::vector<double>& qIn, const std::vector<double>& qOut
)
{
    scalarInterpolateTableGF t(xs, ys, method, ob);
    std::printf("TBL %s method %d ob %d n %d\n", name.c_str(), int(method),
        int(ob), int(xs.size()));
    emit("XS", xs);
    emit("YS", ys);
    emit("QIN", qIn);
    std::vector<double> v;
    for (double q : qIn) v.push_back(t(q));
    emit("VIN", v);
    if (ob != InterpolateTableBaseGF::ERROR)
    {
        emit("QOUT", qOut);
        std::vector<double> vo;
        for (double q : qOut) vo.push_back(t(q));
        emit("VOUT", vo);
    }
    else
    {
        emit("QOUT", std::vector<double>());
        emit("VOUT", std::vector<double>());
    }
    // integrals: powers of the abscissae (xs > 0 for fractional k)
    std::vector<double> ks{1.0, 2.0, 3.0, 0.5, 1.5};
    emit("K", ks);
    std::vector<double> iv;
    for (double k : ks) iv.push_back(t.integral(k));
    emit("INT", iv);
    // getNextPoint for interior queries strictly below the last abscissa
    std::vector<double> nq, nv;
    for (double q : qIn)
        if (q < xs[xs.size() - 1]) { nq.push_back(q); nv.push_back(t.getNextPoint(q)); }
    emit("NQ", nq);
    emit("NP", nv);
}

static void tblAll()
{
    const InterpolateTableBaseGF::interpolationMethod ms[] =
        {InterpolateTableBaseGF::STEP, InterpolateTableBaseGF::LINEAR};
    const InterpolateTableBaseGF::outofBoundsMehtod obs[] =
        {InterpolateTableBaseGF::ERROR, InterpolateTableBaseGF::EXTRAPOLATION,
         InterpolateTableBaseGF::FIXED};
    for (int tbl = 0; tbl < 3; ++tbl)
    {
        const int n = 4 + 5*tbl;               // 4, 9, 14 knots
        scalarField xs(n), ys(n);
        double x = 0.1 + 0.4*u01();
        for (int i = 0; i < n; ++i)
        {
            xs[i] = x;
            ys[i] = uni(-3.0, 8.0);
            x += 0.05 + 0.9*u01();
        }
        std::vector<double> qIn, qOut;
        for (int i = 0; i < n; ++i) qIn.push_back(xs[i]);           // knots
        for (int i = 0; i + 1 < n; ++i) qIn.push_back(0.5*(xs[i] + xs[i + 1]));
        for (int i = 0; i < 12; ++i) qIn.push_back(uni(xs[0], xs[n - 1]));
        for (int i = 0; i < 4; ++i) qOut.push_back(xs[0] - uni(0.01, 2.0));
        for (int i = 0; i < 4; ++i) qOut.push_back(xs[n - 1] + uni(0.01, 2.0));
        for (auto m : ms)
            for (auto ob : obs)
                tblCase("rand" + std::to_string(tbl), m, ob, xs, ys, qIn, qOut);
    }
}

// ------------------------------------------------------------ time profiles
static void tpCase
(
    const Time& runTime, const std::string& name, const std::string& dictText,
    const std::vector<double>& qs
)
{
    IStringStream is(dictText);
    dictionary dict(is);
    timeProfile tp(dict, runTime);
    std::printf("TP %s\n", name.c_str());
    std::printf("DICT %s\n", dictText.c_str());
    emit("Q", qs);
    std::vector<double> v;
    for (double q : qs) v.push_back(tp.value(q));
    emit("V", v);
}

static std::string fmt(double x)
{
    char b[64];
    std::snprintf(b, sizeof b, "%.17g", x);
    return b;
}

static void tpAll(const Time& runTime)
{
    tpCase(runTime, "const", "type constant; constant 3.25;",
        {-5.0, 0.0, 1.0, 1e6});
    for (int tbl = 0; tbl < 4; ++tbl)
    {
        const int n = 3 + 4*tbl;
        std::vector<double> ts, vs;
        double t = uni(0.0, 0.5);
        std::string table = "table table (";
        for (int i = 0; i < n; ++i)
        {
            ts.push_back(t);
            vs.push_back(uni(-1e-3, 5e-3));
            table += "(" + fmt(ts.back()) + " " + fmt(vs.back()) + ")";
            t += 0.1 + 2.0*u01();
        }
        table += ");";
        for (double start : {0.0, 3.5, 100.0})
        {
            std::vector<double> qs;
            for (int i = 0; i < n; ++i) qs.push_back(ts[i] + start);
            for (int i = 0; i + 1 < n; ++i)
                qs.push_back(0.5*(ts[i] + ts[i + 1]) + start);
            for (int i = 0; i < 40; ++i)
                qs.push_back(start + uni(ts[0], ts[n - 1]));
            qs.push_back(start + ts[0] - 1.0);
            qs.push_back(start + ts[n - 1] + 1.0);
            tpCase(runTime, "table" + std::to_string(tbl) + "_start" + fmt(start),
                "type table; startTime " + fmt(start) + "; " + table, qs);
        }
    }
}

int main(int argc, char* argv[])
{
    if (argc < 2) { std::fprintf(stderr, "usage: %s caseDir\n", argv[0]); return 2; }
    // Time(controlDictName, rootPath, caseName): caseDir = root/case
    fileName caseDir(argv[1]);
    Time runTime(Time::controlDictName, caseDir.path(), caseDir.name());

    std::printf("# genfoamCommonDump: GeN-Foam 652b3da0cc55 libCommon on OpenFOAM-ESI v2506\n");
    rbfAll();
    tblAll();
    tpAll(runTime);
    return 0;
}
