! C-compatible wrapper for HWM14.
! Exposes initialization and evaluation functions with the C ABI.
module hwm14_c
    use, intrinsic :: iso_c_binding, only: c_float, c_int
    implicit none

    interface
        subroutine inithwm()
        end subroutine inithwm

        subroutine hwm14(iyd, sec, alt, glat, glon, stl, f107a, f107, ap, w)
            integer(4), intent(in) :: iyd
            real(4), intent(in) :: sec, alt, glat, glon, stl, f107a, f107
            real(4), intent(in) :: ap(2)
            real(4), intent(out) :: w(2)
        end subroutine hwm14
    end interface

contains

    ! Initialize the HWM14 model. Must be called once before evaluate.
    subroutine hwm14_init_c() bind(c, name="hwm14_init")
        implicit none
        call inithwm()
    end subroutine hwm14_init_c

    ! Evaluate HWM14 winds.
    ! Inputs:
    !   iyd        - year and day as yyddd
    !   sec        - UT seconds
    !   alt        - altitude (km)
    !   glat       - geodetic latitude (deg)
    !   glon       - geodetic longitude (deg)
    !   stl        - local solar time (not used by model, pass -1)
    !   f107a      - not used
    !   f107       - not used
    !   ap2        - current 3hr ap index
    ! Outputs:
    !   meridional - northward wind (m/s)
    !   zonal      - eastward wind (m/s)
    subroutine hwm14_evaluate_c(iyd, sec, alt, glat, glon, stl, f107a, f107, ap2, &
                                meridional, zonal) bind(c, name="hwm14_evaluate")
        implicit none
        integer(c_int), intent(in), value :: iyd
        real(c_float), intent(in), value :: sec, alt, glat, glon, stl, f107a, f107, ap2
        real(c_float), intent(out) :: meridional, zonal

        real(c_float) :: ap(2)
        real(c_float) :: w(2)

        ap(1) = -1.0
        ap(2) = ap2

        call hwm14(iyd, sec, alt, glat, glon, stl, f107a, f107, ap, w)

        meridional = w(1)
        zonal = w(2)
    end subroutine hwm14_evaluate_c

end module hwm14_c
