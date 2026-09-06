//! Fire spread model based on Rothermel rate-of-spread and level-set
//! front propagation.
//!
//! Implements the Anderson 13 fuel model and the Rothermel (1972) fire
//! spread equation as used in WRF-SFIRE (module_fr_fire_phys.F).
//! The level-set method propagates the fire front: lfn < 0 means burned,
//! lfn = 0 is the fire line, lfn > 0 is unburned.
//!
//! References:
//! - Rothermel, R.C. (1972). "A mathematical model for predicting fire
//!   spread in wildland fuels." USDA Forest Service Research Paper INT-115.
//! - Anderson, H.E. (1982). "Aids to determining fuel models for fire
//!   behavior prediction." USDA Forest Service General Technical Report INT-122.
//! - Mandel, J., Beezley, J.D., Coen, J.L., Kim, M. (2011). "Data assimilation
//!   for wildland fires." IEEE Computing in Science & Engineering.
//! - Munoz-Esparza, D., Kosovic, B., Jimenez, P., Coen, J. (2018).
//!   "An accurate fire-spread algorithm in WRF using the level-set method."
//!   JAMES 10(5). https://doi.org/10.1002/2017MS001108

use crate::terrain::ElevationGrid;

/// Anderson 13 fire behavior fuel models.
///
/// Category 14 is "no fuel" (unburnable).
/// Categories 1-13 correspond to Anderson (1982) Table 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FuelCategory {
    ShortGrass = 1,
    TimberGrass = 2,
    TallGrass = 3,
    Chaparral = 4,
    Brush = 5,
    DormantBrush = 6,
    SouthernRough = 7,
    ClosedTimberLitter = 8,
    HardwoodLitter = 9,
    TimberLitter = 10,
    LightSlash = 11,
    MediumSlash = 12,
    HeavySlash = 13,
    NoFuel = 14,
}

impl FuelCategory {
    pub fn from_u8(n: u8) -> Self {
        match n {
            1 => Self::ShortGrass,
            2 => Self::TimberGrass,
            3 => Self::TallGrass,
            4 => Self::Chaparral,
            5 => Self::Brush,
            6 => Self::DormantBrush,
            7 => Self::SouthernRough,
            8 => Self::ClosedTimberLitter,
            9 => Self::HardwoodLitter,
            10 => Self::TimberLitter,
            11 => Self::LightSlash,
            12 => Self::MediumSlash,
            13 => Self::HeavySlash,
            _ => Self::NoFuel,
        }
    }
}

/// Fuel model properties for a single Anderson category.
///
/// Values from WRF-SFIRE module_fr_fire_phys.F DATA statements,
/// originally from Anderson (1982).
#[derive(Debug, Clone, Copy)]
pub struct FuelModel {
    pub category: FuelCategory,
    /// Total fuel load (kg/m^2)
    pub fgi: f32,
    /// Fuel depth (m)
    pub fuel_depth: f32,
    /// Surface-area-to-volume ratio (1/m)
    pub savr: f32,
    /// Moisture of extinction (fraction)
    pub moisture_extinction: f32,
    /// Fuel density (lb/ft^3)
    pub fuel_density: f32,
    /// Total mineral content (fraction)
    pub st: f32,
    /// Effective mineral content (fraction)
    pub se: f32,
    /// Fuel loading weight for time constant
    pub weight: f32,
    /// Is chaparral (different spread formula)
    pub is_chaparral: bool,
}

impl FuelModel {
    /// Heat of combustion (J/kg dry fuel)
    pub const CMB_CNST: f32 = 17.433e6;
    /// Heat flux from burning live fuel (W/m^2)
    pub const HF_GL: f32 = 17.0e4;

    /// Get fuel model for an Anderson category (1-14).
    pub fn anderson(cat: FuelCategory) -> Self {
        match cat {
            FuelCategory::ShortGrass => Self {
                category: cat,
                fgi: 0.166,
                fuel_depth: 0.305,
                savr: 3500.0,
                moisture_extinction: 0.12,
                fuel_density: 32.0,
                st: 0.0555,
                se: 0.010,
                weight: 7.0,
                is_chaparral: false,
            },
            FuelCategory::TimberGrass => Self {
                category: cat,
                fgi: 0.896,
                fuel_depth: 0.305,
                savr: 2784.0,
                moisture_extinction: 0.15,
                fuel_density: 32.0,
                st: 0.0555,
                se: 0.010,
                weight: 7.0,
                is_chaparral: false,
            },
            FuelCategory::TallGrass => Self {
                category: cat,
                fgi: 0.674,
                fuel_depth: 0.762,
                savr: 1500.0,
                moisture_extinction: 0.25,
                fuel_density: 32.0,
                st: 0.0555,
                se: 0.010,
                weight: 7.0,
                is_chaparral: false,
            },
            FuelCategory::Chaparral => Self {
                category: cat,
                fgi: 3.591,
                fuel_depth: 1.829,
                savr: 1739.0,
                moisture_extinction: 0.20,
                fuel_density: 32.0,
                st: 0.0555,
                se: 0.010,
                weight: 180.0,
                is_chaparral: true,
            },
            FuelCategory::Brush => Self {
                category: cat,
                fgi: 0.784,
                fuel_depth: 0.61,
                savr: 1683.0,
                moisture_extinction: 0.20,
                fuel_density: 32.0,
                st: 0.0555,
                se: 0.010,
                weight: 100.0,
                is_chaparral: false,
            },
            FuelCategory::DormantBrush => Self {
                category: cat,
                fgi: 1.344,
                fuel_depth: 0.762,
                savr: 1564.0,
                moisture_extinction: 0.25,
                fuel_density: 32.0,
                st: 0.0555,
                se: 0.010,
                weight: 100.0,
                is_chaparral: false,
            },
            FuelCategory::SouthernRough => Self {
                category: cat,
                fgi: 1.091,
                fuel_depth: 0.762,
                savr: 1562.0,
                moisture_extinction: 0.40,
                fuel_density: 32.0,
                st: 0.0555,
                se: 0.010,
                weight: 100.0,
                is_chaparral: false,
            },
            FuelCategory::ClosedTimberLitter => Self {
                category: cat,
                fgi: 1.120,
                fuel_depth: 0.0610,
                savr: 1889.0,
                moisture_extinction: 0.30,
                fuel_density: 32.0,
                st: 0.0555,
                se: 0.010,
                weight: 900.0,
                is_chaparral: false,
            },
            FuelCategory::HardwoodLitter => Self {
                category: cat,
                fgi: 0.780,
                fuel_depth: 0.0610,
                savr: 2484.0,
                moisture_extinction: 0.25,
                fuel_density: 32.0,
                st: 0.0555,
                se: 0.010,
                weight: 900.0,
                is_chaparral: false,
            },
            FuelCategory::TimberLitter => Self {
                category: cat,
                fgi: 2.692,
                fuel_depth: 0.305,
                savr: 1764.0,
                moisture_extinction: 0.25,
                fuel_density: 32.0,
                st: 0.0555,
                se: 0.010,
                weight: 900.0,
                is_chaparral: false,
            },
            FuelCategory::LightSlash => Self {
                category: cat,
                fgi: 2.582,
                fuel_depth: 0.305,
                savr: 1182.0,
                moisture_extinction: 0.15,
                fuel_density: 32.0,
                st: 0.0555,
                se: 0.010,
                weight: 900.0,
                is_chaparral: false,
            },
            FuelCategory::MediumSlash => Self {
                category: cat,
                fgi: 7.749,
                fuel_depth: 0.701,
                savr: 1145.0,
                moisture_extinction: 0.20,
                fuel_density: 32.0,
                st: 0.0555,
                se: 0.010,
                weight: 900.0,
                is_chaparral: false,
            },
            FuelCategory::HeavySlash => Self {
                category: cat,
                fgi: 13.024,
                fuel_depth: 0.914,
                savr: 1159.0,
                moisture_extinction: 0.25,
                fuel_density: 32.0,
                st: 0.0555,
                se: 0.010,
                weight: 900.0,
                is_chaparral: false,
            },
            FuelCategory::NoFuel => Self {
                category: cat,
                fgi: 1.0e-7,
                fuel_depth: 0.305,
                savr: 3500.0,
                moisture_extinction: 0.12,
                fuel_density: 32.0,
                st: 0.0555,
                se: 0.010,
                weight: 7.0,
                is_chaparral: false,
            },
        }
    }

    /// Time constant for fuel burnout: weight / 0.85 (seconds).
    pub fn fuel_time(&self) -> f32 {
        self.weight / 0.85
    }
}

/// Pre-computed Rothermel spread coefficients for a fuel model + moisture.
///
/// These are computed once per cell from the fuel model and fuel moisture,
/// then used in the rate-of-spread calculation each timestep.
#[derive(Debug, Clone)]
pub struct SpreadCoeffs {
    /// Base rate of spread on flat ground with no wind (m/s)
    pub r0: f32,
    /// Wind coefficient exponent (bbb in WRF)
    pub bbb: f32,
    /// Wind coefficient (phiwc in WRF)
    pub phiwc: f32,
    /// Packing ratio (betafl in WRF)
    pub beta: f32,
    /// Initial fuel mass (kg/m^2)
    pub fgip: f32,
    /// Is chaparral
    pub is_chaparral: bool,
    /// Fire intensity × residence time (kJ/m^2)
    pub iboros: f32,
    /// Fuel time constant (s)
    pub fuel_time: f32,
}

impl SpreadCoeffs {
    /// Compute Rothermel spread coefficients from a fuel model and fuel moisture.
    ///
    /// Implements the `set_fire_params` equations from WRF-SFIRE
    /// module_fr_fire_phys.F. Units are converted from the Fortran
    /// imperial-unit intermediate calculations to SI.
    ///
    /// Reference: Rothermel (1972), equations 1-56.
    pub fn compute(model: &FuelModel, fuel_moisture: f32) -> Self {
        if model.category == FuelCategory::NoFuel {
            return Self {
                r0: 0.0,
                bbb: 1.0,
                phiwc: 0.0,
                beta: 1.0,
                fgip: 0.0,
                is_chaparral: false,
                iboros: 0.0,
                fuel_time: model.fuel_time(),
            };
        }

        // Moisture fraction on dry mass basis
        let bmst = fuel_moisture / (1.0 + fuel_moisture);
        let fuel_load_dry = (1.0 - bmst) * model.fgi;

        // Convert to imperial units for Rothermel equations
        // kg/m^2 → lb/ft^2: × 0.3048^2 × 2.205
        let fuel_load_lb_ft2 = fuel_load_dry * 0.3048_f32.powi(2) * 2.205;
        let fuel_depth_ft = model.fuel_depth / 0.3048;

        // Packing ratio
        let beta = fuel_load_lb_ft2 / (fuel_depth_ft * model.fuel_density);

        // Optimum packing ratio
        let beta_op = 3.348 * model.savr.powf(-0.8189);

        // Heat of preignition (btu/lb)
        let qig = 250.0 + 1116.0 * fuel_moisture;

        // Effective heating number
        let epsilon = (-138.0 / model.savr).exp();

        // Oven-dry bulk density (lb/ft^3)
        let rhob = fuel_load_lb_ft2 / fuel_depth_ft;

        // Wind coefficient constants
        let c = 7.47 * (-0.133 * model.savr.powf(0.55)).exp();
        let bbb = 0.02526 * model.savr.powf(0.54);
        let e = 0.715 * (-3.59e-4 * model.savr).exp();

        let phiwc = c * (beta / beta_op).powf(-e);

        // Reaction velocity
        let gammax = model.savr.powf(1.5) / (495.0 + 0.0594 * model.savr.powf(1.5));
        let a = 1.0 / (4.774 * model.savr.powf(0.1) - 7.27);
        let ratio = beta / beta_op;
        let gamma = gammax * ratio.powf(a) * (a * (1.0 - ratio)).exp();

        // Net fuel loading (lb/ft^2)
        let wn = fuel_load_lb_ft2 / (1.0 + model.st);

        // Moisture damping coefficient
        let rtemp1 = fuel_moisture / model.moisture_extinction;
        let etam = 1.0 - 2.59 * rtemp1 + 5.11 * rtemp1.powi(2) - 3.52 * rtemp1.powi(3);

        // Mineral damping coefficient
        let etas = 0.174 * model.se.powf(-0.19);

        // Fuel heat (btu/lb) — cmbcnst converted: J/kg × 4.30e-4
        let fuel_heat = FuelModel::CMB_CNST * 4.30e-4;

        // Reaction intensity (btu/ft^2 min)
        let ir = gamma * wn * fuel_heat * etam * etas;

        // Fire intensity × residence time (kJ/m^2)
        let iboros =
            ir * 1055.0 / (0.3048_f32.powi(2) * 60.0) * 1.0e-3 * (60.0 * 12.6 / model.savr);

        // Propagating flux ratio
        let xifr = ((0.792 + 0.681 * model.savr.powf(0.5)) * (beta + 0.1)).exp()
            / (192.0 + 0.2595 * model.savr);

        // Base spread rate (ft/min)
        let r0_ft_min = ir * xifr / (rhob * epsilon * qig);

        // Convert to m/s: ft/min × 0.00508
        let r0 = r0_ft_min * 0.00508;

        Self {
            r0,
            bbb,
            phiwc,
            beta,
            fgip: model.fgi,
            is_chaparral: model.is_chaparral,
            iboros,
            fuel_time: model.fuel_time(),
        }
    }
}

/// Rate of spread in a direction, decomposed into base, wind, and slope contributions.
///
/// Implements the `fire_ros` subroutine from WRF-SFIRE module_fr_fire_phys.F.
/// Returns (ros_base, ros_wind, ros_slope) in m/s. Total spread rate is the sum.
#[derive(Debug, Clone, Copy)]
pub struct RateOfSpread {
    pub base: f32,
    pub wind: f32,
    pub slope: f32,
}

impl RateOfSpread {
    /// Total rate of spread (m/s).
    pub fn total(&self) -> f32 {
        self.base + self.wind + self.slope
    }

    /// Compute rate of spread from spread coefficients, wind, and slope.
    ///
    /// Arguments:
    /// - `coeffs`: pre-computed Rothermel coefficients for this cell
    /// - `wind_u`, `wind_v`: wind velocity components (m/s)
    /// - `dz_dx`, `dz_dy`: terrain gradient (rise/run, dimensionless)
    /// - `normal_x`, `normal_y`: fire front normal direction (unit vector)
    pub fn compute(
        coeffs: &SpreadCoeffs,
        wind_u: f32,
        wind_v: f32,
        dz_dx: f32,
        dz_dy: f32,
        normal_x: f32,
        normal_y: f32,
    ) -> Self {
        const ROS_MAX: f32 = 6.0; // m/s cap

        if coeffs.r0 == 0.0 {
            return Self {
                base: 0.0,
                wind: 0.0,
                slope: 0.0,
            };
        }

        // Wind speed in spread direction
        let speed = wind_u * normal_x + wind_v * normal_y;
        // Slope in spread direction
        let tan_phi = dz_dx * normal_x + dz_dy * normal_y;

        if coeffs.is_chaparral {
            // Chaparral: spread rate depends only on wind speed
            let spd = speed.max(0.0);
            let ros_back = 0.03333; // backing spread rate for chaparral
            let wind = (1.2974 * spd.powf(1.41)).max(ros_back);
            return Self {
                base: 0.0,
                wind,
                slope: 0.0,
            };
        }

        // Rothermel formula
        let spd = speed.max(0.0);
        let umidm = spd.min(30.0); // cap wind at 30 m/s
        let umid = umidm * 196.850; // m/s → ft/min

        // Wind factor
        let phiw = umid.powf(coeffs.bbb) * coeffs.phiwc;

        // Slope factor
        let phis = if tan_phi > 0.0 {
            5.275 * coeffs.beta.powf(-0.3) * tan_phi.powi(2)
        } else {
            0.0
        };

        let ros_base = coeffs.r0;
        let ros_wind = ros_base * phiw;
        let ros_slope = ros_base * phis;

        // Cap total spread rate at ROS_MAX
        let excess = ros_base + ros_wind + ros_slope - ROS_MAX;
        if excess > 0.0 && (ros_wind + ros_slope) > 0.0 {
            let ros_wind = ros_wind - excess * ros_wind / (ros_wind + ros_slope);
            let ros_slope = ros_slope - excess * ros_slope / (ros_wind + ros_slope);
            Self {
                base: ros_base,
                wind: ros_wind,
                slope: ros_slope,
            }
        } else {
            Self {
                base: ros_base,
                wind: ros_wind,
                slope: ros_slope,
            }
        }
    }
}

/// Fire front state on a regular grid.
///
/// Uses the level-set method: lfn < 0 = burned, lfn = 0 = fire line,
/// lfn > 0 = unburned. Ignition time tign records when each cell caught.
#[derive(Debug, Clone)]
pub struct FireFront {
    pub width: usize,
    pub height: usize,
    /// Cell spacing (m)
    pub dx: f32,
    pub dy: f32,
    /// Level-set function (negative = burned)
    pub lfn: Vec<f32>,
    /// Ignition time per cell (s, NaN = not ignited)
    pub tign: Vec<f32>,
    /// Remaining fuel fraction (0-1)
    pub fuel_frac: Vec<f32>,
    /// Fuel moisture content (fraction)
    pub fuel_moisture: Vec<f32>,
    /// Anderson fuel category per cell (1-14)
    pub fuel_category: Vec<u8>,
    /// Pre-computed spread coefficients per cell
    pub coeffs: Vec<SpreadCoeffs>,
    /// Current simulation time (s)
    pub time: f32,
    /// Sensible heat flux per cell (W/m^2)
    pub heat_flux: Vec<f32>,
    /// Latent heat flux per cell (W/m^2)
    pub moisture_flux: Vec<f32>,
}

impl FireFront {
    /// Create a new fire front with uniform fuel, no ignition.
    pub fn new(
        width: usize,
        height: usize,
        dx: f32,
        dy: f32,
        fuel_cat: u8,
        fuel_moisture: f32,
    ) -> Self {
        let n = width * height;
        let model = FuelModel::anderson(FuelCategory::from_u8(fuel_cat));
        let coeffs = SpreadCoeffs::compute(&model, fuel_moisture);

        Self {
            width,
            height,
            dx,
            dy,
            lfn: vec![f32::MAX; n],
            tign: vec![f32::NAN; n],
            fuel_frac: vec![1.0; n],
            fuel_moisture: vec![fuel_moisture; n],
            fuel_category: vec![fuel_cat; n],
            coeffs: vec![coeffs; n],
            time: 0.0,
            heat_flux: vec![0.0; n],
            moisture_flux: vec![0.0; n],
        }
    }

    /// Set per-cell fuel categories from an elevation grid + fuel map.
    pub fn set_fuel_from_grid(&mut self, fuel_cats: &[u8], moisture: &[f32]) {
        for i in 0..self.width * self.height {
            if i < fuel_cats.len() {
                self.fuel_category[i] = fuel_cats[i];
            }
            if i < moisture.len() {
                self.fuel_moisture[i] = moisture[i];
            }
            let model = FuelModel::anderson(FuelCategory::from_u8(self.fuel_category[i]));
            self.coeffs[i] = SpreadCoeffs::compute(&model, self.fuel_moisture[i]);
        }
    }

    /// Set terrain gradients from an elevation grid.
    pub fn set_terrain(&mut self, elevation: &ElevationGrid) {
        // Terrain gradients are used in rate-of-spread via slope factor.
        // We store them in the coeffs? No — they change with the normal
        // direction. We compute dz/dx, dz/dy on the fly during spread.
        // This method is a placeholder for when we couple to terrain.
        // The actual gradient computation happens in step().
        let _ = elevation;
    }

    /// Ignite a point at (col, row). Sets lfn negative and tign to current time.
    /// Also initializes lfn for neighboring cells as signed distance.
    pub fn ignite_point(&mut self, col: usize, row: usize) {
        if col < self.width && row < self.height {
            let idx = row * self.width + col;
            self.lfn[idx] = -self.dx;
            self.tign[idx] = self.time;

            // Initialize neighbors to signed distance so the gradient is well-formed
            let init_radius = 3.0 * self.dx.max(self.dy);
            for dr in -3..=3i32 {
                for dc in -3..=3i32 {
                    let r = row as i32 + dr;
                    let c = col as i32 + dc;
                    if r < 0 || r >= self.height as i32 || c < 0 || c >= self.width as i32 {
                        continue;
                    }
                    let i = r as usize * self.width + c as usize;
                    if i == idx {
                        continue;
                    }
                    let dx_m = dc as f32 * self.dx;
                    let dy_m = dr as f32 * self.dy;
                    let dist = (dx_m * dx_m + dy_m * dy_m).sqrt();
                    if dist <= init_radius {
                        self.lfn[i] = dist - self.dx; // signed distance from front
                    }
                }
            }
        }
    }

    /// Ignite a circle at world position (x, y) with radius r (meters).
    /// Initializes lfn as signed distance from the circle boundary.
    pub fn ignite_circle(&mut self, cx: f32, cy: f32, radius: f32) {
        let init_band = 3.0 * self.dx.max(self.dy);
        for row in 0..self.height {
            for col in 0..self.width {
                let x = col as f32 * self.dx;
                let y = row as f32 * self.dy;
                let dx = x - cx;
                let dy = y - cy;
                let dist = (dx * dx + dy * dy).sqrt();
                let signed = dist - radius;
                let idx = row * self.width + col;

                if signed <= 0.0 {
                    self.lfn[idx] = signed.min(-self.dx * 0.5);
                    self.tign[idx] = self.time;
                } else if signed <= init_band {
                    self.lfn[idx] = signed;
                }
            }
        }
    }

    /// Check if a cell is currently burning (ignited but fuel remaining).
    pub fn is_burning(&self, col: usize, row: usize) -> bool {
        if col >= self.width || row >= self.height {
            return false;
        }
        let idx = row * self.width + col;
        self.lfn[idx] <= 0.0 && self.fuel_frac[idx] > 0.0
    }

    /// Check if a cell has been consumed (burned out).
    pub fn is_burned(&self, col: usize, row: usize) -> bool {
        if col >= self.width || row >= self.height {
            return false;
        }
        let idx = row * self.width + col;
        self.lfn[idx] <= 0.0 && self.fuel_frac[idx] <= 0.0
    }

    /// Advance the fire front by one timestep.
    ///
    /// Implements first-order Godunov level-set propagation:
    /// ∂lfn/∂t + ros * |∇lfn| = 0
    ///
    /// Uses upwind differences for the gradient. The Godunov Hamiltonian
    /// selects the correct upwind direction:
    ///   |∇lfn| ≈ max(D⁻_x, 0)² + min(D⁺_x, 0)² + max(D⁻_y, 0)² + min(D⁺_y, 0)²
    /// where D⁻/D⁺ are backward/forward differences.
    ///
    /// After propagation, fuel consumption and heat fluxes are updated.
    pub fn step(&mut self, dt: f32, wind_u: &[f32], wind_v: &[f32], dz_dx: &[f32], dz_dy: &[f32]) {
        let w = self.width;
        let h = self.height;
        let n = w * h;

        // 1. Propagate level-set function using Godunov upwind scheme
        let mut new_lfn = self.lfn.clone();

        for row in 1..h - 1 {
            for col in 1..w - 1 {
                let idx = row * w + col;
                let center = self.lfn[idx];

                // Skip cells far from the front
                if center > 5.0 * self.dx.max(self.dy) {
                    // Check if any neighbor is near the front
                    let neighbors = [
                        self.lfn[idx - 1],
                        self.lfn[idx + 1],
                        self.lfn[(row - 1) * w + col],
                        self.lfn[(row + 1) * w + col],
                    ];
                    if neighbors.iter().all(|&n| n > 5.0 * self.dx.max(self.dy)) {
                        continue;
                    }
                }

                // Backward and forward differences
                let dmx = (center - self.lfn[idx - 1]) / self.dx;
                let dpx = (self.lfn[idx + 1] - center) / self.dx;
                let dmy = (center - self.lfn[(row - 1) * w + col]) / self.dy;
                let dpy = (self.lfn[(row + 1) * w + col] - center) / self.dy;

                // Handle MAX values: zero out derivatives involving MAX cells
                let center_max = center >= f32::MAX * 0.5;
                let dmx = if center_max || self.lfn[idx - 1] >= f32::MAX * 0.5 {
                    0.0
                } else {
                    dmx
                };
                let dpx = if center_max || self.lfn[idx + 1] >= f32::MAX * 0.5 {
                    0.0
                } else {
                    dpx
                };
                let dmy = if center_max || self.lfn[(row - 1) * w + col] >= f32::MAX * 0.5 {
                    0.0
                } else {
                    dmy
                };
                let dpy = if center_max || self.lfn[(row + 1) * w + col] >= f32::MAX * 0.5 {
                    0.0
                } else {
                    dpy
                };

                // Godunov Hamiltonian: |∇lfn|² = max(D⁻,0)² + min(D⁺,0)² per axis
                let grad_sq = dmx.max(0.0).powi(2)
                    + dpx.min(0.0).powi(2)
                    + dmy.max(0.0).powi(2)
                    + dpy.min(0.0).powi(2);
                let grad_mag = grad_sq.sqrt();

                if grad_mag < 1e-10 {
                    continue;
                }

                // Normal direction (outward from burned region)
                let nx = (dmx.max(0.0) + dpx.min(0.0)) / grad_mag;
                let ny = (dmy.max(0.0) + dpy.min(0.0)) / grad_mag;

                // Rate of spread in normal direction
                let ros = RateOfSpread::compute(
                    &self.coeffs[idx],
                    wind_u[idx],
                    wind_v[idx],
                    dz_dx[idx],
                    dz_dy[idx],
                    nx,
                    ny,
                );

                let ros_total = ros.total();
                if ros_total <= 0.0 {
                    continue;
                }

                // Level-set update: lfn decreases (front advances)
                new_lfn[idx] = center - ros_total * grad_mag * dt;

                // If this cell just crossed zero, record ignition time
                if center > 0.0 && new_lfn[idx] <= 0.0 {
                    let frac = center / (center - new_lfn[idx]);
                    self.tign[idx] = self.time + frac * dt;
                }
            }
        }

        // Copy boundary cells
        for col in 0..w {
            new_lfn[col] = self.lfn[col];
            new_lfn[(h - 1) * w + col] = self.lfn[(h - 1) * w + col];
        }
        for row in 0..h {
            new_lfn[row * w] = self.lfn[row * w];
            new_lfn[row * w + w - 1] = self.lfn[row * w + w - 1];
        }

        self.lfn = new_lfn;

        // 2. Update fuel consumption and heat fluxes
        for i in 0..n {
            if self.lfn[i] <= 0.0 && !self.tign[i].is_nan() {
                // Time since ignition
                let dt_burn = self.time + dt - self.tign[i];
                if dt_burn > 0.0 {
                    // Exponential fuel decay: f = exp(-t / fuel_time)
                    let burn_frac = 1.0 - (-dt_burn / self.coeffs[i].fuel_time).exp();
                    let prev_frac = self.fuel_frac[i];
                    self.fuel_frac[i] = 1.0 - burn_frac;
                    self.fuel_frac[i] = self.fuel_frac[i].max(0.0);

                    // Fuel burned this step (kg/m^2)
                    let dmass = self.coeffs[i].fgip * (prev_frac - self.fuel_frac[i]);

                    // Heat fluxes (W/m^2)
                    let bmst = self.fuel_moisture[i] / (1.0 + self.fuel_moisture[i]);
                    self.heat_flux[i] = (dmass / dt) * (1.0 - bmst) * FuelModel::CMB_CNST;
                    // Latent heat: xlv ≈ 2.5e6 J/kg, 56% of cellulose is water
                    let xlv = 2.5e6_f32;
                    self.moisture_flux[i] = (bmst + (1.0 - bmst) * 0.56) * (dmass / dt) * xlv;
                }
            } else {
                self.heat_flux[i] = 0.0;
                self.moisture_flux[i] = 0.0;
            }
        }

        self.time += dt;
    }

    /// Count of burning cells.
    pub fn burning_count(&self) -> usize {
        (0..self.width * self.height)
            .filter(|&i| self.lfn[i] <= 0.0 && self.fuel_frac[i] > 0.0)
            .count()
    }

    /// Count of burned cells (fully consumed).
    pub fn burned_count(&self) -> usize {
        (0..self.width * self.height)
            .filter(|&i| self.lfn[i] <= 0.0 && self.fuel_frac[i] <= 0.0)
            .count()
    }

    /// Total area burned (m^2).
    pub fn burned_area(&self) -> f32 {
        let cell_area = self.dx * self.dy;
        (0..self.width * self.height)
            .filter(|&i| self.lfn[i] <= 0.0)
            .count() as f32
            * cell_area
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuel_model_anderson_categories() {
        let m1 = FuelModel::anderson(FuelCategory::ShortGrass);
        assert!((m1.fgi - 0.166).abs() < 0.001);
        assert!((m1.fuel_depth - 0.305).abs() < 0.001);
        assert!(!m1.is_chaparral);

        let m4 = FuelModel::anderson(FuelCategory::Chaparral);
        assert!(m4.is_chaparral);
        assert!((m4.fgi - 3.591).abs() < 0.001);

        let m14 = FuelModel::anderson(FuelCategory::NoFuel);
        assert!(m14.fgi < 1e-5);
    }

    #[test]
    fn test_spread_coeffs_no_fuel() {
        let model = FuelModel::anderson(FuelCategory::NoFuel);
        let coeffs = SpreadCoeffs::compute(&model, 0.08);
        assert_eq!(coeffs.r0, 0.0);
        assert_eq!(coeffs.fgip, 0.0);
    }

    #[test]
    fn test_spread_coeffs_short_grass() {
        let model = FuelModel::anderson(FuelCategory::ShortGrass);
        let coeffs = SpreadCoeffs::compute(&model, 0.08);
        // Base spread rate should be positive and reasonable (0.01-1 m/s)
        assert!(coeffs.r0 > 0.0);
        assert!(coeffs.r0 < 10.0);
        assert!(coeffs.fgip > 0.0);
    }

    #[test]
    fn test_rate_of_spread_no_wind_no_slope() {
        let model = FuelModel::anderson(FuelCategory::ShortGrass);
        let coeffs = SpreadCoeffs::compute(&model, 0.08);
        let ros = RateOfSpread::compute(&coeffs, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0);
        // With no wind and no slope, total = base rate only
        assert!((ros.total() - coeffs.r0).abs() < 0.001);
    }

    #[test]
    fn test_rate_of_spread_with_wind() {
        let model = FuelModel::anderson(FuelCategory::ShortGrass);
        let coeffs = SpreadCoeffs::compute(&model, 0.08);
        let ros_no_wind = RateOfSpread::compute(&coeffs, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0);
        let ros_with_wind = RateOfSpread::compute(&coeffs, 5.0, 0.0, 0.0, 0.0, 1.0, 0.0);
        // Wind should increase spread rate
        assert!(ros_with_wind.total() > ros_no_wind.total());
    }

    #[test]
    fn test_rate_of_spread_with_slope() {
        let model = FuelModel::anderson(FuelCategory::ShortGrass);
        let coeffs = SpreadCoeffs::compute(&model, 0.08);
        let ros_flat = RateOfSpread::compute(&coeffs, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0);
        let ros_slope = RateOfSpread::compute(&coeffs, 0.0, 0.0, 0.5, 0.0, 1.0, 0.0);
        // Uphill slope should increase spread rate
        assert!(ros_slope.total() > ros_flat.total());
    }

    #[test]
    fn test_rate_of_spread_wind_direction() {
        let model = FuelModel::anderson(FuelCategory::ShortGrass);
        let coeffs = SpreadCoeffs::compute(&model, 0.08);
        // Wind in the direction of spread
        let ros_aligned = RateOfSpread::compute(&coeffs, 5.0, 0.0, 0.0, 0.0, 1.0, 0.0);
        // Wind opposite to spread direction
        let ros_opposed = RateOfSpread::compute(&coeffs, -5.0, 0.0, 0.0, 0.0, 1.0, 0.0);
        // Aligned wind should spread faster than opposed
        assert!(ros_aligned.total() > ros_opposed.total());
    }

    #[test]
    fn test_rate_of_spread_high_moisture() {
        let model = FuelModel::anderson(FuelCategory::ShortGrass);
        let coeffs_dry = SpreadCoeffs::compute(&model, 0.05);
        let coeffs_wet = SpreadCoeffs::compute(&model, 0.11);
        // Higher moisture should reduce spread rate
        assert!(coeffs_dry.r0 > coeffs_wet.r0);
    }

    #[test]
    fn test_fire_front_ignite_point() {
        let mut fire = FireFront::new(10, 10, 10.0, 10.0, 1, 0.08);
        fire.ignite_point(5, 5);

        // Center cell is burning
        assert!(fire.is_burning(5, 5));
        assert!(!fire.is_burning(0, 0));
        // Some cells near the ignition point may have lfn <= 0 from initialization
        assert!(fire.burning_count() >= 1);
    }

    #[test]
    fn test_fire_front_ignite_circle() {
        let mut fire = FireFront::new(20, 20, 10.0, 10.0, 1, 0.08);
        fire.ignite_circle(100.0, 100.0, 30.0);

        // Should have ignited multiple cells near center
        assert!(fire.burning_count() > 1);
        assert!(fire.is_burning(10, 10));
        assert!(!fire.is_burning(0, 0));
    }

    #[test]
    fn test_fire_front_spreads_with_wind() {
        let mut fire = FireFront::new(30, 30, 10.0, 10.0, 1, 0.08);
        fire.ignite_point(15, 15);

        let w = vec![5.0_f32; 30 * 30]; // 5 m/s wind in +x
        let v = vec![0.0_f32; 30 * 30];
        let dzdx = vec![0.0_f32; 30 * 30];
        let dzdy = vec![0.0_f32; 30 * 30];

        let initial_burning = fire.burning_count();

        // Step enough for fire to spread multiple cells
        for _ in 0..30 {
            fire.step(1.0, &w, &v, &dzdx, &dzdy);
        }

        // Fire should have spread
        assert!(
            fire.burning_count() > initial_burning || fire.burned_count() > 0,
            "burning={}, burned={}",
            fire.burning_count(),
            fire.burned_count()
        );
        assert!(fire.burned_area() > 100.0);
    }

    #[test]
    fn test_fire_front_no_spread_no_fuel() {
        let mut fire = FireFront::new(10, 10, 10.0, 10.0, 14, 0.08); // NoFuel
        fire.ignite_point(5, 5);

        let w = vec![5.0_f32; 100];
        let v = vec![0.0_f32; 100];
        let dzdx = vec![0.0_f32; 100];
        let dzdy = vec![0.0_f32; 100];

        for _ in 0..5 {
            fire.step(1.0, &w, &v, &dzdx, &dzdy);
        }

        // Fire should not spread beyond the initial ignition area
        assert!(fire.burned_area() <= 900.0); // at most the init band
    }

    #[test]
    fn test_fire_front_fuel_consumption() {
        let mut fire = FireFront::new(5, 5, 10.0, 10.0, 1, 0.08);
        fire.ignite_point(2, 2);

        let w = vec![0.0_f32; 25];
        let v = vec![0.0_f32; 25];
        let dzdx = vec![0.0_f32; 25];
        let dzdy = vec![0.0_f32; 25];

        let initial_fuel = fire.fuel_frac[12];

        for _ in 0..100 {
            fire.step(1.0, &w, &v, &dzdx, &dzdy);
        }

        // Fuel should decrease over time
        assert!(fire.fuel_frac[12] < initial_fuel);
    }

    #[test]
    fn test_fire_front_heat_flux() {
        let mut fire = FireFront::new(5, 5, 10.0, 10.0, 1, 0.08);
        fire.ignite_point(2, 2);

        let w = vec![0.0_f32; 25];
        let v = vec![0.0_f32; 25];
        let dzdx = vec![0.0_f32; 25];
        let dzdy = vec![0.0_f32; 25];

        fire.step(1.0, &w, &v, &dzdx, &dzdy);

        // Burning cell should produce heat flux
        assert!(fire.heat_flux[12] > 0.0);
        // Non-burning cell should have zero heat flux
        assert_eq!(fire.heat_flux[0], 0.0);
    }

    #[test]
    fn test_fire_front_spreads_faster_downwind() {
        let mut fire = FireFront::new(41, 41, 10.0, 10.0, 1, 0.08);
        fire.ignite_point(20, 20);

        let w = vec![5.0_f32; 41 * 41]; // strong wind in +x
        let v = vec![0.0_f32; 41 * 41];
        let dzdx = vec![0.0_f32; 41 * 41];
        let dzdy = vec![0.0_f32; 41 * 41];

        for _ in 0..20 {
            fire.step(1.0, &w, &v, &dzdx, &dzdy);
        }

        // Fire should spread further in +x direction (downwind)
        // Find the furthest burning/burned cell in x and y
        let mut max_burned_col = 0;
        let mut max_burned_row = 0;
        for row in 0..41 {
            for col in 0..41 {
                if fire.lfn[row * 41 + col] <= 0.0 {
                    max_burned_col = max_burned_col.max(col);
                    max_burned_row = max_burned_row.max(row);
                }
            }
        }

        // Should spread further downwind (in +x) than crosswind (in y)
        let downwind_extent = max_burned_col as i32 - 20;
        let crosswind_extent = (max_burned_row as i32 - 20).abs();
        assert!(
            downwind_extent > crosswind_extent,
            "downwind ({downwind_extent}) should exceed crosswind ({crosswind_extent})"
        );
    }
}
