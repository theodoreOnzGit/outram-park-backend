// pkRunLF: gh:#1882 (GeN-Foam port, round 48). Subclasses GeN-Foam 652b3da's
// solvers::pointKineticNeutronics with `liquidFuel true` (controlDict) and drives the REAL
// correctPhysics() (solvePointKineticsLiquidFuel.H) step by step on a real fvMesh and Time, on
// the neutroRegion mesh of Tutorials/reactorCases/1D_MSR_pointKinetics (640 cells, CYCLIC
// top/bottom, wall and empty patches), with PRESCRIBED fluid fields (U, alpha, alphat, mu) and
// feedback fields (TFuel_ ...) set each step from integer formulas that the Rust side repeats
// exactly. Everything is printed at 17 significant digits. The real class' own Info output (neutron
// iterations, neutroResidual, intermediate power, PBiCG performance lines) is kept by the
// script: this driver writes "STEP k" markers on the same stream.
// Before each step (the equilibrium matrix without ddt in the first, the full matrix after) it also
// rebuilds the precursor matrix of the FIRST neutron iteration exactly as
// solvePointKineticsLiquidFuel.H writes it and prints diag, upper, lower, source and the boundary
// coefficients of every patch ("M" lines), so a mismatch of the port can be located in the matrix.
// Original OUTRAM PARK verification code; links, does not modify, upstream.
#include "fvCFD.H"
#include "pointKineticNeutronics.H"
#include <cstdio>
#include <cmath>
#include <cstdlib>
#include <string>

namespace Foam {

static double frac(long i, long k, long a, long b, long m)
{
    return double((a*i + b*k) % m)/double(m);
}

class probeLF : public solvers::pointKineticNeutronics
{
public:
    probeLF(fvMesh& m) : solvers::pointKineticNeutronics(m) {}

    static scalar sumField(const volScalarField& f)
    {
        scalar s = 0;
        forAll(f, i) s += f[i];
        return s;
    }

    void printConfig() const
    {
        printf("CFG pgt %.17g\n", promptGenerationTime_);
        printf("CFG nbeta %d\n", int(betas_.size()));
        forAll(betas_, i) printf("CFG beta %d %.17g\nCFG lambda %d %.17g\n", int(i), betas_[i], int(i), lambdas_[i]);
        printf("CFG coeff %.17g %.17g %.17g %.17g %.17g %.17g %.17g %.17g\n",
               coeffDoppler_, coeffTFuel_, coeffTClad_, coeffTCool_, coeffRhoCool_,
               coeffTStruct_, coeffTStructMech_, coeffDrivelineExp_);
        printf("CFG fast %d\n", int(fastNeutrons_));
        printf("CFG liquid %d initPrec %d ScNo %.17g\n", int(liquidFuel_), int(initPrecursorsLiquidFuel_), ScNo_);
        printf("CFG power0 %.17g powerOld0 %.17g beta %.17g\n", power_, powerOld_, beta_);
        printf("CFG energyGroups %d\n", int(energyGroups_));
        printf("CFG sumFlux0 %.17g sumInitN %.17g domInt %.17g\n", sumField(oneGroupFlux_), sumField(initOneGroupFluxN_),
               domainIntegratedInitOneGroupFluxN_);
        printf("CFG sumFuelCells %.17g sumCoolCells %.17g sumStructCells %.17g sumStructMechCells %.17g sumDriveCells %.17g\n",
               sumField(fuelFeedbackCellField_), sumField(coolFeedbackCellField_),
               sumField(structFeedbackCellField_), sumField(structMechFeedbackCellField_),
               sumField(drivelineFeedbackCellField_));
        printf("CFG nCells %d\n", int(mesh_.nCells()));
        printf("VOL");
        forAll(mesh_.V(), i) printf(" %.17g", mesh_.V()[i]);
        printf("\n");
        printf("INITN");
        forAll(initOneGroupFluxN_, i) printf(" %.17g", initOneGroupFluxN_[i]);
        printf("\n");
        forAll(mesh_.cellZones(), zi)
        {
            printf("ZONE %s %d", mesh_.cellZones()[zi].name().c_str(), int(mesh_.cellZones()[zi].size()));
            forAll(mesh_.cellZones()[zi], j) printf(" %d", int(mesh_.cellZones()[zi][j]));
            printf("\n");
        }
        fflush(stdout);
    }

    // Mesh geometry the precursor matrix is built from (internal faces and every patch).
    void printGeometry() const
    {
        const fvMesh& m = mesh_;
        printf("G nInternal %d\n", int(m.nInternalFaces()));
        printf("G C");
        forAll(m.C(), i) printf(" %.17g %.17g %.17g", m.C()[i].x(), m.C()[i].y(), m.C()[i].z());
        printf("\n");
        printf("G Sf");
        for (label f = 0; f < m.nInternalFaces(); ++f) printf(" %.17g %.17g %.17g", m.Sf()[f].x(), m.Sf()[f].y(), m.Sf()[f].z());
        printf("\n");
        printf("G Cf");
        for (label f = 0; f < m.nInternalFaces(); ++f) printf(" %.17g %.17g %.17g", m.Cf()[f].x(), m.Cf()[f].y(), m.Cf()[f].z());
        printf("\n");
        printf("G magSf");
        for (label f = 0; f < m.nInternalFaces(); ++f) printf(" %.17g", m.magSf()[f]);
        printf("\n");
        printf("G w");
        for (label f = 0; f < m.nInternalFaces(); ++f) printf(" %.17g", m.weights()[f]);
        printf("\n");
        printf("G nonOrthDelta");
        for (label f = 0; f < m.nInternalFaces(); ++f) printf(" %.17g", m.nonOrthDeltaCoeffs()[f]);
        printf("\n");
        printf("G nonOrthCorr");
        for (label f = 0; f < m.nInternalFaces(); ++f)
            printf(" %.17g %.17g %.17g", m.nonOrthCorrectionVectors()[f].x(), m.nonOrthCorrectionVectors()[f].y(), m.nonOrthCorrectionVectors()[f].z());
        printf("\n");
        forAll(m.boundary(), pi)
        {
            const fvPatch& p = m.boundary()[pi];
            printf("GP %d %s %s %d coupled %d\n", int(pi), p.name().c_str(), p.type().c_str(), int(p.size()), int(p.coupled()));
            if (p.size() == 0) continue;
            const label n = (p.size() > 4 ? 4 : p.size());   // wall patches: first faces only
            printf("GP w");           for (label i = 0; i < n; ++i) printf(" %.17g", m.weights().boundaryField()[pi][i]);       printf("\n");
            printf("GP nonOrthDelta");for (label i = 0; i < n; ++i) printf(" %.17g", m.nonOrthDeltaCoeffs().boundaryField()[pi][i]); printf("\n");
            printf("GP nonOrthCorr"); for (label i = 0; i < n; ++i) printf(" %.17g %.17g %.17g", m.nonOrthCorrectionVectors().boundaryField()[pi][i].x(), m.nonOrthCorrectionVectors().boundaryField()[pi][i].y(), m.nonOrthCorrectionVectors().boundaryField()[pi][i].z()); printf("\n");
            printf("GP magSf");       for (label i = 0; i < n; ++i) printf(" %.17g", m.magSf().boundaryField()[pi][i]);          printf("\n");
            printf("GP Sf");          for (label i = 0; i < n; ++i) printf(" %.17g %.17g %.17g", m.Sf().boundaryField()[pi][i].x(), m.Sf().boundaryField()[pi][i].y(), m.Sf().boundaryField()[pi][i].z()); printf("\n");
            printf("GP Cf");          for (label i = 0; i < n; ++i) printf(" %.17g %.17g %.17g", m.Cf().boundaryField()[pi][i].x(), m.Cf().boundaryField()[pi][i].y(), m.Cf().boundaryField()[pi][i].z()); printf("\n");
            printf("GP faceCells");   for (label i = 0; i < n; ++i) printf(" %d", int(p.faceCells()[i])); printf("\n");
        }
        fflush(stdout);
    }

    // Integer-formula fields of this step: feedback fields as in round 36, the prescribed fluid fields
    // and (optionally) alpha, set by the two update paths a coupled run could use.
    void setFields(long k, const std::string& flow, const std::string& alphaMode)
    {
        forAll(TFuel_, i)
        {
            TFuel_[i] = 900.0 + 150.0*frac(i, k, 3, 5, 97);
            TClad_[i] = 850.0 + 60.0*frac(i, k, 5, 3, 89);
            TCool_[i] = 800.0 + 50.0*frac(i, k, 7, 2, 83);
            rhoCool_[i] = 3300.0 - 80.0*frac(i, k, 2, 9, 79);
            TStruct_[i] = 780.0 + 30.0*frac(i, k, 11, 4, 73);
            TStructMech_[i] = 700.0 + 20.0*frac(i, k, 13, 6, 71);
        }
        volVectorField& U = UPtr_();
        forAll(U, i)
        {
            double uz = 0.0;
            if (flow == "flow") uz = 0.4 + 0.3*frac(i, k, 7, 3, 53);
            else if (flow == "rev") uz = frac(i, k, 7, 3, 53) - 0.4;
            else if (flow == "still") uz = 0.0;
            else if (flow == "slow") uz = 0.02 + 0.01*frac(i, k, 7, 3, 53);
            U[i] = vector(0.0, 0.0, uz);
        }
        U.correctBoundaryConditions();
        volScalarField& at = alphatPtr_();
        volScalarField& mu = muPtr_();
        forAll(at, i)
        {
            mu[i] = 4.0 + 2.0*frac(i, k, 5, 7, 43);
            at[i] = 2.0 + 3.0*frac(i, k, 3, 11, 41);
        }
        at.correctBoundaryConditions();
        mu.correctBoundaryConditions();
        if (alphaMode != "const")
        {
            volScalarField anew(alphaPtr_());
            forAll(anew, i) anew[i] = 0.5 + 0.5*frac(i, k, 5, 2, 47);
            if (alphaMode == "assign")
            {
                // operator= : storeOldTimes() first, so alpha.oldTime() is the previous step's alpha
                alphaPtr_() = anew;
            }
            else
            {
                // element writes do not store the old time: oldTime() copies the CURRENT value at the first ddt
                forAll(anew, i) alphaPtr_()[i] = anew[i];
            }
            alphaPtr_().correctBoundaryConditions();
        }
    }

    void setPD()
    {
        forAll(powerDensity_, i)
        {
            powerDensity_[i] = 1.0e6 + 3.0e5*frac(i, 1, 17, 1, 61);
            secondaryPowerDensity_[i] = 2.0e5 + 1.0e5*frac(i, 1, 19, 1, 59);
        }
    }

    void printScalarList(const char* tag, const Foam::scalarField& f) const
    {
        printf("%s", tag);
        forAll(f, i) printf(" %.17g", f[i]);
        printf("\n");
    }

    // The matrix of the first neutron iteration of this step, as solvePointKineticsLiquidFuel.H writes it.
    // withDdt false: the equilibrium matrix of the initialisation (no ddt).
    void dumpMatrix(long k, bool withDdt)
    {
        diffCoeffPrecPtr_() = (alphatPtr_()+muPtr_()/ScNo_)/rhoCool_;
        phiPtr_()=fvc::flux(UPtr_()*alphaPtr_());
        printf("PHI %ld", k);
        forAll(phiPtr_().primitiveField(), f) printf(" %.17g", phiPtr_().primitiveField()[f]);
        printf("\n");
        forAll(phiPtr_().boundaryField(), pi)
        {
            if (mesh_.boundary()[pi].size() == 0) continue;
            printf("PHIB %ld %d", k, int(pi));
            const label n = (mesh_.boundary()[pi].size() > 4 ? 4 : mesh_.boundary()[pi].size());
            for (label i = 0; i < n; ++i) printf(" %.17g", phiPtr_().boundaryField()[pi][i]);
            printf("\n");
        }
        printScalarList("DIFF", diffCoeffPrecPtr_().primitiveField());
        forAll(precPK_, precI)
        {
            if (!getenv("PK_ALLGROUPS") && precI != 0 && precI != 7) continue;
            fvMatrix<scalar> precEq
            (
                withDdt
              ? (
                    fvm::ddt(alphaPtr_(), precPKStar_[precI])
                  + fvm::Sp(dimensionedScalar("",dimless/dimTime,lambdas_[precI])*alphaPtr_(), precPKStar_[precI])
                  - initOneGroupFluxN_*dimensionedScalar("",dimPower,power_)*betas_[precI]/
                    dimensionedScalar("",dimTime,promptGenerationTime_)
                  + fvm::div(phiPtr_(), precPKStar_[precI], "div(phi_,precStar_)")
                  - fvm::laplacian(diffCoeffPrecPtr_(), precPKStar_[precI])
                )
              : (
                    fvm::Sp(dimensionedScalar("",dimless/dimTime,lambdas_[precI])*alphaPtr_(), precPKStar_[precI])
                  - initOneGroupFluxN_*dimensionedScalar("",dimPower,power_)*betas_[precI]/
                    dimensionedScalar("",dimTime,promptGenerationTime_)
                  + fvm::div(phiPtr_(), precPKStar_[precI], "div(phi_,precStar_)")
                  - fvm::laplacian(diffCoeffPrecPtr_(), precPKStar_[precI])
                )
            );
            printf("M %ld %d n %d\n", k, int(precI), int(precEq.diag().size()));
            printScalarList("MD", precEq.diag());
            printScalarList("MU", precEq.upper());
            printScalarList("ML", precEq.lower());
            printf("MS");
            forAll(precEq.source(), i) printf(" %.17g", precEq.source()[i]);
            printf("\n");
            forAll(precEq.internalCoeffs(), pi)
            {
                if (precEq.internalCoeffs()[pi].size() == 0) continue;
                const label n = (precEq.internalCoeffs()[pi].size() > 4 ? 4 : precEq.internalCoeffs()[pi].size());
                printf("MI %d %d", int(pi), int(precEq.internalCoeffs()[pi].size()));
                for (label i = 0; i < n; ++i) printf(" %.17g", precEq.internalCoeffs()[pi][i]);
                printf("\nMB %d %d", int(pi), int(precEq.boundaryCoeffs()[pi].size()));
                for (label i = 0; i < n; ++i) printf(" %.17g", precEq.boundaryCoeffs()[pi][i]);
                printf("\n");
            }
            if (getenv("PK_SOLVE_MIRROR"))
            {
                // Solve this matrix once on the real field and put the field back: isolates the linear solver.
                scalarField saved(precPKStar_[precI].primitiveField());
                SolverPerformance<scalar> sp = precEq.solve();
                printf("SV %ld %d %d %.17g %.17g", k, int(precI), int(sp.nIterations()), sp.initialResidual(), sp.finalResidual());
                forAll(precPKStar_[precI], i) printf(" %.17g", precPKStar_[precI][i]);
                printf("\n");
                precPKStar_[precI].primitiveFieldRef() = saved;
                precPKStar_[precI].correctBoundaryConditions();
            }
        }
        fflush(stdout);
    }

    void stepAndPrint(long idx, scalar t, scalar dt, bool full)
    {
        const scalar pOld = power_;
        Info<< "STEP " << idx << endl;
        correctPhysics();
        printf("K %ld %.17g %.17g | %.17g %.17g %.17g %.17g %.17g %.17g | %.17g %.17g %.17g %.17g | %.17g %.17g %.17g %.17g",
               idx, t, dt, power_, powerOld_, totalReactivity_, precEquilibriumReactivity_, liquidFuelBeta_,
               maxDeltaT(), sumField(oneGroupFlux_),
               energyGroups_ > 0 ? sumField(fluxes_[energyGroups_-1]) : 0.0,
               sumField(powerDensity_), sumField(secondaryPowerDensity_),
               TFuelRef_, TCladRef_, TCoolRef_, pOld);
        printf(" | %.17g %.17g %.17g %.17g", rhoCoolRef_, TStructRef_, TStructMechRef_, TDrivelineRef_);
        printf("\n");
        forAll(precPKStar_, g)
        {
            printf("P %ld %d %.17g %.17g | %.17g %.17g %.17g | %.17g %.17g\n", idx, int(g),
                   sumField(precPKStar_[g]), sumField(precPK_[g]),
                   precPKStar_[g][0], precPKStar_[g][mesh_.nCells()/2], precPKStar_[g][mesh_.nCells()-1],
                   precPK_[g][0], precPK_[g][mesh_.nCells()-1]);
            if (full)
            {
                printf("PS %ld %d", idx, int(g));
                forAll(precPKStar_[g], i) printf(" %.17g", precPKStar_[g][i]);
                printf("\n");
            }
        }
        printf("F %ld %.17g %.17g %.17g\n", idx, oneGroupFlux_[0], oneGroupFlux_[mesh_.nCells()/2], oneGroupFlux_[mesh_.nCells()-1]);
        fflush(stdout);
    }
};

} // namespace Foam

using namespace Foam;

int main(int argc, char* argv[])
{
    #include "setRootCase.H"
    #include "createTime.H"
    Info.stdStream().precision(17);
    int nSteps = 120;
    if (const char* e = getenv("PK_STEPS")) nSteps = atoi(e);
    std::string flow = "flow";
    if (const char* e = getenv("PK_FLOW")) flow = e;
    std::string alphaMode = "const";
    if (const char* e = getenv("PK_ALPHA")) alphaMode = e;
    bool dumpMat = true;
    if (const char* e = getenv("PK_MATRIX")) dumpMat = atoi(e) != 0;
    fvMesh mesh
    (
        IOobject("neutroRegion", runTime.timeName(), runTime, IOobject::MUST_READ)
    );
    probeLF pk(mesh);
    pk.printConfig();
    printf("CFG flow %s alpha %s\n", flow.c_str(), alphaMode.c_str());
    if (getenv("PK_GEOM")) pk.printGeometry();
    pk.setPD();
    const double dts[] = {1e-3, 2e-3, 5e-3, 1e-2, 2e-2, 5e-2, 1e-1, 2e-1};
    for (int k = 0; k < nSteps; ++k)
    {
        const double dt = dts[k % 8];
        runTime.setDeltaT(dt);
        runTime++;
        const scalar t = runTime.timeOutputValue();
        pk.setFields(k, flow, alphaMode);
        if (dumpMat && (k == 0 || k == 1 || k == 2 || k == 7 || k == 40))
        {
            pk.dumpMatrix(k, k > 0);
        }
        const bool full = (k == 0 || k == 1 || k == 2 || k == 3 || k == 10 || k == 40 || k == nSteps-1);
        pk.stepAndPrint(k, t, dt, full);
    }
    return 0;
}
