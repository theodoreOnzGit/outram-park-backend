! Differential driver for the Rust port of NJOY2016.80 VIEWR / graph (gh:#1369).
!
! Calls the UNMODIFIED `graph` module (graph.f90, with locale, phys, mainio, util)
! and the routines `rget`, `iget`, `stripv` (graph.f90, private) and `ascalv`
! (viewr.f90, private), which extract.sh copies out by line range after checking
! the SHA-256 of both files. Prints every number as hex bits plus decimal, and
! leaves the PostScript of six scenes in tape61..tape66.
!
! The Rust mirror is tests/viewr_code_to_code.rs; keep the scenes in step.
program viewr_driver
   use locale
   use graph
   use gutil, only: iget, rget, stripv
   use ascmod, only: ascalv
   implicit none
   integer,parameter::nstr=14
   character(12)::sr(nstr)=[character(12) :: '12','-3','0','.5','.8','1.25','-.7',&
     '10.5','D','X','0.05','-12.5','3-4','7']
   character(40)::st(5)=[character(40) :: '  abc  ','   ','x y  z   ','ab','  1  2  ']
   character(60)::str
   real(kr)::z1,z2,xg,yg,uo,vo,w,x,y,u,v
   integer::i,major,minor,n,l
   real(kr)::cases(3,28)

   ! ---- iget / rget / stripv
   do i=1,nstr
      n=len_trim(sr(i))
      write(*,'(a,i3,1x,i12)') 'IGET ',i,iget(sr(i),n)
      call hex1('RGET',i,rget(sr(i),n))
   enddo
   do i=1,5
      str=st(i)
      call stripv(str,l)
      write(*,'(a,i2,1x,i3,1x,''['',a,'']'')') 'STRIPV ',i,l,str(1:max(l,1))
   enddo

   ! ---- ascalv: m,z1,z2 (branches: bad input, zero ends, huge/tiny ratio,
   ! ---- close values, p<1, p>=1e4, every 1/2/5 choice)
   cases(:,1)=(/4.0_kr,0.3_kr,7.2_kr/)
   cases(:,2)=(/4.0_kr,5.0_kr,5.0_kr/)
   cases(:,3)=(/0.0_kr,1.0_kr,5.0_kr/)
   cases(:,4)=(/21.0_kr,1.0_kr,5.0_kr/)
   cases(:,5)=(/4.0_kr,0.0_kr,13.0_kr/)
   cases(:,6)=(/4.0_kr,-3.0_kr,0.0_kr/)
   cases(:,7)=(/4.0_kr,1.0_kr,5000.0_kr/)
   cases(:,8)=(/4.0_kr,1.0_kr,1.0005_kr/)
   cases(:,9)=(/4.0_kr,100.0_kr,100.001_kr/)
   cases(:,10)=(/4.0_kr,0.001_kr,0.0095_kr/)
   cases(:,11)=(/4.0_kr,1.0e-6_kr,3.0e-6_kr/)
   cases(:,12)=(/4.0_kr,0.0_kr,3.0e6_kr/)
   cases(:,13)=(/4.0_kr,0.0_kr,9.0e9_kr/)
   cases(:,14)=(/4.0_kr,0.0_kr,8.0_kr/)
   cases(:,15)=(/4.0_kr,0.0_kr,8.04_kr/)
   cases(:,16)=(/4.0_kr,0.0_kr,8.1_kr/)
   cases(:,17)=(/4.0_kr,0.0_kr,19.0_kr/)
   cases(:,18)=(/4.0_kr,0.0_kr,20.4_kr/)
   cases(:,19)=(/4.0_kr,-17.0_kr,23.0_kr/)
   cases(:,20)=(/2.0_kr,0.2_kr,1.7_kr/)
   cases(:,21)=(/20.0_kr,0.0_kr,1.0_kr/)
   cases(:,22)=(/4.0_kr,-0.5_kr,-0.1_kr/)
   cases(:,23)=(/4.0_kr,2.0e-4_kr,0.9_kr/)
   cases(:,24)=(/4.0_kr,1.0_kr,2.0_kr/)
   cases(:,25)=(/4.0_kr,50.0_kr,50.04_kr/)
   cases(:,26)=(/4.0_kr,1.0e4_kr,7.5e4_kr/)
   cases(:,27)=(/4.0_kr,3.0_kr,9.99e3_kr/)
   cases(:,28)=(/4.0_kr,-8.0e-3_kr,2.5e-2_kr/)
   do i=1,28
      z1=cases(2,i)
      z2=cases(3,i)
      call ascalv(int(cases(1,i)),z1,z2,major,minor)
      call hex1('ASCALV_Z1',i,z1)
      call hex1('ASCALV_Z2',i,z2)
      write(*,'(a,i3,2i8)') 'ASCALV_MM ',i,major,minor
   enddo

   ! ---- scene 1: linear axes
   call begin(61,xg,yg)
   call axis3(0.0_kr,10.0_kr,2.0_kr,'abc',-3,-1,0,5.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,0)
   call axis3(0.0_kr,1.0_kr,0.25_kr,'ab',2,1,1,5.0_kr,0.0_kr,1.0_kr,0.0_kr,1.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,0)
   call axis3(-5.0_kr,5.0_kr,1.0_kr,'y',1,-1,2,4.0_kr,6.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,-1.0_kr,0.0_kr,0.0_kr,1)
   call axis3(0.0_kr,0.1_kr,0.02_kr,'frac',-4,1,3,4.0_kr,0.0_kr,2.0_kr,0.0_kr,1.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,0)
   call axis3(0.0_kr,10000.0_kr,2000.0_kr,'big',-3,-1,0,5.0_kr,0.0_kr,3.0_kr,0.0_kr,1.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,0)
   call axis3(0.0_kr,3.5_kr,0.5_kr,'half',3,0,0,5.0_kr,0.0_kr,4.0_kr,0.0_kr,1.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,0)
   call axis3(0.0_kr,10.0_kr,5.0_kr,'rot',-3,1,0,4.0_kr,0.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,-1.0_kr,0.0_kr,0.0_kr,1)
   call axis3(0.0_kr,10.0_kr,2.0_kr,'z',1,1,0,3.0_kr,0.0_kr,0.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,1.0_kr,0.0_kr,0)
   call axis3(0.0_kr,5.0e-3_kr,1.0e-3_kr,'.',-1,-1,0,4.0_kr,0.0_kr,5.0_kr,0.0_kr,1.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,0)
   call axis3(0.0_kr,2.0e7_kr,5.0e6_kr,'huge',1,-1,1,4.0_kr,0.0_kr,6.0_kr,0.0_kr,1.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,0)
   call axis3(0.0_kr,10.0_kr,2.0_kr,'p<q',3,1,2,4.0_kr,0.0_kr,6.5_kr,0.0_kr,1.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,1)
   call finish()

   ! ---- scene 2: log axes
   call begin(62,xg,yg)
   call axis3(1.0_kr,1000.0_kr,0.0_kr,'log',-3,-1,0,5.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,0)
   call axis3(0.3_kr,5.0e5_kr,0.0_kr,'decades',3,1,0,8.0_kr,0.0_kr,1.0_kr,0.0_kr,1.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,0)
   call axis3(1.0e-12_kr,1.0e-1_kr,0.0_kr,'neg',-3,-1,0,8.0_kr,0.0_kr,2.0_kr,0.0_kr,1.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,1)
   call axis3(2.0_kr,50.0_kr,0.0_kr,'nondecade',-9,-1,1,5.0_kr,0.0_kr,3.0_kr,0.0_kr,1.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,0)
   call axis3(4.0_kr,6.0_kr,0.0_kr,'short',5,1,3,5.0_kr,0.0_kr,4.0_kr,0.0_kr,1.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,0)
   call axis3(1.0e-3_kr,1.0e20_kr,0.0_kr,'wide',-4,0,2,7.0_kr,0.0_kr,5.0_kr,0.0_kr,1.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,0)
   call axis3(3.0e-2_kr,4.0e1_kr,0.0_kr,'vertical',3,1,0,5.0_kr,8.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,-1.0_kr,0.0_kr,0.0_kr,1)
   call axis3(1.0e-3_kr,1.0e3_kr,0.0_kr,'upwards',-3,1,1,5.0_kr,9.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,-1.0_kr,0.0_kr,0.0_kr,0)
   call axis3(1.0_kr,1.0e12_kr,0.0_kr,'ten12',-4,-1,0,5.0_kr,0.0_kr,6.0_kr,0.0_kr,1.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,0)
   call finish()

   ! ---- scene 3: every symbol
   call begin(63,xg,yg)
   do i=-1,25
      call dsym(1.0_kr+0.3_kr*i,3.0_kr,i,0.2_kr)
   enddo
   call dsym(2.0_kr,4.0_kr,3,0.4_kr)
   call finish()

   ! ---- scene 4: text, instructions, widths
   call begin(64,xg,yg)
   call txscene()
   call finish()

   ! ---- scene 5: 3d
   call begin5(65)
   ! ---- scene 6: curves, grids, shading
   call begin(66,xg,yg)
   call cvscene()
   call finish()
contains

   subroutine hex1(tag,i,x)
      character(*)::tag
      integer::i
      real(kr)::x
      write(*,'(a,1x,i3,1x,z16.16,1x,es25.16e3)') tag,i,transfer(x,1_8),x
   end subroutine hex1

   subroutine begin(unit,xg,yg)
      integer::unit
      real(kr)::xg,yg,uo,vo
      call gplot(1,10.0_kr,7.5_kr,unit)
      call initp(1,10.0_kr,7.5_kr,2,0.3_kr,0.0134_kr,0,1)
      call window(0.0_kr,0.0_kr,10.0_kr,7.5_kr,0.0_kr,'<t>itle one',11,'<t>wo',5,0)
      call init2(uo,vo,xg,yg,0,0)
   end subroutine begin

   subroutine finish()
      call endw
      call endp
      call gdone
   end subroutine finish

   subroutine txscene()
      character(60),dimension(14)::s
      integer::k,ln(14)
      real(kr)::wd
      s(1)='<c>ross section (barns)'
      s(2)='E#EH.8<x#HXEX<=mc'
      s(3)='[a]lpha [b]eta [G]amma'
      s(4)='a#F3<greek'
      s(5)='a#M1<abc#M0< def'
      s(6)='a##b <<c>> [[d]]'
      s(7)='a#E.3<up#X< and #L.2<down#X<'
      s(8)='a#H1.5<big#HX< small'
      s(9)='a#E<x #L<x #H<x'
      s(10)='10#EH.8<-12#HXEX<'
      s(11)='*10#EH.8<3#HXEX<'
      s(12)='(paren) end<'
      s(13)='a#O1<xyz#F1< times'
      s(14)='ab#E.3<#H.5<cd'
      do k=1,14
         ln(k)=len_trim(s(k))
      enddo
      do k=1,14
         wd=txtlen(s(k),ln(k),2,0.3_kr)
         call hex1('TXTLEN2',k,wd)
         wd=txtlen(s(k),ln(k),1,0.25_kr)
         call hex1('TXTLEN1',k,wd)
      enddo
      do k=1,14
         call text3(s(k),ln(k),2,0.3_kr,0.5_kr,0.5_kr*k,0.0_kr,1.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr)
      enddo
      call text3(s(1),ln(1),1,0.3_kr,5.0_kr,1.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,-1.0_kr,0.0_kr,0.0_kr)
      call text3(s(2),ln(2),3,0.2_kr,5.0_kr,2.0_kr,0.0_kr,0.8_kr,0.6_kr,0.0_kr,-0.6_kr,0.8_kr,0.0_kr)
   end subroutine txscene

   subroutine begin5(unit)
      integer::unit
      real(kr)::u,v,x(2000),y(2000),z(2000),px(4),py(4),pz(4)
      integer::k
      call gplot(0,7.5_kr,10.0_kr,unit)
      call initp(0,7.5_kr,10.0_kr,2,0.2_kr,0.0134_kr,0,2)
      call window(0.0_kr,0.0_kr,7.5_kr,10.0_kr,0.0_kr,'<t>hree d',7,' ',0,0)
      call init3(2.5_kr,6.5_kr,2.5_kr,15.0_kr,-15.0_kr,15.0_kr,0)
      call axis3(0.0_kr,10.0_kr,2.0_kr,'x',-1,-1,0,2.5_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,0)
      call axis3(1.0_kr,1.0e3_kr,0.0_kr,'y',-1,-1,0,6.5_kr,2.5_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,-1.0_kr,0.0_kr,0.0_kr,0)
      call axis3(0.0_kr,5.0_kr,1.0_kr,'z',1,1,1,2.5_kr,0.0_kr,0.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,-1.0_kr,0.0_kr,1)
      do k=1,6
         call trans3(0.5_kr*k,1.0_kr*k,0.25_kr*k,u,v)
         call hex1('TRANS3U',k,u)
         call hex1('TRANS3V',k,v)
      enddo
      call grid3(1,1,0)
      call grid3(0,1,1)
      call grid3(1,-1,1)
      do k=1,5
         x(k)=0.5_kr*k
         y(k)=1.0_kr+0.5_kr*k
         z(k)=0.4_kr*k*(6-k)
      enddo
      x(1)=0.1_kr
      call curv3(x,y,z,5,0.0134_kr)
      call vect3(0.0_kr,0.0_kr,0.0_kr,1.0_kr,1.0_kr,1.0_kr,0.0134_kr,1021)
      px=(/0.1_kr,0.2_kr,0.3_kr,0.1_kr/)
      py=(/1.0_kr,2.0_kr,1.5_kr,1.0_kr/)
      pz=(/0.0_kr,1.0_kr,2.0_kr,0.0_kr/)
      call init3(-2.5_kr,6.5_kr,2.5_kr,12.0_kr,-10.0_kr,8.0_kr,0)
      do k=1,6
         call trans3(0.5_kr*k,1.0_kr*k,0.25_kr*k,u,v)
         call hex1('TRANS3NU',k,u)
         call hex1('TRANS3NV',k,v)
      enddo
      call finish()
   end subroutine begin5

   subroutine cvscene()
      real(kr)::x(8),y(8),xx,yy,xi
      integer::k,i
      call axis3(1.0_kr,1.0e4_kr,0.0_kr,'e',-1,-1,0,8.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,0)
      call axis3(0.0_kr,10.0_kr,2.0_kr,'s',1,-1,0,5.0_kr,0.0_kr,0.0_kr,0.0_kr,0.0_kr,1.0_kr,0.0_kr,-1.0_kr,0.0_kr,0.0_kr,1)
      call grid2(1,1)
      do i=1,8
         x(i)=1.7_kr**i
         y(i)=1.0_kr+0.8_kr*i-0.07_kr*i*i
      enddo
      call frame2(8.0_kr,5.0_kr,0.0_kr,2)
      call curv2(x,y,8,0,0.0134_kr,0,0,0.0_kr,0)
      call curv2(x,y,8,1,0.0134_kr,2,1,0.15_kr,0)
      call curv2(x,y,8,2,0.0134_kr,-1,2,0.15_kr,0)
      call curv2(x,y,8,3,0.0134_kr,3,5,0.15_kr,0)
      call curv2(x,y,8,4,0.0268_kr,0,0,0.15_kr,0)
      call endfr
      ! shaded curves: upstream leaves ifg=0 after a fill (an out-of-bounds colour
      ! index at the next moveh), so each one gets its own frame2, which resets ifg.
      call frame2(8.0_kr,5.0_kr,0.0_kr,2)
      call curv2(x,y,8,0,0.0134_kr,0,0,0.15_kr,3)
      call endfr
      call frame2(8.0_kr,5.0_kr,0.0_kr,3)
      call curv2(x,y,8,0,0.0134_kr,0,0,0.15_kr,15)
      call endfr
      call frame2(8.0_kr,5.0_kr,0.0_kr,4)
      call curv2(x,y,8,0,0.0_kr,0,0,0.15_kr,25)
      call endfr
      call frame2(8.0_kr,5.0_kr,0.0_kr,5)
      call curv2(x,y,8,0,0.0134_kr,0,0,0.15_kr,35)
      call endfr
      call frame2(8.0_kr,5.0_kr,0.0_kr,6)
      call curv2(x,y,8,0,0.0134_kr,0,0,0.15_kr,45)
      call endfr
      call frame2(8.0_kr,5.0_kr,0.0_kr,7)
      call curv2(x,y,8,0,0.0134_kr,0,0,0.15_kr,75)
      call endfr
      call frame2(8.0_kr,5.0_kr,0.0_kr,0)
      do i=0,25
         call dsym(1.0_kr+0.2_kr*i,0.5_kr,i,0.1_kr)
      enddo
      xi=xscale(100.0_kr)
      call hex1('XSCALE',1,xi)
      call hex1('YSCALE',1,yscale(3.0_kr))
      call hex1('XINVRS',1,xinvrs(2.0_kr))
      call hex1('YINVRS',1,yinvrs(2.0_kr))
      call endfr
   end subroutine cvscene

end program viewr_driver
