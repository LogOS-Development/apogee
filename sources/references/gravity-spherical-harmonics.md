# Gravity: EGM2008 spherical harmonics + gravity-gradient torque — primary sources

Earth geopotential as a spherical-harmonic expansion (fully-normalized
associated Legendre functions, tide-free coefficients, ICGEM .gfc input),
plus point-mass N-body gravity and the gravity-gradient torque on an
attitude-controlled spacecraft.

**Apogee code**

- `crates/apogee-core/src/gravity/spherical_harmonics.rs` — potential
  expansion, Legendre recursions, ICGEM parser, J2-only closed form (test
  oracle).
- `crates/apogee-core/src/gravity/point_mass.rs` — N-body point-mass
  acceleration.
- `crates/apogee-core/src/gravity/gradient_torque.rs` — gradient torque.
- `crates/apogee-core/src/star_system/definition.rs` — body presets,
  C_2,0 <-> J2 conversion (Earth EGM2008; Moon GL0660B; Mars).

## Primary sources

- **[P12]** Pavlis, N.K., S.A. Holmes, S.C. Kenyon, J.K. Factor (2012),
  "The development and evaluation of the Earth Gravitational Model 2008
  (EGM2008)", J. Geophys. Res. 117, B04406, doi:10.1029/2011JB008916 —
  open access (Wiley Cloudflare-blocks direct fetch; archived copy read):
  http://web.archive.org/web/20230911113509/https://agupubs.onlinelibrary.wiley.com/doi/pdf/10.1029/2011JB008916
- **[HF02]** Holmes, S.A., W.E. Featherstone (2002), "A unified approach to
  the Clenshaw summation and the recursive computation of very high degree
  and order normalised associated Legendre functions", J. Geodesy 76,
  279-299, doi:10.1007/s00190-002-0216-2 — green OA author-accepted
  manuscript: https://ndownloader.figshare.com/files/62426692
  (Springer copyright — not committed; OA manuscript pagination differs
  from the published pages).
- **[L98]** Lemoine, F.G., et al. (1998), "The Development of the Joint NASA
  GSFC and NIMA Geopotential Model EGM96", NASA/TP-1998-206861 —
  https://ntrs.nasa.gov/citations/19980218814
  Committed: `sources/pdfs/egm96.pdf` (US Gov).
- **[S16]** Sun, Y., P. Ditmar, R.E.M. Riva (2016), "Observed changes in the
  Earth's dynamic oblateness from GRACE data and geophysical models",
  J. Geodesy 90(1), 81-89, doi:10.1007/s00190-015-0852-y — open access:
  http://web.archive.org/web/20220302051417/https://link.springer.com/content/pdf/10.1007/s00190-015-0852-y.pdf
- **[NGA14]** NGA (2014), "Department of Defense World Geodetic System 1984"
  (NGA.STND.0036_1.0.0_WGS 84) — official:
  https://earth-info.nga.mil/GandG/publications/NGA_STND_0036_1.0.0_WGS84.pdf
  (public domain; not committed — linked).
- **[SN22]** Sneeuw, N. (2022), "Dynamic Satellite Geodesy" lecture notes,
  Univ. Stuttgart (free):
  https://www.gis.uni-stuttgart.de/lehre/LectureNotes/DynSatGeo_LecNotes_Oct2022.pdf
- **[GK79]** Glaese, J.R., H.F. Kennel (1979), "Torque Equilibrium Attitude
  Control for Skylab Reentry", NASA TM-78252 —
  https://ntrs.nasa.gov/citations/19800005912
  Committed: `sources/pdfs/skylab_tea.pdf` (US Gov).
- **[IS200]** GPS Directorate (2022), "Interface Specification IS-GPS-200N"
  — archived:
  http://web.archive.org/web/20250915232426if_/https://www.gps.gov/technical/icwg/IS-GPS-200N.pdf
- **[ICGEM]** Förste, C., F. Barthelmes, E.S. Ince (2023), "The ICGEM-format"
  (GFZ): https://icgem.gfz.de/docs/ (free).
- **[OS]** OpenStax, "University Physics Volume 1", Section 13.1 (CC BY 4.0):
  https://openstax.org/books/university-physics-volume-1/pages/13-1-newtons-law-of-universal-gravitation

## Equations and constants

| Apogee code | Item | Source | Where | Verbatim (key text) | St. |
| --- | --- | --- | --- | --- | --- |
| spherical_harmonics.rs:214-284 | geopotential expansion V(r,theta,lambda) | P12 | Eq. (1) with Eqs. (2)-(3), PDF p. 4 ("B04406-4") | "V(r,theta,lambda) = GM/r {1 + SUM_n (a/r)^n SUM_m C_nm^s Y_nm(theta,lambda)} ... Equation (1) refers to the permanent part of the gravity field" | OK |
| :249-263 | real cos/sin form of the expansion | SN22 | Eq. (6.5) with normalization factor Eq. (6.6), pp. 69-70 | "V(r,theta,lambda) = (GM/R) SUM SUM (RE/r)^(l+1) (C_lm cos m lambda + S_lm sin m lambda) P_lm(cos theta) (6.5)" | OK |
| :345-417 + 'norm fully_normalized' | fully-normalized surface spherical harmonics convention | P12 | Eq. (2), PDF p. 4 | "The fully normalized surface spherical harmonic functions are defined as [Heiskanen and Moritz, 1967, section 1-14]" | OK |
| :380-384 (sectoral chain), :66 comment | normalization factor N_nm; N_2,0 = sqrt(5) | HF02 | Eq. (8) (adapted from Heiskanen & Moritz Eq. (1-73)) | "P_nm(theta) = sqrt[k(2n+1)(n-m)!/(n+m)!] P_nm(theta) (8), where k = 1 for m = 0 and k = 2 for m > 0" — n=2, m=0 gives sqrt(5) = 2.23606797749979 | OK |
| :391-411 `normalized_legendre` | forward-column recursions a_nm, b_nm (+ zonal, sub-diagonal) | HF02 | Eqs. (11)-(12) (+ Eq. (14) sectoral chain) | "P_nm(theta) = a_nm t P_(n-1),m(theta) - b_nm P_(n-2),m(theta) (11); a_nm = sqrt[(2n-1)(2n+1)/((n-m)(n+m))], b_nm = sqrt[(2n+1)(n+m-1)(n-m-1)/((n-m)(n+m)(2n-3))] (12)" | OK |
| :64-72, 70; definition.rs:455-456 | tide-free C_2,0 = -0.484165143790815e-03 | P12 | Table 13 + footnote a, PDF p. 34 | "Table 13 ... 2: -0.484165143790815D-03 ... a The C_2,0 value is in the tide-free system." | OK |
| :634-651 ('tide_system tide_free') | tide-free system definition | L98 | Section 11.1, pp. 11-1..11-2; Appendix C | "Tide-free (or nontidal) — This geoid would exist for a tide-free Earth with all (direct and indirect) effects of the Sun and Moon removed ... the preferred tide system would be the tide-free system" | OK |
| definition.rs:248-260, 438-474 | C_2,0 = -J2/sqrt(5) relation | S16 | Introduction (article p. 81) | "J2 = -sqrt(5) C20, where the factor sqrt(5) implicitly means that the C20 is normalised" | OK |
| same | C_2,0 = -J2/sqrt(5) (second, standard-form attestation) | NGA14 | Section 3.2 Eq. (3-1), printed p. 3-1 | "J2 and C_2,0 are related by [Eq. (3-1)]" (radical glyph lost by pdftotext; OCR reads sqrt(5); verified numerically: 0.108262982131e-2 / 2.23606797749979 = 4.84166774985e-4 = WGS 84 C_2,0) | OK |
| same | general relation C_2n,0 = -J_2n sqrt(2n+1) | L98 | Eq. (11.4-3), p. 11-5 | "C_2n,0(ref) = -J_2n sqrt(2n+1) [Eq. 11.4-3]" (radical lost by pdftotext; verified numerically against Eq. (11.4-4)) | OK |
| :66 comment | unnormalized J2 ~ 1.08263e-3 | SN22 | Section 4.2/4.3, pp. 54-55 | "So the along-track component of the gradient ... the radial derivative by the usual -(l+1)/r" + J2 value | OK |
| :419-452 `j2_closed_form` | J2-only oblate-Earth Cartesian acceleration (test oracle) | IS200 | Table 20-IV, p. 109 | "F = -(3/2) J2 (mu/r^2)(RE/r)^2 [Eq. 20-19] ... ax = -mu x/r^3 + F [(1 - 5(z/r)^2)(x/r)] [Eq. 20-20]" (verified to machine precision) | OK |
| point_mass.rs:17-44 | N-body point-mass Newtonian acceleration | OS | Section 13.1, Eq. (13.1) | "F = G m1 m2/r^2 ... a1 = G m2/r^2 toward m2" (standard) | OK |
| gradient_torque.rs:7, 38-40 | gravity-gradient torque tau = 3 GM/R^5 (R x (I R)) | GK79 | Appendix B, Eqs. (B8), (B10)-(B12), pp. 68-69 | "T_gg = (3 GM/R0^3)(R0_hat x I R0_hat) (B10) ... omega_0 = sqrt(GM/\|R0\|^3) (B11) ... T_gg = 3 omega_0^2 u x I u_R (B12) — what is commonly referred to as the gravity gradient torque" | OK |
| :77-197, 315-341 | ICGEM .gfc header keys + 'gfc' data rows | ICGEM | key table, p. 2 | "tide_system: either 'zero_tide', 'tide_free', 'mean_tide' or 'unknown' (default); norm: either 'fully_normalized' (=default) or 'unnormalized'; gfc: degree, order, C_lm, S_lm, sigma_C sigma_S" | OK |
| :245-283 | dU/dr, dU/dphi, dU/dlambda partials (-(n+1)/r radial scaling) | SN22 | Sections 3.1, 6.5.1 | "the radial derivative by the usual -(l+1)/r" | OK |

## Flags

- pdftotext drops radical glyphs on [NGA14] Eq. (3-1) and [L98] Eq.
  (11.4-3); both were confirmed via 300-dpi tesseract OCR **and** by
  numeric verification (sqrt(5) x C_2,0 reproduces the code constants for
  Earth, Moon, and Mars exactly).
- The code's normalized_legendre recursion (sectoral/zonal/sub-diagonal
  chains) was verified numerically against closed-form fully-normalized ALFs
  (P_20, P_21, P_22, P_30, P_11, P_33) — exact match to [HF02] Eqs.
  (11)-(14) with t = cos(colatitude) = sin(lat).
- IS-GPS-200N's broadcast J2 = 0.0010826262 is the WGS 84 ellipsoidal value;
  the code's Earth J2 = 1.08262617e-3 derives from EGM2008's tide-free
  C_2,0 ([P12] Table 13) — they differ in the 8th significant digit by
  construction.
- Out of scope of these citations: Moon GL0660B / Mars J2 values and the
  JPL/IAU GM + radius constants (definition.rs:452-474) are stated in-code
  as coming from those solutions; not independently re-verified here.
- Fetch blockers worked around: [P12] and [S16] via Internet Archive
  snapshots; [HF02] via the Curtin figshare green-OA manuscript
  (HTTP 202 to curl — browser download).