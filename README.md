# Apogee

[![CI](https://github.com/LogOS-Development/apogee/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/LogOS-Development/apogee/actions/workflows/ci.yml)

A high-fidelity space physics engine for games, built in Rust. Apogee provides real astrodynamics — JPL ephemeris, spherical harmonic gravity (EGM2008), atmospheric models spanning the troposphere to thermosphere, geomagnetic fields, and multi-rate numerical integration — as a reusable ECS engine with a Godot 4 GDExtension bridge.

The driving vision is a strategy game where orbital mechanics and planetary physics are the gameplay: Hohmann transfers, delta-v budgets, conjunction warnings, surface weather and fire propagation, ecosystem tipping, and infrastructure fragility — all driven by physics that matters. Every physics module is licensable outside the game.

## What's Implemented

### Orbital Dynamics
- JPL SPK/SPICE ephemeris kernel reader (Type 2 Chebyshev, Type 3 Hermite) with SSB center composition for DE441/DE440s
- 11-body n-body validation suite against DE441 (1-year max error 1,502 km, energy drift 2.6e-10)
- Integrators: RK4 (fixed-step), RK45 DOPRI5 (adaptive), RK89 DOP853 (adaptive, energy conservation < 1e-10 per orbit)
- Joint integration of mutually-gravitating bodies (not per-body frozen snapshots)

### Gravity
- Per-body spherical harmonics via ECS components (Earth EGM2008 + point-mass Mars in the same sim)
- GravityConfig enum: PointMass, J2, SphericalHarmonics, FromFile (ICGEM/.gfc/EGM2008)
- Config-driven SystemDefinition: JSON-serializable body definitions, seeded random generation, presets

### Atmosphere
- NRLMSISE-00 — thermosphere empirical density (satellite drag, 90km+)
- Jacchia-Bowman — thermosphere density with solar/geomagnetic indices
- HWM14 — horizontal wind model via Fortran FFI (behind `hwm14` feature)
- WRF Kessler microphysics — warm-rain scheme via Fortran FFI (behind `wrf` feature)
- AtmosphereModel and HorizontalWindModel traits for pluggable models

### Star System
- Live StarSystem manager: config-driven, ephemeris-fed kinematic planets + N-body asteroids
- Asteroid clusters with aggregate GM and member promotion to full N-body
- Config-driven body roles: Star/Central/Planet/Moon (kinematic), Minor/Asteroid/AsteroidCluster (dynamic)

### Other Physics
- Solar radiation pressure with surface modeling
- Geomagnetic field (IGRF) with fixture-based testing
- Gradient torque for attitude dynamics
- TLE parsing and SGP4-compatible state vectors
- Control systems: actuators, magnetorquers, estimators, state machines

### Infrastructure
- hecs ECS World with generational entity arena
- Frame service: IAU 2000/2006 frame transformations, EOP, leap seconds, nutation/precession
- Compile-time SI units via metron (standalone crate: github.com/LogOS-Development/metron)
- CI: fmt + clippy (-D warnings) + test + coverage + cargo-audit on every PR; nightly includes ignored/feature-gated tests
- 503 tests, 0 warnings, 0 clippy violations

## Roadmap

### Phase 1: Core Propagator (in progress)

| Area | Status |
|------|--------|
| Ephemeris (SPK reader, DE441 validation) | Done |
| Integrators (RK4, RK45, RK89) | Done |
| Per-body gravity (SH, J2, point-mass) | Done |
| Config-driven StarSystem | Done |
| N-body joint integration | Done |
| Selectable propagation frames (#186) | Planned |
| FMF drag + STL panel model (#171-179) | Planned |
| Remaining raw-f64 unit gaps (#182) | Planned |

### Phase 2: Surface Simulation (planned)

A coupled surface layer: weather, terrain, fire, and ecological tipping.

| Area | Status |
|------|--------|
| WRF Kessler microphysics (FFI) | Done (PR #191) |
| WRF radiation schemes (HSRAD, SWRAD) | Planned |
| WRF PBL (YSU) + surface layer (sfclayrev) | Planned |
| WRF-SFIRE fire spread (Rothemel + level-set) | Planned |
| USGS 3DEP terrain ingestion (GeoTIFF) | Planned |
| Weather grid with horizontal coupling | Planned |
| Ecosystem model (reaction-diffusion, tipping) | Planned |

WRF v4.8.0 physics source is vendored at `external/wrf/vendor/` (fetched via `scripts/fetch_wrf.sh`). The FFI wrapping pattern follows HWM14: iso_c_binding wrappers compile Fortran to a static library behind a Cargo feature, with typed Rust APIs. 19 microphysics schemes, 12 PBL schemes, 9 radiation schemes, and fire physics are available for incremental wrapping.

### Phase 3: Networking & Multiplayer (deferred)

QUIC networking, FlatBuffer protocol, authoritative headless server. Deferred until single-player physics loop is validated.

### Phase 4: Godot Client (deferred)

Full Godot 4 UI: vessel rendering, trajectory visualization, surface weather display. GDExtension bridge exists with atmosphere visualizer.

## Architecture

```
apogee/
├── crates/
│   ├── apogee-core/       Simulation engine — 21K lines, 86 source files
│   ├── apogee-common/     Shared types, constants, NAIF IDs, metron units
│   ├── apogee-godot/      GDExtension bridge (ApogeeWorld, atmosphere visualizer)
│   ├── apogee-social/     Social/faction systems
│   ├── apogee-strategic/  Strategic-layer gameplay
│   ├── apogee-tactical/   Tactical-layer gameplay
│   ├── apogee-discovery/  Exploration/discovery systems
│   ├── apogee-llm/        LLM-driven dynamic content
│   ├── apogee-net/        QUIC networking (stub)
│   └── apogee-server/     Headless sim server (stub)
├── godot/                 Godot 4 project (client renderer)
├── data/                  Ephemeris kernels, gravity models, space weather
├── scripts/               Setup, data fetching, coverage
└── tests/                 Integration + validation tests
```

### apogee-core modules

| Module | Contents |
|--------|----------|
| `aero/` | NRLMSISE-00, Jacchia-Bowman, HWM14 (FFI), WRF Kessler (FFI), drag, SRP, space weather |
| `components/` | Kinematics, RigidBody, DragSurfaces, SrpSurfaces, SpacecraftDefinition, celestial |
| `control/` | Actuators, magnetorquers, estimators, state machines |
| `ephemeris/` | SPK kernel reader, Chebyshev/Hermite interpolation, EphemerisService |
| `frames/` | IAU 2000/2006 transforms, EOP, leap seconds, ClockService |
| `gravity/` | Point-mass, spherical harmonics (EGM2008), gradient torque |
| `integrator/` | RK4, RK45 (DOPRI5), RK89 (DOP853) |
| `magnetosphere/` | IGRF geomagnetic field |
| `orbit/` | Orbital mechanics utilities |
| `star_system/` | SystemDefinition, StarSystem manager, asteroid clusters |
| `systems/` | ForceModel trait, aggregate_forces, step_world |
| `tle/` | TLE parsing, state vector conversion |
| `world/` | hecs ECS World, entity management, simulation stepping |

### Vendored external code

| Directory | Source | Feature |
|-----------|--------|---------|
| `external/hwm14/` | NRL HWM14 Fortran | `hwm14` |
| `external/nrlmsise00_brahe/` | Brahe project NRLMSISE-00 (Rust port) | default |
| `external/wrf/` | WRF v4.8.0 physics (375MB, fetched) | `wrf` |

## Development

```bash
# Build all crates
cargo build --workspace

# Run tests (503 tests, no external dependencies needed)
cargo test --workspace

# Lint
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

# Fetch data assets (ephemeris, gravity, space weather)
./scripts/fetch_data.sh

# Fetch WRF physics source (for the `wrf` feature)
./scripts/fetch_wrf.sh

# Build with WRF physics (requires gfortran)
cargo build -p apogee-core --features wrf
cargo test -p apogee-core --features wrf --lib aero::wrf
cargo run --example wrf_kessler_samples -p apogee-core --features wrf

# Build with HWM14 winds (requires gfortran)
cargo build -p apogee-core --features hwm14
cargo run --example hwm14_samples -p apogee-core --features hwm14

# Run the headless server (stub)
cargo run -p apogee-server
```

Feature-gated tests requiring gfortran or large data files run in nightly CI (`.github/workflows/nightly.yml`).

## Tech Stack

| Layer            | Technology                          |
|------------------|-------------------------------------|
| Simulation Core  | Rust + hecs ECS                     |
| Numerics         | nalgebra, hifitime, metron (SI units) |
| Ephemeris        | SPICE SPK kernel reader (pure Rust) |
| Atmosphere       | NRLMSISE-00, Jacchia-Bowman, HWM14, WRF physics (FFI) |
| Networking       | Quinn (QUIC) + FlatBuffers (planned) |
| Client Renderer  | Godot 4.x (GDExtension bridge)      |
| CI/CD            | GitHub Actions + cargo-audit        |

## Visualization

Atmosphere and wind fields can be explored in 3D through the Godot 4 scene (`godot/scenes/atmosphere_wind_visualizer.tscn`): loads the `apogee-godot` GDExtension and renders density spheres and animated wind arrows for NRLMSISE-00 and Jacchia-Bowman side-by-side. Enable the `hwm14` feature on the GDExtension crate for real HWM14 wind vectors.

## License

MIT