# NRLMSISE-00 — primary sources

Empirical Earth atmosphere model: thermosphere above the 120 km boundary
(Bates-Walker profiles) plus the extended mesosphere/lower atmosphere
(Hedin 1991 spline), driven by F10.7 / F10.7A / Ap.

**Apogee code**

- `crates/apogee-core/src/aero/nrlmsise00.rs` — `Nrlmsise00: AtmosphereModel`
  adapter (`NRLMSISE00_VERSION`, `full_model_flags()` with all 24 switches
  on, `local_solar_time()`).
- `crates/apogee-core/src/aero/model.rs` — `AtmosphereInput` (`f107`,
  `f107a` 81-day centred mean, `ap`; quiet-time defaults 150 / 150).
- `crates/apogee-core/external/nrlmsise00_brahe/` — vendored algorithm
  (`mod.rs`) and the complete coefficient tables (`data.rs`).

**Lineage:** NRL Fortran `NRLMSISE-00.FOR` -> Brodowski's C port
(brodo.de/space/nrlmsise) -> Brahe (Rust, MIT, duncaneddy/brahe) ->
vendored `nrlmsise00_brahe`.

## Primary sources

- **[P02]** Picone, J.M., Hedin, A.E., Drob, D.P., Aikin, A.C. (2002),
  "NRLMSISE-00 empirical model of the atmosphere: Statistical comparisons
  and scientific issues", J. Geophys. Res. 107(A12), 1468,
  doi:10.1029/2002JA009430 — published PDF (free):
  https://map.nrl.navy.mil/map/pub/nrl/NRLMSIS/NRLMSISE-00/NRLMSISE-00_jgra16630.pdf
  (NTRS 20020038771 is the author preprint with different pagination).
- **[H91]** Hedin, A.E. (1991), "Extension of the MSIS Thermosphere Model
  into the middle and lower atmosphere", J. Geophys. Res. 96(A2),
  1159-1172, doi:10.1029/90JA02125 — no free PDF exists (checked OpenAlex,
  Unpaywall, Semantic Scholar, NTRS 19910040911); quotes verified from the
  Wiley HTML text.
- **[H87]** Hedin, A.E. (1987), "MSIS-86 Thermospheric Model",
  J. Geophys. Res. 92(A5), 4649-4662, doi:10.1029/JA092iA05p04649 —
  paywalled, no open-access copy exists (checked NTRS 19870053927,
  archive.org, OpenAlex); all [H87] equation numbers below are `2H`.
- **[NRL]** U.S. Naval Research Laboratory, NRLMSISE-00 distribution:
  `NRLMSISE-00.FOR` (source + coefficient DATA blocks + test driver) plus
  the "Electronic Data Submission for Paper 2002JA009430" (readme.txt,
  datavsmodels.txt, tables-datasets.doc) —
  https://map.nrl.navy.mil/map/pub/nrl/NRLMSIS/NRLMSISE-00/
  Committed unmodified: `sources/nrlmsise-00.for`,
  `sources/nrlmsise-00-readme.txt`.

## Equations and constants

All quotes `OK` unless marked.

| Apogee code | Item | Source | Where | Verbatim (key text) | St. |
| --- | --- | --- | --- | --- | --- |
| mod.rs:593-738 `densu` | species number-density profiles above 120 km | P02 | App. A1.1 [58] | "the Bates-Walker equations [Walker, 1965] represent the basic profiles of the temperature and of species number density as analytic functions of altitude. These equations are an exact solution for thermal and diffusive equilibrium and include thermal diffusion." | OK |
| mod.rs:768-1047, 1158-1366 `globe7`/`gtd7` | exospheric temperature Tex, T(z) assembly | P02 | App. A1.1 [59] (equations per the Hedin [1987] appendix) | "The Bates-Walker temperature profile variables are the exospheric temperature Tex; the temperature at zlb = 120 km, T120; and the temperature gradient at zlb." | OK |
| mod.rs:1048-1157 `glob7s` | 120-km density boundary conditions as harmonic expansions | H91 | Introduction, p. 1160 | "Parameters of the temperature profile as well as density boundary conditions at 120 km are expanded in low-order spherical harmonics and Fourier series (in both time of day and day of year) ... For this model extension, the 120-km boundary conditions are unchanged" | OK |
| mod.rs:857; model.rs:24-27 | solar-EUV dependence incl. new cross term; F10.7/F10.7A/Ap definitions | P02 | App. A1.1 item 1; [NRL] Fortran header | "G(solar) = A dF (1 + B \<dF\>) + C dF^2 + D \<dF\> + E (\<dF\>)^2"; "F107 - DAILY F10.7 FLUX FOR PREVIOUS DAY"; "AP(2) = 3 HR AP INDEX FOR CURRENT TIME" | OK |
| mod.rs:739-767 `g0`/`sumex`/`sg0` | 3-hr Ap magnetic-activity history functions | H87 | Eqs. A24d (G0), A24c (SUMEX), A24a (SG0) — attested by comments in [NRL] Fortran ll. ~976-994 | "C Eq. A24d / FUNCTION G0(A) ... C Eq. A24c / FUNCTION SUMEX(EX) ... C Eq. A24a / FUNCTION SG0(EX,P,AP)" | 2H |
| mod.rs:250-302 `ccor`/`ccor2` | lower-thermosphere chemistry multiplier C1, with O/O2 constants (ll. 1832-1843) | H87 via P02 | P02 App. A1.1 item 3 | "The constants are Rc(O) = 0.076, Rc(O2) = 0.75, af = 0.031, Hc1(O) = -Hc1(O2) = 34.5 km, Hc2(O) = -Hc2(O2) = 12.9 km, and zc(O) = zc(O2) = 126.5 km." | OK |
| mod.rs:1390 `alpha` | thermal diffusion factors [-0.38, 0, 0, 0, 0.17, 0, -0.38, 0, 0] | P02 | App. A1.1 item 4 (citing Banks and Kockarts [1973]) | "ai = -0.38 (i = He, H), 0.17 (i = Ar), 0.0 (other species). This represents a change from MSISE-90, which had ai = -0.4" | OK |
| mod.rs:351-455 splines; 457-592 `densm` | mesosphere/lower-atmosphere inverse-temperature cubic spline, zn1/zn2/zn3 nodes | H91 | Model Formulation, p. 1161 | "a cubic spline formulation is used to calculate inverse temperature ... The nodes were chosen to be at 122.8, 110, 100, 90, 72.5, 55, 45, 32.5, 20, 15, 10, and 0 km" | OK |
| mod.rs:919-1010 (sg0 terms) | vertical cutoffs in the lower atmosphere | H91 | p. 1162 | "temperature variations have been included only down to 72.5 km ... The F10.7 and Ap variations have not been carded [as printed; 'carried'] below 110 and 90 km, respectively" | OK |
| mod.rs:227-248 `glatf` | latitude-dependent effective Earth radius and gravity | H91 | p. 1162 | "A latitude dependent acceleration of gravity and effective Earth radius have been used to be consistent with the usual practice in the lower atmosphere." | OK |
| mod.rs:457-592 merge; gtd7 | 0-120 km hydrostatic boundary conditions | P02 | App. A2 [64], A3 | "pressure and density are defined by hydrostatic equilibrium and the ideal gas law [Hedin, 1991] ... NRLMSISE-00 retained the MSISE-90 coefficients below 72.5 km" | OK |
| mod.rs:2112-2136; 2191-2208 `gts7`/`gtd7d` | anomalous oxygen; effective total mass density above ~500 km | P02 | Abstract; App. A1.1 item 2; [NRL] Fortran header | "A new component, 'anomalous oxygen,' allows for appreciable O+ and hot atomic oxygen contributions to the total mass density at high altitudes and applies primarily to drag estimation above 500 km."; "[Oa](z) = [Oa](zlb) exp{-x(z,zlb)/H(zlb,Ta)} [1 - exp{-Cha|za-z|/(C*H)}], with C = 76 km, za = 550 km, Ta = 4000 K" | OK |
| data.rs:12-743 | coefficient tables PT/PD/PS/PDL/PTM/PDM/PTL/PMA/SAM/PAVGM | NRL | DATA blocks, Fortran ll. 1673-2432; P02 App. A3/A4 | "The present NRLMSISE-00 distribution package is an ASCII file containing the model source, a test driver, and the expected output of the test driver." | OK |

## Flags

- The paper is sometimes cited as doi:10.1029/2002JA009290 titled
  "...statistical advances" — both are wrong; the published header reads
  doi:10.1029/2002JA009430 / "Statistical comparisons and scientific
  issues" (also confirmed by NRL's readme: "Electronic Data Submission for
  Paper 2002JA009430").
- Hedin (1987) could not be fetched (Wiley 403 / Cloudflare challenge; no
  open-access copy exists). Every [H87] equation number is `2H` — attested
  via the fetched [P02] appendix text and [NRL] Fortran source comments.
- The vendored Rust tags both `g0` and `sg0` as "Equation A24a"; the NRL
  Fortran labels G0 as Eq. A24d (transcription quirk of the port chain).
- Direct downloads from map.nrl.navy.mil fail from this host (TLS resets);
  the published PDF and Fortran source were fetched through a browser
  session. Quotes from the PDF text layer are ligature-normalized.