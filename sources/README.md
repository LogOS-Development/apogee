# Reference sources for integrator coefficients

Primary reference implementations for the embedded Runge-Kutta pairs in
`crates/apogee-core/src/integrator/`:

- `rk45.rs` implements DOPRI5, the 5(4) pair of Dormand & Prince (1980).
- `rk89.rs` implements DOP853, the 8(5,3) triple of Prince & Dormand (1981)
  as distributed by Hairer & Wanner.

Both files are the authors' own Fortran sources, retrieved unmodified on
2026-10-22 from the official page
https://www.unige.ch/~hairer/software.html (nonstiff problem set).

The test suite parses these files at test time and cross-checks every
coefficient constant against the primary source — see
`crates/apogee-core/src/integrator/tableau_sources.rs` and the
`test_rk45_tableau_matches_dopri5_source` /
`test_rk89_tableau_matches_dop853_source` tests. Any transcription error
in the Rust constants fails CI with the offending name and both values.

## Files

| file       | version                              | sha256                                                         |
| ---------- | ------------------------------------ | -------------------------------------------------------------- |
| `dop853.f` | VERSION OF OCTOBER 11, 2009          | `8bfc0a954a38af1a458f5ab8025fb09c923373a936f52ff61e32f1a15c8f298a` |
| `dopri5.f` | VERSION OF APRIL 25, 1996            | `e4fbbe080508134f743d9934e76e45f49f03a320b7b024273757a7c278804c95` |

## Bibliography

- J. R. Dormand, P. J. Prince, "A family of embedded Runge-Kutta formulae",
  Journal of Computational and Applied Mathematics 6(1), 19-26 (1980).
  DOI 10.1016/0377-0427(80)90013-3.
- P. J. Prince, J. R. Dormand, "High order embedded Runge-Kutta formulae",
  Journal of Computational and Applied Mathematics 7(1), 67-75 (1981).
  DOI 10.1016/0771-050X(81)90010-3.
- E. Hairer, S. P. Norsett, G. Wanner, "Solving Ordinary Differential
  Equations I: Nonstiff Problems", 2nd ed., Springer Series in Computational
  Mathematics 8, Springer-Verlag (1993) — DOP853 coefficients in section II.10
  table.

The classic fourth-order tableau in `rk4.rs` is W. Kutta,
"Beitrag zur näherungsweisen Integration totaler Differentialgleichungen",
Zeitschrift für Mathematik und Physik 46, 435-453 (1901) — public domain;
volume scans are hosted by archive.org (Wikisource source listing:
"Zeitschrift für Mathematik und Physik, Band 46"). Its weights carry no
transcription risk beyond the Simpson quadrature, which the test
`test_rk4_quadrature_is_exact_for_polynomials` pins exactly, so no source
file is committed for it.

## License note

Hairer's Fortran sources carry no license header; his page distributes them
free of charge for scientific and educational use. They are committed here
unmodified as reference material for coefficient verification. Remove them
(and the `tableau_sources` tests) before any commercial redistribution.