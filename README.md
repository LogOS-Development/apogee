# Apogee

[![CI](https://github.com/ryansennis/apogee/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/ryansennis/apogee/actions/workflows/ci.yml)
[![Coverage](https://raw.githubusercontent.com/ryansennis/apogee/main/.github/badge-coverage.svg)](https://github.com/ryansennis/apogee/actions/workflows/ci.yml)

A multiplayer space simulation with an authoritative headless physics server and a Godot 4 client. Built in Rust for numerical fidelity — real JPL ephemeris, spherical harmonic gravity, NRLMSISE-00 atmosphere, and multi-rate integration.

## Architecture

```
apogee/
├── crates/
│   ├── apogee-core/       Simulation engine (no I/O, no rendering)
│   ├── apogee-net/        QUIC networking + FlatBuffer protocol
│   ├── apogee-server/     Headless sim server binary
│   ├── apogee-godot/      GDExtension bridge (cdylib)
│   └── apogee-common/     Shared types, constants, errors
├── godot/                 Godot 4 project (client renderer)
├── schemas/               FlatBuffer .fbs definitions
├── data/                  Ephemeris kernels, gravity models, space weather
├── tests/                 Integration + validation tests
└── ci/                    CI pipeline configs
```

### Tech Stack

| Layer            | Technology                          |
|------------------|-------------------------------------|
| Simulation Core  | Rust + Bevy ECS                     |
| Numerics         | nalgebra, odeint, spice-rs          |
| Networking       | Quinn (QUIC) + FlatBuffers           |
| Client Renderer  | Godot 4.x (GDExtension bridge)      |
| Persistence      | PostgreSQL + TimescaleDB + Redis    |
| CI/CD            | GitHub Actions                      |

## Development

```bash
# Build all crates
cargo build --workspace

# Run tests
cargo test --workspace

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

See `plan.md` for the full development work plan.

## License

MIT