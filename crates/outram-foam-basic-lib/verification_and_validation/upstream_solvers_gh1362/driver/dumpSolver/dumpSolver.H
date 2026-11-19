/*---------------------------------------------------------------------------*\
  gh:#1362 helper (OUTRAM PARK's own code; it only CALLS upstream
  OpenFOAM-v2506 ESI, GPL-3.0-or-later, https://www.openfoam.com).

  An lduMatrix solver that does not solve. Selected with `solver dumpSolver;`
  in fvSolution (or loaded with `libs ("libdumpSolver.so")` into an upstream
  application), it prints the assembled matrix and source it is handed as
  IEEE-754 hex, plus every registered volScalarField internal field alive at
  that moment (this is how a model's local G, GbyNu, ... become visible), and
  keeps a copy in memory.

  With the environment variable DUMP_SOLVERS set it also runs the REAL
  upstream solvers listed in it (e.g. "PCG:DIC,PBiCGStab:DILU,smoothSolver:
  GaussSeidel") on the matrix for 1..DUMP_NITER iterations from the incoming
  psi, printing the residual history.
\*---------------------------------------------------------------------------*/
#ifndef dumpSolver_H
#define dumpSolver_H
#include "lduMatrix.H"
#include "objectRegistry.H"
#include "HashTable.H"
namespace Foam
{
class dumpSolver : public lduMatrix::solver
{
public:
    TypeName("dumpSolver");
    // registry of the case mesh, set by the host program (may stay null)
    static const objectRegistry* registry;
    // in-memory copies: field name -> {diag, upper, lower, source}
    static HashTable<List<scalarField>> captured;

    dumpSolver
    (
        const word& fieldName,
        const lduMatrix& matrix,
        const FieldField<Field, scalar>& interfaceBouCoeffs,
        const FieldField<Field, scalar>& interfaceIntCoeffs,
        const lduInterfaceFieldPtrsList& interfaces,
        const dictionary& solverControls
    );
    virtual ~dumpSolver() = default;
    virtual solverPerformance solve
    (
        scalarField& psi,
        const scalarField& source,
        const direction cmpt = 0
    ) const;
};
}
#endif
