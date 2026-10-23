# Primary-source reference sheets

One sheet per physics/algorithm module under `sources/references/`, produced
by the 2026-10-22 full-codebase citation audit. Each sheet traces every
implemented formula, constant, and coefficient table to its primary source:
the exact equation/table number, a verbatim quote, the page/section, and the
URL the source was fetched from.

Status marks used in the sheets:

- `OK` verified — quote read from the fetched source text during the audit
- `2H` secondhand — attested only through a fetched intermediary (the
  primary is paywalled but its equation numbers appear verbatim in a
  fetched source, e.g. a code distribution or companion paper)
- `UNV` unverified — could not be checked against any fetched document

## Sheets

| Module | Sheet |
| --- | --- |
| NRLMSISE-00 empirical atmosphere | `aero-nrlmsise00.md` |
| Jacchia-Bowman 2008 thermosphere | `aero-jb2008.md` |
| HWM14 horizontal winds (FFI) | `aero-hwm14.md` |
| WRF Kessler warm-rain microphysics (FFI) | `aero-wrf-kessler.md` |
| Rothermel fire spread + level-set front | `verdant-fire-rothermel.md` |
| Horn slope/aspect, bilinear interpolation, GeoTIFF | `verdant-terrain-grid.md` |
| EGM2008 geopotential + gravity-gradient torque | `gravity-spherical-harmonics.md` |
| IGRF-13 main geomagnetic field | `magnetosphere-igrf.md` |
| SPICE SPK DAF reader (types 2 / 3 / 13) | `ephemeris-spk-chebyshev.md` |
| IAU 2006/2000B frames, ERA, EOP, time scales | `frames-time.md` |

The embedded Runge-Kutta coefficients (`crates/apogee-core/src/integrator/`)
are covered by `sources/README.md` and cross-checked bit-exact at test time.

## Committed source files

Redistributable primary sources are committed under `sources/` and
`sources/pdfs/`: US Government works (USDA, NASA/NTRS, NRL, NOAA), CC-BY
open-access papers, and free-distribution technical notes (IERS).
Copyrighted publisher PDFs (AIAA, Elsevier, Springer, Wiley, AMS, UCAR,
OUP) are linked in the sheets, never committed.

| file | sha256 |
| --- | --- |
| `nrlmsise-00.for` | `cce0420e90781c256bc6705c4cc8056b054812d6308adccb7081bf09af0d44cb` |
| `nrlmsise-00-readme.txt` | `3a6e3d6ff89985d13fdbff4c348d9161bf10f50fcbba8f6d09b501b2625058dc` |
| `igrf13coeffs.txt` | `460b8d8beb9b4df84febe4f0b639f0dd54dccfe8ff0970616287b015fa721425` |
| `pdfs/rothermel1972_int115.pdf` | `42a272dd975f3a92de1af31aecfe94e92ef017a89544114b9c66ba161b28c86a` |
| `pdfs/anderson1982_int122.pdf` | `0a44624d1403f3cd1dd2a7bc6376664c2c3d63577d38a549dd5ee659c37ac647` |
| `pdfs/albini1976_int30.pdf` | `68f688655cebcbc04c9a2f9bb95c86005ad622323edd983aa13448bd6ee4d301` |
| `pdfs/finney_farsite_rp4.pdf` | `7b367c4348b0c6c5773af440d415872ebf7a24e6ef4a79c25317c6dc54e8a935` |
| `pdfs/jacchia1970_sr313.pdf` | `40788f95744df71cee8f86f5103d7ed1d086070ce83647c6bf62a8fab3117e25` |
| `pdfs/alken2021_igrf13.pdf` | `b13127800ade7bb859b2b0ace23bfaa912a41d1130cc798608cfd308964448e3` |
| `pdfs/sugiura1965_dst.pdf` | `9ca882a686913dfa6d784b7c862059d6296db941648e0e65c046da42bbb14c49` |
| `pdfs/skylab_tea.pdf` | `bc3203ba0dcf600c5add8ea74744c634474a352e594e839bd9c59ba498e7fd62` |
| `pdfs/egm96.pdf` | `e28ae2ac6fe0316d68800b651042c7ca4d5c58883feada0f7789c434dd0721ba` |
| `pdfs/tn36.pdf` | `b0570ff2456d76977dc56918c64130cc5bf68b679fa8fffb989137f2b833eea` |

## Notable findings of the audit

- SPK type-3 velocity handling diverges from SPICE by a `1/RADIUS` factor
  (velocities wrong against real type-3 segments) —
  `ephemeris-spk-chebyshev.md`, fix tracked in issue #205.
- `jacchia_bowman.rs:79-104` is a local exponential-atmosphere stopgap, not
  JB2008 — `aero-jb2008.md`.
- The verdant terrain `aspect` returns the gradient (steepest-ascent)
  azimuth, 180 degrees from the ESRI/GDAL downslope convention —
  `verdant-terrain-grid.md`.
- Several stale or phantom citations exist in code comments (broken ppigrf
  URL, nonexistent "Lowes & Winch 1991" and "IAGA VMOD technical note 6",
  ERA cited as Eq. 5.15 where 5.14 is implemented) — see the Flags section
  of the respective sheets.

## Method

Every quote was fetched from its URL on 2026-10-22 — nothing is cited from
memory. Scanned documents (Rothermel 1972, Anderson 1982, Albini 1976,
Jacchia 1970/1971, Yau 1980, Wicker & Wilhelmson 1995, EGM96, Sugiura 1965,
Skylab TM-78252) were read via pdftotext / 300-dpi tesseract OCR; extraction
artifacts are marked [sic] / [as printed] where they matter. A sample of
quotes from every module was independently re-checked against the fetched
files before these sheets were written.