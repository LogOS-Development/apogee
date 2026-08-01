# Apogee — Master Development Plan (v2.0)

> **Goal:** Build Apogee as a self-extending civilization engine. The physics
> core remains the critical path; everything else hangs off it. Simulation layers
> activate dynamically, organizations form contextually, and the LLM writes new
> simulation logic as civilization discovers it.

---

## What Changed From v1.0

The original plan was a physics simulator with multiplayer. The revised plan adds:

- **Layer -1 Service Bus:** WASM runtime, dynamic service spawning,
  channel-based communication, and an LLM gateway.
- **Layer 5 Civilization:** Contextual organizations, dynamic policies,
  culture, and LLM mediation (ported from Libertas).
- **Layer 6 Discovery Engine:** LLM-generated services, runtime ECS extension,
  milestone-to-discovery pipeline, and a WASM sandbox.

The physics foundation is unchanged and remains the gate for all later work.

---

## Revised Layer Stack

```
┌─────────────────────────────────────────────────────┐
│  Layer 6: Dynamic Discovery Engine                     │
│  LLM-generated services, runtime ECS extension,        │
│  milestone-to-discovery pipeline, WASM sandbox         │
├─────────────────────────────────────────────────────┤
│  Layer 5: Civilization                                 │
│  Contextual organizations, dynamic policies,            │
│  culture, LLM mediation (ported from Libertas)          │
├─────────────────────────────────────────────────────┤
│  Layer 4: Strategic                                    │
│  Space combat, fleet ops, settlement management        │
├─────────────────────────────────────────────────────┤
│  Layer 3: Tactical                                     │
│  Infantry, FPS, inventory, EVA, crafting                │
├─────────────────────────────────────────────────────┤
│  Layer 2: Interior                                     │
│  CW/TH relative motion, contact dynamics               │
├─────────────────────────────────────────────────────┤
│  Layer 1: Spacecraft                                  │
│  6DOF, flexible body, all perturbations                │
├─────────────────────────────────────────────────────┤
│  Layer 0: Celestial                                    │
│  N-body, ephemeris, frames, spherical harmonics        │
├─────────────────────────────────────────────────────┤
│  Layer -1: Service Bus                                 │
│  WASM runtime, dynamic service spawning,               │
│  channel-based communication, LLM gateway              │
└─────────────────────────────────────────────────────┘
```

---

## Tech Stack (Updated)

| Layer | Technology | Role |
|-------|------------|------|
| Simulation Core | Rust + Bevy ECS | Headless server, authoritative physics |
| Numerics | nalgebra, odeint, spice-rs | Math, integration, ephemeris |
| Networking | Quinn (QUIC) + FlatBuffers | Transport + serialization |
| Client Renderer | Godot 4.x (GDExtension bridge) | Visuals, input, UI |
| Bridge | Rust → Godot via GDExtension | Shared memory or IPC |
| Persistence | PostgreSQL + TimescaleDB + Redis | State, telemetry, sessions |
| Dynamic Services | WASM runtime (wasmtime) | Sandboxed LLM-generated simulation modules |
| LLM Gateway | Ollama (local) / Cloud API fallback | Agent cognition, policy interpretation, discovery evaluation, service generation |
| Social Sim | Libertas (port from Python → Rust) | Agent-based governance, economy, organizations |
| CI/CD | GitHub Actions + Hermes agent | Automated dev, review, testing |
| Infra | Docker + Kubernetes | Shard orchestration, service isolation |

---

## Updated Project Structure

```
apogee/
├── crates/
│   ├── apogee-core/              # Physics simulation engine
│   │   ├── src/
│   │   │   ├── components/       # ECS component definitions
│   │   │   ├── systems/         # ECS systems (force agg, integrator, etc.)
│   │   │   ├── ephemeris/        # JPL kernel loading + Chebyshev eval
│   │   │   ├── frames/          # Frame transformation service
│   │   │   ├── gravity/         # Spherical harmonics, N-body
│   │   │   ├── aero/            # Aerodynamic models
│   │   │   ├── integrator/      # Multi-rate integration
│   │   │   ├── interior/        # CW/TH propagation, contact solver
│   │   │   └── tests/
│   │   └── Cargo.toml
│   ├── apogee-net/               # Networking layer
│   │   ├── src/
│   │   │   ├── server/          # QUIC listener, snapshot builder
│   │   │   ├── client/          # Connection, prediction, reconciliation
│   │   │   ├── protocol/        # FlatBuffer schemas + generated code
│   │   │   └── interest/        # Visibility, bandwidth allocation
│   │   └── Cargo.toml
│   ├── apogee-server/            # Binary: headless sim server
│   │   ├── src/main.rs
│   │   └── Cargo.toml
│   ├── apogee-godot/             # GDExtension bridge
│   │   ├── src/
│   │   │   ├── bridge.rs
│   │   │   ├── nodes/
│   │   │   └── lib.rs
│   │   └── Cargo.toml
│   ├── apogee-social/            # Libertas port (social simulation)
│   │   ├── src/
│   │   │   ├── agent/            # LLM-powered autonomous agents
│   │   │   ├── governance/       # Constitution, voting, motions
│   │   │   ├── economy/          # Production, markets, resources
│   │   │   ├── organization/     # Contextual groups, relationships
│   │   │   ├── cognitive/         # Personality, mood, background
│   │   │   └── tools/            # LLM tool definitions
│   │   └── Cargo.toml
│   ├── apogee-discovery/         # Dynamic simulation extension engine
│   │   ├── src/
│   │   │   ├── triggers/         # Milestone detection, discovery evaluation
│   │   │   ├── service_spec/     # LLM-generated service schemas
│   │   │   ├── wasm_runtime/     # Sandboxed execution of dynamic services
│   │   │   ├── registry/         # Active service registry, schema versioning
│   │   │   ├── codegen/          # LLM → Rust DSL → WASM compilation
│   │   │   └── channel/         # Typed communication between services
│   │   └── Cargo.toml
│   ├── apogee-tactical/          # Player avatar, combat, inventory
│   │   ├── src/
│   │   │   ├── avatar/           # Player entity, movement modes
│   │   │   ├── inventory/        # Items, slots, equipment
│   │   │   ├── combat/          # Weapons, damage, projectiles
│   │   │   ├── eva/             # Spacewalk, suit systems
│   │   │   └── interaction/     # Doors, switches, crafting stations
│   │   └── Cargo.toml
│   ├── apogee-strategic/         # Settlement management, strategic combat
│   │   ├── src/
│   │   │   ├── settlement/      # Settlement state, infrastructure
│   │   │   ├── territory/       # Province generation, resource maps
│   │   │   ├── military/        # Strategic units, fleet ops
│   │   │   └── diplomacy/       # Treaties, relations
│   │   └── Cargo.toml
│   ├── apogee-llm/               # LLM gateway and mediation
│   │   ├── src/
│   │   │   ├── gateway.rs        # Ollama/cloud abstraction
│   │   │   ├── mediation/        # Periodic nonlinear effect generation
│   │   │   ├── policy/           # Natural language policy interpretation
│   │   │   ├── discovery/        # Discovery evaluation and service generation
│   │   │   └── prompt/           # Prompt templates and builders
│   │   └── Cargo.toml
│   └── apogee-common/            # Shared types, constants, errors
│       └── Cargo.toml
├── godot/                        # Godot project files
│   ├── project.godot
│   ├── scenes/
│   ├── shaders/
│   ├── scripts/
│   └── addons/
├── schemas/                      # FlatBuffer .fbs definitions
├── data/                         # Ephemeris kernels, gravity models
├── tests/                        # Integration + validation tests
├── ci/                           # CI pipeline configs
└── docker/                       # Container definitions
```

---

## Phase Roadmap (Revised)

### Phase 0: Foundation (Weeks 1–4)

| Task | Description | Owner | Exit Criterion |
|------|-------------|-------|----------------|
| 0.1 | Workspace scaffolding | Hermes | `cargo build --workspace` succeeds |
| 0.2 | Data acquisition | Hermes | `fetch_data.sh` populates `data/` |
| 0.3 | CI pipeline | Hermes | PR triggers all jobs |
| 0.4 | Hermes review process | Ryan | Hermes PRs blocked on review |
| 0.5 | LLM gateway scaffold | Hermes | Can query local Ollama from Rust |

**Exit:** Compiling workspace, CI green, LLM gateway responds to test query.

### Phase 1: Core Propagator (Weeks 5–20)

| Task | Description | Dependencies | Exit Criterion |
|------|-------------|--------------|----------------|
| 1.1 | Clock + frames | TDB/TAI/UTC, ICRF↔ECI↔ECEF, nutation/precession | 0.2 | < 1 arcsec vs SPICE |
| 1.2 | Ephemeris service | DE441 loader, Chebyshev evaluator, cache | 0.2 | Mars vs Horizons < 100km/yr |
| 1.3 | Gravity module | N-body, spherical harmonics (Cunningham), GG torque | 1.2 | LEO 24h error < 1km vs TLE |
| 1.4 | Atmosphere model | NRLMSISE-00 port, space weather data | 0.2 | Density matches reference |
| 1.5 | Multi-rate integrator | RK8(9) adaptive + RK4(5) fixed, coupling | 1.3, 1.4 | Energy conservation < 10⁻¹² |
| 1.6 | 6DOF single spacecraft | Force aggregator, SRP, drag, mass tracking | 1.3–1.5 | 24h ISS propagation < 1km |

**Exit (G1):** Single spacecraft propagates 24h in LEO with validated physics.

### Phase 2: Service Bus & Dynamic Architecture (Weeks 8–16, parallel with Phase 1)

| Task | Description | Dependencies | Exit Criterion |
|------|-------------|--------------|----------------|
| 2.1 | WASM runtime integration | wasmtime embedding, module loading, memory management | 0.1 | Can load and execute a WASM module from Rust |
| 2.2 | Typed channel system | Bidirectional communication between core ECS and WASM services | 2.1 | Message round-trip < 1ms |
| 2.3 | Service registry | Track active services, schemas, versions | 2.1 | Can register/unregister services at runtime |
| 2.4 | Service lifecycle | Spawn, pause, resume, terminate services | 2.1–2.3 | Service spawns on signal, shuts down cleanly |
| 2.5 | LLM code generation | LLM → service spec → Rust DSL → WASM compilation | 0.5, 2.1 | LLM generates valid WASM service from natural language |
| 2.6 | Schema versioning | Handle service upgrades without losing state | 2.3 | Service can hot-update with state migration |
| 2.7 | Fallback safety | Malformed service doesn't crash server | 2.1, 2.4 | Bad WASM is caught and logged |

**Exit:** A WASM service can be dynamically spawned, receive inputs from the ECS, compute outputs, and be shut down. The LLM can generate a simple service from a text description.

```rust
// The interface that makes this all work
pub trait DynamicService: Send + Sync {
    fn id(&self) -> ServiceId;
    fn domain(&self) -> SimulationDomain;
    fn process(&mut self, input: &ServiceInput) -> ServiceOutput;
    fn schema(&self) -> &ServiceSchema;
}
```

### Phase 3: Networking & Multiplayer (Weeks 20–32)

| Task | Description | Dependencies | Exit Criterion |
|------|-------------|--------------|----------------|
| 3.1 | FlatBuffer schemas | Snapshot, command, event schemas | Phase 1 | Round-trip serialization verified |
| 3.2 | QUIC server | Connection manager, heartbeat, auth | 3.1 | 100 concurrent connections stable |
| 3.3 | Snapshot builder | Delta-encoded state, priority queue | 3.1, Phase 1 | Packet < 1KB per spacecraft |
| 3.4 | Client prediction | Input queue, local propagation, reconciliation | 3.2, Phase 1 | < 1m error at 100ms RTT |
| 3.5 | Interest management | Octree, relevance scoring, variable rate | 3.2–3.4 | < 5Mbps for 100 players |
| 3.6 | Cross-shard handoff | SOI transition migration protocol | 3.5 | < 0.1m discontinuity |

**Exit (G2 Alpha):** Multiplayer spacecraft simulation functional.

### Phase 4: Godot Client (Weeks 24–40, overlaps Phase 3)

| Task | Description | Dependencies | Exit Criterion |
|------|-------------|--------------|----------------|
| 4.1 | GDExtension bridge | Rust → Godot, shared state buffer | Phase 1 | Cube orbits in Godot at 60fps |
| 4.2 | Floating origin | Rebasing, precision management | 4.1 | No artifacts at 380,000km |
| 4.3 | Camera & LOD | Free/chase/map modes, distance-based LOD | 4.2 | Smooth LEO to lunar transfer |
| 4.4 | Planet rendering | Chunked sphere LOD, heightmaps, atmosphere | 4.3 | Earth visible from surface to orbit |
| 4.5 | HUD & instruments | Orbital elements, attitude, nav, maneuver planner | 4.1 | All displays from authoritative sim |
| 4.6 | Map view | 2D orbital map, trajectory display | 4.1 | Conics match propagation |

**Exit (G2 Alpha cont.):** Playable single-player orbital mechanics with UI.

### Phase 5: Vehicle Systems (Weeks 36–52)

| Task | Description | Dependencies | Exit Criterion |
|------|-------------|--------------|----------------|
| 5.1 | Propulsion & staging | Thrusters, tanks, gimbal, separation events | Phase 1 | Δv within 5% of rocket equation |
| 5.2 | Aero database system | Component buildup, interference, regime blending | Phase 1 | Wave drag within 20% of area rule |
| 5.3 | Flexible body & slosh | Modal FEM, spring-mass slosh, CG tracking | 5.1 | Slosh period within 10% of reference |
| 5.4 | Interior dynamics | CW/TH propagator, indoor aero, contact solver | Phase 1 | Object returns to start ±1m/10 orbits |
| 5.5 | Vehicle editor | Part library, assembly UI, staging editor | 4.5, 5.1 | Player builds and launches custom vehicle |

**Exit (G3 Beta):** Full vehicle systems with interior dynamics.

### Phase 6: Social Simulation — Libertas Integration (Weeks 16–32, parallel)

| Task | Description | Dependencies | Exit Criterion |
|------|-------------|--------------|----------------|
| 6.1 | Port Libertas core | Agent, governance, economy, organization → Rust | 0.5 | Feature parity with Python version |
| 6.2 | LLM agent system | Autonomous agents with cognition, mood, personality | 6.1, 0.5 | Agents make decisions, vote, trade |
| 6.3 | Constitutional engine | Natural language constitutions → governance rules | 6.1 | Agents follow constitution they read |
| 6.4 | Economic simulation | Production, markets, resources, trade routes | 6.1 | Supply/demand pricing emerges |
| 6.5 | LLM mediation layer | Periodic nonlinear effect generation | 6.1, 0.5 | Revolution cascade across settlements |
| 6.6 | Dynamic policy engine | Natural language → mechanical effects | 6.1, 0.5 | Policy text → sim effects with predictions |

**Exit:** Social simulation runs standalone, agents govern themselves, policies are interpreted by LLM.

### Phase 7: Contextual Organization System (Weeks 24–36, depends on 6)

| Task | Description | Dependencies | Exit Criterion |
|------|-------------|--------------|----------------|
| 7.1 | Group node component | Dynamic groups with memberships, roles, authority | 6.1 | Groups spawn/dissolve at runtime |
| 7.2 | Context system | Domain, scope, temporal triggers | 7.1 | Militia forms on raid, dissolves after |
| 7.3 | Relationship graph | Group-to-group relations, classification | 7.1 | Nation/rebel/corp classifications emerge |
| 7.4 | Context transitions | Event-driven group formation/dissolution | 7.2, 7.3 | Same individuals in multiple groups |
| 7.5 | Constitutional grammar | Constitution → context transition rules | 6.3, 7.4 | LLM interprets constitution as org rules |
| 7.6 | Hierarchical emergence | Leaders, authority scopes, revocation | 7.1–7.4 | Leadership emerges from group dynamics |

**Exit:** Organizations are fully dynamic, contextual, and relational. Same individuals form different structures in different contexts.

### Phase 8: Dynamic Discovery Engine (Weeks 20–40, depends on 2)

| Task | Description | Dependencies | Exit Criterion |
|------|-------------|--------------|----------------|
| 8.1 | Milestone system | Track player/org achievements as milestone events | Phase 1, 6.1 | "Iron mine operational for 1 year" detected |
| 8.2 | Discovery evaluation | LLM evaluates milestones, decides if discovery triggers | 0.5, 8.1 | LLM says "yes, activate metallurgy sim" |
| 8.3 | Service spec generation | LLM generates typed input/output schema + initial logic | 0.5, 2.5 | Metallurgy service spec generated from text |
| 8.4 | WASM compilation | Service spec → Rust DSL → WASM module | 2.5, 8.3 | Generated service compiles and runs |
| 8.5 | Service integration | Dynamic service hooks into main ECS via channels | 2.2, 8.4 | Furnace entities call metallurgy service |
| 8.6 | Service evolution | LLM adds rules/upgrades to running services | 8.5 | Steel discovered → new alloy rule injected |
| 8.7 | Discovery tree | Graph of milestone → discovery relationships | 8.1–8.6 | Tech progression from stone to iron validated |
| 8.8 | Era progression | Tech level gates simulation layer activation | 8.7 | Orbital mechanics "boots up" when calculus discovered |

**Exit:** The simulation self-extends. New domains activate when milestones are reached. The LLM writes and injects new simulation logic at runtime.

### Phase 9: Player Avatar & Tactical (Weeks 40–56)

| Task | Description | Dependencies | Exit Criterion |
|------|-------------|--------------|----------------|
| 9.1 | Player entity + physics | Zero-G movement, gravity walking, mag boots | Phase 1, 4.1 | Walk inside station, EVA on hull |
| 9.2 | Inventory & equipment | Slots, items, equip/unequip, stacks | 9.1 | Carry items, drop as physics objects |
| 9.3 | Clothing & suits | EVA suits, armor, thermal, pressure | 9.2 | Suit required for spacewalk |
| 9.4 | Weapons & combat | Hitscan, projectile, beam, melee, recoil | 9.1 | Fire weapon in zero-G, recoil drifts player |
| 9.5 | Health & injury | HP, injuries, bleeding, suit breach | 9.1 | Suit breach kills in vacuum |
| 9.6 | Crafting & interaction | Stations, doors, switches, item combination | 9.2 | Operate furnace, open door, craft tool |
| 9.7 | NPC workers | Autonomous laborers with needs, skills, morale | 6.1, 9.1 | NPC walks to mine, works, eats, sleeps |

**Exit:** Player physically inhabits the world. Can walk, fight, craft, and interact with simulation objects.

### Phase 10: Strategic & Settlement Management (Weeks 44–60)

| Task | Description | Dependencies | Exit Criterion |
|------|-------------|--------------|----------------|
| 10.1 | Territory system | Province generation, resource deposits | Phase 4 | Voronoi provinces on any planet |
| 10.2 | Resource maps | Deposits, surveying, accessibility | 10.1 | Player walks to deposit, builds mine |
| 10.3 | Settlement building | Place buildings, infrastructure, roads | 10.1, 9.6 | Colony grows from camp to town |
| 10.4 | Production chains | Mine → smelter → factory → goods | 9.7, 10.3 | Ore extracted, processed, manufactured |
| 10.5 | Pop system | Victoria-style pops with needs, jobs, politics | 6.1, 10.3 | Pops demand reforms when unhappy |
| 10.6 | Trade routes | Between settlements, over land and space | 10.4, Phase 3 | Goods flow between settlements |
| 10.7 | Map UI | Political/terrain/trade/resource/population modes | 10.1–10.6 | Map displays all sim states |

**Exit (G4 Feature Complete):** Grand strategy layer functional. Settlements grow, trade, war, and develop.

### Phase 11: Era Progression & Case Studies (Weeks 48–64)

| Task | Description | Dependencies | Exit Criterion |
|------|-------------|--------------|----------------|
| 11.1 | Tech graph | Non-linear, era-gated, randomized options | 8.7, 10.5 | Multiple viable paths through tech |
| 11.2 | Organization types | Tribe → city-state → kingdom → nation → corp | 7.1, 11.1 | Org type evolves with era |
| 11.3 | Diegetic sim activation | Physics layers boot when tech unlocks them | 8.8, 11.1 | Orbital mechanics activates on calculus |
| 11.4 | Case study: agriculture | Stone age food problem | 10.5, 6.1 | Player solves crop rotation |
| 11.5 | Case study: metallurgy | Bronze → iron → steel progression | 8.5, 9.6 | Player discovers steel through experimentation |
| 11.6 | Case study: governance | Classical city management | 6.2, 7.5 | Player designs political system |
| 11.7 | Case study: industrialization | Pollution, labor, climate | 10.4, 6.5 | Player faces environmental consequences |
| 11.8 | Case study: orbital | "How to hit a moving target 400,000km away" | Phase 1, 11.3 | Player reaches orbit using discovered physics |

**Exit:** Full era progression from stone age to orbital age, with emergent case studies.

### Phase 12: Combat Systems (Weeks 52–68)

| Task | Description | Dependencies | Exit Criterion |
|------|-------------|--------------|----------------|
| 12.1 | Space combat | Missiles, railguns, lasers, point defense | 9.4, Phase 1 | Destroy target at 100km |
| 12.2 | Ground combat | Infantry on planetary surfaces | 9.4, 10.1 | Fight on Mars, Moon, Earth |
| 12.3 | Vehicle combat | Rovers, tanks, small craft | 12.2 | Drive and shoot from vehicle |
| 12.4 | Boarding actions | Dock, breach, enter, fight | 12.1, 9.6 | Board enemy station |
| 12.5 | Damage model | Component destruction, hull breach, fires | 12.1 | Shoot reactor → explosion |
| 12.6 | AI combatants | NPC soldiers, pilots, defenders | 9.7, 12.2 | NPCs fight with basic tactics |
| 12.7 | Strategic military | Units, supply lines, fortifications | 10.3, 12.6 | Wage war between organizations |

**Exit:** Full combat across all scales — infantry to fleet engagements.

### Phase 13: MMO Infrastructure (Weeks 56–72)

| Task | Description | Dependencies | Exit Criterion |
|------|-------------|--------------|----------------|
| 13.1 | Persistent universe | State survives server restarts | Phase 3 | Player logs in, stuff is where they left it |
| 13.2 | Cross-shard economy | Unified market across all shards | 10.6, 13.1 | Buy ore on Mars, sell on Earth |
| 13.3 | Player housing | Personal quarters on stations/planets | 9.2 | Decorate and store items |
| 13.4 | Guilds / corporations | Player organizations with shared assets | 7.1 | Guild owns a station collectively |
| 13.5 | Server meshing | Multiple shards for different regions | Phase 3, 13.1 | 1000 players across solar system |
| 13.6 | Cross-shard social sim | LLM mediation across all shards | 6.5, 13.5 | Revolution cascades across planets |

**Exit (G5 Launch):** Persistent MMO universe with player-driven civilization.

### Phase 14: Hardening & Launch (Weeks 64–80)

| Task | Description | Exit Criterion |
|------|-------------|--------------|
| 14.1 | Performance optimization | 200 spacecraft at < 10ms tick |
| 14.2 | Validation campaign | All TCE/TAE/TMN tests pass |
| 14.3 | Deployment infrastructure | Docker, Kubernetes, monitoring — 1000 players stable |
| 14.4 | Content production | 20 vehicles, 50 missions |
| 14.5 | Localization | English first, extensible — all strings externalized |
| 14.6 | Documentation | API docs, modding guide, player wiki |
| 14.7 | Beta testing | Closed beta, open beta — NFR targets met |

**Exit (G6 Launch):** Public release.

---

## Critical Path (Updated)

```
Phase 0: Foundation
  ├── 0.1-0.4 Scaffolding ──────────────┐
  ├── 0.2 Data Acquisition ─────────────┤
  ├── 0.5 LLM Gateway ─────────────────┤
  └── Hermes Review Process ───────────┘
         │
         ├────────────────────────────────────────┐
         ▼                                        ▼
Phase 1: Core Propagator              Phase 2: Service Bus
  1.1 Time/Frames ───┐                  2.1 WASM Runtime ──────┐
  1.2 Ephemeris ─────┤                  2.2 Channel System ───┤
  1.3 Gravity ───────┤                  2.3 Service Registry ──┤
  1.4 Atmosphere ────┤                  2.4 Lifecycle ─────────┤
  1.5 Integrator ────┤                  2.5 LLM Codegen ───────┤
  1.6 6DOF Demo ─────┘ ← G1            2.6 Schema Versioning ──┤
         │                              2.7 Safety ────────────┘
         │                                        │
         ├──────────┬─────────────┐               │
         ▼          ▼             ▼               ▼
Phase 3: Net    Phase 4: Godot  Phase 6: Libertas  Phase 8: Discovery
  3.1 Schemas    4.1 Bridge      6.1 Port core      8.1 Milestones
  3.2 QUIC       4.2 Float Orig  6.2 Agents         8.2 Evaluation
  3.3 Snaps     4.3 Camera      6.3 Constitution    8.3 Spec Gen
  3.4 Predict    4.4 Planets    6.4 Economy         8.4 WASM Compile
  3.5 Interest   4.5 HUD        6.5 Mediation      8.5 Integration
  3.6 Shards    4.6 Map          6.6 Policy         8.6 Evolution
  └── G2 Alpha ─┘ ← G2 Alpha    └──────────┐        8.7 Tree
         │                              ▼        8.8 Eras
         │                   Phase 7: Orgs          │
         │                    7.1 Groups             │
         │                    7.2 Context            │
         │                    7.3 Relations         │
         │                    7.4 Transitions        │
         │                    7.5 Grammar            │
         │                    7.6 Hierarchy          │
         │                           │               │
         ├───────────┬───────────────┘               │
         ▼           ▼                               │
Phase 5: Vehicles  Phase 9: Avatar                   │
  5.1 Propulsion    9.1 Player physics                │
  5.2 Aero DB      9.2 Inventory                     │
  5.3 Flex/Slosh   9.3 Clothing                      │
  5.4 Interior     9.4 Weapons                       │
  5.5 Editor       9.5 Health                        │
  └── G3 Beta ──┘  9.6 Crafting                     │
         │          9.7 NPC workers                  │
         │                 │                          │
         ├─────────────────┘                          │
         ▼                                            ▼
Phase 10: Settlements                    Phase 11: Era Progression
  10.1 Territory                          11.1 Tech graph
  10.2 Resources                          11.2 Org types
  10.3 Building                           11.3 Diegetic activation
  10.4 Production                         11.4-11.8 Case studies
  10.5 Pops                                          │
  10.6 Trade                                         │
  10.7 Map UI                                        │
  └── G4 Feature Complete ──────────────────────────┘
         │
         ▼
Phase 12: Combat     Phase 13: MMO Infra
  12.1 Space combat    13.1 Persistence
  12.2 Ground combat   13.2 Cross-shard economy
  12.3 Vehicles        13.3 Housing
  12.4 Boarding        13.4 Guilds
  12.5 Damage          13.5 Server meshing
  12.6 AI              13.6 Cross-shard social
  12.7 Strategic                │
  └─────────────────────────────┘
         │
         ▼
Phase 14: Hardening
  14.1-14.7 Optimization, validation, content, docs
  └── G5/G6 Launch
```

---

## Effort Estimate (Revised)

| Phase | Duration (solo + Hermes) | Parallelizable? |
|-------|--------------------------|-----------------|
| 0: Foundation | 4 weeks | No |
| 1: Core Propagator | 16 weeks | No |
| 2: Service Bus | 8 weeks | Yes (with Phase 1) |
| 3: Networking | 12 weeks | After Phase 1 |
| 4: Godot Client | 16 weeks | After Phase 1 |
| 5: Vehicle Systems | 16 weeks | After Phase 1 |
| 6: Libertas Port | 12 weeks | Yes (with Phase 1) |
| 7: Contextual Orgs | 12 weeks | After Phase 6 |
| 8: Discovery Engine | 16 weeks | After Phase 2 |
| 9: Player Avatar | 16 weeks | After Phase 4 |
| 10: Settlements | 16 weeks | After Phase 7 |
| 11: Era Progression | 16 weeks | After Phase 8 |
| 12: Combat | 16 weeks | After Phase 9 |
| 13: MMO Infra | 16 weeks | After Phase 3 |
| 14: Hardening | 16 weeks | After all |

With Hermes accelerating boilerplate/tests/scaffolding and parallel phase execution:

| Scenario | Timeline |
|----------|----------|
| Solo + Hermes | ~24–30 months |
| Solo + Hermes + 1 engineer (social/client) | ~18–24 months |
| Team of 3 (sim, social, client) + Hermes | ~14–18 months |

The critical bottleneck remains **Phase 1: Core Propagator**. Everything depends on correct physics. But Phases 2, 6, and parts of 4 can proceed in parallel once Phase 0 is done.

---

## Hermes Task Assignment

### Immediate (This Week)

| Task | Crate | Hermes Can Autonomously Do |
|------|-------|----------------------------|
| Scaffold workspace | Root | Create all crates, Cargo.toml, CI config |
| Fetch data scripts | `data/` | Write download + hash verification scripts |
| LLM gateway stub | `apogee-llm` | Ollama HTTP client, prompt builder, response parser |
| WASM runtime stub | `apogee-discovery` | wasmtime integration, basic module loading |
| FlatBuffer schemas | `schemas/` | Define initial snapshot/command/event schemas |
| CI pipeline | `.github/` | Test/clippy/fmt/build jobs, PR template |

### Phase 1 Support

| Task | Hermes Can Do | Ryan Reviews |
|------|---------------|--------------|
| Ephemeris SPICE kernel parser | Binary format reader, segment locator | Physical correctness |
| Chebyshev evaluation math | Polynomial math, caching | Math correctness |
| NRLMSISE-00 port | Fortran → Rust translation | Physical correctness of density outputs |
| Frame transformation matrices | Matrix construction, caching | Math correctness of rotations |
| Integrator scaffolding | RK4/RK8 boilerplate, step control | Error tolerances, stability analysis |
| Validation test harness | Test runner, Horizons API client | Acceptance thresholds |
| ECS component definitions | All structs, derive macros | Component granularity, storage types |

### Phase 2 Support

| Task | Hermes Can Do | Ryan Reviews |
|------|---------------|--------------|
| WASM module interface | Trait definitions, module loading | Security boundary, memory limits |
| Channel system | Typed bidirectional channels | Performance, deadlock potential |
| Service registry | Registration, lookup, versioning | Schema migration logic |
| LLM codegen pipeline | Prompt templates, DSL parser | Generated code safety review |

### Phase 6 Support (Libertas Port)

| Task | Hermes Can Do | Ryan Reviews |
|------|---------------|--------------|
| Python → Rust translation | Mechanical port of existing code | Behavioral equivalence testing |
| LLM agent async runtime | Tokio task management | Cognition quality, prompt design |
| Governance engine port | Constitution parsing, voting logic | Constitutional interpretation correctness |
| Economic model port | Production, market, resource logic | Economic balance, realism |

---

## Risk Register (Updated)

| Risk | Severity | Mitigation |
|------|----------|------------|
| Physics core correctness | Critical | Validation suite gates all merges; Hermes can't bypass failing tests |
| WASM security | High | Sandboxed execution, memory limits, timeout enforcement |
| LLM hallucination in generated services | High | Generated code reviewed by Ryan; fallback to hardcoded services on failure |
| Libertas port behavioral drift | Medium | Differential testing: run Python and Rust versions side by side, compare outputs |
| Performance at scale | Medium | CW for interior objects, LOD for rendering, interest management for networking |
| Scope creep | Medium | Phase freeze: no new features until current phase passes exit criteria |
| Hermes generating incorrect numerics | High | Ryan reviews ALL physics code; Hermes restricted to scaffolding/tests for numerics |
| LLM latency blocking sim tick | Medium | All LLM calls async, never block physics tick; mediation runs in background |
| Dynamic service crashes | Medium | WASM isolation: crashed service doesn't crash server; automatic restart with fallback |

---

## Validation Suite (Updated)

| Test ID | Test | Source | Criterion |
|---------|------|--------|-----------|
| TCE-01 | Mars ephemeris 1yr | JPL Horizons | < 100 km |
| TCE-02 | LEO 24h propagation | ISS TLE | < 1 km |
| TAE-01 | Drag decay 7 days | Starlink TLE | < 10% Cd error |
| TAE-02 | Launch trajectory | SpaceX webcast | < 5% velocity at MECO |
| TMN-01 | 200 spacecraft stress | Synthetic | < 10ms tick, < 5Mbps |
| TMN-02 | Shard handoff | Synthetic | < 0.1m discontinuity |
| TSO-01 | Libertas behavioral parity | Python reference | Agent decisions match within 95% |
| TDS-01 | Dynamic service spawn | Synthetic milestone | Service activates in < 5s |
| TDS-02 | LLM-generated service correctness | Manual test case | Output within 10% of hardcoded reference |
| TDS-03 | Service crash isolation | Inject malformed WASM | Server continues running, service restarted |
| TCT-01 | Contextual group formation | Raid event trigger | Militia forms, assigns roles, dissolves after |
| TPP-01 | Policy interpretation | "Carbon tax" text | Mechanical effects match expected ranges |

---

## Milestone Gates (Updated)

| Gate | Criteria | Phase Complete |
|------|----------|----------------|
| G1: Physics MVP | Single spacecraft propagates 24h LEO, validated | Phase 1 |
| G2: Alpha | Multiplayer orbital mechanics, Godot client | Phases 3, 4 |
| G2.5: Social MVP | Libertas port runs, agents govern settlements | Phase 6 |
| G2.7: Discovery MVP | Dynamic service spawns from LLM-generated spec | Phase 8 |
| G3: Beta | Vehicle systems, interior dynamics, avatar | Phases 5, 9 |
| G3.5: Grand Strategy | Settlements, trade, pops, era progression | Phases 10, 11 |
| G4: Feature Complete | Combat, MMO infra, all eras playable | Phases 12, 13 |
| G5: Launch | All NFRs met, validation green, docs complete | Phase 14 |

---

## What Hermes Gets First

Immediate priority order:

1. **Scaffold the workspace** (0.1) — create all v2.0 crates, workspace `Cargo.toml`, and ensure `cargo build --workspace` passes.
2. **Data acquisition scripts** (0.2) — fetch and validate DE441, EGM2008, NRLMSISE-00 source, EOP, leap seconds, sample TLEs.
3. **CI pipeline** (0.3) — test, clippy, fmt, build, validation jobs; PR template.
4. **LLM gateway stub** (0.5) — Ollama HTTP client, prompt builder, response parser.
5. **WASM runtime stub** (2.1) — wasmtime integration, module loading.
6. **FlatBuffer schemas** (3.1) — snapshot, command, event definitions.

All other work is gated on Phase 0 exiting cleanly and Phase 1 physics validation passing.

---

*This is the complete plan. Every box in the architecture, every phase, every Hermes task, every risk, every validation test. The physics core is the foundation. The service bus is the extensibility mechanism. Libertas is the social simulation. The discovery engine is the self-extending brain. Everything connects.*
