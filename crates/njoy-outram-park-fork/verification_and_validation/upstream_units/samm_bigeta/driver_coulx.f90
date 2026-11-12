! Copyright (C) 2026 Theodore Ong and the outram-park contributors. AGPL-3.0-only.
! gh:#1368: print NJOY2016 samm `coulx` results as hex for the finite-eta branches
! (everything `coulx` does NOT send to `bigeta`): eta >= 5 with asymp1+taylor, and
! eta < 5 with xsigll+asymp2(+taylor), then getfg (or end1) and getps.
! Links the verbatim extract (module sammx) of 2016.79 or 2016.80.
program drvc
   use locale
   use sammx
   implicit none
   real(kr),parameter::etas(11)=(/0.1_kr,0.5_kr,1.0_kr,2.0_kr,3.0_kr,4.9_kr,5.5_kr,6.0_kr,8.0_kr,12.0_kr,20.0_kr/)
   real(kr),parameter::rhos(10)=(/0.01_kr,0.05_kr,0.1_kr,0.5_kr,1.0_kr,2.0_kr,5.0_kr,10.0_kr,30.0_kr,100.0_kr/)
   integer,parameter::llv(3)=(/0,1,3/)
   integer::ie,ir,il,ic,jdopha,jdoder,ishift,lll,llmax
   real(kr)::eta,rho,p,s,dp,sinphi,cosphi,dphi
   real(kr)::f(100),fpr(100),g(100),gpr(100),sigma(100)
   do ie=1,size(etas)
   do ir=1,size(rhos)
      eta=etas(ie); rho=rhos(ir)
      if (eta>10*rho .and. eta>5) cycle            ! that is bigeta, covered elsewhere
      do il=1,3
      do ic=1,2
         lll=llv(il); llmax=lll+2
         if (ic==1) then
            jdopha=0;jdoder=0;ishift=-1
         else
            jdopha=1;jdoder=1;ishift=1
         endif
         p=0;s=0;dp=0;sinphi=0;cosphi=1;dphi=0
         f=0;fpr=0;g=0;gpr=0;sigma=0
         call coulx(eta,rho,lll,llmax,f,fpr,g,gpr,sigma,p,s,dp,sinphi,cosphi,dphi,jdopha,jdoder,ishift)
         write(*,'(a,2(1x,z16.16),3i3,6(1x,z16.16))') 'C',tr(eta),tr(rho),lll,jdopha,jdoder,&
           tr(p),tr(s),tr(dp),tr(sinphi),tr(cosphi),tr(dphi)
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
end program drvc
