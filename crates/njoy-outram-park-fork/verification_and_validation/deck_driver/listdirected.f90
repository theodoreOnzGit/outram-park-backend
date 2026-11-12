! List-directed read oracle for the NJOY deck card reader (gh:#1369).
! Each case presets its items, performs ONE `read(5,*)`, and prints every item
! (reals as hex bit patterns, integers in decimal, strings between bars).
! The Rust side (tests/deck_driver_code_to_code.rs) replays the same sequence
! over listdirected.in with `deck::cards::CardReader` and must print the same.
program ld
   implicit none
   integer :: i1,i2,i3,ios
   real(8) :: r1,r2,r3,r4
   character(16) :: s1,s2

   ! 1 plain ints
   call seti; read(5,*) i1,i2,i3; call pi(1)
   ! 2 slash leaves defaults
   call seti; read(5,*) i1,i2,i3; call pi(2)
   ! 3 null between commas
   call seti; read(5,*) i1,i2,i3; call pi(3)
   ! 4 leading comma
   call seti; read(5,*) i1,i2,i3; call pi(4)
   ! 5 repeat count
   call seti; read(5,*) i1,i2,i3; call pi(5)
   ! 6 repeat of nulls
   call seti; read(5,*) i1,i2,i3; call pi(6)
   ! 7 spans records
   call seti; read(5,*) i1,i2,i3; call pi(7)
   ! 8 excess values discarded, next read starts on the next record
   call seti; read(5,*) i1,i2; call pi(8)
   call seti; read(5,*) i1,i2; call pi(9)
   ! 10 blank record skipped
   call seti; read(5,*) i1,i2; call pi(10)
   ! 11 reals
   call setr; read(5,*) r1,r2,r3,r4; call pr(11)
   ! 12 reals from integers
   call setr; read(5,*) r1,r2,r3,r4; call pr(12)
   ! 13 exponent without letter, d and q
   call setr; read(5,*) r1,r2,r3,r4; call pr(13)
   ! 14 quoted string longer than the variable
   call sets; read(5,*) s1; call ps(14)
   ! 15 doubled quote
   call sets; read(5,*) s1; call ps(15)
   ! 16 double-quoted with blank
   call sets; read(5,*) s1; call ps(16)
   ! 17 unquoted string, rest of record discarded
   call sets; read(5,*) s1; call ps(17)
   ! 18 slash inside quotes
   call sets; call seti; read(5,*) s1,i1; call ps(18); call pi(18)
   ! 19 mixed: title then ints
   call sets; call seti; read(5,*) s1,i1,i2; call ps(19); call pi(19)
   ! 20 mixed: ints then quoted name then slash
   call sets; call seti; read(5,*) i1,i2,s1; call ps(20); call pi(20)
   ! 21 tabs as separators
   call seti; read(5,*) i1,i2,i3; call pi(21)
   ! 22 repeat of a string
   call sets; read(5,*) s1,s2; call ps(22)
   ! 23 comma then slash
   call seti; read(5,*) i1,i2,i3; call pi(23)
   ! 24 empty quoted string
   call sets; read(5,*) s1; call ps(24)
   ! 25 quoted name then slash, slash with trailing comment
   call sets; call seti; read(5,*) i1,i2,s1; call ps(25); call pi(25)
   ! 26 string left alone by a slash
   call sets; call seti; read(5,*) i1,s1; call ps(26); call pi(26)
   ! 27 repeat count larger than needed
   call seti; read(5,*) i1,i2; call pi(27)
   call seti; read(5,*) i1,i2; call pi(28)
   ! 29 negative and signed ints
   call seti; read(5,*) i1,i2,i3; call pi(29)
   ! 30 reals with repeat and null
   call setr; read(5,*) r1,r2,r3,r4; call pr(30)
   ! 31 long record with a value after many blanks
   call seti; read(5,*) i1,i2,i3; call pi(31)
   ! 32.. error cases: iostat only
   call seti; read(5,*,iostat=ios) i1,i2; call pe(32,ios)
   call sets; read(5,*,iostat=ios) i1,i2; call pe(33,ios)
   call setr; read(5,*,iostat=ios) r1,r2; call pe(34,ios)
   call seti; read(5,*,iostat=ios) i1,i2,i3; call pe(35,ios)
   ! end of file
   call seti; read(5,*,iostat=ios) i1,i2,i3; call pe(36,ios)
contains
   subroutine seti
      i1=-1; i2=-2; i3=-3
   end subroutine
   subroutine setr
      r1=-1.d0; r2=-2.d0; r3=-3.d0; r4=-4.d0
   end subroutine
   subroutine sets
      s1='<unset1>'; s2='<unset2>'
   end subroutine
   subroutine pi(n)
      integer :: n
      write(6,'(''case '',i0,'' ints'',3(1x,i0))') n,i1,i2,i3
   end subroutine
   subroutine pr(n)
      integer :: n
      write(6,'(''case '',i0,'' reals'',4(1x,z16.16))') n,transfer(r1,0_8),transfer(r2,0_8),transfer(r3,0_8),transfer(r4,0_8)
   end subroutine
   subroutine ps(n)
      integer :: n
      write(6,'(''case '',i0,'' str |'',a,''|'',a,''|'')') n,s1,s2
   end subroutine
   subroutine pe(n,io)
      integer :: n,io
      write(6,'(''case '',i0,'' ios-nonzero '',l1)') n,io.ne.0
   end subroutine
end program
