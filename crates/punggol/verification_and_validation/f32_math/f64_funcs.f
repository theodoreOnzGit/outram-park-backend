      program d
      implicit none
      real*8 x, y, a(8)
      integer*8 ia(8)
      equivalence (a, ia)
      integer i, k
      do i = 1, 20000
        x = 1.0d-3 * (1.0d0 + 0.00731d0 * dble(i)) ** 2
        y = -0.3333333d0
        a(1) = exp(x - 5.0d0)
        a(2) = log(x)
        a(3) = x ** y
        a(4) = sin(x)
        a(5) = cos(x)
        a(6) = tan(x * 0.01d0)
        a(7) = atan(x)
        a(8) = log10(x)
        write(6,'(8Z17.16)') (ia(k), k = 1, 8)
      end do
      end
