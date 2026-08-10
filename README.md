# Apogee

[![CI](https://github.com/LogOS-Development/apogee/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/LogOS-Development/apogee/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/badge/coverage-80.6%25-yellow)](https://github.com/LogOS-Development/apogee/actions/workflows/ci.yml)

A high-fidelity orbital mechanics and space physics engine for games. Apogee provides real astrodynamics — JPL ephemeris, spherical harmonic gravity (EGM2008), atmospheric models (NRLMSISE-00, Jacchia-Bowman, HWM14), geomagnetic field (IGRF), and multi-rate numerical integration — as a reusable Rust engine with a Godot 4 GDExtension bridge and an authoritative headless server for multiplayer.

Designed for game developers who need more than a kinematic orbit animator: propagators that respect perturbation physics, an ECS world that manages spacecraft and celestial bodies as entities, and an FFI surface that lets a Godot client create, step, and query the simulation without holding Rust references across frames.

## Architecture

```
apogee/
├── crates/
│   ├── apogee-core/       Simulation engine (no I/O, no rendering)
│   ├── apogee-net/        QUIC networking + FlatBuffer protocol
│   ├── apogee-server/     Headless sim server binary
│   ├── apogee-godot/      GDExtension bridge (cdylib)
│   ├── apogee-common/     Shared types, constants, errors
│   ├── apogee-social/     Social/faction systems
│   ├── apogee-strategic/  Strategic-layer gameplay
│   ├── apogee-tactical/   Tactical-layer gameplay
│   ├── apogee-discovery/  Exploration/discovery systems
│   └── apogee-llm/        LLM-driven dynamic content
├── godot/                 Godot 4 project (client renderer)
├── schemas/               FlatBuffer .fbs definitions
├── data/                  Ephemeris kernels, gravity models, space weather
├── tests/                 Integration + validation tests
└── ci/                    CI pipeline configs
```

### Tech Stack

| Layer            | Technology                          |
|------------------|-------------------------------------|
| Simulation Core  | Rust + hecs ECS                     |
| Numerics         | nalgebra, odeint, hifitime, spice-rs|
| Networking       | Quinn (QUIC) + FlatBuffers          |
| Client Renderer  | Godot 4.x (GDExtension bridge)      |
| Persistence      | PostgreSQL + TimescaleDB + Redis    |
| CI/CD            | GitHub Actions                      |

## Development

```bash
# Build all crates
cargo build --workspace

# Run tests
cargo test --workspace

# Optional feature-gated tests (e.g., HWM14 Fortran model) run nightly;
# see `.github/workflows/nightly.yml`.
# Lint
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check

# Run the headless server
cargo run -p apogee-server

# Fetch data assets (ephemeris, gravity, space weather)
./scripts/fetch_data.sh
```

## Roadmap

| Phase | Description               | Status       |
|-------|---------------------------|--------------|
| 0     | Foundation                | In Progress  |
| 1     | Core Propagator (MVP G1)  | Planned      |
| 2     | Networking & Multiplayer  | Planned      |
| 3     | Godot Client (G2 Alpha)   | Planned      |
| 4     | Vehicle Systems (G3 Beta) | Planned      |
| 5     | Gameplay (G4 Complete)     | Planned      |
| 6     | Hardening (G5 Launch)      | Planned      |

## Visualization

Atmosphere and wind fields can be explored in 3D through the
**Godot 4 scene** (`godot/scenes/atmosphere_wind_visualizer.tscn`): loads the
`apogee-godot` GDExtension and renders density spheres + animated wind arrows
for NRLMSISE-00 and Jacchia-Bowman side-by-side. Enable the `hwm14` feature
on the GDExtension crate for real HWM14 wind vectors.

See `plan.md` for the full development work plan.

## License

MIT