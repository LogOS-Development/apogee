# Verdant terrain/grid geometry — primary sources

Slope, aspect, and elevation sampling on a GeoTIFF DEM: Horn's 3x3
weighted-difference gradient stencils, bilinear interpolation, and the
ModelTiepointTag / ModelPixelScaleTag -> affine geotransform mapping.

**Apogee code**

- `crates/verdant/src/terrain.rs` — `GeoTransform`,
  `interpolate()`, `slope()`, `aspect()`, GeoTIFF tag parsing.
- `crates/verdant/src/grid.rs` — `SurfaceGrid::from_elevation`,
  `elevation_at` / `slope_at`.

## Primary sources

- **[H81]** Horn, B.K.P. (1981), "Hill shading and the reflectance map",
  Proc. IEEE 69(1), 14-47, doi:10.1109/PROC.1981.11918 — free author scan:
  https://people.csail.mit.edu/bkph/papers/Hill-Shading.pdf
  (IEEE copyright — not committed). The gradient formulas are **unnumbered**
  throughout the paper; they appear in Section IX "Gradient estimation"
  (journal p. 18).
- **[BM98]** Burrough, P.A., R.A. McDonnell (1998), "Principles of
  Geographical Information Systems", OUP — Ch. 8, pp. 190-191,
  Eqs. (8.7)-(8.10) — archive.org scan:
  https://archive.org/details/principles-of-geographical-information-systems
  (copyright — not committed).
- **[ESRI-s]/[ESRI-a]** ESRI, "How Slope works" / "How Aspect works",
  ArcGIS Pro Spatial Analyst tool reference —
  https://pro.arcgis.com/en/pro-app/latest/tool-reference/spatial-analyst/how-slope-works.htm
  and .../how-aspect-works.htm (free web docs).
- **[GDAL-dem]/[GDAL-rm]** GDAL, `gdaldem` documentation and Raster Data
  Model — https://gdal.org/programs/gdaldem.html and
  https://gdal.org/user/raster_data_model.html (free web docs).
- **[OGC]** Open Geospatial Consortium (2019), "OGC GeoTIFF Standard 1.1",
  OGC 19-008r4 — https://docs.ogc.org/is/19-008r4/19-008r4.html (free).
- **[NR92]** Press et al., Numerical Recipes in C, 2nd ed., CUP, Section
  3.6, Eqs. (3.6.4)-(3.6.5) — unofficial mirror
  https://300m.us/docs/computing/Numerical_Recipes/C/c3-6.pdf (copyright —
  not committed); Wikipedia's unit-square form carries the same formula:
  https://en.wikipedia.org/wiki/Bilinear_interpolation
- **[GITTA]** GITTA e-learning, "Terrain Analysis" §§1.2.1-1.2.2 —
  http://www.gitta.info/TerrainAnaly/en/text/TerrainAnaly.pdf (free).

## Equations and conventions

| Apogee code | Item | Source | Where | Verbatim (key text) | St. |
| --- | --- | --- | --- | --- | --- |
| terrain.rs:151-153, 179-181 | dz/dx 3x3 weighted stencil | H81 | Section IX, p. 18 (unnumbered) | "P_w = [(z_++ + 2 z_+o + z_+-) - (z_-+ + 2 z_o + z_--)]/8 dx — [weighted average of three central differences]" | OK |
| same | dz/dx stencil, textbook statement | BM98 | Ch. 8, Eq. (8.9), p. 190 | "[dz/dx] = [(z(i+1,j+1) + 2z(i+1,j) + z(i+1,j-1)) - (z(i-1,j+1) + 2z(i-1,j) + z(i-1,j-1))]/8 dx" | OK |
| same | dz/dx stencil (NoData-weighted form) | ESRI-s | unnumbered code block | "[dz/dx] = (( c + 2 f + i )*4/ wght1 - ( a + 2 d + g )*4/ wght2 ) / (8 * x_cellsize) [reduces to Horn's plain stencil when all 9 cells valid]" | OK |
| terrain.rs:154-156, 182-184 | dz/dy stencil (south - north) | H81 | Section IX, p. 18 | "Q_w = [(z_++ + 2 z_o+ + z_-+) - (z_+o + 2 z_o- + z_--)]/8 dy [south-to-north slope]" (code computes south - north = -q) | OK |
| same | dz/dy stencil, textbook + ESRI | BM98 / ESRI-s | Eq. (8.10), p. 190 / ESRI y-direction formula | "[dz/dy] = [(z(i+1,j+1) + 2z(i,j+1) + z(i-1,j+1)) - (z(i+1,j-1) + 2z(i,j-1) + z(i-1,j-1))]/8 dy" | OK |
| terrain.rs:158 | slope magnitude atan(sqrt(dzdx^2 + dzdy^2)) | ESRI-s + BM98 | ESRI "planar method"; BM98 Eq. (8.7) | "slope_radians = ATAN ( sqrt ([dz/dx]^2 + [dz/dy]^2) )" | OK |
| terrain.rs:190-196 | aspect atan2(dz_dx, -dz_dy) | ESRI-a + BM98 | ESRI aspect formula + conversion rule; BM98 Eq. (8.8) | "aspect = 57.29578 * atan2 ([dz/dy], -[dz/dx]) [then converted to compass directions]" — see Flags: the code's sign returns the **ascent** azimuth | OK |
| terrain.rs:166, 190 | azimuth semantics (0 = N, clockwise) | GDAL-dem | aspect section | "0 degrees means that the slope is facing the North, 90 degrees it's facing the East" | OK |
| terrain.rs:136-149 (algorithm choice) | Horn vs Zevenbergen-Thorne | GDAL-dem | -alg option | "The literature suggests Zevenbergen & Thorne to be more suited to smooth landscapes, whereas Horn's formula to perform better on rougher terrain." | OK |
| terrain.rs:97-134 `interpolate` | bilinear interpolation on the grid square | NR92 | Section 3.6, Eqs. (3.6.4)-(3.6.5), p. 123 | "The simplest interpolation in two dimensions is bilinear interpolation on the grid square ... y(x1,x2) = (1-t)(1-u)y1 + t(1-u)y2 + tu y3 + (1-t)u y4 (3.6.5)" | OK |
| terrain.rs:19-49 | affine geotransform GT(0)..GT(5), pixel centers at +0.5 | GDAL-rm | Affine Geotransform section | "Xgeo = GT(0) + Xpixel*GT(1) + Yline*GT(2) ... (GT(0),GT(3)) position is the top left corner of the top left pixel ... The pixel/line location of the center of the top left pixel would therefore be (0.5,0.5)." | OK |
| terrain.rs:294-315 | ModelTiepointTag (33922) semantics: 6 values/tiepoint | OGC | Section 7.3.1, Requirements 9.1 / 9.3 | "Requirement 9.1: The ModelTiepointTag SHALL have ID = 33922 ... Requirement 9.3: The ModelTiepointTag SHALL have 6 values for each of the tiepoints" | OK |
| terrain.rs:294-315 | tiepoint ordering (raster -> model) | OGC | Annex B.6, Table B.6.2 | "raster space coordinate (I,J,K) ... then model space coordinate (X,Y,Z)" | OK |
| terrain.rs:301-315 | ModelPixelScaleTag (33550) + negative y scale | OGC | Section 7.3.2, Requirements 10.1-10.5 | "Requirement 10.1: The ModelPixelScaleTag SHALL have ID = 33550 ... [ScaleY]: positive value => model Y decreases as raster J increases" (code: pixel_height = -ps[1]) | OK |
| terrain.rs:37-47 | raster-space origin, PixelIsArea default | OGC | Annex B.2.2 | "The 'PixelIsArea' raster grid space R, which is the default, uses coordinates I and J, with (0,0) denoting the upper-left corner of the image, and increasing I to the right, increasing J down." | OK |

## Flags

- **Aspect sign convention (verified numerically):** Horn's q is the
  south-to-north slope, but the code computes dz_dy = (south - north)/dy =
  -q (north-up raster). Therefore `dz_dx.atan2(-dz_dy)` returns the azimuth
  of the **gradient (steepest ascent)** — exactly 180 degrees from the
  ESRI/GDAL **downslope** aspect convention. The test
  `test_aspect_north_facing` (terrain.rs:433-449) confirms it: a ramp rising
  southward (downslope = north = 0) yields aspect = PI (south). The doc
  comments at :166/:190 ("radians, 0 = north, clockwise") do not disambiguate
  ascent vs descent. If downslope aspect is intended, use
  `dz_dx.atan2(dz_dy)` (and re-wrap).
- Horn (1981) has no numbered equations; all [H81] quotes are OCR of the
  author's scan with artifacts noted in-bracket in the audit record.
- ESRI publishes the stencils with NoData weights (`*4/wght`); the code
  implements Horn's plain stencil and propagates NaN when any neighbor is
  NaN — the cleaner equivalent.
- The dz_dx / dz_dy stencils are duplicated between `slope()` and
  `aspect()` (terrain.rs:151-156 vs 179-184) rather than shared.
- `pixel_height = -ps[1]` is consistent with OGC Req. 10.4 and GDAL's
  north-up GT(5) < 0; slope/aspect take `pixel_height.abs()` so the sign
  choice is invisible to them but matters for the bbox and
  `pixel_to_world`.