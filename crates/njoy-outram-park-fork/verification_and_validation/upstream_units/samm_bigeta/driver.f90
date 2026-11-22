! Copyright (C) 2026 Theodore Ong and the outram-park contributors. AGPL-3.0-only.
! gh:#1366: print NJOY2016 samm `bigeta` (and its K0/K1 series) results as hex.
! Links the verbatim extract (module sammx) of either 2016.79 or 2016.80.
program drv
   use locale
   use sammx
   use physics, only: pi
   implicit none
   real(kr),parameter::etas(8)=(/5.5_kr,6.0_kr,8.0_kr,12.0_kr,20.0_kr,35.0_kr,50.0_kr,100.0_kr/)
   real(kr),parameter::rhos(9)=(/0.001_kr,0.01_kr,0.05_kr,0.1_kr,0.3_kr,0.5_kr,0.55_kr,1.0_kr,3.0_kr/)
   integer,parameter::llv(3)=(/0,1,3/)
   integer::ie,ir,il,ic,jdopha,jdoder,ishift,lll,llmax,n
   real(kr)::eta,rho,p,s,dp,sinphi,cosphi,dphi
   real(kr)::f(100),fpr(100),g(100),gpr(100)
   do ie=1,size(etas)
   do ir=1,size(rhos)
      eta=etas(ie); rho=rhos(ir)
      if (.not.(eta>10*rho .and. eta>5)) cycle     ! coulx's dispatch condition
      if (2*rho*eta>80) cycle                       ! series would not converge in 100 terms
      f=0;fpr=0;g=0;gpr=0
      call kseries(eta,rho,0,1,f,fpr,g,gpr,p,s,dp,sinphi,cosphi,dphi,0,0,-1)
      write(*,'(a,6(1x,z16.16))') 'K',tr(eta),tr(rho),tr(kout(1)),tr(kout(2)),tr(kout(3)),tr(kout(4))
      write(*,'(a,2(1x,z16.16),3(1x,z16.16))') 'X',tr(eta),tr(rho),tr(exp(-pi*eta)),tr(sqrt(pi*rho)),tr(sqrt(2*pi*eta))
      do il=1,3
      do ic=1,2
         lll=llv(il); llmax=lll+2
         if (ic==1) then
            jdopha=0;jdoder=0;ishift=-1
         else
            jdopha=1;jdoder=1;ishift=1
         endif
         p=0;s=0;dp=0;sinphi=0;cosphi=1;dphi=0
         f=0;fpr=0;g=0;gpr=0
         call bigeta(eta,rho,lll,llmax,f,fpr,g,gpr,p,s,dp,sinphi,cosphi,dphi,jdopha,jdoder,ishift)
         n=lll+1
         write(*,'(a,2(1x,z16.16),3i3,10(1x,z16.16))') 'B',tr(eta),tr(rho),lll,jdopha,jdoder,&
           tr(p),tr(s),tr(dp),tr(sinphi),tr(cosphi),tr(dphi),tr(f(n)),tr(g(n)),tr(fpr(n)),tr(gpr(n))
      enddo
      enddo
   enddo
   enddo
contains
   function tr(x) result(i)
      real(kr),intent(in)::x
      integer(8)::i
      i=transfer(x,i)
   end function tr
end program drv
