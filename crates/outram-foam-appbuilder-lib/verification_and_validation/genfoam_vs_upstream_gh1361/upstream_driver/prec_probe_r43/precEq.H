forAll(prec_,precI)
{
    if(liquidFuel_)
    {
        fvScalarMatrix precEq
        (
            fvm::ddt(alphaPtr_()*(1-eigenvalueNeutronics_), precStar_[precI])
        +   fvm::Sp(lambda[precI]*alphaPtr_(), precStar_[precI])
        -   neutroSource_/keff_*Beta[precI]
        +   fvm::div(phiPtr_(), precStar_[precI], "div(phi_,precStar_)")
        -   fvm::laplacian(diffCoeffPrecPtr_(), precStar_[precI])                
        );
        int r43This = -1;
        {
            static int r43Call = 0;
            if (r43Call < 16)
            {
                r43This = r43Call;
                if (r43Call == 0)
                {
                    OFstream og("r43dump_geom.txt");
                    og.precision(17);
                    og << "Sf " << mesh_.Sf().primitiveField() << nl;
                    og << "Cf " << mesh_.Cf().primitiveField() << nl;
                    og << "C " << mesh_.C().primitiveField() << nl;
                    og << "w " << mesh_.weights().primitiveField() << nl;
                    og << "nonOrthDeltaCoeffs " << mesh_.nonOrthDeltaCoeffs().primitiveField() << nl;
                    og << "V " << mesh_.V() << nl;
                    {
                        const labelList bad({13218, 13219, 13221, 13252, 13159});
                        forAll(bad, bi)
                        {
                            const face& ff = mesh_.faces()[bad[bi]];
                            og << "FACE " << bad[bi] << " moving " << mesh_.moving() << " Sf " << mesh_.faceAreas()[bad[bi]] << " pts";
                            forAll(ff, pj) { og << " " << mesh_.points()[ff[pj]]; }
                            og << nl;
                        }
                        vectorField fc2, fa2;
                        primitiveMeshTools::makeFaceCentresAndAreas(mesh_.faces(), mesh_.points(), fc2, fa2);
                        forAll(bad, bi) { og << "RECOMP " << bad[bi] << " " << fa2[bad[bi]] << nl; }
                        label nDiff = 0;
                        forAll(fa2, fi2) { if (fa2[fi2] != mesh_.faceAreas()[fi2]) { nDiff++; } }
                        og << "RECOMPDIFF " << nDiff << " of " << fa2.size() << nl;
                    }
                    forAll(mesh_.Sf().boundaryField(), pI)
                    {
                        og << "SfB" << pI << " " << mesh_.Sf().boundaryField()[pI] << nl;
                        og << "nonOrthDeltaCoeffsB" << pI << " " << mesh_.nonOrthDeltaCoeffs().boundaryField()[pI] << nl;
                    }
                }
                OFstream os("r43dump_prec_" + Foam::name(r43Call) + ".txt");
                os.precision(17);
                os << "call " << r43Call << " precI " << precI << " keff " << keff_
                   << " deltaT " << mesh_.time().deltaTValue() << " eig " << eigenvalueNeutronics_ << nl;
                os << "neutroSource " << neutroSource_.primitiveField() << nl;
                os << "Beta " << Beta[precI].primitiveField() << nl;
                os << "lambda " << lambda[precI].primitiveField() << nl;
                os << "alpha " << alphaPtr_().primitiveField() << nl;
                os << "diffCoeffPrec " << diffCoeffPrecPtr_().primitiveField() << nl;
                os << "rhoCool " << rhoCool_.primitiveField() << nl;
                os << "alphat " << alphatPtr_().primitiveField() << nl;
                os << "mu " << muPtr_().primitiveField() << nl;
                os << "precStarOld " << precStar_[precI].oldTime().primitiveField() << nl;
                os << "precStarCur " << precStar_[precI].primitiveField() << nl;
                os << "phi " << phiPtr_().primitiveField() << nl;
                os << "Ucell " << UPtr_().primitiveField() << nl;
                forAll(phiPtr_().boundaryField(), pI) { os << "phiB" << pI << " " << phiPtr_().boundaryField()[pI] << nl; }
                os << "diag " << precEq.diag() << nl;
                os << "upper " << precEq.upper() << nl;
                os << "lower " << precEq.lower() << nl;
                os << "source " << precEq.source() << nl;
                forAll(precEq.internalCoeffs(), pI)
                {
                    os << "internalCoeffs" << pI << " " << precEq.internalCoeffs()[pI] << nl;
                    os << "boundaryCoeffs" << pI << " " << precEq.boundaryCoeffs()[pI] << nl;
                }
                r43Call++;
            }
        }
        precEq.solve();
        if (r43This >= 0)
        {
            OFstream os2("r43dump_prec_sol_" + Foam::name(r43This) + ".txt");
            os2.precision(17);
            os2 << "precStarNew " << precStar_[precI].primitiveField() << nl;
        }
        prec_[precI] = precStar_[precI] * alphaPtr_(); // Precs per total volume
        prec_[precI].correctBoundaryConditions();
        /*
        //- Correct but works on discontinuous fields
        fvScalarMatrix precEq
        (
            fvm::ddt((1-eigenvalueNeutronics_), prec_[precI])
            + fvm::Sp(lambda[precI], prec_[precI])
            - neutroSource_/keff_*Beta[precI]
            + fvm::div(phi_/fvc::interpolate(porosity_), prec_[precI])
            - fvm::laplacian(diffCoeffPrec_, prec_[precI])                
        );
        precEq.solve();
        */
    }
    else
    {
        fvScalarMatrix precEq
        (
            fvm::ddt((1-eigenvalueNeutronics_), prec_[precI])
        +   fvm::Sp(lambda[precI], prec_[precI])
        -   neutroSource_/keff_*Beta[precI]
        );
        precEq.solve();
    }
}
