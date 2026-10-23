# JB2008 (Jacchia-Bowman 2008) — primary sources

Empirical thermospheric density model: Jacchia 1970/1971 static-structure
temperature and composition, driven by the F10.7 / S10 / Mg10 / Y10 solar
indices and a Dst-based exospheric-temperature storm correction.

**Apogee code:** `crates/apogee-core/src/aero/jacchia_bowman.rs` —
`JacchiaBowman::evaluate` (`jb2008_core`, the faithful path), plus a local
stopgap (see Flags).

**Lineage:** official `jb2008.for` (Space Environment Technologies /
AFSPC) -> pyatmos `JB2008_subfunc.py` (lcx366/ATMOS) -> this Rust port.
Coefficient-by-coefficient verified against `jb2008.for`: all 19 `B` + 23
`C` dTc coefficients, 5 `FZM` + 10 `GTM` semiannual coefficients, and every
xambar / xgrav / xlocal / tsubc / tsubl / seasonal constant match exactly.

## Primary sources

- **[B08]** Bowman, B.R., W.K. Tobiska, F.A. Marcos, C.Y. Huang, C.S. Lin,
  W.J. Burke (2008), "A New Empirical Thermospheric Density Model JB2008
  Using New Solar and Geomagnetic Indices", AIAA/AAS Astrodynamics
  Specialist Conference, 18-21 August 2008, Honolulu, Hawaii,
  AIAA 2008-6438, 19 pp. — free PDF:
  https://sol.spacenvironment.net/JB2008/pubs/AIAA_2008-6438_JB2008_Model.pdf
  (also mirrored at https://apps.dtic.mil/sti/pdfs/ADA637128.pdf).
- **[T08]** Tobiska, W.K., S.D. Bouwer, B.R. Bowman (2008), "The development
  of new solar indices for use in thermospheric density modeling",
  J. Atmos. Sol.-Terr. Phys. 70, 803-819 — free PDF:
  https://sol.spacenvironment.net/JB2008/pubs/JB2008_solar_geomag_indices-2.pdf
- **[J06]** Bowman, B.R., W.K. Tobiska, F.A. Marcos, C. Valladares (2008),
  "The JB2006 empirical thermospheric density model", J. Atmos. Sol.-Terr.
  Phys. 70, 774-793 — free PDF:
  https://sol.spacenvironment.net/JB2008/pubs/JASTP_Bowman_JB2006_2008.pdf
- **[J70]** Jacchia, L.G. (1970), "New Static Models of the Thermosphere and
  Exosphere with Empirical Temperature Profiles", Smithsonian Astrophysical
  Observatory Special Report 313 —
  https://ntrs.nasa.gov/api/citations/19700027590/downloads/19700027590.pdf
  Committed: `sources/pdfs/jacchia1970_sr313.pdf`.
- **[J71]** Jacchia, L.G. (1971), "Revised Static Models of the Thermosphere
  and Exosphere with Empirical Temperature Profiles", SAO Special Report 332
  (basis of CIRA 1972) —
  https://ntrs.nasa.gov/api/citations/19710024820/downloads/19710024820.pdf
  (95 MB scan — linked, not committed).
- **[FOR]** Official JB2008 FORTRAN source `jb2008.for` —
  https://sol.spacenvironment.net/JB2008/ (linked, not committed).

## Equations and constants

All quotes `OK` (born-digital PDFs except [J70]/[J71], which are tesseract
OCR of typewriter scans).

| Apogee code | Item | Source | Where | Verbatim (key text) | St. |
| --- | --- | --- | --- | --- | --- |
| :142-152 `tsubc` | nighttime-minimum exospheric temperature Tc | B08 | Eq. (2), p. 5 | "Tc = 392.4 + 3.227 FS + 0.298 dF10 + 2.259 dS10 + 0.312 dM10 + 0.178 dY10" | OK |
| :143 | F10/S10 blend weight WT | B08 | Eq. (1), p. 5 | "FS = F10 x WT + S10 x (1 - WT), where WT is the 4th root of (F10/240)" | OK |
| input struct | delta-index definition + per-index lags | B08 | Section III, p. 5 | "the difference of the daily and 81-day centered average value of each index"; "a lag time of 1 day ... for the F10 and S10 indices ... 2 days [M10] ... 5 days [Y10]" | OK |
| :329-334 `semian08` | semiannual correction | B08 | Eq. (3), p. 7 | "dSA log10 rho = F(z) G(t)" | OK |
| :659 | FSMJ solar blend for F(z) | B08 | Eq. (4), p. 8 | "FSMJ = 1.00 FJ - 0.70 SJ - 0.04 MJ" | OK |
| :651 `FZM` | F(z) height function + 5 coefficients | B08 | Eq. (5), p. 8 + Table 1, p. 9 | "F(z) = B1 + B2 FSMJ + B3 z FSMJ + B4 z2 FSMJ + B5 z FSMJ2" (all 5 match `DATA FZM`) | OK |
| :667 | FSM solar blend for G(t) | B08 | Eq. (6), p. 9 | "FSM = 1.00 F10 - 0.75 S10 - 0.37 M10" | OK |
| :655 `GTM` | G(t) Fourier series + 10 coefficients | B08 | Eq. (7), p. 9 + Table 2, p. 10 | "G(t) = C1 + C2 sin(w) + C3 cos(w) + C4 sin(2w) + C5 cos(2w) + FSM{C6 ... C10 cos(2w)}" (all 10 match `DATA GTM`) | OK |
| :171 storm `dstdtc` input | main-phase / recovery storm Dst corrections | B08 | Eqs. (8)-(10), pp. 13; (11)-(13), pp. 14-15 | "dTc1 = 0.846 dTc0 + 1.58[Dst1 - 0.870 Dst0]"; "S = -1.5050 x 10-5 (DstMIN)2 - 1.0604 x 10-2 DstMIN - 3.20 ... If DstMIN < -450 then S = -1.40" | OK |
| input F10.7/S10/Mg10/Y10 | solar index definitions | T08 | Table 1, p. 807 + Section 3, pp. 807-812 | "S10.7 UV EUV 26-34 [nm] Chromosphere, corona ..."; "the Mg II core-to-wing ratio (cwr) is calculated between the variable lines and nearly non-varying wings"; "X-rays+H Lyman-a ... 0.1-0.8, 121 [nm]" | OK |
| :404-645 `dtsub` | dTc correction polynomials per altitude band + B(19)/C(23) arrays | J06 | Eqs. (7)-(9), p. 785; definitions p. 786; Table 4, p. 787 | "either the DTc or the DTx values computed from Eq. (7), (8), or (9) are added to the Tc or Tx values (from Jacchia, 1970)"; "F = (F10 - 100)/100 ... y = (local solar time (h))/24"; all 19 B + 23 C values verified against `jb2008.for` | OK |
| :369-379 `xambar` | mean molecular mass M(z), 7-term polynomial | J70 | Eq. (1), p. 6 (PDF p. 10) | "M(z) = 28.15204 - 0.085586 z + 1.2840e-4 z2 - 1.0056e-5 z3 + ..." (all 7 coefficients match) | OK |
| :112, :115 | molecular weights AMW + sea-level volume fractions FRAC | J70 | composition table, p. 5 (PDF p. 9) | "Nitrogen (N2) 0.78110 [28.0134] ... Helium (He) 0.00001289 [4.0026] ... M0 = 28.960" | OK |
| :218-245 | number-density assembly below 105 km | J70 | Eqs. (2)-(4), pp. 6-7 (PDF pp. 10-11) | "N = A rho / M-bar ... n(i) = q0(i) M / M-bar N" | OK |
| :118, :219, :183 | RSTAR = 8314.32, rho0 = 3.46e-6 kg/m3, T0 = 183 K | J70 | Eq. (5), p. 8 (PDF p. 12) | "R* = 8.31432 x 10^7 ergs (deg K)^-1 mole^-1 ... rho0 = 3.46 x 10^-9 g cm^-3 ... T0 = 183 deg K" | OK |
| :237-238 | helium thermal diffusion alpha = -0.38 | J70 | Eq. (6), p. 8 | "alpha = -0.38 (helium)" | OK |
| :312 `alnh5` | helium scale-height relation | J70 | Eq. (7), p. 8 (PDF p. 12); identical in J71 Eq. (7), p. 12 | "log n(H) = 73.13 - 39.40 log T500 + 5.5 (log T500)^2" | OK |
| :382-384 `xgrav` | g(z), R0 = 6356.766 km | J70 | Eq. (8), p. 9 (PDF p. 13) | "g = 980.665 (1 + z/R0)^-2 cm sec^-2 ... R0 = 6.356766 x 10^8 cm" | OK |
| :175 | exopause temperature Tx(Tinf) | J70 | Eq. (9), p. 10 (PDF p. 14) | "a = 444.3807, b = 0.02385, c = -392.8292, k = -0.0021357" | OK |
| :176 | gradient constant G0 | J70 | p. 11 region (PDF p. 15) | "the best value was found to be 1.90 [deg K/km; 1.90/35 = 0.054285714]" | OK |
| :387-395 `xlocal` | temperature profile: arctangent tail above 125 km | J70 | Eq. (13), p. 11 (PDF p. 15) | "T = Tinf + A tan^-1 [...] ... B ~ 4.5 x 10^x [exponent unreadable in all scans; canonical value 4.5e-6 from `jb2008.for` XLOCAL]" | OK |
| :392 | below-125 km closed-form 4th-degree polynomial | J70 | Eqs. (10)-(12), pp. 10-11 (PDF pp. 14-15) | boundary conditions T(90) = 183 K, T'(90) = 0, T(125) = Tx, T'(125) = G0; the printed constants 9.8204695e-6 / -7.3039742e-4 are the algebraic closed form, verified numerically, canonical from `jb2008.for` ll. 297-303 | OK |
| :154-163 `tsubl` | diurnal bulge (m = 2.5, R = 0.31, n = 3, B = -37 deg, p = +6 deg, gamma = +43 deg) | J70 | Eqs. (15)-(17), pp. 18-19 (PDF pp. 22-23) | "m = 2.5 ... R = 0.31 ... [tau = H + B + p sin(H + gamma)]" | OK |
| :317-327 | seasonal-latitudinal density correction | J70 | Eq. (24), p. 25 (PDF p. 29) | "d log rho = 0.02 (z - 90) exp[-0.045 (z - 90)] sin^2(phi) sin[360 deg (d + 100)/Y]" (code phase 2*pi*psi + 1.72 rad is algebraically identical: 2*pi*100/365.2422 = 1.7201) | OK |
| :318 | year phase psi | J71 | Eq. (23), p. 44 (PDF p. 48) | "psi = 2*pi (t - 36204)/365.2422 ... M.J.D. 36204 corresponds to January 1.0, 1958" | OK |
| :124-130 `WT` | Newton-Cotes closed 5-point quadrature weights | FOR | ll. 197-201 | "DATA WT(1) /0.311111111111111D0/ (= 14/45) ... DATA WT(2) /1.422222222222222D0/ (= 64/45) ... DATA WT(3) /0.533333333333333D0/ (= 24/45)" | OK |
| :133, :344-363 `CHT`/`FEX` | high-altitude (1000-1500 km) exospheric density correction | FOR | ll. 203-205, 530-549 | "DATA CHT /0.22D0, -0.20D-02, 0.115D-02, -0.211D-05/" — no equation for this exists in [B08] (the code comment says "Equation (??)") | OK |
| context (not in Rust) | Kp-based geomagnetic heating (replaced by the Dst storm input) | J70 | Eq. (21), p. 21 (PDF p. 25) | "dTc = 28 Kp + 0.03 e^(Kp)" | OK |
| context (not in Rust) | tabulated semiannual form (replaced by [B08] Eqs. (3)-(7)) | J71 | Eqs. (21)-(23), pp. 43-44 (PDF pp. 47-48) | "d log10 rho semiannual = f(z) g(t)" | OK |

## Flags

- **Fabricated stopgap (audit 2026-10-19):** lines 79-104 — `evaluate_approx`
  plus `impl AtmosphereModel for JacchiaBowman` (which delegates to it at
  line 103) — are a local exponential-atmosphere approximation (scale height
  `h = 7 + 0.02*(F10.7-150)` clamped at 5, `rho = 1.225*exp(-alt/h)`,
  `T = 200 + 4*alt + 0.5*(F10.7-70)`). None of these formulas or constants
  appear in [B08], `jb2008.for`, pyatmos, or any Jacchia report. Everything
  else in the file was verified faithful. Flagged for removal or explicit
  re-citation as a local approximation.
- [J70] is SAO Special Report **313** (not 321); [B08]'s own reference list
  prints "Special Report No. 313" (1970) and "No. 332" (1971).
- AIAA 2008-6438 was presented at the AIAA/AAS Astrodynamics Specialist
  Conference (Honolulu), not the Aerospace Sciences Meeting.
- The Rust "Equation (n)" comments follow the Jacchia-1970/1971 numbering used
  by `jb2008.for`, not [B08]'s own Eqs. (1)-(13) (which cover solar blend
  through storm). Where J70 and J71/CIRA72 differ, the code follows J70:
  diurnal m = 2.5 / R = 0.31 (J71 uses 2.2 / 0.3), seasonal 0.02(z-90)
  exp[-0.045(z-90)] sin^2(phi) (J71 Eq. (24) uses 0.014 - 0.0013(z-90)^2 and
  sin(phi), not sin^2).
- Jacchia (1977) SAO SR 375 was fetched (97 pp.) but no direct equation
  dependency on it was found in the code; context reference only.