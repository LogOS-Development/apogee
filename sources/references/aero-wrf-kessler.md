# WRF Kessler warm-rain microphysics — primary sources

Single-moment warm-cloud scheme (water vapor + cloud water + rain water),
vendored from WRF v4.8.0 (`module_mp_kessler.F`, gitignored under
`external/`) behind the `wrf` cargo feature, driven through a single-column
C-ABI wrapper.

**Apogee code**

- `crates/apogee-core/src/aero/wrf.rs` — Rust FFI wrapper
  (`KesslerInput`/`KesslerOutput`, `constants`).
- `crates/apogee-core/external/wrf/vendor/phys/module_mp_kessler.F` —
  vendored scheme, verified line-for-line identical to upstream
  `wrf-model/WRF` master.
- `crates/apogee-core/external/wrf/wrf_kessler_c.F90` — 1x1-tile C-ABI
  column wrapper.

**Lineage:** Kessler (1969) -> Soong & Ogura (1973, per the NCAR ESCOMP
header) -> Klemp & Wilhelmson (1978) -> COMMAS (Wicker & Wilhelmson 1995) ->
WRF ("taken from the COMMAS code - WCS 10 May 1999", module_mp_kessler.F:20;
WCS = William C. Skamarock).

## Primary sources

- **[K69]** Kessler, E. (1969), "On the Distribution and Continuity of Water
  Substance in Atmospheric Circulations", Meteorological Monographs 10(32),
  AMS, doi:10.1007/978-1-935704-36-2_1 — publisher preview only (through
  Eq. 2.7 of Section 2); the parameterization sections could not be fetched
  (no OA scan exists). Kessler's own equation numbers for
  autoconversion/collection/evaporation/fallspeed remain `UNV`.
- **[TN4]** Skamarock, W.C., J.B. Klemp, J. Dudhia, et al. (2019), "A
  Description of the Advanced Research WRF Model Version 4", NCAR/TN-556+STR
  (162 pp., covers ARW through V4.1) — free PDF:
  https://www2.mmm.ucar.edu/wrf/users/docs/technote/v4_technote.pdf
  (not committed: UCAR copyright). Note: Section 8.1.1 is prose-only; chapter
  8 contains **no numbered equations** — citable equation numbers had to be
  traced via [Y80] and [ESC].
- **[ESC]** NCAR/ESCOMP `atmospheric_physics` `kessler.F90` (CCPP; "Based on
  a code by Joseph Klemp") — carries authoritative inline equation-number
  attributions for every process —
  https://raw.githubusercontent.com/ESCOMP/atmospheric_physics/main/schemes/kessler/kessler.F90
- **[Y80]** Yau, M.K. (1980), J. Atmos. Sci. 37, 488-494 — restates the
  Kessler (1969) warm-rain formulation with equation numbers. Scanned
  (image-only) via Wayback:
  http://web.archive.org/web/20250424032804/https://journals.ametsoc.org/downloadpdf/view/journals/atsc/37/2/1520-0469_1980_037_0488_teoewl_2_0_co_2.pdf
  (AMS copyright — not committed; quotes are 300-dpi tesseract OCR).
- **[WW95]** Wicker, L.J., R.B. Wilhelmson (1995), J. Atmos. Sci. 52,
  2675-2703 (COMMAS, the vendored module's direct ancestor) — via Wayback:
  http://web.archive.org/web/20190918234449/https://journals.ametsoc.org/doi/pdf/10.1175/1520-0469%281995%29052%3C2675%3ASAAOTD%3E2.0.CO%3B2
- **[KW78]** Klemp, J.B., R.B. Wilhelmson (1978), J. Atmos. Sci. 35 — source
  of Eqs. (2.11), (2.13a,b), (2.14a,b), (2.15). NOT fetched (AMS WAF; no
  archived OA copy); equation numbers `2H` via [ESC] source comments.
- **[DK83]** Durran, D.R., J.B. Klemp (1983), Mon. Wea. Rev. 111,
  2341-2361 — source of Eqs. (A1)-(A4), (A8)-(A9), (A13)-(A14). NOT fetched
  (AMS WAF); equation numbers `2H` via [ESC].

## Equations and constants

| Apogee code | Item | Source | Where | Verbatim (key text) | St. |
| --- | --- | --- | --- | --- | --- |
| module_mp_kessler.F whole | scheme identity + COMMAS provenance | TN4 | Section 8.1.1, printed p. 70 (PDF p. 87) | "This scheme (Kessler, 1969), which was taken from the COMMAS model (Wicker and Wilhelmson, 1995), is a simple warm cloud scheme that includes water vapor, cloud water, and rain." | OK |
| signature t/qv/qc/qr INOUT; wrf.rs KesslerInput/Output | end-of-timestep adjustment (no tendencies) | TN4 | Section 8.1, p. 70 | "microphysics is carried out at the end of the time-step as an adjustment process, and so does not provide tendencies but directly updates the state variables" | OK |
| :134-193 `column_sedimentation`; :142-147 | time-split flux-upstream fallout under 0.75 Courant limit; surface accumulation | TN4 | Section 8.1, p. 70 | "the sedimentation process is accounted for inside the individual microphysics modules ... either a smaller time step is allowed or a Lagrangian scheme is used" | OK |
| :203-236 | three-species continuity framework | K69 | Section 2 "Basic equations", Eqs. (2.1), (2.3)-(2.7) | "yields the continuity equation (2.1) ... m = rho_T - rho_s (2.4)" | OK |
| :23, :207, :210-212 | autoconversion (c1 = .001 s^-1 rate, c2 = .001 kg/kg threshold) | Y80 | p. 489 (below his Eq. (7)); values p. 492 | "AC = {K(q_c - a/rho_0), q_c > a/rho_0 ... The autoconversion rate K and autoconversion threshold a are 10^-3 s^-1 and 1 g m^-3" | OK |
| :25, :205-207, :210-212 | collection/accretion, semi-implicit (c3 = 2.2, c4 = .875 = 7/8) | Y80 + ESC | Y80 p. 489; ESC header citing KW78 Eq. (2.13a,b) | "CC = 928 q_c (rho_0 q_r)^(7/8) exp(kz/2)"; "the collection process is handled with a semi-implicit time stepping approach" | KW78 `2H` |
| :222-225 `ern` | rain evaporation | ESC | ESC citing KW78 Eq. (2.14a,b) + DK83 Eqs. (A8)-(A9) | "Evaporation rate following Klemp and Wilhelmson (1978) Eq. (2.14a,b), also Durran and Klemp (1983) Eqs. (A8-A9)" | `2H` |
| :125-127, :110-127 `vt` | terminal rain fallspeed 36.34 (qrr)^0.1364 + density correction | ESC | ESC citing KW78 Eq. (2.15); Y80 Eq. (8) is a variant with different constants | "VT = 36.34 qrr^0.1364 (rho0/rho)^0.5 [after KW78 Eq. (2.15)]" | `2H` |
| :216-219 | Tetens saturation es, qvs | ESC | ESC citing KW78 Eq. (2.11) + DK83 Eqs. (A13)-(A14) | saturation adjustment "following Klemp and Wilhelmson (1978) Eq. (2.11), also Durran and Klemp (1983) Eqs. (A13-A14)" | `2H` |
| :214-216, :221 | condensation/evaporation prod + f5 | ESC | ESC citing KW78 Eq. (3.10) + DK83 Eqs. (A1)-(A4) | "the condition is corrected using a saturation adjustment scheme (A1-A4)" | `2H` |
| :142-147 RAINNC/RAINNCV | accumulated precipitation convention | TN4 | Section 8.1.1, p. 70 | "the production, fall, and evaporation of rain" (RAINNC = accumulated mm) | OK |
| wrf_kessler_c.F90:34-70 | single-column, bottom-to-top convention | ESC | ESC header | "KESSLER is called at the end of each time step ... once for each vertical column of grid cells. ... The vertical loops in the column are ordered from the surface to the top." | OK |
| :20 module header | COMMAS ancestry | WW95 | Appendix, p. 2702 (following Eq. A20) | "Here M in (A15) represents the time tendencies due to the microphysical parameterizations. The microphysical calculations are computed as in Klemp and Durran (1983)." | OK |

## Flags

- The task-supplied WRF v4 tech-note DOI (10.5065/D1DZ57FT) is dead; the
  v4-era document is NCAR/TN-556+STR (DOI 10.5065/1dfh-6p97). The vendored
  tree is WRF v4.8.0 but the module is unchanged from the 2019 description.
- Kessler (1969) full text is paywalled (Springer preview only through
  Eq. 2.7; no OA scan on archive.org / HathiTrust / NOAA repositories).
  Kessler's own numbering for the parameterization equations is `UNV`; the
  scheme's constants in the vendored module follow the Klemp/COMMAS lineage,
  not Yau's variants.
- Y80 quotes are OCR of a scan; his Eq. (8) exponent readings are low-legibility
  and his variant constants differ from the vendored module's.
- journals.ametsoc.org blocks automated fetches (AWS WAF); Y80/WW95 were
  obtained as image-only scans via Wayback Machine snapshots.
- "Klemp and Durran (1983)" (WW95) vs "Durran and Klemp (1983)" (ESC) may be
  two distinct 1983 papers; the [DK83] equation-number attributions are
  exactly as printed in [ESC], not independently reconciled.
- Anthes & Warner (1978) could not be fetched and no text tying it to the
  Kessler parameterization was verified (optional lineage source).