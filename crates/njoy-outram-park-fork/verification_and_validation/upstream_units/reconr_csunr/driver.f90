! Copyright (C) 2026 Theodore Ong and the outram-park contributors. AGPL-3.0-only.
! gh:#1448: NJOY2016.80 reconr csunr1 / csunr2 (and their unfac, gnrl) as hex bits.
! The routines are the VERBATIM text extracted by extract.sh and `include`d into module unrx, which
! holds the `reconr` module variables they use. The flat `res` scratch array is filled with the layout
! the routines read (rdf2u0/rdf2u1/rdf2u2 store it so), from cases.in:
!   case  := kind naps ap spi awri nls ne fe(1:ne)   (ne = 0 unless kind=B)
!   L     := l njs
!   J (A) := d aj amun gno gg
!   J (B) := muf d aj amun gno gg gf(1:ne)
!   J (C) := aj int amux amun amuf npts, then npts rows  e d gx gno gg gf
!   then:  nener e(1:nener)   (calls in this order, ascending, as genunr makes them)
! Output per call: S <case> <call> <E bits> <total bits> <elastic bits> <fission bits> <capture bits>
! then the same as decimals.
module unrx
   use locale
   implicit none
   real(kr)::res(200000)
   integer::ibaset(20),isect
   real(kr)::elst(20),slst(20,4),enxt(20),snxt(20,4)
   integer::ilast(20)
contains
   include 'extracted.inc'
end module unrx

program drv
   use locale
   use unrx
   implicit none
   integer::ncase,ic,nls,l,il,njs,j,ne,i,k,nener,p,npts,intv,muf,ie
   character(len=1)::kind
   integer::naps
   real(kr)::ap,spi,awri,d,aj,amun,gno,gg,amux,amuf,fe(100),gf(100),en(200),e,sigp(4),sigu(4)
   real(kr)::row(6)
   read(*,*) ncase
   do ic=1,ncase
      res=0
      read(*,*) kind,naps,ap,spi,awri,nls,ne,(fe(i),i=1,ne)
      ibaset(1)=1
      res(1)=1.0e-5_kr
      res(2)=1.0e7_kr
      res(3)=2
      res(4)=1
      if (kind.eq.'B') res(5)=1
      res(6)=naps
      res(7)=spi
      res(8)=ap
      res(9)=0
      res(10)=0
      if (kind.eq.'B') then
         res(11)=ne
         res(12)=nls
         p=13
         do i=1,ne
            res(p)=fe(i)
            p=p+1
         enddo
      else
         res(11)=nls
         res(12)=0
         p=13
      endif
      do il=1,nls
         read(*,*) l,njs
         res(p)=awri
         res(p+2)=l
         if (kind.eq.'A') then
            res(p+4)=6*njs
            res(p+5)=njs
         else
            res(p+4)=njs
         endif
         p=p+6
         do j=1,njs
            if (kind.eq.'A') then
               read(*,*) d,aj,amun,gno,gg
               res(p)=d
               res(p+1)=aj
               res(p+2)=amun
               res(p+3)=gno
               res(p+4)=gg
               p=p+6
            else if (kind.eq.'B') then
               read(*,*) muf,d,aj,amun,gno,gg,(gf(i),i=1,ne)
               res(p+3)=muf
               res(p+4)=6+ne
               res(p+6)=d
               res(p+7)=aj
               res(p+8)=amun
               res(p+9)=gno
               res(p+10)=gg
               do i=1,ne
                  res(p+11+i)=gf(i)
               enddo
               p=p+12+ne
            else
               read(*,*) aj,intv,amux,amun,amuf,npts
               res(p)=aj
               res(p+2)=intv
               res(p+4)=6*npts+6
               res(p+5)=npts
               res(p+8)=amux
               res(p+9)=amun
               res(p+11)=amuf
               do i=1,npts
                  read(*,*) row
                  do k=1,6
                     res(p+12+6*(i-1)+k-1)=row(k)
                  enddo
               enddo
               p=p+12+6*npts
            endif
         enddo
      enddo
      read(*,*) nener,(en(i),i=1,nener)
      isect=1
      e=0
      call csunr1(e,sigu)
      call csunr2(e,sigu)
      do i=1,nener
         e=en(i)
         if (kind.eq.'C') then
            call csunr2(e,sigp)
         else
            call csunr1(e,sigp)
         endif
         write(*,'(a,2(1x,i3),5(1x,z16.16))') 'S',ic,i,tr(en(i)),tr(sigp(1)),tr(sigp(2)),tr(sigp(3)),tr(sigp(4))
         write(*,'(a,2(1x,i3),5(1x,es24.16e3))') 'D',ic,i,en(i),sigp(1),sigp(2),sigp(3),sigp(4)
      enddo
   enddo
   !--gnrl: every branch, every id, all degrees of freedom; guards
   call gcases()
   !--unfac: l = 0..3 with sentinel vl, ps (l >= 3 leaves them)
   call ucases()
contains
   subroutine gcases()
      integer::mu,nu,lam,id,ib,ig
      real(kr)::sg
      real(kr)::gb(6),dff(6)
      gb=(/0.0_kr,0.0_kr,0.3_kr,0.3_kr,-0.1_kr,0.3_kr/)
      dff=(/0.0_kr,0.2_kr,0.0_kr,0.2_kr,0.0_kr,-0.2_kr/)
      do ib=1,6
         do mu=1,4
            do nu=1,4
               do lam=1,4
                  do id=1,3
                     call gnrl(0.05_kr,gb(ib),0.04_kr,mu,nu,lam,sg,dff(ib),id)
                     write(*,'(a,5(1x,i2),1x,z16.16)') 'G',ib,mu,nu,lam,id,tr(sg)
                  enddo
               enddo
            enddo
         enddo
      enddo
      do ig=1,3
         if (ig.eq.1) call gnrl(0.0_kr,0.3_kr,0.04_kr,2,2,2,sg,0.2_kr,1)
         if (ig.eq.2) call gnrl(0.05_kr,0.3_kr,0.0_kr,2,2,2,sg,0.2_kr,1)
         if (ig.eq.3) call gnrl(-0.05_kr,0.3_kr,0.04_kr,2,2,2,sg,0.2_kr,2)
         write(*,'(a,1x,i2,1x,z16.16)') 'H',ig,tr(sg)
      enddo
   end subroutine gcases
   subroutine ucases()
      integer::l,k
      real(kr)::rho(3),rhoc(3),vl,ps
      rho=(/0.05_kr,0.8_kr,2.5_kr/)
      rhoc=(/0.06_kr,0.9_kr,2.2_kr/)
      do l=0,3
         do k=1,3
            vl=7.0_kr
            ps=8.0_kr
            call unfac(l,rho(k),rhoc(k),2.0_kr,vl,ps)
            write(*,'(a,2(1x,i2),2(1x,z16.16))') 'U',l,k,tr(vl),tr(ps)
         enddo
      enddo
   end subroutine ucases
   function tr(x) result(ii)
      real(kr),intent(in)::x
      integer(8)::ii
      ii=transfer(x,ii)
   end function tr
end program drv
