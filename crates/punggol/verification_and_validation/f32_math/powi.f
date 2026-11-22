      program p
      implicit none
      real*4 x, a
      integer*4 ia, n, i
      equivalence (a, ia)
      do i = 1, 3000
        x = 0.5 + 0.00131 * real(i)
        do n = -6, 9
          a = x ** n
          write(6,'(Z8.8)') ia
        end do
      end do
      end
