/*---------------------------------------------------------------------------*\
  gh:#1362 helper, see dumpSolver.H. OUTRAM PARK's own code; only CALLS
  upstream OpenFOAM-v2506 (ESI). No upstream source is copied.
\*---------------------------------------------------------------------------*/
#include "dumpSolver.H"
#include "volFields.H"
#include <cstdio>
#include <cstring>
#include <cstdint>
#include <cstdlib>
#include <string>

namespace Foam
{
defineTypeNameAndDebug(dumpSolver, 0);
lduMatrix::solver::addsymMatrixConstructorToTable<dumpSolver> addDumpSymConstructorToTable_;
lduMatrix::solver::addasymMatrixConstructorToTable<dumpSolver> addDumpAsymConstructorToTable_;
const objectRegistry* dumpSolver::registry = nullptr;
HashTable<List<scalarField>> dumpSolver::captured;

static void hexLine(std::FILE* f, const std::string& key, label i, scalar a)
{
    uint64_t u;
    std::memcpy(&u, &a, 8);
    std::fprintf(f, "%s %d %016llx\n", key.c_str(), int(i), (unsigned long long)u);
}

static void dumpField(std::FILE* f, const std::string& key, const scalarField& s)
{
    forAll(s, i) hexLine(f, key, i, s[i]);
}

// Run a real upstream solver for 1..nIter iterations from psi0 and print the
// residual after each (normalised initial/final residual as OpenFOAM defines).
static void history
(
    std::FILE* f,
    const word& fieldName,
    const lduMatrix& matrix,
    const scalarField& source,
    const scalarField& psi0,
    const std::string& solverName,
    const std::string& precond,
    label nIter
)
{
    const FieldField<Field, scalar> bou(0), intc(0);
    const lduInterfaceFieldPtrsList interfaces;
    for (label n = 1; n <= nIter; ++n)
    {
        dictionary d;
        d.add("solver", word(solverName));
        if (precond == "GaussSeidel" || precond == "symGaussSeidel")
        {
            d.add("smoother", word(precond));
            d.add("nSweeps", label(1));
        }
        else if (!precond.empty())
        {
            d.add("preconditioner", word(precond));
        }
        d.add("tolerance", scalar(0));
        d.add("relTol", scalar(0));
        d.add("minIter", n);
        d.add("maxIter", n);
        autoPtr<lduMatrix::solver> s = lduMatrix::solver::New
        (
            fieldName, matrix, bou, intc, interfaces, d
        );
        scalarField psi(psi0);
        solverPerformance perf = s->solve(psi, source);
        const std::string key = "hist_" + solverName + "_" + precond;
        hexLine(f, key + "_init", n, perf.initialResidual());
        hexLine(f, key + "_final", n, perf.finalResidual());
        hexLine(f, key + "_iters", n, scalar(perf.nIterations()));
        forAll(psi, i) hexLine(f, key + "_psi" + std::to_string(n), i, psi[i]);
    }
}
}

Foam::dumpSolver::dumpSolver
(
    const word& fieldName,
    const lduMatrix& matrix,
    const FieldField<Field, scalar>& interfaceBouCoeffs,
    const FieldField<Field, scalar>& interfaceIntCoeffs,
    const lduInterfaceFieldPtrsList& interfaces,
    const dictionary& solverControls
)
:
    lduMatrix::solver
    (
        fieldName, matrix, interfaceBouCoeffs, interfaceIntCoeffs,
        interfaces, solverControls
    )
{}

Foam::solverPerformance Foam::dumpSolver::solve
(
    scalarField& psi,
    const scalarField& source,
    const direction
) const
{
    const char* path = std::getenv("DUMP_FILE");
    std::FILE* f = path ? std::fopen(path, "a") : stdout;
    const word& n = fieldName_;
    const scalarField diag(matrix_.diag());
    const scalarField upper(matrix_.hasUpper() ? matrix_.upper() : scalarField());
    const scalarField lower(matrix_.hasLower() ? matrix_.lower() : scalarField());
    dumpField(f, "eq_" + n + "_diag", diag);
    dumpField(f, "eq_" + n + "_source", source);
    dumpField(f, "eq_" + n + "_upper", upper);
    dumpField(f, "eq_" + n + "_lower", lower);
    dumpField(f, "eq_" + n + "_psi", psi);
    List<scalarField> keep(4);
    keep[0] = diag;
    keep[1] = upper;
    keep[2] = lower;
    keep[3] = source;
    captured.set(n, keep);

    if (registry)
    {
        const HashTable<const DimensionedField<scalar, volMesh>*> reg
        (
            registry->lookupClass<DimensionedField<scalar, volMesh>>()
        );
        wordList names(reg.sortedToc());
        forAll(names, i)
        {
            const DimensionedField<scalar, volMesh>& fld = *reg[names[i]];
            std::string nm = names[i];
            for (char& c : nm)
            {
                if (c == ':' || c == ' ') c = '_';
            }
            dumpField(f, "reg_" + n + "_" + nm, fld.field());
        }
    }

    const char* sl = std::getenv("DUMP_SOLVERS");
    if (sl)
    {
        const char* ni = std::getenv("DUMP_NITER");
        const label nIter = ni ? atoi(ni) : 20;
        std::string all(sl);
        size_t pos = 0;
        while (pos <= all.size())
        {
            size_t e = all.find(',', pos);
            if (e == std::string::npos) e = all.size();
            const std::string item = all.substr(pos, e - pos);
            const size_t c = item.find(':');
            if (!item.empty())
            {
                history
                (
                    f, n, matrix_, source, psi, item.substr(0, c),
                    c == std::string::npos ? "" : item.substr(c + 1), nIter
                );
            }
            pos = e + 1;
        }
    }
    if (path) std::fclose(f);

    solverPerformance perf(typeName, fieldName_);
    perf.initialResidual() = 0;
    perf.finalResidual() = 0;
    perf.nIterations() = 0;
    perf.checkConvergence(tolerance_, relTol_);
    return perf;
}
