// gh:#1362 row 6: ODE solvers against the UNMODIFIED OpenFOAM-v2506 (ESI) libODE.
// Transcribes applications/test/ODE/Test-ODE.C line for line (the testODE
// system, the 15 relTol = exp(-(i+1)) single steps from x = 1 with dxNext = 0.6,
// and the final solve to x = 2 at relTol 1e-4 from dxEst = 0.5), for the solvers
// Euler, RKF45 and Rosenbrock23. Every double is printed as its hex bit pattern.
#include "ODESystem.H"
#include "ODESolver.H"
#include "dictionary.H"
#include "scalarField.H"
#include <cstdint>
#include <cstdio>
#include <cstring>

using namespace Foam;

class testODE
:
    public ODESystem
{

public:

    testODE()
    {}

    label nEqns() const
    {
        return 4;
    }

    void derivatives
    (
        const scalar x,
        const scalarField& y,
        scalarField& dydx
    ) const
    {
        dydx[0] = -y[1];
        dydx[1] = y[0] - (1.0/x)*y[1];
        dydx[2] = y[1] - (2.0/x)*y[2];
        dydx[3] = y[2] - (3.0/x)*y[3];
    }

    void jacobian
    (
        const scalar x,
        const scalarField& y,
        scalarField& dfdx,
        scalarSquareMatrix& dfdy
    ) const
    {
        dfdx[0] = 0.0;
        dfdx[1] = (1.0/sqr(x))*y[1];
        dfdx[2] = (2.0/sqr(x))*y[2];
        dfdx[3] = (3.0/sqr(x))*y[3];

        dfdy(0, 0) = 0.0;
        dfdy(0, 1) = -1.0;
        dfdy(0, 2) = 0.0;
        dfdy(0, 3) = 0.0;

        dfdy(1, 0) = 1.0;
        dfdy(1, 1) = -1.0/x;
        dfdy(1, 2) = 0.0;
        dfdy(1, 3) = 0.0;

        dfdy(2, 0) = 0.0;
        dfdy(2, 1) = 1.0;
        dfdy(2, 2) = -2.0/x;
        dfdy(2, 3) = 0.0;

        dfdy(3, 0) = 0.0;
        dfdy(3, 1) = 0.0;
        dfdy(3, 2) = 1.0;
        dfdy(3, 3) = -3.0/x;
    }
};

static void hex(const char* what, const char* solver, int i, const double* v, int n)
{
    std::printf("%s %s %d", solver, what, i);
    for (int k = 0; k < n; k++)
    {
        std::uint64_t u;
        std::memcpy(&u, &v[k], 8);
        std::printf(" %016llx", (unsigned long long)u);
    }
    std::printf("\n");
}

static void runSolver(const char* name)
{
    testODE ode;

    dictionary dict;
    dict.add("solver", word(name));
    // Euler cannot reach x = 2 at relTol 1e-4 in the default 10000 steps (upstream
    // FatalError at x = 1.23094); allow enough steps for every solver.
    dict.add("maxSteps", 400000);

    autoPtr<ODESolver> odeSolver = ODESolver::New(ode, dict);

    scalar xStart = 1.0;
    scalarField yStart(ode.nEqns());
    yStart[0] = ::Foam::j0(xStart);
    yStart[1] = ::Foam::j1(xStart);
    yStart[2] = ::Foam::jn(2, xStart);
    yStart[3] = ::Foam::jn(3, xStart);
    {
        double v[4] = {yStart[0], yStart[1], yStart[2], yStart[3]};
        hex("ystart", name, 0, v, 4);
    }

    for (label i=0; i<15; i++)
    {
        scalar relTol = ::Foam::exp(-scalar(i + 1));

        scalar x = xStart;
        scalarField y(yStart);

        scalar dxEst = 0.6;
        scalar dxNext = dxEst;

        odeSolver->relTol() = relTol;
        odeSolver->solve(x, y, dxNext);

        // relTol, x - xStart, dxNext, y[0..3]
        double v[7] = {relTol, x - xStart, dxNext, y[0], y[1], y[2], y[3]};
        hex("step", name, i, v, 7);
    }

    scalar x = xStart;
    scalar xEnd = x + 1.0;
    scalarField y(yStart);

    scalar dxEst = 0.5;

    odeSolver->relTol() = 1e-4;
    odeSolver->solve(x, xEnd, y, dxEst);

    double v[5] = {y[0], y[1], y[2], y[3], dxEst};
    hex("final", name, 0, v, 5);
}

int main()
{
    runSolver("Euler");
    runSolver("RKF45");
    runSolver("Rosenbrock23");
    return 0;
}
