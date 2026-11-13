! tc_driver.F90 -- reference driver for the outram-park-fork-thermochimica differential
! (gh:#1407).  Links against an UNMODIFIED Thermochimica 0c35c8d (BSD-3-Clause,
! ORNL-CEES/thermochimica) and dumps, for each case read from stdin:
!   the converged state (assemblage, species moles, mole fractions, chemical and
!   element potentials, Gibbs energy, iteration counts) and the inputs a
!   data-free solver needs (dimensionless standard Gibbs energies g0/RT, atom
!   matrix, RKMP parameters).  All reals are written with 17 significant digits.
!
! stdin, one case after another (list-directed):
!   <datafile path> <T [K]> <P [atm]> <tol1 (0 = upstream default 1e-5)> <nel>
!   then nel lines: <atomic number> <moles>
! stdout: line-oriented records, first token is the record type.
program tcdriver
    USE ModuleThermo
    USE ModuleThermoIO
    USE ModuleGEMSolver
    USE ModuleThermoTolerance
    implicit none
    character(512) :: fname
    real(8) :: temp, pres, tol1, amount
    integer :: nel, ie, iz, icase, i, k, ip, i0, i1, ios
    icase = 0
    do
        read (*, *, iostat = ios) fname, temp, pres, tol1, nel
        if (ios /= 0) exit
        icase = icase + 1
        dElementMass = 0D0
        do ie = 1, nel
            read (*, *) iz, amount
            dElementMass(iz) = amount
        end do
        cInputUnitTemperature = 'K'
        cInputUnitPressure    = 'atm'
        cInputUnitMass        = 'moles'
        dTemperature          = temp
        dPressure             = pres
        cThermoFileName       = trim(fname)
        iPrintResultsMode     = 0
        call ParseCSDataFile(cThermoFileName)
        if (INFOThermo /= 0) then
            write (*, '(A,I0,A,I0)') 'case ', icase, ' PARSE_INFO ', INFOThermo
            call ResetThermoAll
            cycle
        end if
        ! `Thermochimica` calls InitThermo, which restores the default tolerances and does not
        ! reapply an override.  The shipped coupling entry `ThermochimicaAPI` is the same
        ! sequence with InitThermoAPI (= InitThermo + ApplyToleranceOverride), so a tightened
        ! run goes through it; the default run goes through `Thermochimica` unchanged.
        if (tol1 > 0D0) then
            call SetToleranceOverrideValue(tol1)
            call ThermochimicaAPI
        else
            call Thermochimica
        end if
        write (*, '(A,I0,A,I0,A,I0,A,I0,A,I0)') 'case ', icase, ' INFO ', INFOThermo, ' iterGlobal ', iterGlobal, &
            ' nCon ', nConPhases, ' nSoln ', nSolnPhases
        write (*, '(A,ES25.17E3,A,ES25.17E3,A,ES25.17E3,A,ES25.17E3)') 'cond T ', dTemperature, ' P_atm ', dPressure, &
            ' R ', dIdealConstant, ' tol1 ', dTolerance(1)
        write (*, '(A,ES25.17E3)') 'gibbs_J ', dGibbsEnergySys
        write (*, '(A,I0,A,I0,A,I0)') 'dims nElements ', nElements, ' nSpecies ', nSpecies, ' nSolnPhasesSys ', nSolnPhasesSys
        do i = 1, nElements
            write (*, '(A,I0,A,I0,A,ES25.17E3,A,ES25.17E3)') 'elem ', i, ' Z ', iElementSystem(i), ' b ', &
                dMolesElement(i), ' pi ', dElementPotential(i)
        end do
        do i = 1, nConPhases
            write (*, '(A,I0,A,I0,A,ES25.17E3)') 'assem_con ', i, ' species ', iAssemblage(i), ' moles ', dMolesPhase(i)
        end do
        do i = 1, nSolnPhases
            ip = -iAssemblage(nElements - i + 1)
            write (*, '(A,I0,A,I0,A,ES25.17E3)') 'assem_soln ', i, ' phase ', ip, ' moles ', dMolesPhase(nElements - i + 1)
        end do
        do ip = 1, nSolnPhasesSys
            write (*, '(A,I0,A,A,A,A,A,I0,A,I0)') 'phase ', ip, ' name ', trim(cSolnPhaseName(ip)), ' type ', &
                trim(cSolnPhaseType(ip)), ' first ', nSpeciesPhase(ip-1) + 1, ' last ', nSpeciesPhase(ip)
        end do
        write (*, '(A,I0,A,I0)') 'conspecies first ', nSpeciesPhase(nSolnPhasesSys) + 1, ' last ', nSpecies
        do i = 1, nSpecies
            write (*, '(A,I0,A,A,A,I0,A,ES25.17E3,A,ES25.17E3,A,ES25.17E3,A,ES25.17E3)') 'species ', i, ' name ', &
                trim(cSpeciesName(i)), ' ppm ', iParticlesPerMole(i), ' g0rt ', dStdGibbsEnergy(i), &
                ' mu ', dChemicalPotential(i), ' n ', dMolesSpecies(i), ' x ', dMolFraction(i)
            write (*, '(A,I0,A)', advance = 'no') 'stoich ', i, ' '
            do k = 1, nElements
                write (*, '(ES25.17E3,A)', advance = 'no') dStoichSpecies(i, k), ' '
            end do
            write (*, *)
        end do
        do ip = 1, nSolnPhasesSys
            i0 = nParamPhase(ip-1) + 1
            i1 = nParamPhase(ip)
            do k = i0, i1
                write (*, '(A,I0,A,I0,A,I0,A,I0,A,I0,A,I0,A,ES25.17E3)') 'rkmp phase ', ip, ' type ', iRegularParam(k,1), &
                    ' i ', iRegularParam(k,2), ' j ', iRegularParam(k,3), ' e4 ', iRegularParam(k,4), ' e5 ', &
                    iRegularParam(k,5), ' L ', dExcessGibbsParam(k)
            end do
        end do
        call ResetThermoAll
    end do
end program tcdriver
