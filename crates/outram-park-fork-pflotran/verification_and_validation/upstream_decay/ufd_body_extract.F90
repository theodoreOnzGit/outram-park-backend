! EXTRACTED by extract.sh from PFLOTRAN 2af572b8 pm_ufd_decay.F90
! lines 1868-1994 (PMUFDDecaySolveISPDIAtCell). Only edit: 'this%' deleted.
  if (.not.implicit_solution) then

  ! 3-generation analytical solution derived for multiple parents and
  ! grandparents and non-zero initial daughter concentrations (see Section
  ! 3.2.3 of Mariner et al. (2016), SAND2016-9610R), where the solution is
  ! obtained explicitly in time

    ! FIRST PASS decay ==============================================
    do i = 1,num_isotopes
      ! update the initial value of the isotope coefficient:
      coeff(i) = mass_old(i)
      ! loop through the isotope's parents:
      do p = 1,isotope_parents(0,i)
        ip = isotope_parents(p,i)
        coeff(i) = coeff(i) - (isotope_decay_rate(ip) * mass_old(ip)) / &
          (isotope_decay_rate(i) - isotope_decay_rate(ip))
        ! loop through the isotope's parent's parents:
        do g = 1,isotope_parents(0,ip)
          ig = isotope_parents(g,ip)
          coeff(i) = coeff(i) - &
            ((isotope_decay_rate(ip) * isotope_decay_rate(ig) * &
            mass_old(ig)) / ((isotope_decay_rate(ip) - &
            isotope_decay_rate(ig)) * (isotope_decay_rate(i) - &
            isotope_decay_rate(ig)))) + ((isotope_decay_rate(ip) * &
            isotope_decay_rate(ig) * mass_old(ig)) / &
            ((isotope_decay_rate(ip) - isotope_decay_rate(ig)) * &
            (isotope_decay_rate(i) - isotope_decay_rate(ip))))
        enddo ! grandparent loop
      enddo ! parent loop
    enddo ! isotope loop
    ! SECOND PASS decay =============================================
    do i = 1,num_isotopes
      ! decay the isotope species:
      mass_iso_tot1(i) = coeff(i)*exp(-1.d0*isotope_decay_rate(i)*dt)
      ! loop through the isotope's parents:
      do p = 1,isotope_parents(0,i)
        ip = isotope_parents(p,i)
        mass_iso_tot1(i) = mass_iso_tot1(i) + &
              (((isotope_decay_rate(ip) * mass_old(ip)) / &
              (isotope_decay_rate(i) - isotope_decay_rate(ip))) * &
              exp(-1.d0 * isotope_decay_rate(ip) * dt))
        ! loop through the isotope's parent's parents:
        do g = 1,isotope_parents(0,ip)
          ig = isotope_parents(g,ip)
          mass_iso_tot1(i) = mass_iso_tot1(i) - &
            ((isotope_decay_rate(ip) * isotope_decay_rate(ig) * &
            mass_old(ig) * exp(-1.d0 * isotope_decay_rate(ip) * dt)) / &
            ((isotope_decay_rate(ip) - isotope_decay_rate(ig)) * &
            (isotope_decay_rate(i) - isotope_decay_rate(ip)))) + &
            ((isotope_decay_rate(ip) * isotope_decay_rate(ig) * &
            mass_old(ig) * exp(-1.d0 * isotope_decay_rate(ig) * dt)) / &
          ((isotope_decay_rate(ip) - isotope_decay_rate(ig)) * &
          (isotope_decay_rate(i) - isotope_decay_rate(ig))))
        enddo ! grandparent loop
      enddo ! parent loop
    enddo ! isotope loop

  else
    ! implicit solution approach
    prev_solution = 1.d0 ! to start, must set bigger than tolerance
    solution = mass_iso_tot0 ! to start, set solution to initial mass
    it = 0
    do ! nonlinear loop
      ! inf norm relative change in concentration
      if (maxval(abs(solution-prev_solution)/prev_solution) < tolerance) exit
      prev_solution = solution
      it = it + 1
      residual = 0.d0 ! set to zero because we are summing
      ! f(M_e^{k+1,p}) = (M_e^{k+1,p} - M_e^k)/dt -R(M_e^{k+1,p})
      Jacobian = 0.d0 ! set to zero because we are summing
      ! J_ij = del[f_i(M_e^{k+1,p})]/del[M_ej^{k+1,p}]
      ! isotope loop
      do iiso = 1, num_isotopes
        ! ----accumulation term for isotope------------------------!-units--
        ! dM_e/dt = (M_e^{k+1,p} - M_e^k)/dt
        residual(iiso) = residual(iiso) + &                        ! mol/sec
                         (solution(iiso) - mass_iso_tot0(iiso)) * &! mol
                         one_over_dt                               ! 1/sec
        ! d[(M_e^{k+1,p} - M_e^k)/dt]/d[M_e^{k+1,p}] = 1/dt
        Jacobian(iiso,iiso) = Jacobian(iiso,iiso) + &              ! 1/sec
                              one_over_dt                          ! 1/sec
        ! ----source/sink term for isotope-------------------------!-units--
        ! -R(M_e^{k+1,p}) = -(-L*(M_e^{k+1,p}))    L=lambda
        rate_constant = isotope_decay_rate(iiso)              ! 1/sec
        rate = rate_constant * solution(iiso)                      ! mol/sec
        residual(iiso) = residual(iiso) + rate                     ! mol/sec
        ! d[-(-L*(M_e^{k+1,p}))]/d[M_e^{k+1,p}] = L
        Jacobian(iiso,iiso) = Jacobian(iiso,iiso) + rate_constant  ! 1/sec
        ! daughter loop
        do i = 1, isotope_daughters(0,iiso)
          ! ----source/sink term for daughter----------------------!-units--
          idaughter = isotope_daughters(i,iiso)
          stoich = isotope_daughter_stoich(i,iiso)            ! -
          ! -R(M_e^{k+1,p}) = -(L*S*(M_e^{k+1,p}))    L=lambda
          residual(idaughter) = residual(idaughter) - &            ! mol/sec
                                (rate * stoich)                    ! mol/sec
          ! d[-(L*S*(M_e^{k+1,p}))]/d[M_e^{k+1,p}] = -L*S
          Jacobian(idaughter,iiso) = Jacobian(idaughter,iiso) - &  ! 1/sec
                                     (rate_constant * stoich)      ! 1/sec
        enddo
        ! k=time, p=iterate, M_e=element mass
      enddo
      ! scale Jacobian
      do iiso = 1, num_isotopes
        norm = max(1.d0,maxval(abs(Jacobian(iiso,:))))
        norm = 1.d0/norm
        rhs(iiso) = residual(iiso)*norm
        ! row scaling
        Jacobian(iiso,:) = Jacobian(iiso,:)*norm
      enddo
      ! log formulation for derivatives, column scaling
      do iiso = 1, num_isotopes
        Jacobian(:,iiso) = Jacobian(:,iiso)*solution(iiso)
      enddo
      ! linear solve steps
      ! solve step 1/2: get LU decomposition
      call LUDecomposition(Jacobian,num_isotopes,indices,i)
      ! solve step 2/2: LU back substitution linear solve
      call LUBackSubstitution(Jacobian,num_isotopes,indices,rhs)
      rhs = dsign(1.d0,rhs)*min(dabs(rhs),10.d0)
      ! update the solution
      solution = solution*exp(-rhs)
    enddo
    mass_iso_tot1 = solution
  endif

  mass_iso_tot1 = max(mass_iso_tot1,1.d-90)
