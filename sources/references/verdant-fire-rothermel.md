# Rothermel wildland fire spread + level-set front — primary sources

Rate of spread for wildland fuels: Rothermel (1972) reaction-intensity /
propagating-flux formulation with Anderson (1982) 13 fuel model classes,
propagated by a first-order Godunov level-set scheme, following the
WRF-SFIRE `module_fr_fire_phys.F` structure.

**Apogee code:** `crates/verdant/src/fire.rs` — `FuelModel::anderson`
parameter table, `RateOfSpread::compute`, `FireFront::step` (level set),
fuel burnout and heat fluxes.

## Primary sources

- **[R72]** Rothermel, R.C. (1972), "A mathematical model for predicting
  fire spread in wildland fuels", USDA Forest Service Intermountain Forest
  and Range Experiment Station, Research Paper INT-115, 40 pp. — free PDF:
  https://www.fs.usda.gov/rm/pubs_int/int_rp115.pdf
  Committed: `sources/pdfs/rothermel1972_int115.pdf`. Scanned typewriter
  document — quotes are OCR ("Summary of Basic Fire Spread Equations",
  printed p. 26 / PDF p. 33, cross-checked against the main-text pages).
- **[A82]** Anderson, H.E. (1982), "Aids to Determining Fuel Models for
  Estimating Fire Behavior", USDA INT-122, 22 pp. —
  https://www.fs.usda.gov/rm/pubs_int/int_gtr122.pdf
  Committed: `sources/pdfs/anderson1982_int122.pdf`.
- **[AL76]** Albini, F.A. (1976), "Estimating Wildfire Behavior and Effects",
  USDA INT-30, 92 pp. —
  https://fs.usda.gov/rm/pubs_series/int/gtr/int_gtr030.pdf
  Committed: `sources/pdfs/albini1976_int30.pdf`.
- **[OS88]** Osher, S., Sethian, J.A. (1988), "Fronts propagating with
  curvature-dependent speed: algorithms based on Hamilton-Jacobi
  formulations", J. Comput. Phys. 79, 12-49 — free author copy:
  https://math.berkeley.edu/~sethian/2006/Papers/sethian.osher.88.pdf
  (Elsevier copyright — not committed).
- **[F98]** Finney, M.A. (1998), "FARSITE: Fire Area Simulator — Model
  Development and Evaluation", USDA RMRS-RP-4 —
  https://fs.usda.gov/rm/pubs/rmrs_rp004.pdf
  Committed: `sources/pdfs/finney_farsite_rp4.pdf`.
- **[M18]** Muñoz-Esparza, D., B. Kosović, P.A. Jiménez, J.L. Coen (2018),
  "An Accurate Fire-Spread Algorithm in the Weather Research and Forecasting
  Model Using the Level-Set Method", JAMES 10, 908-926,
  doi:10.1002/2017MS001108 (open access) — quotes via AGU search snippet,
  not re-fetched; `UNV`.
- **[WRF]** WRF-SFIRE `module_fr_fire_phys.F` — the direct structural
  ancestor cited in the module header (Mandel et al. 2011, GMD 4:591-610;
  not fetched as a document).

## Equations and constants

| Apogee code | Item | Source | Where | Verbatim (key text) | St. |
| --- | --- | --- | --- | --- | --- |
| :386-387, 480-482 | rate of spread R | R72 | Eq. (52); summary p. 26; derivation end of p. 24 | "R = [xi I_R (1 + phi_w + phi_s)] / (rho_b eps Q_ig) — Rate of spread, ft./min. (52)" | OK |
| :372-376 `ir` | reaction intensity I_R | R72 | Eq. (27), summary p. 26 | "I_R = Gamma w_n h eta_M eta_S — Reaction intensity, B.t.u./ft.^2 min. (27)" | OK |
| :356-360 `gamma` | reaction velocity Gamma | R72 | Eq. (38), summary p. 26 | "Gamma' = Gamma_max (beta/beta_op)^A exp[A(1 - beta/beta_op)] (38)" | OK |
| :357 `gammax` | maximum reaction velocity | R72 | Eq. (36), summary p. 26 | "Gamma_max = sigma^1.5 (495 + 0.0594 sigma^1.5)^-1 — Maximum reaction velocity, min^-1 (36)" | OK |
| :337-338 `beta_op` | optimum packing ratio | R72 | Eq. (37), summary p. 26 | "beta_op = 3.348 sigma^-0.8189 — Optimum packing ratio (37)" | OK |
| :358 `a` | coefficient A | R72 | Eq. (39), summary p. 26 | "A = 1/(4.774 sigma^0.1 - 7.27) (39)" | OK |
| :365-367 `etam` | moisture damping | R72 | Eq. (29), summary p. 26 | "eta_M = 1 - 2.59(M_f/M_x) + 5.11(M_f/M_x)^2 - 3.52(M_f/M_x)^3 — Moisture damping coefficient (29)" | OK |
| :369-370 `etas` | mineral damping | R72 | Eq. (30), summary p. 26 | "eta_S = 0.174 S_e^-0.19 — Mineral damping coefficient (30)" | OK |
| :382-387 `xifr` | propagating flux ratio xi | R72 | Eq. (42), summary p. 26 | "xi = (192 + 0.2595 sigma)^-1 exp[(0.792 + 0.681 sigma^0.5)(beta + 0.1)] — Propagating flux ratio (42)" | OK |
| :349-354, 471 `phiwc`/`phiw` | wind coefficient phi_w and C/B/E | R72 | Eqs. (47)-(50), p. 23 (PDF p. 30) | "phi_w = C U^B (beta/beta_op)^-E (47); C = 7.47 exp(-0.133 sigma^0.55) (48); B = 0.02526 sigma^0.54 (49); E = 0.715 exp(-3.59 x 10^-4 sigma) (50)" | OK |
| :474-478 `phis` | slope factor | R72 | Eq. (51), p. 24 (PDF p. 31) | "phi_s = 5.275 beta^-0.3 (tan phi)^2 — Slope factor (51)" | OK |
| :340-341 `qig` | heat of preignition Q_ig | R72 | Eq. (12), summary p. 26; per-size-class (Eq. 78, PDF p. 39) | "Q_ig = 250 + 1,116 M_f — Heat of preignition, B.t.u./lb. (12)" | OK |
| :343-344 `epsilon` | effective heating number | R72 | Eq. (14), summary p. 26 | "eps = exp(-138/sigma) — Effective heating number (14)" | OK |
| :346-347 `rhob` | ovendry bulk density rho_b | R72 | Eq. (40), summary p. 26 | "rho_b = w_o/delta — Ovendry bulk density, lb./ft.^3 (40)" | OK |
| :362-363 `wn` | net fuel loading w_n | R72 | Eq. (24), summary p. 26 | "w_n = w_o/(1 + S_T) — Net fuel loading, lb./ft.^2 (24)" | OK |
| :334-335 `beta` | packing ratio | R72 | Eq. (3), summary p. 26 | "beta = rho_b/rho_p — Packing ratio (3)" | OK |
| :100-269 `anderson()` | 13 fuel model class parameters (loads, depths, moistures of extinction) | A82 | Table 1, p. 2 (PDF p. 5) | "Table 1. Description of fuel models used in fire behavior as documented by Albini (1976)"; e.g. row 1 "Short grass (1 foot) 0.74 ... 1.0 12"; row 4 "Chaparral (6 feet) 5.01 4.01 2.00 5.01 6.0 20" (all 13 models verified numerically: tons/acre -> kg/m^2 max deviation 0.11%) | OK |
| :100-269 `savr` values | characteristic surface-area-to-volume ratios (3500, 2784, ...) | AL76 | Table 7 (SAV values area-weighted per class; not printed in A82) | verified numerically to within 0.03% for all 13 models against AL76 per-class SAV/load data | OK |
| :378-380 `iboros` | I_R to kJ/m^2/min via 60*12.6/savr residence time | F98 | Eq. [20], p. 15 of RMRS-RP-4 | "60*12.6/savr — flaming residence time (minutes)"; "I_R = ... [from Rothermel 1972] ... multiplied by the flaming residence time" | OK |
| :664-783 `FireFront::step` | first-order Godunov level-set propagation d(lfn)/dt + ros*\|grad lfn\| = 0; one-sided differences; max(D-,0)^2 + min(D+,0)^2 Hamiltonian | OS88 | Eqs. (3.7), (3.9)-(3.10) (the upwind HJ-Godunov flux); algorithm Section 3 | "fronts propagating with curvature-dependent speed: algorithms based on Hamilton-Jacobi formulations" (title/abstract; the code implements the F > 0, convex/concave case of their scheme with lfn < 0 = burnt instead of phi > 1) | OK |
| :664-667 (doc) | accuracy caveat: plain first-order scheme carries 10-35% ROS error at 12.5-100 m grids | M18 | Abstract | "It is demonstrated that the common implementation used by level-set-based wildfire models yields to rate-of-spread errors in the range 10-35% for typical grid sizes (Delta = 12.5-100 m)" | UNV |

## Flags

- **Constants not from [R72]/[A82]:** `CMB_CNST` = 17.433e6 J/kg heat of
  combustion ([R72] treats h as an input, its list p. 27/PDF p. 34),
  `fuel_density` 32 lb/ft^3, `st` 0.0555, `se` 0.010, `weight`,
  `fuel_time` = weight/0.85, `HF_GL`, the 6 m/s `ROS_MAX` cap, the 30 m/s
  wind cap, the chaparral wind-only formula (1.2974 spd^1.41 with
  0.03333 m/s backing), the exponential fuel burnout model, `xlv` = 2.5e6
  J/kg and the 0.56 water fraction are WRF-SFIRE
  `module_fr_fire_phys.F` implementation constants — their primary
  provenance (Mandel et al. 2011; WRF source) was not fetched as a document.
- The code computes Rothermel's equations internally in English units
  (kg/m^2 -> lb/ft^2 via 0.3048^2 x 2.205; m/s wind -> ft/min via 196.850;
  ft/min -> m/s via 0.00508), consistent with the 250/1116 Btu-lb constants.
- Single-particle form: one characteristic `savr` and `beta` per cell —
  Rothermel's extended mixed-fuel summation over size classes (Eqs. 68-78,
  printed p. 32 / PDF p. 39) is not implemented.
- Level-set sign convention: OS88 evolve phi_t - F(K)\|grad phi\| = 0 with
  phi > 1 in burnt gas; fire.rs (following WRF-SFIRE) uses lfn < 0 burnt and
  updates lfn -= ros\|grad lfn\| dt with ros >= 0 — same flux family,
  flipped sign, no reinitialization PDE.