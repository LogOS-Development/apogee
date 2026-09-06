! C-binding wrapper for WRF Kessler microphysics scheme.
! Exposes a single-column interface for calling from Rust.
! The original WRF kessler subroutine operates on 3D tiled arrays;
! this wrapper sets up a 1x1 tile so a single vertical column can be processed.

MODULE wrf_kessler_c
  USE, INTRINSIC :: iso_c_binding
  USE module_mp_kessler
  IMPLICIT NONE
CONTAINS

  ! Kessler microphysics for a single vertical column.
  !
  ! Inputs (all arrays length nk, bottom to top):
  !   t_in   — temperature (K, potential — multiplied by pii to get actual)
  !   qv_in  — water vapor mixing ratio (kg/kg)
  !   qc_in  — cloud water mixing ratio (kg/kg)
  !   qr_in  — rain water mixing ratio (kg/kg)
  !   rho_in — density (kg/m^3)
  !   pii_in — Exner function (dimensionless)
  !   z_in   — height (m)
  !   dz8w_in— layer thickness (m)
  !
  ! Scalar inputs:
  !   dt      — timestep (s)
  !   xlv     — latent heat of vaporization (J/kg)
  !   cp      — specific heat at constant pressure (J/kg/K)
  !   ep2     — Rv/Rd - 1 (0.622 for Earth)
  !   svp1..3 — saturation vapor pressure constants
  !   svpt0   — reference temperature for SVP (K)
  !   rhowater— density of liquid water (kg/m^3)
  !
  ! Outputs:
  !   t_out, qv_out, qc_out, qr_out — updated state (arrays length nk)
  !   rainnc  — accumulated rain at surface (mm)
  !   rainncv — rain rate this step (mm)
  SUBROUTINE wrf_kessler_column( &
      nk, &
      t_in, qv_in, qc_in, qr_in, rho_in, pii_in, z_in, dz8w_in, &
      dt, xlv, cp, ep2, svp1, svp2, svp3, svpt0, rhowater, &
      t_out, qv_out, qc_out, qr_out, &
      rainnc, rainncv) &
      BIND(C, name="wrf_kessler_column")

    INTEGER(c_int), VALUE, INTENT(IN) :: nk
    REAL(c_float), DIMENSION(nk), INTENT(IN) :: t_in, qv_in, qc_in, qr_in
    REAL(c_float), DIMENSION(nk), INTENT(IN) :: rho_in, pii_in, z_in, dz8w_in
    REAL(c_float), VALUE, INTENT(IN) :: dt, xlv, cp
    REAL(c_float), VALUE, INTENT(IN) :: ep2, svp1, svp2, svp3, svpt0, rhowater
    REAL(c_float), DIMENSION(nk), INTENT(OUT) :: t_out, qv_out, qc_out, qr_out
    REAL(c_float), INTENT(OUT) :: rainnc, rainncv

    ! WRF uses 3D arrays with memory dims >= tile dims.
    ! For a single column: ims=its=1, ime=ite=1, kms=kts=1, kme=kte=nk, jms=jts=1, jme=jte=1
    INTEGER, PARAMETER :: ids=1, ide=1, jds=1, jde=1, kds=1
    INTEGER :: kde, ims, ime, jms, jme, kms, kme
    INTEGER :: its, ite, jts, jte, kts, kte
    REAL, ALLOCATABLE :: t3d(:,:,:), qv3d(:,:,:), qc3d(:,:,:), qr3d(:,:,:)
    REAL, ALLOCATABLE :: rho3d(:,:,:), pii3d(:,:,:), z3d(:,:,:), dz8w3d(:,:,:)
    REAL, ALLOCATABLE :: rainnc2d(:,:), rainncv2d(:,:)
    INTEGER :: k

    kde = nk
    kms = 1; kme = nk
    ims = 1; ime = 1
    jms = 1; jme = 1
    its = 1; ite = 1
    kts = 1; kte = nk
    jts = 1; jte = 1

    ALLOCATE(t3d(ims:ime, kms:kme, jms:jme))
    ALLOCATE(qv3d(ims:ime, kms:kme, jms:jme))
    ALLOCATE(qc3d(ims:ime, kms:kme, jms:jme))
    ALLOCATE(qr3d(ims:ime, kms:kme, jms:jme))
    ALLOCATE(rho3d(ims:ime, kms:kme, jms:jme))
    ALLOCATE(pii3d(ims:ime, kms:kme, jms:jme))
    ALLOCATE(z3d(ims:ime, kms:kme, jms:jme))
    ALLOCATE(dz8w3d(ims:ime, kms:kme, jms:jme))
    ALLOCATE(rainnc2d(ims:ime, jms:jme))
    ALLOCATE(rainncv2d(ims:ime, jms:jme))

    ! Copy 1D input into 3D arrays
    DO k = 1, nk
      t3d(1,k,1)    = REAL(t_in(k))
      qv3d(1,k,1)   = REAL(qv_in(k))
      qc3d(1,k,1)   = REAL(qc_in(k))
      qr3d(1,k,1)   = REAL(qr_in(k))
      rho3d(1,k,1)  = REAL(rho_in(k))
      pii3d(1,k,1)  = REAL(pii_in(k))
      z3d(1,k,1)    = REAL(z_in(k))
      dz8w3d(1,k,1) = REAL(dz8w_in(k))
    END DO

    rainnc2d(1,1) = 0.0
    rainncv2d(1,1) = 0.0

    CALL kessler( &
      t3d, qv3d, qc3d, qr3d, rho3d, pii3d, &
      REAL(dt), z3d, REAL(xlv), REAL(cp), &
      REAL(ep2), REAL(svp1), REAL(svp2), REAL(svp3), REAL(svpt0), REAL(rhowater), &
      dz8w3d, &
      rainnc2d, rainncv2d, &
      ids, ide, jds, jde, kds, kde, &
      ims, ime, jms, jme, kms, kme, &
      its, ite, jts, jte, kts, kte)

    ! Copy 3D output back to 1D
    DO k = 1, nk
      t_out(k)  = REAL(t3d(1,k,1), c_float)
      qv_out(k) = REAL(qv3d(1,k,1), c_float)
      qc_out(k) = REAL(qc3d(1,k,1), c_float)
      qr_out(k) = REAL(qr3d(1,k,1), c_float)
    END DO

    rainnc  = REAL(rainnc2d(1,1), c_float)
    rainncv = REAL(rainncv2d(1,1), c_float)

    DEALLOCATE(t3d, qv3d, qc3d, qr3d, rho3d, pii3d, z3d, dz8w3d)
    DEALLOCATE(rainnc2d, rainncv2d)

  END SUBROUTINE wrf_kessler_column

END MODULE wrf_kessler_c