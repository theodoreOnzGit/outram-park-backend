/*---------------------------------------------------------------------------*\
  gh:#1363 row 2: upstream rhoCentralFoam (OpenFOAM-v2506, ESI) for the
  inviscid shockTube, with ONE substitution.

  This is `applications/solvers/compressible/rhoCentralFoam/rhoCentralFoam.C`
  line for line (directed interpolation, Kurganov flux, solves of rho, rhoU,
  rhoE through fvm::ddt + fvc::div, e = rhoE/rho - 0.5|U|^2, p = rho/psi),
  built on the unmodified upstream libOpenFOAM and libfiniteVolume. The ONLY
  difference is that psiThermo / the turbulence model / the thermophysical
  libraries (not built here, disk) are replaced by the same upstream
  thermodynamic classes used directly:

      constTransport<species::thermo<hConstThermo<perfectGas<specie>>,
                                     sensibleInternalEnergy>>

  and a transcription of hePsiThermo::calculate() (T = THE(e,p,T), psi) on the
  cells and on the boundary faces. The shockTube tutorial has mu = 0, so
  `inviscid` is true and upstream's viscous branches (turbulence->muEff(),
  tauMC, the laplacian solves) are not executed; the code below keeps the
  `inviscid` test and omits those branches. Fixed time step (no
  adjustTimeStep) so both codes take identical steps.

  This file only calls upstream. Output: IEEE-754 bit patterns, per cell, of
  rho, rhoU (x), T, p, e, psi at the steps listed in controlDict key
  `dumpSteps`.
\*---------------------------------------------------------------------------*/

#include "fvCFD.H"
#include "directionInterpolate.H"
#include "IOdictionary.H"
#include "specie.H"
#include "perfectGas.H"
#include "hConstThermo.H"
#include "sensibleInternalEnergy.H"
#include "thermo.H"
#include "constTransport.H"
#include <cstring>
#include <cstdio>
#include <cstdint>

using namespace Foam;

typedef constTransport
<
    species::thermo
    <
        hConstThermo<perfectGas<specie>>,
        sensibleInternalEnergy
    >
> MixtureType;

static void hexDump(const char* name, const scalarField& f)
{
    for (label i = 0; i < f.size(); ++i)
    {
        std::uint64_t b;
        const double v = f[i];
        std::memcpy(&b, &v, sizeof(b));
        std::printf("%s %d %016llx\n", name, int(i), (unsigned long long)b);
    }
}

// hePsiThermo::calculate(): cells and boundary faces
static void thermoCorrect
(
    const MixtureType& mix,
    const volScalarField& p,
    volScalarField& T,
    volScalarField& e,
    volScalarField& psi
)
{
    const scalarField& pCells = p.primitiveField();
    const scalarField& eCells = e.primitiveField();
    scalarField& TCells = T.primitiveFieldRef();
    scalarField& psiCells = psi.primitiveFieldRef();

    forAll(TCells, celli)
    {
        TCells[celli] = mix.THE(eCells[celli], pCells[celli], TCells[celli]);
        psiCells[celli] = mix.psi(pCells[celli], TCells[celli]);
    }

    volScalarField::Boundary& pBf = const_cast<volScalarField&>(p).boundaryFieldRef();
    volScalarField::Boundary& TBf = T.boundaryFieldRef();
    volScalarField::Boundary& psiBf = psi.boundaryFieldRef();
    volScalarField::Boundary& eBf = e.boundaryFieldRef();

    forAll(TBf, patchi)
    {
        fvPatchScalarField& pp = pBf[patchi];
        fvPatchScalarField& pT = TBf[patchi];
        fvPatchScalarField& ppsi = psiBf[patchi];
        fvPatchScalarField& phe = eBf[patchi];

        if (pT.fixesValue())
        {
            forAll(pT, facei)
            {
                phe[facei] = mix.HE(pp[facei], pT[facei]);
                ppsi[facei] = mix.psi(pp[facei], pT[facei]);
            }
        }
        else
        {
            forAll(pT, facei)
            {
                pT[facei] = mix.THE(phe[facei], pp[facei], pT[facei]);
                ppsi[facei] = mix.psi(pp[facei], pT[facei]);
            }
        }
    }
}

int main(int argc, char *argv[])
{
    #include "setRootCase.H"
    #include "createTime.H"
    #include "createMesh.H"

    IOdictionary thermoDict
    (
        IOobject
        (
            "thermophysicalProperties", runTime.constant(), mesh,
            IOobject::MUST_READ_IF_MODIFIED, IOobject::NO_WRITE
        )
    );
    const MixtureType mix(thermoDict.subDict("mixture"));

    {
        // constants of the mixture, bit patterns (R = RR/W, Cp, Cv at a sample state)
        scalarField k(4);
        k[0] = mix.R();
        k[1] = mix.Cp(1e5, 300.0);
        k[2] = mix.Cv(1e5, 300.0);
        k[3] = mix.W();
        std::printf("# CONSTANTS\n");
        hexDump("const", k);
    }

    IOdictionary ctrl
    (
        IOobject
        (
            "controlDict", runTime.system(), mesh,
            IOobject::MUST_READ, IOobject::NO_WRITE
        )
    );
    const labelList dumpSteps(ctrl.lookup("dumpSteps"));

    // --- createFields (thermo part replaced, see header)
    volScalarField p
    (
        IOobject("p", runTime.timeName(), mesh, IOobject::MUST_READ, IOobject::NO_WRITE),
        mesh
    );
    volScalarField T
    (
        IOobject("T", runTime.timeName(), mesh, IOobject::MUST_READ, IOobject::NO_WRITE),
        mesh
    );
    volScalarField psi
    (
        IOobject("thermo:psi", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
        mesh,
        dimensionedScalar(dimTime*dimTime/dimLength/dimLength, 0)
    );
    // he = HE(p, T) cells and boundary, as the thermo constructor
    volScalarField e
    (
        IOobject("e", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
        mesh,
        dimensionedScalar(sqr(dimLength)/sqr(dimTime), 0),
        T.boundaryField().types()
    );
    forAll(e.primitiveFieldRef(), celli)
    {
        e.primitiveFieldRef()[celli] = mix.HE(p[celli], T[celli]);
    }
    forAll(e.boundaryFieldRef(), patchi)
    {
        fvPatchScalarField& pe = e.boundaryFieldRef()[patchi];
        const fvPatchScalarField& pp = p.boundaryField()[patchi];
        const fvPatchScalarField& pT = T.boundaryField()[patchi];
        forAll(pe, facei)
        {
            pe[facei] = mix.HE(pp[facei], pT[facei]);
        }
    }
    thermoCorrect(mix, p, T, e, psi);

    volVectorField U
    (
        IOobject("U", runTime.timeName(), mesh, IOobject::MUST_READ, IOobject::NO_WRITE),
        mesh
    );

    volScalarField rho
    (
        IOobject("rho", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
        psi*p
    );
    volVectorField rhoU
    (
        IOobject("rhoU", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
        rho*U
    );
    volScalarField rhoE
    (
        IOobject("rhoE", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
        rho*(e + 0.5*magSqr(U))
    );
    surfaceScalarField pos
    (
        IOobject("pos", runTime.timeName(), mesh),
        mesh,
        dimensionedScalar("pos", dimless, 1.0)
    );
    surfaceScalarField neg
    (
        IOobject("neg", runTime.timeName(), mesh),
        mesh,
        dimensionedScalar("neg", dimless, -1.0)
    );
    surfaceScalarField phi("phi", fvc::flux(rhoU));

    // inviscid iff max(mu) == 0 (thermo.mu())
    bool inviscid(true);
    forAll(T.primitiveField(), celli)
    {
        if (mix.mu(p[celli], T[celli]) > 0.0)
        {
            inviscid = false;
        }
    }
    if (!inviscid)
    {
        FatalErrorInFunction << "viscous case: not supported by this driver"
            << abort(FatalError);
    }

    #include "readFluxScheme.H"
    const dimensionedScalar v_zero(dimVolume/dimTime, Zero);

    Info<< "\nStarting time loop\n" << endl;
    label step = 0;

    while (runTime.run())
    {
        ++runTime;
        ++step;

        // --- Directed interpolation of primitive fields onto faces
        surfaceScalarField rho_pos(interpolate(rho, pos));
        surfaceScalarField rho_neg(interpolate(rho, neg));

        surfaceVectorField rhoU_pos(interpolate(rhoU, pos, U.name()));
        surfaceVectorField rhoU_neg(interpolate(rhoU, neg, U.name()));

        volScalarField rPsi("rPsi", 1.0/psi);
        surfaceScalarField rPsi_pos(interpolate(rPsi, pos, T.name()));
        surfaceScalarField rPsi_neg(interpolate(rPsi, neg, T.name()));

        surfaceScalarField e_pos(interpolate(e, pos, T.name()));
        surfaceScalarField e_neg(interpolate(e, neg, T.name()));

        surfaceVectorField U_pos("U_pos", rhoU_pos/rho_pos);
        surfaceVectorField U_neg("U_neg", rhoU_neg/rho_neg);

        surfaceScalarField p_pos("p_pos", rho_pos*rPsi_pos);
        surfaceScalarField p_neg("p_neg", rho_neg*rPsi_neg);

        surfaceScalarField phiv_pos("phiv_pos", U_pos & mesh.Sf());
        phiv_pos.setOriented(false);
        surfaceScalarField phiv_neg("phiv_neg", U_neg & mesh.Sf());
        phiv_neg.setOriented(false);

        // thermo.Cp()/thermo.Cv() as heThermo::Cp/Cv: cells then boundary
        volScalarField Cp
        (
            IOobject("Cp", runTime.timeName(), mesh), mesh,
            dimensionedScalar(sqr(dimLength)/sqr(dimTime)/dimTemperature, 0)
        );
        volScalarField Cv
        (
            IOobject("Cv", runTime.timeName(), mesh), mesh,
            dimensionedScalar(sqr(dimLength)/sqr(dimTime)/dimTemperature, 0)
        );
        forAll(Cp.primitiveFieldRef(), celli)
        {
            Cp.primitiveFieldRef()[celli] = mix.Cp(p[celli], T[celli]);
            Cv.primitiveFieldRef()[celli] = mix.Cv(p[celli], T[celli]);
        }
        forAll(Cp.boundaryFieldRef(), patchi)
        {
            fvPatchScalarField& pCp = Cp.boundaryFieldRef()[patchi];
            fvPatchScalarField& pCv = Cv.boundaryFieldRef()[patchi];
            const fvPatchScalarField& pp = p.boundaryField()[patchi];
            const fvPatchScalarField& pT = T.boundaryField()[patchi];
            forAll(pCp, facei)
            {
                pCp[facei] = mix.Cp(pp[facei], pT[facei]);
                pCv[facei] = mix.Cv(pp[facei], pT[facei]);
            }
        }

        volScalarField c("c", sqrt(Cp/Cv*rPsi));
        surfaceScalarField cSf_pos
        (
            "cSf_pos",
            interpolate(c, pos, T.name())*mesh.magSf()
        );
        surfaceScalarField cSf_neg
        (
            "cSf_neg",
            interpolate(c, neg, T.name())*mesh.magSf()
        );

        surfaceScalarField ap
        (
            "ap",
            max(max(phiv_pos + cSf_pos, phiv_neg + cSf_neg), v_zero)
        );
        surfaceScalarField am
        (
            "am",
            min(min(phiv_pos - cSf_pos, phiv_neg - cSf_neg), v_zero)
        );

        surfaceScalarField a_pos("a_pos", ap/(ap - am));

        surfaceScalarField amaxSf("amaxSf", max(mag(am), mag(ap)));

        surfaceScalarField aSf("aSf", am*a_pos);

        if (fluxScheme == "Tadmor")
        {
            aSf = -0.5*amaxSf;
            a_pos = 0.5;
        }

        surfaceScalarField a_neg("a_neg", 1.0 - a_pos);

        phiv_pos *= a_pos;
        phiv_neg *= a_neg;

        surfaceScalarField aphiv_pos("aphiv_pos", phiv_pos - aSf);
        surfaceScalarField aphiv_neg("aphiv_neg", phiv_neg + aSf);

        // Reuse amaxSf for the maximum positive and negative fluxes
        // estimated by the central scheme
        amaxSf = max(mag(aphiv_pos), mag(aphiv_neg));

        Info<< "Time = " << runTime.timeName() << nl << endl;

        phi = aphiv_pos*rho_pos + aphiv_neg*rho_neg;

        surfaceVectorField phiU(aphiv_pos*rhoU_pos + aphiv_neg*rhoU_neg);
        phiU.setOriented(true);

        surfaceVectorField phiUp(phiU + (a_pos*p_pos + a_neg*p_neg)*mesh.Sf());

        surfaceScalarField phiEp
        (
            "phiEp",
            aphiv_pos*(rho_pos*(e_pos + 0.5*magSqr(U_pos)) + p_pos)
          + aphiv_neg*(rho_neg*(e_neg + 0.5*magSqr(U_neg)) + p_neg)
          + aSf*p_pos - aSf*p_neg
        );

        // --- Solve density
        solve(fvm::ddt(rho) + fvc::div(phi));

        // --- Solve momentum
        solve(fvm::ddt(rhoU) + fvc::div(phiUp));

        U.ref() = rhoU()/rho();
        U.correctBoundaryConditions();
        rhoU.boundaryFieldRef() == rho.boundaryField()*U.boundaryField();

        // (inviscid: the `if (!inviscid)` momentum solve is not executed)

        // --- Solve energy
        // (inviscid: sigmaDotU is zero: muEff = 0, tauMC = 0)
        solve
        (
            fvm::ddt(rhoE)
          + fvc::div(phiEp)
        );

        e = rhoE/rho - 0.5*magSqr(U);
        e.correctBoundaryConditions();
        thermoCorrect(mix, p, T, e, psi);
        rhoE.boundaryFieldRef() ==
            rho.boundaryField()*
            (
                e.boundaryField() + 0.5*magSqr(U.boundaryField())
            );

        p.ref() = rho()/psi();
        p.correctBoundaryConditions();
        rho.boundaryFieldRef() == psi.boundaryField()*p.boundaryField();

        bool dump = false;
        forAll(dumpSteps, i)
        {
            if (dumpSteps[i] == step) dump = true;
        }
        if (dump)
        {
            std::printf("# STEP %d\n", int(step));
            hexDump("rho", rho.primitiveField());
            scalarField ux(rhoU.primitiveField().component(0)/rho.primitiveField());
            hexDump("Ux", U.primitiveField().component(0));
            hexDump("T", T.primitiveField());
            hexDump("p", p.primitiveField());
            hexDump("e", e.primitiveField());
            hexDump("rhoU", rhoU.primitiveField().component(0));
            hexDump("rhoE", rhoE.primitiveField());
        }
    }

    Info<< "End\n" << endl;
    return 0;
}
